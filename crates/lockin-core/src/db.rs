//! Database access: opening SQLite and saving/reading class blocks.

use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// The database connection pool. Re-exported so the app and daemon
/// don't need their own sqlx dependency.
pub type Pool = SqlitePool;

use crate::filter;
use crate::model::{ClassBlock, FeedEvent, FilterRule};
use crate::{new_id, now_text};

/// Open (or create) the database file and run any new migrations.
pub async fn open(file: &Path) -> Result<SqlitePool> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }

    let options = SqliteConnectOptions::new()
        .filename(file)
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// An empty in-memory database, handy for tests.
pub async fn open_in_memory() -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Find a TimeEdit source by URL, or add it. Returns its id.
pub async fn add_timeedit_source(pool: &SqlitePool, url: &str) -> Result<String> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM calendar_source WHERE url = ?")
            .bind(url)
            .fetch_optional(pool)
            .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO calendar_source (id, kind, url, updated_at) VALUES (?, 'timeedit', ?, ?)",
    )
    .bind(&id)
    .bind(url)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(id)
}

/// Add an "ignore" rule on the title, for all sources.
pub async fn add_title_rule(pool: &SqlitePool, pattern: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO filter_rule (id, field, pattern, updated_at) VALUES (?, 'title', ?, ?)",
    )
    .bind(new_id())
    .bind(pattern)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// All active filter rules.
pub async fn filter_rules(pool: &SqlitePool) -> Result<Vec<FilterRule>> {
    let rules = sqlx::query_as("SELECT id, field, pattern FROM filter_rule WHERE deleted = 0")
        .fetch_all(pool)
        .await?;
    Ok(rules)
}

/// What one sync changed.
#[derive(Debug, Default, PartialEq)]
pub struct SyncResult {
    pub added: u32,
    pub updated: u32,
    pub cancelled: u32,
}

/// Save the events of one feed.
///
/// - New UID: insert a new block.
/// - Known UID: update it. If the time changed, mark it as "moved".
///   The block keeps its id, so linked activities stay attached.
/// - A future block that is no longer in the feed: mark it as "cancelled"
///   instead of deleting it.
pub async fn save_feed(
    pool: &SqlitePool,
    source_id: &str,
    events: &[FeedEvent],
) -> Result<SyncResult> {
    let rules = filter_rules(pool).await?;
    let now = now_text();
    let mut result = SyncResult::default();

    let mut transaction = pool.begin().await?;

    for event in events {
        let start = time_text(event.start);
        let end = time_text(event.end);
        let ignored = filter::is_ignored(event, &rules);

        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT start_at, end_at FROM class_block WHERE ical_uid = ?")
                .bind(&event.uid)
                .fetch_optional(&mut *transaction)
                .await?;

        match existing {
            None => {
                sqlx::query(
                    "INSERT INTO class_block
                       (id, source_id, ical_uid, title, location, description,
                        start_at, end_at, ignored_by_rule, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(new_id())
                .bind(source_id)
                .bind(&event.uid)
                .bind(&event.title)
                .bind(&event.location)
                .bind(&event.description)
                .bind(&start)
                .bind(&end)
                .bind(ignored)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
                result.added += 1;
            }
            Some((old_start, old_end)) => {
                let moved = old_start != start || old_end != end;

                // Keep "moved" once it's set; a block that comes back after
                // being cancelled becomes "normal" again.
                let status_sql = if moved {
                    "'moved'"
                } else {
                    "CASE WHEN status = 'cancelled' THEN 'normal' ELSE status END"
                };

                let sql = format!(
                    "UPDATE class_block
                     SET title = ?, location = ?, description = ?, start_at = ?, end_at = ?,
                         ignored_by_rule = ?, status = {status_sql}, updated_at = ?
                     WHERE ical_uid = ?"
                );

                sqlx::query(&sql)
                    .bind(&event.title)
                    .bind(&event.location)
                    .bind(&event.description)
                    .bind(&start)
                    .bind(&end)
                    .bind(ignored)
                    .bind(&now)
                    .bind(&event.uid)
                    .execute(&mut *transaction)
                    .await?;
                result.updated += 1;
            }
        }
    }

    // Future blocks from this source that are no longer in the feed.
    let known_uids: Vec<String> =
        sqlx::query_scalar("SELECT ical_uid FROM class_block WHERE source_id = ? AND status != 'cancelled' AND start_at > ?")
            .bind(source_id)
            .bind(&now)
            .fetch_all(&mut *transaction)
            .await?;

    for uid in known_uids {
        let still_in_feed = events.iter().any(|e| e.uid == uid);
        if !still_in_feed {
            sqlx::query(
                "UPDATE class_block SET status = 'cancelled', updated_at = ? WHERE ical_uid = ?",
            )
            .bind(&now)
            .bind(&uid)
            .execute(&mut *transaction)
            .await?;
            result.cancelled += 1;
        }
    }

    sqlx::query("UPDATE calendar_source SET last_fetched_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&now)
        .bind(source_id)
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await?;
    Ok(result)
}

/// All blocks that start between `from` and `to`, sorted by start time.
pub async fn blocks_between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<ClassBlock>> {
    let blocks = sqlx::query_as(
        "SELECT id, ical_uid, title, location, start_at, end_at,
                ignored_by_rule, ignored_override, status
         FROM class_block
         WHERE deleted = 0 AND start_at >= ? AND start_at < ?
         ORDER BY start_at",
    )
    .bind(time_text(from))
    .bind(time_text(to))
    .fetch_all(pool)
    .await?;
    Ok(blocks)
}
