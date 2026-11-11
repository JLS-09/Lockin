//! Database access: opening SQLite and saving/reading class blocks.

use std::path::Path;

use anyhow::{Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::SqliteConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// The database connection pool. Re-exported so the app and daemon
/// don't need their own sqlx dependency.
pub type Pool = SqlitePool;

use crate::model::{CalendarSource, ClassBlock, Course, CourseKind, FeedEvent, FilterRule};
use crate::{activity, filter};
use crate::{new_id, now_text, title};

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
    add_source(pool, url, "timeedit").await
}

/// Find a source by URL, or add it as `kind` ("timeedit" or "ufora"). Returns its id.
pub async fn add_source(pool: &SqlitePool, url: &str, kind: &str) -> Result<String> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM calendar_source WHERE url = ?")
            .bind(url)
            .fetch_optional(pool)
            .await?;

    if let Some(id) = existing {
        sqlx::query(
            "UPDATE calendar_source SET deleted = 0, updated_at = ? WHERE id = ? AND deleted = 1",
        )
        .bind(now_text())
        .bind(&id)
        .execute(pool)
        .await?;
        return Ok(id);
    }

    let id = new_id();
    sqlx::query("INSERT INTO calendar_source (id, kind, url, updated_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(kind)
        .bind(url)
        .bind(now_text())
        .execute(pool)
        .await?;
    Ok(id)
}

/// All saved calendar sources.
pub async fn sources(pool: &SqlitePool) -> Result<Vec<CalendarSource>> {
    let sources = sqlx::query_as(
        "SELECT id, kind, url, last_fetched_at FROM calendar_source WHERE deleted = 0",
    )
    .fetch_all(pool)
    .await?;
    Ok(sources)
}

/// How many (not deleted) classes each source holds, by source id.
pub async fn classes_per_source(pool: &SqlitePool) -> Result<Vec<(String, i64)>> {
    let counts = sqlx::query_as(
        "SELECT source_id, COUNT(*) FROM class_block
         WHERE deleted = 0 AND source_id IS NOT NULL
         GROUP BY source_id",
    )
    .fetch_all(pool)
    .await?;
    Ok(counts)
}

/// Remove a source and hide the blocks and deadlines that belong to it.
/// Blocks that another feed still contains come back, with the same id,
/// the next time that feed is imported; so do deadlines when the link is added
/// again. Returns how many blocks or deadlines were hidden.
pub async fn remove_source(pool: &SqlitePool, source_id: &str) -> Result<u32> {
    let now = now_text();
    let mut transaction = pool.begin().await?;

    sqlx::query("UPDATE calendar_source SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(source_id)
        .execute(&mut *transaction)
        .await?;

    let blocks = sqlx::query(
        "UPDATE class_block SET deleted = 1, updated_at = ? WHERE source_id = ? AND deleted = 0",
    )
    .bind(&now)
    .bind(source_id)
    .execute(&mut *transaction)
    .await?;

    let deadlines = sqlx::query(
        "UPDATE deadline SET deleted = 1, updated_at = ? WHERE source_id = ? AND deleted = 0",
    )
    .bind(&now)
    .bind(source_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok((blocks.rows_affected() + deadlines.rows_affected()) as u32)
}

/// Find a source by the first characters of its id, as the `feeds` command shows them.
pub async fn find_source_id(pool: &SqlitePool, start_of_id: &str) -> Result<String> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM calendar_source WHERE deleted = 0 AND id LIKE ?")
            .bind(format!("{start_of_id}%"))
            .fetch_all(pool)
            .await?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no feed with an id starting with {start_of_id}"),
        _ => bail!("more than one feed starts with {start_of_id}, type more characters"),
    }
}

/// Add an "ignore" rule on the title, for all sources, and apply it right away.
/// Returns how many blocks changed.
pub async fn add_title_rule(pool: &SqlitePool, pattern: &str) -> Result<u32> {
    sqlx::query(
        "INSERT INTO filter_rule (id, field, pattern, updated_at) VALUES (?, 'title', ?, ?)",
    )
    .bind(new_id())
    .bind(pattern)
    .bind(now_text())
    .execute(pool)
    .await?;
    apply_rules(pool).await
}

