//! Tests for study sessions: planning, weekly repeats, the log and break credit,
//! and what the daemon does every tick. Times are in 2099, like the sample feeds.

use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime, Timelike, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::model::StudySession;
use lockin_core::session;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn clock(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

/// A local time on a 2099 day, e.g. at(4, 13, 0) = Wed 4 Nov 2099 13:00 local.
fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    session::local_time(date(2099, 11, day), clock(hour, minute)).unwrap()
}

/// "Now" in the tests: Mon 2 Nov 2099, 09:00.
fn monday() -> DateTime<Utc> {
    at(2, 9, 0)
}

fn minutes(count: i64) -> Duration {
    Duration::minutes(count)
}

async fn empty_database() -> Pool {
    db::open_in_memory().await.unwrap()
}

/// Plan Wed 4 Nov 13:00–15:30 and return it.
async fn wednesday_session(pool: &Pool) -> StudySession {
    let id = session::plan(
        pool,
        None,
        Some("Exercises"),
        at(4, 13, 0),
        at(4, 15, 30),
        monday(),
    )
    .await
    .unwrap();
    session::get(pool, &id).await.unwrap()
}

#[test]
fn semester_ends_in_january_or_july() {
    assert_eq!(session::semester_end(date(2099, 11, 4)), date(2100, 1, 31));
    assert_eq!(session::semester_end(date(2100, 1, 10)), date(2100, 1, 31));
    assert_eq!(session::semester_end(date(2100, 2, 1)), date(2100, 7, 31));
    assert_eq!(session::semester_end(date(2100, 7, 31)), date(2100, 7, 31));
    assert_eq!(session::semester_end(date(2100, 8, 1)), date(2101, 1, 31));
}

#[test]
fn credit_comes_in_whole_steps() {
    assert_eq!(session::credit_for(29 * 60), 0);
    assert_eq!(session::credit_for(30 * 60), 5 * 60);
    assert_eq!(session::credit_for(89 * 60), 10 * 60);
    assert_eq!(session::credit_for(90 * 60), 15 * 60);
}

#[tokio::test]
async fn planning_checks_the_times() {
    let pool = empty_database().await;
    let session = wednesday_session(&pool).await;
    assert_eq!(session.status, "planned");
    assert_eq!(session.title.as_deref(), Some("Exercises"));

    let plan = |start, end| session::plan(&pool, None, None, start, end, monday());
    // Ends before it starts, in the past, too long.
    assert!(plan(at(5, 13, 0), at(5, 12, 0)).await.is_err());
    assert!(plan(at(1, 13, 0), at(1, 14, 0)).await.is_err());
    assert!(plan(at(5, 6, 0), at(5, 19, 0)).await.is_err());
    // Overlaps Wednesday's session.
    let error = plan(at(4, 15, 0), at(4, 16, 0)).await.unwrap_err();
    assert!(error.to_string().contains("overlaps"));
    // Right after it is fine.
    assert!(plan(at(4, 15, 30), at(4, 16, 30)).await.is_ok());
}

#[tokio::test]
async fn a_blank_title_is_no_title() {
    let pool = empty_database().await;
    let id = session::plan(
        &pool,
        None,
        Some("  "),
        at(4, 13, 0),
        at(4, 14, 0),
        monday(),
    )
    .await
    .unwrap();
    assert_eq!(session::get(&pool, &id).await.unwrap().title, None);
}

