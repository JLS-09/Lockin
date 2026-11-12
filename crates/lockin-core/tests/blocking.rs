//! Tests for block profiles: making them, their apps, what the daemon blocks
//! when, and the rule that a running session's profile can't be loosened.

use chrono::{Duration, Utc};
use lockin_core::apps::InstalledApp;
use lockin_core::db::{self, Pool};
use lockin_core::model::SessionDetails;
use lockin_core::{block, session};

fn discord() -> InstalledApp {
    InstalledApp {
        id: "discord".into(),
        name: "Discord".into(),
        exec_names: vec!["discord".into(), "Discord".into()],
        flatpak_id: None,
    }
}

async fn database() -> Pool {
    db::open_in_memory().await.unwrap()
}

/// A session with this profile that started 40 minutes ago (so it has 5 minutes of credit).
async fn running_session(pool: &Pool, profile_id: &str) -> String {
    let now = Utc::now();
    let details = SessionDetails {
        profile_id: Some(profile_id.to_string()),
        ..Default::default()
    };
    let id = session::plan(
        pool,
        &details,
        now + Duration::minutes(1),
        now + Duration::hours(2),
        now,
    )
    .await
    .unwrap();
    session::start(pool, &id, now - Duration::minutes(40), None)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn profiles_have_their_own_names() {
    let pool = database().await;
    let strict = block::add_profile(&pool, " Strict ").await.unwrap();
    assert!(block::add_profile(&pool, "strict").await.is_err());
    assert!(block::add_profile(&pool, "  ").await.is_err());
    block::add_profile(&pool, "Light").await.unwrap();
    block::rename_profile(&pool, &strict, "Exam week")
        .await
        .unwrap();

    let names: Vec<String> = block::profiles(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["Exam week", "Light"]);
}

#[tokio::test]
async fn apps_and_programs_on_a_profile() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    block::add_app(&pool, &strict, &discord()).await.unwrap();
    // Not twice.
    assert!(block::add_app(&pool, &strict, &discord()).await.is_err());
    // A program without a menu entry, by name.
    block::add_program(&pool, &strict, "steam").await.unwrap();
    assert!(
        block::add_program(&pool, &strict, "/usr/bin/steam")
            .await
            .is_err()
    );

    let apps = block::apps_of(&pool, &strict).await.unwrap();
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].name, "Discord");
    assert_eq!(apps[0].exec_names, "discord\nDiscord");
    assert_eq!(apps[1].app_id, "program:steam");

    block::remove_app(&pool, &apps[1].id).await.unwrap();
    assert_eq!(block::apps_of(&pool, &strict).await.unwrap().len(), 1);
}

#[tokio::test]
async fn lockin_and_the_desktop_cant_be_blocked() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    assert!(
        block::add_program(&pool, &strict, "plasmashell")
            .await
            .is_err()
    );
    assert!(
        block::add_program(&pool, &strict, "lockin-daemon")
            .await
            .is_err()
    );
    let lockin = InstalledApp {
        id: "io.github.jls_09.lockin".into(),
        name: "Lockin".into(),
        exec_names: vec!["lockin".into()],
        flatpak_id: None,
    };
    assert!(block::add_app(&pool, &strict, &lockin).await.is_err());
}

#[tokio::test]
async fn blocking_follows_the_running_session() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    block::add_app(&pool, &strict, &discord()).await.unwrap();

    // No session: nothing blocked.
    let (session_id, apps) = block::blocked_now(&pool, Utc::now()).await.unwrap();
    assert!(session_id.is_none() && apps.is_empty());

    // Studying: the profile's apps.
    let id = running_session(&pool, &strict).await;
    let (session_id, apps) = block::blocked_now(&pool, Utc::now()).await.unwrap();
    assert_eq!(session_id.as_deref(), Some(id.as_str()));
    assert_eq!(apps.len(), 1);

    // On a break: nothing.
    session::take_break(&pool, &id, Utc::now()).await.unwrap();
    let (_, apps) = block::blocked_now(&pool, Utc::now()).await.unwrap();
    assert!(apps.is_empty());

    // Back at work: blocked again.
    session::resume(&pool, &id, Utc::now(), None).await.unwrap();
    let (_, apps) = block::blocked_now(&pool, Utc::now()).await.unwrap();
    assert_eq!(apps.len(), 1);
}

#[tokio::test]
async fn a_running_profile_can_only_get_stricter() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    let entry = block::add_app(&pool, &strict, &discord()).await.unwrap();
    running_session(&pool, &strict).await;

    let profiles = block::profiles(&pool).await.unwrap();
    assert!(profiles[0].in_use);
    // Adding works, removing and deleting don't.
    block::add_program(&pool, &strict, "steam").await.unwrap();
    assert!(block::remove_app(&pool, &entry).await.is_err());
    assert!(block::remove_profile(&pool, &strict).await.is_err());
}

#[tokio::test]
async fn deleting_a_profile_frees_its_planned_sessions() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    let now = Utc::now();
    let details = SessionDetails {
        profile_id: Some(strict.clone()),
        ..Default::default()
    };
    let id = session::plan(
        &pool,
        &details,
        now + Duration::hours(1),
        now + Duration::hours(2),
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        session::last_profile(&pool).await.unwrap().as_deref(),
        Some(strict.as_str())
    );
    assert_eq!(
        block::planned_sessions_using(&pool, &strict).await.unwrap(),
        1
    );

    assert_eq!(block::remove_profile(&pool, &strict).await.unwrap(), 1);
    assert_eq!(session::get(&pool, &id).await.unwrap().profile_id, None);
    assert!(block::profiles(&pool).await.unwrap().is_empty());
    assert_eq!(session::last_profile(&pool).await.unwrap(), None);
}

#[tokio::test]
async fn closes_are_counted_per_app() {
    let pool = database().await;
    let strict = block::add_profile(&pool, "Strict").await.unwrap();
    block::add_app(&pool, &strict, &discord()).await.unwrap();
    block::add_program(&pool, &strict, "steam").await.unwrap();
    let id = running_session(&pool, &strict).await;
    let apps = block::apps_of(&pool, &strict).await.unwrap();

    let now = Utc::now();
    block::record_close(&pool, &id, &apps[1], now - Duration::minutes(10))
        .await
        .unwrap();
    block::record_close(&pool, &id, &apps[0], now - Duration::minutes(5))
        .await
        .unwrap();
    block::record_close(&pool, &id, &apps[1], now)
        .await
        .unwrap();

    let closes = block::closes(&pool, &id).await.unwrap();
    assert_eq!(closes.len(), 2);
    assert_eq!((closes[0].name.as_str(), closes[0].count), ("steam", 2));
    assert_eq!((closes[1].name.as_str(), closes[1].count), ("Discord", 1));
}