/// Remove a rule and apply the remaining rules right away.
/// Returns how many blocks changed.
pub async fn remove_rule(pool: &SqlitePool, rule_id: &str) -> Result<u32> {
    sqlx::query("UPDATE filter_rule SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(rule_id)
        .execute(pool)
        .await?;
    apply_rules(pool).await
}

/// Check every block against the current rules again.
/// Only blocks whose result changes are written, so `updated_at` stays meaningful.
/// Returns how many blocks changed.
pub async fn apply_rules(pool: &SqlitePool) -> Result<u32> {
    let rules = filter_rules(pool).await?;
    let blocks: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, title, location, description FROM class_block WHERE deleted = 0",
    )
    .fetch_all(pool)
    .await?;

    let now = now_text();
    let mut changed = 0;
    let mut transaction = pool.begin().await?;

    for (id, title, location, description) in blocks {
        let ignored =
            filter::is_ignored(&title, location.as_deref(), description.as_deref(), &rules);

        let result = sqlx::query(
            "UPDATE class_block SET ignored_by_rule = ?, updated_at = ?
             WHERE id = ? AND ignored_by_rule != ?",
        )
        .bind(ignored)
        .bind(&now)
        .bind(&id)
        .bind(ignored)
        .execute(&mut *transaction)
        .await?;
        changed += result.rows_affected() as u32;
    }

    transaction.commit().await?;
    // Blocks that are no longer ignored get their template activities.
    activity::apply_templates(pool).await?;
    Ok(changed)
}

/// Set the manual exception for one block:
/// Some(true) = always ignore, Some(false) = never ignore, None = follow the rules again.
pub async fn set_ignored_override(
    pool: &SqlitePool,
    block_id: &str,
    ignored: Option<bool>,
) -> Result<()> {
    sqlx::query("UPDATE class_block SET ignored_override = ?, updated_at = ? WHERE id = ?")
        .bind(ignored)
        .bind(now_text())
        .bind(block_id)
        .execute(pool)
        .await?;
    activity::apply_templates(pool).await?;
    Ok(())
}

/// Find a block by the first characters of its id, as the `week` command shows them.
pub async fn find_block_id(pool: &SqlitePool, start_of_id: &str) -> Result<String> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM class_block WHERE deleted = 0 AND id LIKE ?")
            .bind(format!("{start_of_id}%"))
            .fetch_all(pool)
            .await?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no block with an id starting with {start_of_id}"),
        _ => bail!("more than one block starts with {start_of_id}, type more characters"),
    }
}

/// Find a rule by the first characters of its id, as the `rules` command shows them.
pub async fn find_rule_id(pool: &SqlitePool, start_of_id: &str) -> Result<String> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM filter_rule WHERE deleted = 0 AND id LIKE ?")
            .bind(format!("{start_of_id}%"))
            .fetch_all(pool)
            .await?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no rule with an id starting with {start_of_id}"),
        _ => bail!("more than one rule starts with {start_of_id}, type more characters"),
    }
}

/// All active filter rules.
pub async fn filter_rules(pool: &SqlitePool) -> Result<Vec<FilterRule>> {
    let rules = sqlx::query_as("SELECT id, field, pattern FROM filter_rule WHERE deleted = 0")
        .fetch_all(pool)
        .await?;
    Ok(rules)
}

