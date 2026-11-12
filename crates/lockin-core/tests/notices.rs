//! Tests for what the daemon tells you: notifications (each once, only when
//! switched on) and the tray's lines. Times are in 2099, like the other tests.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::i18n::{Language, Texts};
use lockin_core::model::{BlockedApp, SessionDetails};
use lockin_core::notices::{self, Kind, Notice};
use lockin_core::{block, deadline, session, settings};

/// A local time on a 2099 day: at(4, 13, 0) = Wed 4 Nov 2099 13:00.
fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    session::local_time(
        NaiveDate::from_ymd_opt(2099, 11, day).unwrap(),
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap(),
    )
    .unwrap()
}

fn english() -> Texts {
    Texts::new(Language::En)
}

/// "Exercises", Wed 4 Nov 13:00–15:30, with the profile "Strict".
async fn setup() -> (Pool, String) {
    let pool = db::open_in_memory().await.unwrap();
    let profile = block::add_profile(&pool, "Strict").await.unwrap();
    let id = session::plan(
        &pool,
        &SessionDetails {
            title: Some("Exercises".into()),
            profile_id: Some(profile),
            ..Default::default()
        },
        at(4, 13, 0),
        at(4, 15, 30),
        at(2, 9, 0),
    )
    .await
    .unwrap();
    (pool, id)
}

/// Like the daemon: the notices due now, each marked as sent.
async fn take(pool: &Pool, now: DateTime<Utc>) -> Vec<Notice> {
    let due = notices::due(pool, &english(), now).await.unwrap();
    for notice in &due {
        notices::mark_sent(pool, &notice.key, now).await.unwrap();
    }
    due
}

fn titles(found: &[Notice]) -> Vec<&str> {
    found.iter().map(|notice| notice.title.as_str()).collect()
}

#[tokio::test]
async fn a_session_from_reminder_to_end() {
    let (pool, id) = setup().await;

    // Nothing an hour before.
    assert!(take(&pool, at(4, 12, 0)).await.is_empty());

    // 5 minutes before, once.
    let soon = take(&pool, at(4, 12, 55)).await;
    assert_eq!(titles(&soon), ["Study session in 5 minutes"]);
    assert_eq!(soon[0].body, "Exercises · 13:00–15:30 · blocks Strict");
    assert_eq!(soon[0].kind, Kind::Sessions);
    assert!(take(&pool, at(4, 12, 57)).await.is_empty());

    // The daemon starts it.
    session::tick(&pool, at(4, 13, 0)).await.unwrap();
    let started = take(&pool, at(4, 13, 0)).await;
    assert_eq!(titles(&started), ["Study session started"]);
    assert_eq!(started[0].body, "Exercises · until 15:30 · blocks Strict");

    // 30 minutes in: a break earned, once.
    let earned = take(&pool, at(4, 13, 30)).await;
    assert_eq!(titles(&earned), ["You earned a break"]);
    assert!(earned[0].body.starts_with("5 min of break credit"));
    assert!(take(&pool, at(4, 13, 40)).await.is_empty());

    // A break that runs out by itself.
    session::take_break(&pool, &id, at(4, 13, 45))
        .await
        .unwrap();
    session::tick(&pool, at(4, 13, 51)).await.unwrap();
    let over = take(&pool, at(4, 13, 51)).await;
    assert_eq!(titles(&over), ["Break over"]);
    assert_eq!(over[0].body, "Blocking is back on until 15:30.");

    // The end.
    session::tick(&pool, at(4, 15, 30)).await.unwrap();
    let done = take(&pool, at(4, 15, 30)).await;
    assert_eq!(titles(&done), ["Study session done"]);
    assert_eq!(done[0].body, "Exercises · 2 h 25 min studied");
    assert!(take(&pool, at(4, 15, 31)).await.is_empty());
}

#[tokio::test]
async fn an_old_start_is_not_announced() {
    let (pool, _) = setup().await;
    session::tick(&pool, at(4, 13, 0)).await.unwrap();
    // The daemon was asleep for an hour: no "started" any more, only what's new.
    let later = take(&pool, at(4, 14, 0)).await;
    assert_eq!(titles(&later), ["You earned a break"]);
}

