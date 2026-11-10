//! Tests for the TimeEdit import, using a saved .ics file
//! so we never hit the real TimeEdit server.

use chrono::{TimeZone, Utc};
use lockin_core::{db, timeedit};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");

#[test]
fn parses_all_events() {
    let events = timeedit::parse(SAMPLE).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].uid, "sample-1@timeedit");
    assert_eq!(events[0].title, "Besturingssystemen - HOC");
}

#[test]
fn brussels_time_becomes_utc() {
    let events = timeedit::parse(SAMPLE).unwrap();
    // 09:00 in Brussels in November (UTC+1) is 08:00 UTC.
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
    assert_eq!(first.added, 3);

    let second = db::save_feed(&pool, &source, &events).await.unwrap();
    assert_eq!(second.added, 0);
    assert_eq!(second.updated, 3);
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
    assert_eq!(ignored[0].title, "Zelfstandig werk - Wiskunde");
}

#[tokio::test]
async fn moved_and_cancelled_blocks_are_marked() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    let mut events = timeedit::parse(SAMPLE).unwrap();
    db::save_feed(&pool, &source, &events).await.unwrap();

    // The HOC moves one hour later, and the WPO disappears from the feed.
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
    assert_eq!(status_of("Besturingssystemen - HOC"), "moved");
    assert_eq!(status_of("Besturingssystemen - WPO"), "cancelled");
}