/// Find the course with this code, or add it. Returns its id.
async fn course_id_for(conn: &mut SqliteConnection, code: &str, name: &str) -> Result<String> {
    let existing: Option<String> = sqlx::query_scalar("SELECT id FROM course WHERE code = ?")
        .bind(code)
        .fetch_optional(&mut *conn)
        .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = new_id();
    let color = activity::next_course_color(&mut *conn).await?;
    sqlx::query("INSERT INTO course (id, name, code, color, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(code)
        .bind(color)
        .bind(now_text())
        .execute(&mut *conn)
        .await?;
    Ok(id)
}

/// All courses, sorted by name.
pub async fn courses(pool: &SqlitePool) -> Result<Vec<Course>> {
    let courses =
        sqlx::query_as("SELECT id, code, name, color FROM course WHERE deleted = 0 ORDER BY name")
            .fetch_all(pool)
            .await?;
    Ok(courses)
}

/// The kinds of class each course has ("hoorcollege", "werkcollege", …),
/// to choose from when making a template.
pub async fn course_kinds(pool: &SqlitePool) -> Result<Vec<CourseKind>> {
    let kinds = sqlx::query_as(
        "SELECT DISTINCT course_id, kind FROM class_block
         WHERE deleted = 0 AND course_id IS NOT NULL AND kind IS NOT NULL
         ORDER BY course_id, kind",
    )
    .fetch_all(pool)
    .await?;
    Ok(kinds)
}

/// What one sync changed.
#[derive(Debug, Default, PartialEq, serde::Serialize)]
pub struct SyncResult {
    pub added: u32,
    /// Known blocks seen again, changed or not.
    pub updated: u32,
    /// Of those, the ones whose time changed.
    pub moved: u32,
    pub cancelled: u32,
}

/// Save the events of one feed.
///
/// - New UID: insert a new block.
/// - Known UID: update it. If the time changed, mark it as "moved".
///   The block keeps its id, so linked activities stay attached.
///   It now belongs to this source, and comes back if its old source was removed.
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
        let ignored = filter::is_ignored(
            &event.title,
            event.location.as_deref(),
            event.description.as_deref(),
            &rules,
        );

        // "E702010A. Signalen en systemen GR02, werkcollege" -> course + kind.
        let parsed = title::parse(&event.title);
        let course_id = match &parsed {
            Some(t) => Some(course_id_for(&mut transaction, &t.code, &t.course).await?),
            None => None,
        };
        let kind = parsed.and_then(|t| t.kind);

        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT start_at, end_at FROM class_block WHERE ical_uid = ?")
                .bind(&event.uid)
                .fetch_optional(&mut *transaction)
                .await?;

        match existing {
            None => {
                sqlx::query(
                    "INSERT INTO class_block
                       (id, source_id, ical_uid, course_id, kind, title, location, description,
                        start_at, end_at, ignored_by_rule, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(new_id())
                .bind(source_id)
                .bind(&event.uid)
                .bind(&course_id)
                .bind(&kind)
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
                     SET source_id = ?, deleted = 0,
                         course_id = ?, kind = ?, title = ?, location = ?, description = ?,
                         start_at = ?, end_at = ?,
                         ignored_by_rule = ?, status = {status_sql}, updated_at = ?
                     WHERE ical_uid = ?"
                );

                sqlx::query(&sql)
                    .bind(source_id)
                    .bind(&course_id)
                    .bind(&kind)
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

                // Remember where it was before its first move, and show the change again.
                if moved {
                    result.moved += 1;
                    sqlx::query(
                        "UPDATE class_block
                         SET moved_from = COALESCE(moved_from, ?), change_seen = 0
                         WHERE ical_uid = ?",
                    )
                    .bind(&old_start)
                    .bind(&event.uid)
                    .execute(&mut *transaction)
                    .await?;
                }
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
                "UPDATE class_block SET status = 'cancelled', change_seen = 0, updated_at = ?
                 WHERE ical_uid = ?",
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
    // New blocks get the activities of their course's templates.
    activity::apply_templates(pool).await?;
    Ok(result)
}

/// Import past blocks once, from a schedule downloaded by hand (TimeEdit's
/// "Download > iCal"). Subscriptions leave out everything older than 14 days,
/// so this is how the start of the semester gets in.
///
/// - Only blocks that started before now are added; the live feed handles the rest.
/// - A block that Lockin already has (same UID, or same title and start time) is skipped,
///   so it never overwrites what the live feed saved.
/// - The file is not saved as a feed: `sync` never reads it again.
///
/// Returns how many blocks were added.
pub async fn import_history(pool: &SqlitePool, events: &[FeedEvent]) -> Result<u32> {
    let rules = filter_rules(pool).await?;
    let now = Utc::now();
    let mut added = 0;
    let mut transaction = pool.begin().await?;

    for event in events {
        if event.start >= now {
            continue;
        }

        let start = time_text(event.start);
        let known: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM class_block
             WHERE ical_uid = ? OR (title = ? AND start_at = ?)",
        )
        .bind(&event.uid)
        .bind(&event.title)
        .bind(&start)
        .fetch_one(&mut *transaction)
        .await?;
        if known > 0 {
            continue;
        }

        let parsed = title::parse(&event.title);
        let course_id = match &parsed {
            Some(t) => Some(course_id_for(&mut transaction, &t.code, &t.course).await?),
            None => None,
        };
        let kind = parsed.and_then(|t| t.kind);
        let ignored = filter::is_ignored(
            &event.title,
            event.location.as_deref(),
            event.description.as_deref(),
            &rules,
        );

        // No source_id: the block doesn't belong to a feed that gets synced.
        sqlx::query(
            "INSERT INTO class_block
               (id, ical_uid, course_id, kind, title, location, description,
                start_at, end_at, ignored_by_rule, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(&event.uid)
        .bind(&course_id)
        .bind(&kind)
        .bind(&event.title)
        .bind(&event.location)
        .bind(&event.description)
        .bind(&start)
        .bind(time_text(event.end))
        .bind(ignored)
        .bind(now_text())
        .execute(&mut *transaction)
        .await?;
        added += 1;
    }

    transaction.commit().await?;
    activity::apply_templates(pool).await?;
    Ok(added)
}

/// All blocks that start between `from` and `to`, sorted by start time.
pub async fn blocks_between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<ClassBlock>> {
    let blocks = sqlx::query_as(
        "SELECT id, ical_uid, course_id, kind, title, location, start_at, end_at,
                ignored_by_rule, ignored_override, status, moved_from, change_seen
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

/// Mark the changes of these blocks (moved, cancelled) as seen: "Got it" in the week view.
pub async fn acknowledge_changes(pool: &SqlitePool, block_ids: &[String]) -> Result<()> {
    let mut transaction = pool.begin().await?;
    for id in block_ids {
        sqlx::query("UPDATE class_block SET change_seen = 1, updated_at = ? WHERE id = ?")
            .bind(now_text())
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await?;
    Ok(())
}