#[tokio::test]
async fn switched_off_kinds_stay_quiet() {
    let (pool, _) = setup().await;
    settings::set(&pool, settings::NOTIFY_SESSIONS, "off")
        .await
        .unwrap();
    assert!(take(&pool, at(4, 12, 55)).await.is_empty());
    settings::set(&pool, settings::NOTIFY_SESSIONS, "on")
        .await
        .unwrap();
    assert_eq!(take(&pool, at(4, 12, 56)).await.len(), 1);
    // Only known settings, with sensible values.
    assert!(settings::set(&pool, "notify.other", "on").await.is_err());
    assert!(
        settings::set(&pool, settings::LANGUAGE, "fr")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn deadlines_a_day_before() {
    let (pool, _) = setup().await;
    let report = deadline::add(&pool, None, "Lab report 2", at(5, 23, 59))
        .await
        .unwrap();
    deadline::add(&pool, None, "Done already", at(5, 12, 0))
        .await
        .unwrap();
    let done_id = deadline::all(&pool).await.unwrap()[0].id.clone();
    deadline::set_done(&pool, &done_id, true).await.unwrap();

    // Two days before: nothing yet.
    assert!(take(&pool, at(3, 23, 0)).await.is_empty());
    // Within a day: once, and not the one that's done.
    let found = take(&pool, at(5, 8, 0)).await;
    assert_eq!(titles(&found), ["Deadline today 23:59"]);
    assert_eq!(found[0].body, "Lab report 2");
    assert!(found[0].key.contains(&report));
    assert!(take(&pool, at(5, 9, 0)).await.is_empty());
}

#[tokio::test]
async fn a_closed_app_once_per_session() {
    let (pool, id) = setup().await;
    session::tick(&pool, at(4, 13, 0)).await.unwrap();
    take(&pool, at(4, 13, 0)).await;
    let steam = BlockedApp {
        id: "x".into(),
        profile_id: "p".into(),
        app_id: "steam".into(),
        name: "Steam".into(),
        exec_names: "steam".into(),
        flatpak_id: None,
    };
    block::record_close(&pool, &id, &steam, at(4, 13, 5))
        .await
        .unwrap();
    let found = take(&pool, at(4, 13, 5)).await;
    assert_eq!(titles(&found), ["Steam was closed"]);
    block::record_close(&pool, &id, &steam, at(4, 13, 8))
        .await
        .unwrap();
    assert!(take(&pool, at(4, 13, 8)).await.is_empty());
}

#[tokio::test]
async fn the_tray_lines() {
    let (pool, id) = setup().await;
    let texts = english();
    let idle = notices::tray_status(&pool, &texts, at(4, 9, 0))
        .await
        .unwrap();
    assert_eq!(idle.headline, "Next session today 13:00");
    assert!(!idle.running);

    session::tick(&pool, at(4, 13, 0)).await.unwrap();
    let studying = notices::tray_status(&pool, &texts, at(4, 13, 40))
        .await
        .unwrap();
    assert_eq!(studying.headline, "Studying · 1 h 50 min left");
    assert_eq!(studying.credit.as_deref(), Some("Break credit: 5 min"));
    assert_eq!(studying.take_break.as_deref(), Some("Take a 5-min break"));

    session::take_break(&pool, &id, at(4, 13, 40))
        .await
        .unwrap();
    let pause = notices::tray_status(&pool, &texts, at(4, 13, 41))
        .await
        .unwrap();
    assert_eq!(pause.headline, "On a break until 13:45");
    assert!(pause.on_break);
    assert_eq!(pause.take_break, None);

    // In Dutch, after the session.
    session::tick(&pool, at(4, 15, 30)).await.unwrap();
    let dutch = notices::tray_status(&pool, &Texts::new(Language::Nl), at(4, 16, 0))
        .await
        .unwrap();
    assert_eq!(dutch.headline, "Geen studiesessie gepland");
}
