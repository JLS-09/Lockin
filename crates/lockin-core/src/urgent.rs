//! The Urgent list: open activities and deadlines in one list, most urgent first.
//!
//! The plan asks for "a combination of how long an activity has been open and
//! how close a deadline is". Both are turned into one number, the score, in
//! "days of pressure":
//!
//! - An activity scores the number of days it has been open. It becomes open
//!   when its class ends, or for an activity without a class, on the Monday of
//!   its week. Activities that aren't open yet (a class still to come) aren't
//!   in the list: the week view shows those.
//! - A deadline scores twice the number of days it is inside the last week
//!   before it's due: 0 a week before, 8 four days before, 14 when it's due,
//!   and still more once it's late. A deadline further away scores below 0, so
//!   it sits at the bottom without disappearing.
//!
//! So a deadline due in three days (8) comes just above an activity that has
//! been open a week (7). Both weights are constants below, easy to tune.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::sqlite::SqlitePool;

use crate::activity;
use crate::model::UrgentItem;

/// An open activity counts as behind (red) after this many days.
pub const BEHIND_AFTER_DAYS: f64 = 2.0;

/// A deadline counts as approaching (orange) this many days before it's due.
pub const APPROACHING_DAYS: f64 = 7.0;

/// Each day closer to a deadline counts as this many days an activity is open.
pub const DEADLINE_WEIGHT: f64 = 2.0;

/// Days from `earlier` to `later`, with fractions: 36 hours is 1.5.
fn days_between(earlier: DateTime<Utc>, later: DateTime<Utc>) -> f64 {
    (later - earlier).num_seconds() as f64 / 86_400.0
}

/// The score of an activity that has been open `days_open` days.
pub fn activity_score(days_open: f64) -> f64 {
    days_open
}

/// The score of a deadline due in `days_left` days (below 0 when it's late).
pub fn deadline_score(days_left: f64) -> f64 {
    DEADLINE_WEIGHT * (APPROACHING_DAYS - days_left)
}

/// "behind" or "planned" for an activity open `days_open` days.
pub fn activity_status(days_open: f64) -> &'static str {
    if days_open >= BEHIND_AFTER_DAYS {
        "behind"
    } else {
        "planned"
    }
}

/// "behind" (late), "approaching" (within a week) or "planned" for a deadline.
pub fn deadline_status(days_left: f64) -> &'static str {
    if days_left < 0.0 {
        "behind"
    } else if days_left <= APPROACHING_DAYS {
        "approaching"
    } else {
        "planned"
    }
}

/// An open activity as `list` reads it from the database.
#[derive(sqlx::FromRow)]
struct ActivityRow {
    id: String,
    title: String,
    course_id: Option<String>,
    block_id: Option<String>,
    week: String,
    /// End of its class; None for an activity without a class.
    end_at: Option<String>,
}

/// An open deadline as `list` reads it from the database.
#[derive(sqlx::FromRow)]
struct DeadlineRow {
    id: String,
    title: String,
    course_id: Option<String>,
    due_at: String,
    source_id: Option<String>,
    url: Option<String>,
    moved_from: Option<String>,
}

/// Parse a time we stored ourselves (RFC 3339).
fn parse_time(text: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(text)?.with_timezone(&Utc))
}

/// The Urgent list at time `now`: every open activity that is due (its class
/// is over, or its week has started) and every deadline that isn't done or
/// cancelled, sorted with the most urgent first.
pub async fn list(pool: &SqlitePool, now: DateTime<Utc>) -> Result<Vec<UrgentItem>> {
    let mut items = Vec::new();

    // Open activities. The view already leaves out deleted ones and the
    // template activities of ignored classes.
    let activities: Vec<ActivityRow> = sqlx::query_as(
        "SELECT a.id, a.title, a.course_id, a.block_id, a.week, b.end_at
         FROM activity_with_week a
         LEFT JOIN class_block b ON b.id = a.block_id
         WHERE a.done = 0",
    )
    .fetch_all(pool)
    .await?;

    for row in activities {
        let open_since = match &row.end_at {
            Some(end_at) => parse_time(end_at)?,
            // No class: open from the start of its week (Monday 00:00 local time).
            None => {
                let monday = NaiveDate::parse_from_str(&row.week, "%Y-%m-%d")?;
                activity::week_range(monday).0
            }
        };
        if open_since > now {
            continue;
        }

        let days_open = days_between(open_since, now);
        items.push(UrgentItem {
            kind: "activity".to_string(),
            id: row.id,
            title: row.title,
            course_id: row.course_id,
            block_id: row.block_id,
            week: Some(row.week),
            open_since: Some(open_since.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            due_at: None,
            source_id: None,
            url: None,
            moved_from: None,
            status: activity_status(days_open).to_string(),
            score: activity_score(days_open),
        });
    }

    // Deadlines that still need doing.
    let deadlines: Vec<DeadlineRow> = sqlx::query_as(
        "SELECT id, title, course_id, due_at, source_id, url, moved_from FROM deadline
         WHERE deleted = 0 AND done = 0 AND status != 'cancelled'",
    )
    .fetch_all(pool)
    .await?;

    for row in deadlines {
        let days_left = days_between(now, parse_time(&row.due_at)?);
        items.push(UrgentItem {
            kind: "deadline".to_string(),
            id: row.id,
            title: row.title,
            course_id: row.course_id,
            block_id: None,
            week: None,
            open_since: None,
            due_at: Some(row.due_at),
            source_id: row.source_id,
            url: row.url,
            moved_from: row.moved_from,
            status: deadline_status(days_left).to_string(),
            score: deadline_score(days_left),
        });
    }

    // Most urgent first; equal scores by title, so the order never jumps around.
    items.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.title.cmp(&b.title)));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_close_deadline_beats_an_old_activity() {
        // Due in three days vs. open for a week.
        assert!(deadline_score(3.0) > activity_score(7.0));
        // Due in six days vs. open for three days.
        assert!(deadline_score(6.0) < activity_score(3.0));
    }

    #[test]
    fn a_late_deadline_keeps_rising() {
        assert!(deadline_score(-1.0) > deadline_score(0.0));
        assert_eq!(deadline_score(APPROACHING_DAYS), 0.0);
    }

    #[test]
    fn statuses() {
        assert_eq!(activity_status(0.5), "planned");
        assert_eq!(activity_status(2.0), "behind");
        assert_eq!(deadline_status(-0.1), "behind");
        assert_eq!(deadline_status(2.0), "approaching");
        assert_eq!(deadline_status(9.0), "planned");
    }
}
