//! Points: every change is one line in `points_ledger` (earned +, lost −), so
//! the balance is simply the sum of the lines.
//!
//! For now only the emergency button writes here (−50, see `session.rs`).
//! Phase 6 adds points for studying, streaks and spending in the shop.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;

use crate::model::PointsEntry;
use crate::{new_id, now_text};

/// The reason written for the emergency button's penalty.
pub const REASON_EMERGENCY: &str = "emergency";

/// Write one line: `points` (negative for a penalty) at `at`, for `reason`,
/// optionally tied to a study session.
pub async fn add(
    pool: &SqlitePool,
    points: i64,
    reason: &str,
    session_id: Option<&str>,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO points_ledger (id, at, points, reason, session_id, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    .bind(points)
    .bind(reason)
    .bind(session_id)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// Your points right now: the sum of every line.
pub async fn balance(pool: &SqlitePool) -> Result<i64> {
    let total: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(points), 0) FROM points_ledger WHERE deleted = 0")
            .fetch_one(pool)
            .await?;
    Ok(total)
}

/// The most recent lines, newest first.
pub async fn recent(pool: &SqlitePool, limit: i64) -> Result<Vec<PointsEntry>> {
    let lines = sqlx::query_as(
        "SELECT id, at, points, reason, session_id FROM points_ledger
         WHERE deleted = 0 ORDER BY at DESC, rowid DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(lines)
}

/// The points each of these sessions earned or cost, summed per session.
/// Sessions without any lines are left out.
pub async fn for_sessions(
    pool: &SqlitePool,
    session_ids: &[String],
) -> Result<HashMap<String, i64>> {
    let mut totals = HashMap::new();
    for id in session_ids {
        let total: Option<i64> = sqlx::query_scalar(
            "SELECT SUM(points) FROM points_ledger WHERE deleted = 0 AND session_id = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        if let Some(total) = total {
            totals.insert(id.clone(), total);
        }
    }
    Ok(totals)
}
