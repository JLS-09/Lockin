//! Tests for the TimeEdit import, using a saved .ics file
//! so we never hit the real TimeEdit server.

use chrono::{TimeZone, Utc};
use lockin_core::{db, timeedit};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");

#[test]
fn parses_all_events() {
    let events = timeedit::parse(SAMPLE).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].uid, "sample-1@timeedit");
    assert_eq!(
        events[0].title,
        "E702010A. Signalen en systemen, hoorcollege"
    );
}

#[test]
fn brussels_time_becomes_utc() {
    let events = timeedit::parse(SAMPLE).unwrap();
    // The Kennisclips block is in Brussels time: 09:00 in November (UTC+1) is 08:00 UTC.
    assert_eq!(
        events[2].start,
        Utc.with_ymd_and_hms(2099, 11, 12, 8, 0, 0).unwrap()
    );
}

#[tokio::test]
async fn sync_twice_gives_no_duplicates() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    let events = timeedit::parse(SAMPLE).unwrap();

    let first = db::save_feed(&pool, &source, &events).await.unwrap();
    assert_eq!(first.added, 4);

    let second = db::save_feed(&pool, &source, &events).await.unwrap();
    assert_eq!(second.added, 0);
    assert_eq!(second.updated, 4);
}

#[tokio::test]
async fn rule_ignores_zelfstandig_werk() {
    let pool = db::open_in_memory().await.unwrap();
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();

    let all = db::blocks_between(
        &pool,
        Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    let ignored: Vec<_> = all.iter().filter(|b| b.is_ignored()).collect();
    assert_eq!(ignored.len(), 1);
    assert_eq!(
        ignored[0].title,
        "E701033A. Wiskunde I, zelfstandig werk, Kennisclips"
    );
}

#[tokio::test]
async fn moved_and_cancelled_blocks_are_marked() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    let mut events = timeedit::parse(SAMPLE).unwrap();
    db::save_feed(&pool, &source, &events).await.unwrap();

    // The hoorcollege moves one hour later, and the werkcollege disappears from the feed.
    events[0].start += chrono::Duration::hours(1);
    events[0].end += chrono::Duration::hours(1);
    events.remove(1);

    let result = db::save_feed(&pool, &source, &events).await.unwrap();
    assert_eq!(result.cancelled, 1);

    let all = db::blocks_between(
        &pool,
        Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    let status_of = |title: &str| {
        all.iter()
            .find(|b| b.title == title)
            .unwrap()
            .status
            .clone()
    };
    assert_eq!(
        status_of("E702010A. Signalen en systemen, hoorcollege"),
        "moved"
    );
    assert_eq!(
        status_of("E702010A. Signalen en systemen GR02, werkcollege"),
        "cancelled"
    );
}

#[test]
fn room_comes_from_description_when_location_is_missing() {
    let events = timeedit::parse(SAMPLE).unwrap();
    assert_eq!(
        events[3].location.as_deref(),
        Some("GSCHD.1.106 Auditorium D")
    );
}

#[tokio::test]
async fn blocks_are_linked_to_their_course() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();

    // Three different course codes in the sample.
    let courses = db::courses(&pool).await.unwrap();
    assert_eq!(courses.len(), 3);

    let all = db::blocks_between(
        &pool,
        Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap();

    // The hoorcollege and the GR02 werkcollege belong to the same course.
    assert!(all[0].course_id.is_some());
    assert_eq!(all[0].course_id, all[1].course_id);
    assert_eq!(all[0].kind.as_deref(), Some("hoorcollege"));
    assert_eq!(all[1].kind.as_deref(), Some("werkcollege"));
}
