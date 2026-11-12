//! Shared logic for the Lockin app and daemon:
//! data model, database access, schedule import, deadlines and (later) sync.

pub mod activity;
pub mod apps;
pub mod block;
pub mod db;
pub mod deadline;
pub mod emergency;
pub mod filter;
pub mod import;
pub mod ipc;
pub mod model;
pub mod paths;
pub mod points;
pub mod session;
pub mod timeedit;
pub mod title;
pub mod ufora;
pub mod urgent;

/// The current time as RFC 3339 text in UTC, the format we store in SQLite.
pub fn now_text() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// A fresh random UUID as text.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
