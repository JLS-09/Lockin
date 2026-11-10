//! The data types used across Lockin.

use chrono::{DateTime, Utc};

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
#[derive(Debug, Clone, sqlx::FromRow)]
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
}

impl ClassBlock {
    /// A manual exception wins; otherwise the filter rules decide.
    pub fn is_ignored(&self) -> bool {
        self.ignored_override.unwrap_or(self.ignored_by_rule)
    }
}

/// A rule like "ignore everything with 'zelfstandig werk' in the title".
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FilterRule {
    pub id: String,
    pub field: String,
    pub pattern: String,
}

/// A course, e.g. "Signalen en systemen" with code E702010A.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Course {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
}

/// A saved iCal feed: TimeEdit now, Ufora later.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarSource {
    pub id: String,
    pub kind: String,
    pub url: String,
}