#[tokio::test]
async fn weekly_repeat_runs_to_the_end_of_the_semester() {
    let pool = empty_database().await;
    // A one-off session in the way on Wed 18 Nov.
    session::plan(&pool, None, None, at(18, 14, 0), at(18, 15, 0), monday())
        .await
        .unwrap();

    let result = session::plan_weekly(
        &pool,
        None,
        Some("Databanken"),
        date(2099, 11, 4),
        clock(13, 0),
        clock(15, 30),
        None,
        monday(),
    )
    .await
    .unwrap();
    // Wednesdays from 4 Nov 2099 to 27 Jan 2100: 13 weeks, one overlapping.
    assert_eq!(result.planned, 12);
    assert_eq!(result.overlapping, 1);

    let sessions = session::between(&pool, at(1, 0, 0), at(1, 0, 0) + Duration::days(120))
        .await
        .unwrap();
    let weekly: Vec<_> = sessions
        .iter()
        .filter(|s| s.series_id.as_deref() == Some(result.series_id.as_str()))
        .collect();
    assert_eq!(weekly.len(), 12);
    assert!(
        weekly
            .iter()
            .all(|s| s.title.as_deref() == Some("Databanken"))
    );
}

#[tokio::test]
async fn weekly_repeat_keeps_the_local_time_when_the_clock_changes() {
    let pool = empty_database().await;
    let now = session::local_time(date(2100, 3, 1), clock(9, 0)).unwrap();
    // Wednesdays 10 March to 14 April 2100: the clock goes forward on 28 March.
    let result = session::plan_weekly(
        &pool,
        None,
        None,
        date(2100, 3, 10),
        clock(13, 0),
        clock(14, 0),
        Some(date(2100, 4, 14)),
        now,
    )
    .await
    .unwrap();
    assert_eq!(result.planned, 6);

    let sessions = session::between(&pool, now, now + Duration::days(60))
        .await
        .unwrap();
    for item in sessions {
        let start = DateTime::parse_from_rfc3339(&item.start_at)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!((start.hour(), start.minute()), (13, 0));
    }
}

#[tokio::test]
async fn weekly_repeat_needs_a_future_start_and_room() {
    let pool = empty_database().await;
    let weekly = |first, until| {
        session::plan_weekly(
            &pool,
            None,
            None,
            first,
            clock(13, 0),
            clock(14, 0),
            until,
            monday(),
        )
    };
    // The first session is already past.
    assert!(weekly(date(2099, 11, 1), None).await.is_err());
    // Ends before it starts.
    assert!(
        weekly(date(2099, 11, 4), Some(date(2099, 11, 3)))
            .await
            .is_err()
    );
    // Every week taken: refused, and the repeat isn't kept.
    weekly(date(2099, 11, 4), Some(date(2099, 11, 18)))
        .await
        .unwrap();
    assert!(
        weekly(date(2099, 11, 4), Some(date(2099, 11, 18)))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn only_sessions_still_to_come_can_change() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;

    session::update(
        &pool,
        &item.id,
        None,
        Some("Review"),
        at(4, 14, 0),
        at(4, 16, 0),
        monday(),
    )
    .await
    .unwrap();
    let changed = session::get(&pool, &item.id).await.unwrap();
    assert_eq!(changed.title.as_deref(), Some("Review"));
    assert_eq!(
        DateTime::parse_from_rfc3339(&changed.start_at).unwrap(),
        at(4, 14, 0)
    );

    // Once it has started, it can't be changed or removed.
    session::start(&pool, &item.id, at(4, 14, 0), Some("daemon"))
        .await
        .unwrap();
    let later = at(4, 14, 10);
    assert!(
        session::update(
            &pool,
            &item.id,
            None,
            None,
            at(4, 15, 0),
            at(4, 16, 0),
            later
        )
        .await
        .is_err()
    );
    assert!(session::remove(&pool, &item.id, later).await.is_err());
}

#[tokio::test]
async fn removing_and_bringing_back() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;
    session::remove(&pool, &item.id, monday()).await.unwrap();
    assert!(
        session::between(&pool, at(4, 0, 0), at(5, 0, 0))
            .await
            .unwrap()
            .is_empty()
    );

    session::restore(&pool, &item.id, monday()).await.unwrap();
    assert_eq!(
        session::between(&pool, at(4, 0, 0), at(5, 0, 0))
            .await
            .unwrap()
            .len(),
        1
    );

    // Another session took its time: it can't come back.
    session::remove(&pool, &item.id, monday()).await.unwrap();
    session::plan(&pool, None, None, at(4, 13, 0), at(4, 14, 0), monday())
        .await
        .unwrap();
    assert!(session::restore(&pool, &item.id, monday()).await.is_err());
}

