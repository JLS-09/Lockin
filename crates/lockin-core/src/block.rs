//! Block profiles: your own lists of apps to close during a study session.
//!
//! You make the profiles ("Strict", "Light") and pick their apps; Lockin ships
//! none. A session blocks with one profile (`study_session.profile_id`).
//! While the running session uses a profile, apps can be added to it but not
//! removed, and it can't be deleted: a running session can't be loosened.
//! Matching apps to processes is in `apps.rs`; closing them in the daemon.

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;

use crate::apps::{self, InstalledApp, LOCKIN_APP_ID};
use crate::model::{AppCloses, BlockProfile, BlockedApp};
use crate::{new_id, now_text};

const APP_COLUMNS: &str = "id, profile_id, app_id, name, exec_names, flatpak_id";

/// Every profile with its apps, sorted by name.
pub async fn profiles(pool: &SqlitePool) -> Result<Vec<BlockProfile>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM block_profile WHERE deleted = 0 ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(pool)
    .await?;
    let running = running_profile(pool).await?;
    let mut list = Vec::new();
    for (id, name) in rows {
        list.push(BlockProfile {
            apps: apps_of(pool, &id).await?,
            in_use: running.as_deref() == Some(id.as_str()),
            id,
            name,
        });
    }
    Ok(list)
}

/// The apps of one profile, sorted by name.
pub async fn apps_of(pool: &SqlitePool, profile_id: &str) -> Result<Vec<BlockedApp>> {
    let sql = format!(
        "SELECT {APP_COLUMNS} FROM block_app
         WHERE profile_id = ? AND deleted = 0 ORDER BY name COLLATE NOCASE"
    );
    Ok(sqlx::query_as(&sql)
        .bind(profile_id)
        .fetch_all(pool)
        .await?)
}

/// The profile of the running session, if it has one.
pub async fn running_profile(pool: &SqlitePool) -> Result<Option<String>> {
    let profile: Option<Option<String>> = sqlx::query_scalar(
        "SELECT s.profile_id FROM study_session s
         JOIN block_profile p ON p.id = s.profile_id AND p.deleted = 0
         WHERE s.deleted = 0 AND s.status = 'running' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(profile.flatten())
}

async fn check_not_in_use(pool: &SqlitePool, profile_id: &str) -> Result<()> {
    if running_profile(pool).await?.as_deref() == Some(profile_id) {
        bail!(
            "the running study session blocks with this profile: you can add apps now, and remove them after the session"
        );
    }
    Ok(())
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("give the profile a name");
    }
    Ok(name.to_string())
}

/// Refuse a name another profile already has (case doesn't matter).
async fn check_unique(pool: &SqlitePool, name: &str, except: &str) -> Result<()> {
    let taken: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM block_profile
         WHERE deleted = 0 AND name = ? COLLATE NOCASE AND id != ?",
    )
    .bind(name)
    .bind(except)
    .fetch_one(pool)
    .await?;
    if taken > 0 {
        bail!("there's already a profile called \"{name}\"");
    }
    Ok(())
}

