//! Feed syncs only write rows that really changed, so `updated_at` stays put
//! for everything else. The cloud sync (phase 5) sends rows by `updated_at`:
//! an hourly feed sync must not mark every class as changed, or one computer's
//! sync would overwrite a check-off or "ignore" made on another.

use chrono::{DateTime, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::{deadline, timeedit, ufora};

const TIMEEDIT: &str = include_str!("fixtures/timeedit_sample.ics");
const UFORA: &str = include_str!("fixtures/ufora_sample.ics");

/// A time long ago, to see whether a sync touched a row.
const OLD: &str = "2000-01-01T00:00:00Z";

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2099-11-12T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

/// Set `updated_at` of every row of a table to OLD.
async fn age(pool: &Pool, table: &str) {
    sqlx::query(&format!("UPDATE {table} SET updated_at = ?"))
        .bind(OLD)
        .execute(pool)
        .await
        .unwrap();
}

/// The ical_uid of each row of a table whose `updated_at` isn't OLD any more.
async fn touched(pool: &Pool, table: &str) -> Vec<String> {
    sqlx::query_scalar(&format!(
        "SELECT ical_uid FROM {table} WHERE updated_at != ? ORDER BY ical_uid"
    ))
    .bind(OLD)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn source_updated_at(pool: &Pool, source: &str) -> String {
    sqlx::query_scalar("SELECT updated_at FROM calendar_source WHERE id = ?")
        .bind(source)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_unchanged_class_is_not_written_again() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/te.ics")
        .await
        .unwrap();
    let events = timeedit::parse(TIMEEDIT).unwrap();
    db::save_feed(&pool, &source, &events).await.unwrap();
    age(&pool, "class_block").await;
    age(&pool, "calendar_source").await;

    // The same feed again: every class is seen, none is written.
    let result = db::save_feed(&pool, &source, &events).await.unwrap();
    assert_eq!(result.added, 0);
    assert_eq!(result.updated as usize, events.len());
    assert!(touched(&pool, "class_block").await.is_empty());
    // Fetching isn't a change either: last_fetched_at stays on this computer.
    assert_eq!(source_updated_at(&pool, &source).await, OLD);

    // One class moves and another gets a new room: only those two are written.
    let mut changed = events.clone();
    changed[0].start += chrono::Duration::hours(1);
    changed[0].end += chrono::Duration::hours(1);
    changed[1].location = Some("Another room".to_string());
    let result = db::save_feed(&pool, &source, &changed).await.unwrap();
    assert_eq!(result.moved, 1);
    assert_eq!(
        touched(&pool, "class_block").await,
        [changed[0].uid.clone(), changed[1].uid.clone()]
    );
}

#[tokio::test]
async fn an_unchanged_ufora_deadline_is_not_written_again() {
    let pool = db::open_in_memory().await.unwrap();
    let timeedit_source = db::add_timeedit_source(&pool, "https://example.test/te.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &timeedit_source, &timeedit::parse(TIMEEDIT).unwrap())
        .await
        .unwrap();
    let source = db::add_source(&pool, "https://example.test/ufora.ics", "ufora")
        .await
        .unwrap();
    let deadlines = ufora::parse(UFORA).unwrap();
    deadline::save_ufora(&pool, &source, &deadlines, now())
        .await
        .unwrap();
    age(&pool, "deadline").await;
    age(&pool, "calendar_source").await;

    let result = deadline::save_ufora(&pool, &source, &deadlines, now())
        .await
        .unwrap();
    assert_eq!(result.added, 0);
    assert!(result.updated > 0);
    assert!(touched(&pool, "deadline").await.is_empty());
    assert_eq!(source_updated_at(&pool, &source).await, OLD);

    // A new title is a change.
    let mut changed = deadlines.clone();
    changed[0].title = "Verslag 1 (nieuwe versie)".to_string();
    deadline::save_ufora(&pool, &source, &changed, now())
        .await
        .unwrap();
    assert_eq!(touched(&pool, "deadline").await, [changed[0].uid.clone()]);
}
