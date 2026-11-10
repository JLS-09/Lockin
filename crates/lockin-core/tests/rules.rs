//! Tests for filter rules, manual exceptions and importing a saved source.

use chrono::{TimeZone, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::model::ClassBlock;
use lockin_core::{import, timeedit};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");
const KENNISCLIPS: &str = "E701033A. Wiskunde I, zelfstandig werk, Kennisclips";

/// A fresh database with the sample feed already imported.
async fn database_with_sample() -> Pool {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    pool
}

async fn all_blocks(pool: &Pool) -> Vec<ClassBlock> {
    db::blocks_between(
        pool,
        Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap()
}

async fn block(pool: &Pool, title: &str) -> ClassBlock {
    all_blocks(pool)
        .await
        .into_iter()
        .find(|b| b.title == title)
        .unwrap()
}

#[tokio::test]
async fn new_rule_applies_without_sync() {
    let pool = database_with_sample().await;

    let changed = db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();
    assert_eq!(changed, 1);
    assert!(block(&pool, KENNISCLIPS).await.is_ignored());
}

#[tokio::test]
async fn removed_rule_stops_ignoring() {
    let pool = database_with_sample().await;
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();

    let rules = db::filter_rules(&pool).await.unwrap();
    let changed = db::remove_rule(&pool, &rules[0].id).await.unwrap();
    assert_eq!(changed, 1);
    assert!(!block(&pool, KENNISCLIPS).await.is_ignored());
    assert!(db::filter_rules(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn manual_exception_wins_until_reset() {
    let pool = database_with_sample().await;
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();
    let id = block(&pool, KENNISCLIPS).await.id;

    // Keep the block although the rule matches.
    db::set_ignored_override(&pool, &id, Some(false))
        .await
        .unwrap();
    assert!(!block(&pool, KENNISCLIPS).await.is_ignored());

    // Back to the rules.
    db::set_ignored_override(&pool, &id, None).await.unwrap();
    assert!(block(&pool, KENNISCLIPS).await.is_ignored());
}

#[tokio::test]
async fn block_found_by_start_of_id() {
    let pool = database_with_sample().await;
    let id = block(&pool, KENNISCLIPS).await.id;

    let found = db::find_block_id(&pool, &id[..8]).await.unwrap();
    assert_eq!(found, id);
    assert!(db::find_block_id(&pool, "zzzzzzzz").await.is_err());
}

#[tokio::test]
async fn file_source_is_imported_again() {
    let pool = db::open_in_memory().await.unwrap();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/timeedit_sample.ics"
    );
    let url = format!("file://{path}");
    let source = db::add_timeedit_source(&pool, &url).await.unwrap();

    let first = import::import_source(&pool, &source, &url).await.unwrap();
    assert_eq!(first.added, 4);

    // What `lockin-daemon sync` does: go over the saved sources.
    let sources = db::sources(&pool).await.unwrap();
    assert_eq!(sources.len(), 1);
    let again = import::import_source(&pool, &sources[0].id, &sources[0].url)
        .await
        .unwrap();
    assert_eq!(again.updated, 4);
}

#[tokio::test]
async fn removing_a_feed_keeps_blocks_another_feed_has() {
    let pool = db::open_in_memory().await.unwrap();
    let events = timeedit::parse(SAMPLE).unwrap();

    // The old feed has all 4 blocks, the new feed only the first 3.
    let old = db::add_timeedit_source(&pool, "https://example.test/old.ics")
        .await
        .unwrap();
    let new = db::add_timeedit_source(&pool, "https://example.test/new.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &old, &events).await.unwrap();
    let ids_before: Vec<String> = all_blocks(&pool).await.into_iter().map(|b| b.id).collect();

    // Remove the old feed first: all 4 blocks are hidden.
    let hidden = db::remove_source(&pool, &old).await.unwrap();
    assert_eq!(hidden, 4);
    assert!(all_blocks(&pool).await.is_empty());

    // Importing the new feed brings its 3 blocks back, with the same ids.
    db::save_feed(&pool, &new, &events[..3]).await.unwrap();
    let ids_after: Vec<String> = all_blocks(&pool).await.into_iter().map(|b| b.id).collect();
    assert_eq!(ids_after, ids_before[..3]);
    assert_eq!(db::sources(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn history_adds_only_unknown_past_blocks() {
    let pool = database_with_sample().await;
    let mut history = timeedit::parse(SAMPLE).unwrap();

    // Two blocks from the start of a past semester, with their own UIDs.
    for (i, event) in history.iter_mut().take(2).enumerate() {
        event.uid = format!("past-{i}@timeedit");
        event.start = Utc
            .with_ymd_and_hms(2020, 9, 21 + i as u32, 8, 0, 0)
            .unwrap();
        event.end = event.start + chrono::Duration::hours(2);
    }
    // A copy of a known past block with a different UID (title and start time match).
    let mut copy = history[0].clone();
    copy.uid = "other-uid@timeedit".to_string();
    history.push(copy);

    // The 2099 blocks are skipped (in the future and already known), as is the copy.
    let added = db::import_history(&pool, &history).await.unwrap();
    assert_eq!(added, 2);

    // Importing the same file again adds nothing.
    let again = db::import_history(&pool, &history).await.unwrap();
    assert_eq!(again, 0);

    // The imported blocks don't belong to a feed, so `sync` won't touch them.
    let past = db::blocks_between(
        &pool,
        Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(past.len(), 2);
    assert!(past[0].course_id.is_some());
}
