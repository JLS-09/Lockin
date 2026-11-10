//! Shared logic for the Lockin app and daemon:
//! data model, database access, schedule import and (later) sync.

pub mod db;
pub mod filter;
pub mod model;
pub mod paths;
pub mod timeedit;

/// The current time as RFC 3339 text in UTC, the format we store in SQLite.
pub fn now_text() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// A fresh random UUID as text.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