#[tokio::test]
async fn removing_this_and_following_weeks() {
    let pool = empty_database().await;
    session::plan_weekly(
        &pool,
        None,
        None,
        date(2099, 11, 4),
        clock(13, 0),
        clock(14, 0),
        Some(date(2099, 12, 2)),
        monday(),
    )
    .await
    .unwrap();
    let all = session::between(&pool, monday(), monday() + Duration::days(40))
        .await
        .unwrap();
    assert_eq!(all.len(), 5);

    // From the third week (18 Nov) on.
    let removed = session::remove_following(&pool, &all[2].id, monday())
        .await
        .unwrap();
    assert_eq!(removed, 3);
    let left = session::between(&pool, monday(), monday() + Duration::days(40))
        .await
        .unwrap();
    assert_eq!(left.len(), 2);
}

#[tokio::test]
async fn studying_earns_break_credit() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;
    let start = at(4, 13, 0);

    // Not started yet.
    let state = session::state_now(&pool, &item, start - minutes(5))
        .await
        .unwrap();
    assert_eq!(state.phase, "planned");

    session::start(&pool, &item.id, start, Some("daemon"))
        .await
        .unwrap();
    let item = session::get(&pool, &item.id).await.unwrap();
    assert_eq!(item.status, "running");

    // 29 minutes in: no credit yet, so no break.
    let state = session::state_now(&pool, &item, start + minutes(29))
        .await
        .unwrap();
    assert_eq!(state.phase, "working");
    assert_eq!(state.credit_left_seconds, 0);
    assert!(!state.can_take_break);
    assert!(
        session::take_break(&pool, &item.id, start + minutes(29))
            .await
            .is_err()
    );

    // An hour in: 10 minutes of credit. A break, ended after 3 minutes.
    session::take_break(&pool, &item.id, start + minutes(60))
        .await
        .unwrap();
    let state = session::state_now(&pool, &item, start + minutes(61))
        .await
        .unwrap();
    assert_eq!(state.phase, "break");
    assert_eq!(
        state.break_ends_at.as_deref(),
        Some(
            (start + minutes(65))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                .as_str()
        )
    );
    session::resume(&pool, &item.id, start + minutes(63), None)
        .await
        .unwrap();

    // The 2 minutes not taken stay as credit; study time doesn't count the break.
    let state = session::state_now(&pool, &item, start + minutes(70))
        .await
        .unwrap();
    assert_eq!(state.phase, "working");
    assert_eq!(state.worked_seconds, 67 * 60);
    assert_eq!(state.credit_used_seconds, 3 * 60);
    assert_eq!(state.credit_left_seconds, 7 * 60);
    assert!(state.can_take_break);
}

#[tokio::test]
async fn a_break_ends_by_itself() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;
    let start = at(4, 13, 0);
    session::start(&pool, &item.id, start, None).await.unwrap();
    let item = session::get(&pool, &item.id).await.unwrap();

    // 30 minutes in: 5 minutes of credit. Take 2, then a break that may last only 3.
    session::take_break(&pool, &item.id, start + minutes(30))
        .await
        .unwrap();
    session::resume(&pool, &item.id, start + minutes(32), None)
        .await
        .unwrap();
    session::take_break(&pool, &item.id, start + minutes(40))
        .await
        .unwrap();

    // No "resume" in the log, yet the break is over after 3 minutes.
    let state = session::state_now(&pool, &item, start + minutes(50))
        .await
        .unwrap();
    assert_eq!(state.phase, "working");
    assert_eq!(state.credit_used_seconds, 5 * 60);
    assert_eq!(state.credit_left_seconds, 0);
    assert_eq!(state.worked_seconds, (30 + 8 + 7) * 60);

    // The daemon's tick writes the resume, at the moment the break ended.
    let result = session::tick(&pool, start + minutes(50)).await.unwrap();
    assert_eq!(result.resumed, vec![item.id.clone()]);
    let events = session::events(&pool, &item.id).await.unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.kind, "resume");
    assert_eq!(last.detail.as_deref(), Some("auto"));
    assert_eq!(
        DateTime::parse_from_rfc3339(&last.at).unwrap(),
        start + minutes(43)
    );
    // And not twice.
    let again = session::tick(&pool, start + minutes(51)).await.unwrap();
    assert!(again.resumed.is_empty());
}

