//! Activities to check off, and the templates that create them per course.

use anyhow::{Result, bail};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use sqlx::SqliteConnection;
use sqlx::sqlite::SqlitePool;

use crate::model::{Activity, ActivityTemplate, WeekProgress};
use crate::{new_id, now_text};

/// Default course colors: muted, readable on the dark background,
/// and away from the green, red and orange of the status colors.
/// The same list is in migration 0003.
pub const COURSE_COLORS: [&str; 8] = [
    "#6b9bd1", "#a685d6", "#d47fae", "#4fb3bf", "#c8b45a", "#8b93e8", "#c98b6b", "#7fa88f",
];

/// The color for a new course: the next one in the palette.
pub async fn next_course_color(conn: &mut SqliteConnection) -> Result<&'static str> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM course")
        .fetch_one(&mut *conn)
        .await?;
    Ok(COURSE_COLORS[count as usize % COURSE_COLORS.len()])
}

/// The Monday of the week a date is in, as text: "2026-11-09".
pub fn week_of(date: NaiveDate) -> String {
    let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);
    monday.format("%Y-%m-%d").to_string()
}

/// Start and end of the week `date` is in: Monday 00:00 up to the next Monday 00:00,
/// in local time, as UTC. Use it with `db::blocks_between`.
pub fn week_range(date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);
    // Each end on its own, so a week with a summer/winter time change is still right.
    (midnight(monday), midnight(monday + Duration::days(7)))
}

/// 00:00 local time on `day`, as UTC.
fn midnight(day: NaiveDate) -> DateTime<Utc> {
    Local
        .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .unwrap()
        .with_timezone(&Utc)
}

// ---------- Templates ----------

/// Add a template and create its activities right away.
/// `kind` None means every block of the course.
/// Returns how many activities were created.
pub async fn add_template(
    pool: &SqlitePool,
    course_id: &str,
    kind: Option<&str>,
    title: &str,
) -> Result<u32> {
    sqlx::query(
        "INSERT INTO activity_template (id, course_id, kind, title, updated_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(course_id)
    .bind(kind)
    .bind(title)
    .bind(now_text())
    .execute(pool)
    .await?;
    apply_templates(pool).await
}

/// Remove a template. The activities it made that are not done yet go too;
/// the done ones stay, so the history doesn't change.
/// Returns how many activities were removed.
pub async fn remove_template(pool: &SqlitePool, template_id: &str) -> Result<u32> {
    let now = now_text();
    let mut transaction = pool.begin().await?;

    sqlx::query("UPDATE activity_template SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(template_id)
        .execute(&mut *transaction)
        .await?;

    let result = sqlx::query(
        "UPDATE activity SET deleted = 1, updated_at = ?
         WHERE template_id = ? AND done = 0 AND deleted = 0",
    )
    .bind(&now)
    .bind(template_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(result.rows_affected() as u32)
}

/// All templates, per course.
pub async fn templates(pool: &SqlitePool) -> Result<Vec<ActivityTemplate>> {
    let templates = sqlx::query_as(
        "SELECT id, course_id, kind, title FROM activity_template
         WHERE deleted = 0 ORDER BY course_id, kind, title",
    )
    .fetch_all(pool)
    .await?;
    Ok(templates)
}

/// Create the missing activities for every template.
///
/// - Every block of the course (or only the blocks of that kind) gets one,
///   past blocks too, so you can catch up from the start of the semester.
/// - Cancelled and ignored blocks get none. When a block stops being ignored,
///   the next run gives it its activities.
/// - A template never makes a second activity for the same block, also not
///   when you deleted the first one.
///
/// Runs after every sync and every change to the rules or exceptions.
/// Returns how many activities were created.
pub async fn apply_templates(pool: &SqlitePool) -> Result<u32> {
    let missing: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT t.id, t.course_id, t.title, b.id
         FROM activity_template t
         JOIN class_block b ON b.course_id = t.course_id
                           AND (t.kind IS NULL OR b.kind = t.kind)
         WHERE t.deleted = 0 AND b.deleted = 0 AND b.status != 'cancelled'
           AND COALESCE(b.ignored_override, b.ignored_by_rule) = 0
           AND NOT EXISTS (SELECT 1 FROM activity a
                           WHERE a.template_id = t.id AND a.block_id = b.id)",
    )
    .fetch_all(pool)
    .await?;

    let now = now_text();
    let mut transaction = pool.begin().await?;

    for (template_id, course_id, title, block_id) in &missing {
        sqlx::query(
            "INSERT INTO activity (id, block_id, template_id, course_id, title, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(new_id())
        .bind(block_id)
        .bind(template_id)
        .bind(course_id)
        .bind(title)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(missing.len() as u32)
}

// ---------- Activities ----------

/// Add an activity to a class block, by hand. Returns its id.
pub async fn add_to_block(pool: &SqlitePool, block_id: &str, title: &str) -> Result<String> {
    let course_id: Option<Option<String>> =
        sqlx::query_scalar("SELECT course_id FROM class_block WHERE id = ? AND deleted = 0")
            .bind(block_id)
            .fetch_optional(pool)
            .await?;
    let Some(course_id) = course_id else {
        bail!("no block with id {block_id}");
    };

    let id = new_id();
    sqlx::query(
        "INSERT INTO activity (id, block_id, course_id, title, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(block_id)
    .bind(course_id)
    .bind(title)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(id)
}

/// Add an activity that isn't linked to a block, in the week of `date`.
/// The course is optional. Returns its id.
pub async fn add_to_week(
    pool: &SqlitePool,
    course_id: Option<&str>,
    date: NaiveDate,
    title: &str,
) -> Result<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO activity (id, course_id, title, week, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(course_id)
    .bind(title)
    .bind(week_of(date))
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(id)
}

/// Check an activity off, or uncheck it.
pub async fn set_done(pool: &SqlitePool, activity_id: &str, done: bool) -> Result<()> {
    let now = now_text();
    let done_at = if done { Some(now.clone()) } else { None };
    sqlx::query("UPDATE activity SET done = ?, done_at = ?, updated_at = ? WHERE id = ?")
        .bind(done)
        .bind(done_at)
        .bind(&now)
        .bind(activity_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Change the title of an activity.
pub async fn rename(pool: &SqlitePool, activity_id: &str, title: &str) -> Result<()> {
    sqlx::query("UPDATE activity SET title = ?, updated_at = ? WHERE id = ?")
        .bind(title)
        .bind(now_text())
        .bind(activity_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete an activity (soft delete, so a template doesn't make it again).
pub async fn remove(pool: &SqlitePool, activity_id: &str) -> Result<()> {
    sqlx::query("UPDATE activity SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(activity_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// All visible activities in the week of `date`.
pub async fn in_week(pool: &SqlitePool, date: NaiveDate) -> Result<Vec<Activity>> {
    let activities = sqlx::query_as(
        "SELECT id, block_id, template_id, course_id, title, week, done, done_at
         FROM activity_with_week WHERE week = ? ORDER BY title",
    )
    .bind(week_of(date))
    .fetch_all(pool)
    .await?;
    Ok(activities)
}

/// Done and total activities per course per week: the semester overview.
/// Activities without a course are left out.
pub async fn progress(pool: &SqlitePool) -> Result<Vec<WeekProgress>> {
    let rows = sqlx::query_as(
        "SELECT week, course_id, COUNT(*) AS total, SUM(done) AS done
         FROM activity_with_week
         WHERE course_id IS NOT NULL
         GROUP BY week, course_id
         ORDER BY week",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
