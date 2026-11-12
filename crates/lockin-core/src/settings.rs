//! Settings the app and the daemon share (the `setting` table): the language
//! and which notifications to send. A missing key means its default.

use std::collections::HashMap;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

use crate::now_text;

/// "en" or "nl"; missing = follow the system language.
pub const LANGUAGE: &str = "language";
/// Notifications 5 minutes before a study session, at its start and its end.
pub const NOTIFY_SESSIONS: &str = "notify.sessions";
/// When you earned a break, and when a break is over.
pub const NOTIFY_BREAKS: &str = "notify.breaks";
/// A day before a deadline.
pub const NOTIFY_DEADLINES: &str = "notify.deadlines";
/// When a blocked app was closed (once per app per session).
pub const NOTIFY_CLOSED_APPS: &str = "notify.closedApps";

/// The keys the app may change (`set` refuses others).
pub const KEYS: [&str; 5] = [
    LANGUAGE,
    NOTIFY_SESSIONS,
    NOTIFY_BREAKS,
    NOTIFY_DEADLINES,
    NOTIFY_CLOSED_APPS,
];

/// One setting, or None when it was never set.
pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let value = sqlx::query_scalar("SELECT value FROM setting WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(value)
}

/// Every setting that was set.
pub async fn all(pool: &SqlitePool) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM setting")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

/// Change a setting. Only the keys in `KEYS`, and only sensible values.
pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    let valid = match key {
        LANGUAGE => matches!(value, "en" | "nl"),
        _ if KEYS.contains(&key) => matches!(value, "on" | "off"),
        _ => anyhow::bail!("unknown setting: {key}"),
    };
    if !valid {
        anyhow::bail!("{value:?} isn't a value for {key}");
    }
    sqlx::query(
        "INSERT INTO setting (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// A notify.* switch: on unless it was turned off.
pub async fn is_on(pool: &SqlitePool, key: &str) -> Result<bool> {
    Ok(get(pool, key).await?.as_deref() != Some("off"))
}