#[tokio::test]
async fn the_daemon_starts_and_ends_sessions_on_time() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;

    // Before the start: nothing happens.
    let result = session::tick(&pool, at(4, 12, 59)).await.unwrap();
    assert!(result.started.is_empty());

    // A few seconds after the start: it starts.
    let result = session::tick(&pool, at(4, 13, 0) + Duration::seconds(5))
        .await
        .unwrap();
    assert_eq!(result.started, vec![item.id.clone()]);
    let events = session::events(&pool, &item.id).await.unwrap();
    assert_eq!(events[0].kind, "start");
    assert_eq!(events[0].detail.as_deref(), Some("daemon"));
    assert!(session::running(&pool).await.unwrap().is_some());

    // After the end: done, with the end logged at the planned end.
    let result = session::tick(&pool, at(4, 15, 31)).await.unwrap();
    assert_eq!(result.ended, vec![item.id.clone()]);
    let done = session::get(&pool, &item.id).await.unwrap();
    assert_eq!(done.status, "done");
    let events = session::events(&pool, &item.id).await.unwrap();
    assert_eq!(
        DateTime::parse_from_rfc3339(&events.last().unwrap().at).unwrap(),
        at(4, 15, 30)
    );
    let state = session::state_now(&pool, &done, at(4, 16, 0))
        .await
        .unwrap();
    assert_eq!(state.phase, "over");
    // Started 5 seconds after 13:00.
    assert_eq!(state.worked_seconds, 150 * 60 - 5);
    assert!(session::running(&pool).await.unwrap().is_none());
}

#[tokio::test]
async fn a_start_the_daemon_didnt_see_is_missed() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;
    // The PC was off at 13:00; the daemon comes up at 13:10.
    let result = session::tick(&pool, at(4, 13, 10)).await.unwrap();
    assert_eq!(result.missed, vec![item.id.clone()]);
    assert!(result.started.is_empty());
    assert_eq!(
        session::get(&pool, &item.id).await.unwrap().status,
        "missed"
    );
    // Missed sessions don't come back as the next one.
    assert!(session::next(&pool, at(4, 13, 10)).await.unwrap().is_none());
}

#[tokio::test]
async fn the_emergency_button_stops_a_session() {
    let pool = empty_database().await;
    let item = wednesday_session(&pool).await;
    let start = at(4, 13, 0);
    session::start(&pool, &item.id, start, None).await.unwrap();
    session::emergency_stop(&pool, &item.id, start + minutes(40))
        .await
        .unwrap();

    let stopped = session::get(&pool, &item.id).await.unwrap();
    assert_eq!(stopped.status, "stopped");
    let state = session::state_now(&pool, &stopped, start + minutes(90))
        .await
        .unwrap();
    assert_eq!(state.phase, "over");
    assert_eq!(state.worked_seconds, 40 * 60);
    // A stopped session isn't ended again by the daemon.
    let result = session::tick(&pool, at(4, 16, 0)).await.unwrap();
    assert!(result.ended.is_empty());
}

#[tokio::test]
async fn next_is_the_first_planned_session() {
    let pool = empty_database().await;
    assert!(session::next(&pool, monday()).await.unwrap().is_none());
    let item = wednesday_session(&pool).await;
    session::plan(&pool, None, None, at(5, 9, 0), at(5, 10, 0), monday())
        .await
        .unwrap();
    assert_eq!(
        session::next(&pool, monday()).await.unwrap().unwrap().id,
        item.id
    );
}
