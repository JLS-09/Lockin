//! Deadlines: something due at a set time, like "Lab report 2, Mon 16 Nov 23:59".
//! Added by hand now; the Ufora feed adds them too later in phase 3.

use anyhow::{Result, bail};
use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use sqlx::sqlite::SqlitePool;

use crate::model::Deadline;
use crate::{new_id, now_text};

/// The columns every deadline query reads, in the order of `Deadline`.
const COLUMNS: &str = "id, source_id, course_id, title, description, due_at, done, done_at,
                       status, moved_from, change_seen";

/// RFC 3339 text in UTC with seconds, the format every time is stored in.
fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// A due time from a local date and an optional local time.
/// Without a time, a deadline is due at the end of that day (23:59).
pub fn due_time(date: NaiveDate, time: Option<NaiveTime>) -> Result<DateTime<Utc>> {
    let time = time.unwrap_or_else(|| NaiveTime::from_hms_opt(23, 59, 0).unwrap());
    // `earliest` picks the first one when the clock goes back in autumn.
    let Some(local) = Local.from_local_datetime(&date.and_time(time)).earliest() else {
        bail!("{date} {time} doesn't exist in local time (the clock skips it)");
    };
    Ok(local.with_timezone(&Utc))
}

/// A title must have some text in it. Returns it without spaces around it.
fn clean_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail!("a deadline needs a title");
    }
    Ok(title)
}

/// Add a deadline by hand. The course is optional. Returns its id.
pub async fn add(
    pool: &SqlitePool,
    course_id: Option<&str>,
    title: &str,
    due: DateTime<Utc>,
) -> Result<String> {
    let title = clean_title(title)?;
    let id = new_id();
    sqlx::query(
        "INSERT INTO deadline (id, course_id, title, due_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(course_id)
    .bind(title)
    .bind(time_text(due))
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(id)
}

/// Change the course, title and due time of a deadline.
pub async fn update(
    pool: &SqlitePool,
    deadline_id: &str,
    course_id: Option<&str>,
    title: &str,
    due: DateTime<Utc>,
) -> Result<()> {
    let title = clean_title(title)?;
    sqlx::query(
        "UPDATE deadline SET course_id = ?, title = ?, due_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(course_id)
    .bind(title)
    .bind(time_text(due))
    .bind(now_text())
    .bind(deadline_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Check a deadline off, or uncheck it.
pub async fn set_done(pool: &SqlitePool, deadline_id: &str, done: bool) -> Result<()> {
    let now = now_text();
    let done_at = if done { Some(now.clone()) } else { None };
    sqlx::query("UPDATE deadline SET done = ?, done_at = ?, updated_at = ? WHERE id = ?")
        .bind(done)
        .bind(done_at)
        .bind(&now)
        .bind(deadline_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a deadline (soft delete, so `restore` can bring it back).
pub async fn remove(pool: &SqlitePool, deadline_id: &str) -> Result<()> {
    sqlx::query("UPDATE deadline SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(deadline_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Undo `remove`: the deadline comes back as it was.
pub async fn restore(pool: &SqlitePool, deadline_id: &str) -> Result<()> {
    sqlx::query("UPDATE deadline SET deleted = 0, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(deadline_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Every deadline, done ones and cancelled ones too, sorted by due time.
pub async fn all(pool: &SqlitePool) -> Result<Vec<Deadline>> {
    let sql = format!("SELECT {COLUMNS} FROM deadline WHERE deleted = 0 ORDER BY due_at, title");
    let deadlines = sqlx::query_as(&sql).fetch_all(pool).await?;
    Ok(deadlines)
}

/// The deadlines due between `from` and `to` (cancelled ones too, so a
/// cancellation stays visible), sorted by due time. For "Coming 2 weeks".
pub async fn between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Deadline>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM deadline
         WHERE deleted = 0 AND due_at >= ? AND due_at < ?
         ORDER BY due_at, title"
    );
    let deadlines = sqlx::query_as(&sql)
        .bind(time_text(from))
        .bind(time_text(to))
        .fetch_all(pool)
        .await?;
    Ok(deadlines)
}

/// Find a deadline by the first characters of its id, as the `deadlines` command shows them.
pub async fn find_id(pool: &SqlitePool, start_of_id: &str) -> Result<String> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM deadline WHERE deleted = 0 AND id LIKE ?")
            .bind(format!("{start_of_id}%"))
            .fetch_all(pool)
            .await?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no deadline with an id starting with {start_of_id}"),
        _ => bail!("more than one deadline starts with {start_of_id}, type more characters"),
    }
}