/// Make a new, empty profile. Returns its id.
pub async fn add_profile(pool: &SqlitePool, name: &str) -> Result<String> {
    let name = clean_name(name)?;
    check_unique(pool, &name, "").await?;
    let id = new_id();
    sqlx::query("INSERT INTO block_profile (id, name, updated_at) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&name)
        .bind(now_text())
        .execute(pool)
        .await?;
    Ok(id)
}

pub async fn rename_profile(pool: &SqlitePool, profile_id: &str, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    check_unique(pool, &name, profile_id).await?;
    sqlx::query("UPDATE block_profile SET name = ?, updated_at = ? WHERE id = ?")
        .bind(&name)
        .bind(now_text())
        .bind(profile_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete a profile. Planned sessions that used it block nothing from now on;
/// returns how many. Past sessions keep it, for the statistics.
pub async fn remove_profile(pool: &SqlitePool, profile_id: &str) -> Result<u32> {
    check_not_in_use(pool, profile_id).await?;
    let mut transaction = pool.begin().await?;
    let unassigned = sqlx::query(
        "UPDATE study_session SET profile_id = NULL, updated_at = ?
         WHERE profile_id = ? AND status = 'planned' AND deleted = 0",
    )
    .bind(now_text())
    .bind(profile_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    sqlx::query("UPDATE session_series SET profile_id = NULL, updated_at = ? WHERE profile_id = ?")
        .bind(now_text())
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("UPDATE block_profile SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(unassigned as u32)
}

/// How many planned sessions use a profile (for "Delete? 4 planned sessions use it").
pub async fn planned_sessions_using(pool: &SqlitePool, profile_id: &str) -> Result<u32> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM study_session
         WHERE profile_id = ? AND status = 'planned' AND deleted = 0",
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await?;
    Ok(count as u32)
}

/// Add an installed app (from `apps::installed_apps`) to a profile. Returns the new entry's id.
pub async fn add_app(pool: &SqlitePool, profile_id: &str, app: &InstalledApp) -> Result<String> {
    if app.id == LOCKIN_APP_ID || app.exec_names.iter().any(|n| apps::is_protected(n)) {
        bail!(
            "{} can't be blocked: Lockin or your desktop needs it",
            app.name
        );
    }
    if app.exec_names.is_empty() && app.flatpak_id.is_none() && app.id.starts_with("program:") {
        bail!("give the program's name");
    }
    let already: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM block_app WHERE profile_id = ? AND app_id = ? AND deleted = 0",
    )
    .bind(profile_id)
    .bind(&app.id)
    .fetch_one(pool)
    .await?;
    if already > 0 {
        bail!("{} is already on this profile", app.name);
    }
    let id = new_id();
    sqlx::query(
        "INSERT INTO block_app (id, profile_id, app_id, name, exec_names, flatpak_id, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(profile_id)
    .bind(&app.id)
    .bind(&app.name)
    .bind(app.exec_names.join("\n"))
    .bind(&app.flatpak_id)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(id)
}

/// Add a program that has no menu entry, by its process name (e.g. a game).
pub async fn add_program(pool: &SqlitePool, profile_id: &str, program: &str) -> Result<String> {
    let program = program.trim();
    if program.is_empty() || program.contains('/') || program.contains(char::is_whitespace) {
        bail!("give the program's name as it runs, without spaces or a folder, e.g. \"steam\"");
    }
    let app = InstalledApp {
        id: format!("program:{program}"),
        name: program.to_string(),
        exec_names: vec![program.to_string()],
        flatpak_id: None,
    };
    add_app(pool, profile_id, &app).await
}

/// Take an app off its profile, unless the running session blocks with that profile.
pub async fn remove_app(pool: &SqlitePool, entry_id: &str) -> Result<()> {
    let profile_id: Option<String> =
        sqlx::query_scalar("SELECT profile_id FROM block_app WHERE id = ?")
            .bind(entry_id)
            .fetch_optional(pool)
            .await?;
    let Some(profile_id) = profile_id else {
        bail!("no app with id {entry_id} on a profile");
    };
    check_not_in_use(pool, &profile_id).await?;
    sqlx::query("UPDATE block_app SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(entry_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// What the daemon blocks right now: the apps of the running session's
/// profile while you study, nothing during a break or without a session.
pub async fn blocked_now(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<(Option<String>, Vec<BlockedApp>)> {
    let Some(running) = crate::session::running(pool).await? else {
        return Ok((None, Vec::new()));
    };
    if crate::session::state_now(pool, &running, now).await?.phase != "working" {
        return Ok((Some(running.id), Vec::new()));
    }
    let apps = match running_profile(pool).await? {
        Some(profile) => apps_of(pool, &profile).await?,
        None => Vec::new(),
    };
    Ok((Some(running.id), apps))
}

/// Write down that the daemon closed `app` during a session.
pub async fn record_close(
    pool: &SqlitePool,
    session_id: &str,
    app: &BlockedApp,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO app_closed (id, session_id, app_id, name, at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(session_id)
    .bind(&app.app_id)
    .bind(&app.name)
    .bind(at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// How often each app was closed during a session, most often first.
pub async fn closes(pool: &SqlitePool, session_id: &str) -> Result<Vec<AppCloses>> {
    let rows = sqlx::query_as(
        "SELECT app_id, MAX(name) AS name, COUNT(*) AS count, MAX(at) AS last_at
         FROM app_closed WHERE session_id = ? AND deleted = 0
         GROUP BY app_id ORDER BY count DESC, last_at DESC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Bring the apps on every profile up to date with the apps installed now:
/// what to look for is copied when an app is added, and an update (or a better
/// Lockin) can change it. Programs added by name stay as they are. Returns how
/// many entries changed. The daemon runs this when it starts and when a
/// session starts.
pub async fn refresh_apps(pool: &SqlitePool, installed: &[InstalledApp]) -> Result<u32> {
    let sql = format!("SELECT {APP_COLUMNS} FROM block_app WHERE deleted = 0");
    let entries: Vec<BlockedApp> = sqlx::query_as(&sql).fetch_all(pool).await?;
    let mut changed = 0;
    for entry in entries {
        let Some(app) = installed.iter().find(|a| a.id == entry.app_id) else {
            continue; // added by name, or uninstalled: keep what we have
        };
        let exec_names = app.exec_names.join("\n");
        if exec_names == entry.exec_names && app.flatpak_id == entry.flatpak_id {
            continue;
        }
        sqlx::query(
            "UPDATE block_app SET exec_names = ?, flatpak_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&exec_names)
        .bind(&app.flatpak_id)
        .bind(now_text())
        .bind(&entry.id)
        .execute(pool)
        .await?;
        changed += 1;
    }
    Ok(changed)
}
