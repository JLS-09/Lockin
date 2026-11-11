//! The data types used across Lockin.

use chrono::{DateTime, Utc};
use serde::Serialize;

/// One event read from an iCal feed, before it goes into the database.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedEvent {
    pub uid: String,
    pub title: String,
    pub location: Option<String>,
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// A class block as stored in the database.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassBlock {
    pub id: String,
    pub ical_uid: Option<String>,
    pub course_id: Option<String>,
    pub kind: Option<String>,
    pub title: String,
    pub location: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub ignored_by_rule: bool,
    pub ignored_override: Option<bool>,
    pub status: String,
    /// Start time before the block was first moved, for "Moved · was Fri 13:00".
    pub moved_from: Option<String>,
    /// True once you've dismissed this change in the week view.
    pub change_seen: bool,
}

impl ClassBlock {
    /// A manual exception wins; otherwise the filter rules decide.
    pub fn is_ignored(&self) -> bool {
        self.ignored_override.unwrap_or(self.ignored_by_rule)
    }
}

/// A rule like "ignore everything with 'zelfstandig werk' in the title".
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub id: String,
    pub field: String,
    pub pattern: String,
}

/// A course, e.g. "Signalen en systemen" with code E702010A.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
    /// Hex color, e.g. "#6b9bd1": class blocks, dots and charts of this course.
    pub color: Option<String>,
}

/// A saved iCal feed: TimeEdit now, Ufora later.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarSource {
    pub id: String,
    pub kind: String,
    pub url: String,
    pub last_fetched_at: Option<String>,
}

/// A saved feed as the app shows it: everything except the URL, which holds a
/// personal key and never leaves the Rust side.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedInfo {
    pub id: String,
    /// "timeedit" (or "ufora" later).
    pub kind: String,
    /// Where it comes from without the key: "cloud.timeedit.net", or the file name.
    pub origin: String,
    pub last_fetched_at: Option<String>,
    /// How many classes this feed holds now.
    pub classes: i64,
}

/// The result of syncing one feed, for the app: what changed, or why it failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedSync {
    pub source_id: String,
    pub added: u32,
    pub updated: u32,
    /// Classes whose time changed.
    pub moved: u32,
    pub cancelled: u32,
    /// Set when this feed couldn't be fetched; the others still sync.
    pub error: Option<String>,
}

/// Something to check off, e.g. "Do the exercises".
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    /// The class block it belongs to; None for an activity on its own.
    pub block_id: Option<String>,
    /// The template that created it; None if it was added by hand.
    pub template_id: Option<String>,
    pub course_id: Option<String>,
    pub title: String,
    /// Monday of the week it belongs to, e.g. "2026-11-09".
    pub week: String,
    pub done: bool,
    pub done_at: Option<String>,
}

/// "After every werkcollege of this course: do the exercises."
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTemplate {
    pub id: String,
    pub course_id: String,
    /// The kind of class it applies to; None = every block of the course.
    pub kind: Option<String>,
    pub title: String,
}

/// How many activities of one course in one week are done.
/// One cell of the semester overview.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekProgress {
    /// Monday of the week, e.g. "2026-11-09".
    pub week: String,
    pub course_id: String,
    pub total: i64,
    pub done: i64,
    /// Not done, on a class that has already ended.
    pub overdue: i64,
}

/// One kind of class a course has, e.g. Signalen en systemen has "werkcollege".
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseKind {
    pub course_id: String,
    pub kind: String,
}

/// How many classes one course has in one week.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekClasses {
    /// Monday of the week, e.g. "2026-11-09".
    pub week: String,
    pub course_id: String,
    pub classes: i64,
}
