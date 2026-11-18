//! The cloud sync: your data in your Lockin account, the same on every computer.
//!
//! SQLite stays the source of truth and Lockin works offline; the daemon runs
//! `sync` every few minutes while you're logged in:
//!
//! 1. **Send** every row changed since the last send (`updated_at` at or after
//!    the `push_mark`) to the cloud table of the same name, as an upsert. The
//!    cloud keeps the row with the later `updated_at` (the `keep_newer` trigger),
//!    so a computer that was offline can't overwrite a newer change.
//! 2. **Fetch** every row the cloud got since the last fetch (its `synced_at`,
//!    the server's clock, after the newest one seen per table) and write it
//!    here, unless the row here is newer.
//!
//! Each side keeps the newer version, so all computers end up the same. Rows are
//! never deleted, only soft-deleted (`deleted = 1`), so deletes sync like any
//! other change.
//!
//! - Tables and columns: the 16 tables in `TABLES`, with the same columns as here
//!   (supabase/migrations); `calendar_source.last_fetched_at` stays on each computer.
//! - Feed links hold a personal key: they're encrypted with the account's feed key
//!   (`account::feed_key`) before they leave the computer (`encrypt_url`).
//! - A database belongs to one account: the first sync remembers it, and logging
//!   in with another account doesn't sync this database into that one.
//! - Where it stands is kept in `sync_state` (migration 0014), on this computer only.

use std::time::Duration;

use anyhow::Context;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use sqlx::{Connection, SqliteConnection};

use crate::account::{Cloud, Session};
use crate::db::Pool;

/// The synced tables, parents before the rows that point at them.
pub const TABLES: &[&str] = &[
    "course",
    "calendar_source",
    "history_import",
    "filter_rule",
    "class_block",
    "activity_template",
    "activity",
    "deadline",
    "block_profile",
    "block_app",
    "block_site",
    "session_series",
    "study_session",
    "session_event",
    "app_closed",
    "points_ledger",
];

/// Columns that stay on each computer: (table, column).
const LOCAL_ONLY: &[(&str, &str)] = &[("calendar_source", "last_fetched_at")];

/// Rows per request when fetching (the most Supabase returns at once).
const PAGE: usize = 1000;
/// Rows per request when sending.
const SEND_BATCH: usize = 500;
/// Fetch a bit before the newest `synced_at` seen, for rows whose transaction
/// was still running at the last fetch. Fetching a row twice changes nothing.
const FETCH_OVERLAP_SECONDS: i64 = 60;
/// Give up on one request after this long.
const TIMEOUT_SECONDS: u64 = 60;

/// An encrypted feed link starts with this.
const ENCRYPTED: &str = "enc1:";

/// A time as stored here: RFC 3339 in UTC with seconds.
fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// What one sync did.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// Rows sent to the cloud.
    pub sent: u32,
    /// Rows from the cloud written here (newer than what was here).
    pub received: u32,
    /// Rows from the cloud that couldn't be written here, e.g. a class this
    /// computer already has under another id (see the plan, phase 5 step 5).
    pub skipped: Vec<String>,
}

/// Why a sync didn't work.
#[derive(Debug)]
pub enum SyncError {
    /// This database already syncs with another account.
    OtherAccount,
    /// The cloud couldn't be reached.
    Offline,
    /// The login ran out or was refused: log in again.
    LoggedOut,
    /// The cloud answered with an error.
    Server(String),
    /// Something went wrong here (the database).
    Local(anyhow::Error),
}

impl SyncError {
    /// The translation key the app shows (en.json / nl.json).
    pub fn key(&self) -> &'static str {
        match self {
            SyncError::OtherAccount => "sync.error.otherAccount",
            SyncError::Offline => "sync.error.offline",
            SyncError::LoggedOut => "sync.error.loggedOut",
            SyncError::Server(_) => "sync.error.server",
            SyncError::Local(_) => "sync.error.local",
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::OtherAccount => {
                write!(f, "this computer's data belongs to another Lockin account")
            }
            SyncError::Offline => write!(f, "the cloud can't be reached"),
            SyncError::LoggedOut => write!(f, "the login ran out: log in again"),
            SyncError::Server(message) => write!(f, "the cloud said: {message}"),
            SyncError::Local(error) => write!(f, "{error:#}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<anyhow::Error> for SyncError {
    fn from(error: anyhow::Error) -> Self {
        SyncError::Local(error)
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(error: sqlx::Error) -> Self {
        SyncError::Local(error.into())
    }
}

/// Send what changed here, then fetch what changed in the cloud. The outcome
/// (when, and the error if any) is kept in `sync_state` for the app.
pub async fn sync(
    pool: &Pool,
    cloud: &Cloud,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<SyncReport, SyncError> {
    let result = sync_once(pool, cloud, session, now).await;
    let error = match &result {
        Ok(_) => String::new(),
        Err(error) => error.key().to_string(),
    };
    set_state(pool, "last_error", &error).await?;
    if result.is_ok() {
        set_state(pool, "last_sync_at", &time_text(now)).await?;
    }
    result
}

async fn sync_once(
    pool: &Pool,
    cloud: &Cloud,
    session: &Session,
    now: DateTime<Utc>,
) -> Result<SyncReport, SyncError> {
    // The first sync ties this database to the account.
    match state(pool, "account").await? {
        Some(owner) if owner != session.user_id => return Err(SyncError::OtherAccount),
        Some(_) => {}
        None => set_state(pool, "account", &session.user_id).await?,
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECONDS))
        .build()
        .map_err(|error| SyncError::Server(error.to_string()))?;
    let mut report = SyncReport::default();

    // 1. Send. Changes made while sending have an updated_at at or after `mark`,
    //    so they go next time (some may go twice, which changes nothing).
    let since = state(pool, "push_mark").await?.unwrap_or_default();
    let mark = time_text(now);
    for table in TABLES {
        let rows = rows_to_send(pool, table, &since, session).await?;
        for batch in rows.chunks(SEND_BATCH) {
            send(&client, cloud, session, table, batch).await?;
            report.sent += batch.len() as u32;
        }
    }
    set_state(pool, "push_mark", &mark).await?;

    // 2. Fetch. Foreign keys are off while writing: rows can arrive before the
    //    row they point at (a skipped one, say). They point at the right ids.
    let mut connection = pool.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await?;
    let fetched = fetch_all(&mut connection, &client, cloud, session, &mut report).await;
    let restored = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await;
    if restored.is_err() {
        // Never hand a connection without foreign keys back to the pool.
        connection.detach();
    }
    fetched?;
    Ok(report)
}

/// Fetch every table's new rows, page by page.
async fn fetch_all(
    connection: &mut SqliteConnection,
    client: &reqwest::Client,
    cloud: &Cloud,
    session: &Session,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    for table in TABLES {
        let cursor_name = format!("pull:{table}");
        let cursor = state_on(connection, &cursor_name).await?;
        let after = cursor.as_deref().map(with_overlap);
        let mut newest = cursor.clone();
        let mut offset = 0;
        loop {
            let rows = fetch_page(client, cloud, session, table, after.as_deref(), offset).await?;
            let mut transaction = connection.begin().await?;
            let applied = apply_rows(&mut transaction, table, &rows, &session.feed_key).await?;
            transaction.commit().await?;
            report.received += applied.written;
            report.skipped.extend(applied.skipped);
            for row in &rows {
                if let Some(synced_at) = row["synced_at"].as_str()
                    && newer_time(synced_at, newest.as_deref())
                {
                    newest = Some(synced_at.to_string());
                }
            }
            if rows.len() < PAGE {
                break;
            }
            offset += rows.len();
        }
        if newest != cursor
            && let Some(newest) = newest
        {
            set_state_on(connection, &cursor_name, &newest).await?;
        }
    }
    Ok(())
}

// ---------- Sending ----------

/// The rows of a table changed at or after `since` (all of them when it's
/// empty), as JSON for the cloud: the same columns, plus the account's user id,
/// with the feed link encrypted.
pub async fn rows_to_send(
    pool: &Pool,
    table: &str,
    since: &str,
    session: &Session,
) -> Result<Vec<Value>, SyncError> {
    let columns = synced_columns(pool, table).await?;
    // json_object('id', id, 'name', name, …): SQLite builds the JSON itself,
    // with numbers as numbers and NULL as null.
    let pairs: Vec<String> = columns.iter().map(|c| format!("'{c}', {c}")).collect();
    let sql = format!(
        "SELECT json_object({}) FROM {table} WHERE updated_at >= ? ORDER BY updated_at",
        pairs.join(", ")
    );
    let texts: Vec<String> = sqlx::query_scalar(&sql).bind(since).fetch_all(pool).await?;
    let mut rows = Vec::with_capacity(texts.len());
    for text in texts {
        let mut row: Map<String, Value> =
            serde_json::from_str(&text).context("reading a row to send")?;
        row.insert("user_id".to_string(), Value::from(session.user_id.clone()));
        if table == "calendar_source"
            && let Some(url) = row.get("url").and_then(Value::as_str)
            && !is_locked(url)
        {
            let encrypted = encrypt_url(&session.feed_key, url)?;
            row.insert("url".to_string(), Value::from(encrypted));
        }
        rows.push(Value::Object(row));
    }
    Ok(rows)
}

/// Upsert rows into a cloud table (PostgREST: POST with merge-duplicates).
async fn send(
    client: &reqwest::Client,
    cloud: &Cloud,
    session: &Session,
    table: &str,
    rows: &[Value],
) -> Result<(), SyncError> {
    let response = client
        .post(format!("{}/{table}", cloud.rest_url))
        .query(&[("on_conflict", "user_id,id")])
        .header("apikey", &cloud.key)
        .bearer_auth(&session.access_token)
        .header("content-type", "application/json")
        .header("prefer", "resolution=merge-duplicates,return=minimal")
        .body(Value::Array(rows.to_vec()).to_string())
        .send()
        .await
        .map_err(|_| SyncError::Offline)?;
    check(response).await.map(|_| ())
}

// ---------- Fetching ----------

/// One page of a table's rows that reached the cloud after `after`, oldest first.
async fn fetch_page(
    client: &reqwest::Client,
    cloud: &Cloud,
    session: &Session,
    table: &str,
    after: Option<&str>,
    offset: usize,
) -> Result<Vec<Value>, SyncError> {
    let mut query = vec![
        ("select", "*".to_string()),
        ("order", "synced_at.asc,id.asc".to_string()),
        ("limit", PAGE.to_string()),
        ("offset", offset.to_string()),
    ];
    if let Some(after) = after {
        query.push(("synced_at", format!("gt.{after}")));
    }
    let response = client
        .get(format!("{}/{table}", cloud.rest_url))
        .query(&query)
        .header("apikey", &cloud.key)
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|_| SyncError::Offline)?;
    let text = check(response).await?;
    let rows: Vec<Value> = serde_json::from_str(&text)
        .map_err(|_| SyncError::Server(format!("not a list of rows: {text}")))?;
    Ok(rows)
}

/// What writing fetched rows did.
#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// Rows written (new here, or newer than the row here).
    pub written: u32,
    /// Rows that couldn't be written: "table id: why".
    pub skipped: Vec<String>,
}

/// Write rows fetched from the cloud into a table, keeping the newer version of
/// each: a row here with a later (or the same) `updated_at` stays as it is.
/// A row that breaks a rule here (e.g. a class this computer already has under
/// another id) is skipped and listed.
pub async fn apply_rows(
    connection: &mut SqliteConnection,
    table: &str,
    rows: &[Value],
    feed_key: &str,
) -> Result<Applied, SyncError> {
    let mut applied = Applied::default();
    if rows.is_empty() {
        return Ok(applied);
    }
    let columns = synced_columns_on(connection, table).await?;
    let placeholders = vec!["?"; columns.len()].join(", ");
    let updates: Vec<String> = columns
        .iter()
        .filter(|c| *c != "id")
        .map(|c| format!("{c} = excluded.{c}"))
        .collect();
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders})
         ON CONFLICT (id) DO UPDATE SET {}
         WHERE excluded.updated_at > {table}.updated_at",
        columns.join(", "),
        updates.join(", ")
    );

    for row in rows {
        let id = row["id"].as_str().unwrap_or("?").to_string();
        let mut query = sqlx::query(&sql);
        for column in &columns {
            let mut value = row.get(column).cloned().unwrap_or(Value::Null);
            if table == "calendar_source"
                && column == "url"
                && let Some(text) = value.as_str()
            {
                // Can't decrypt (another password): keep it encrypted, the
                // feed then asks for its link again (`is_locked`).
                value =
                    Value::from(decrypt_url(feed_key, text).unwrap_or_else(|| text.to_string()));
            }
            query = match value {
                Value::Null => query.bind(None::<String>),
                Value::String(text) => query.bind(text),
                Value::Number(number) => query.bind(number.as_i64()),
                Value::Bool(flag) => query.bind(flag),
                other => query.bind(other.to_string()),
            };
        }
        match query.execute(&mut *connection).await {
            Ok(done) => applied.written += done.rows_affected() as u32,
            Err(error) => applied.skipped.push(format!("{table} {id}: {error}")),
        }
    }
    Ok(applied)
}

/// The body of a successful answer, or the error it means.
async fn check(response: reqwest::Response) -> Result<String, SyncError> {
    let status = response.status().as_u16();
    let text = response.text().await.map_err(|_| SyncError::Offline)?;
    match status {
        200..=299 => Ok(text),
        // An expired or refused login (PostgREST: "JWT expired").
        401 => Err(SyncError::LoggedOut),
        _ => Err(SyncError::Server(format!("{status}: {text}"))),
    }
}

// ---------- Feed links ----------

/// Encrypt a feed link with the account's feed key (XChaCha20-Poly1305, a random
/// nonce each time): "enc1:" + hex(nonce + ciphertext).
pub fn encrypt_url(feed_key: &str, url: &str) -> anyhow::Result<String> {
    let cipher = cipher(feed_key)?;
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher
        .encrypt(&nonce, url.as_bytes())
        .map_err(|_| anyhow::anyhow!("couldn't encrypt a feed link"))?;
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    Ok(format!("{ENCRYPTED}{}", to_hex(&bytes)))
}

/// The feed link in an encrypted text, or None when this key can't open it
/// (another password) or it isn't one. A plain link comes back as it is.
pub fn decrypt_url(feed_key: &str, text: &str) -> Option<String> {
    let Some(hex) = text.strip_prefix(ENCRYPTED) else {
        return Some(text.to_string());
    };
    let bytes = from_hex(hex)?;
    if bytes.len() < 24 {
        return None;
    }
    let (nonce, sealed) = bytes.split_at(24);
    let opened = cipher(feed_key)
        .ok()?
        .decrypt(XNonce::from_slice(nonce), sealed)
        .ok()?;
    String::from_utf8(opened).ok()
}

/// True for a feed link that arrived encrypted and couldn't be opened here.
pub fn is_locked(url: &str) -> bool {
    url.starts_with(ENCRYPTED)
}

fn cipher(feed_key: &str) -> anyhow::Result<XChaCha20Poly1305> {
    let key = from_hex(feed_key)
        .filter(|k| k.len() == 32)
        .context("not a feed key")?;
    XChaCha20Poly1305::new_from_slice(&key).map_err(|_| anyhow::anyhow!("not a feed key"))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

// ---------- Columns and state ----------

/// The columns of a table that sync, from SQLite itself, so a new column in a
/// migration syncs without changing this file (the cloud needs it too: the
/// `columns_match_the_cloud` test checks that).
pub async fn synced_columns(pool: &Pool, table: &str) -> Result<Vec<String>, SyncError> {
    let mut connection = pool.acquire().await?;
    synced_columns_on(&mut connection, table).await
}

async fn synced_columns_on(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<Vec<String>, SyncError> {
    let all: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
        .bind(table)
        .fetch_all(&mut *connection)
        .await?;
    Ok(all
        .into_iter()
        .filter(|column| !LOCAL_ONLY.contains(&(table, column.as_str())))
        .collect())
}

/// A value from `sync_state`.
pub async fn state(pool: &Pool, name: &str) -> Result<Option<String>, SyncError> {
    let mut connection = pool.acquire().await?;
    state_on(&mut connection, name).await
}

async fn state_on(
    connection: &mut SqliteConnection,
    name: &str,
) -> Result<Option<String>, SyncError> {
    let value = sqlx::query_scalar("SELECT value FROM sync_state WHERE name = ?")
        .bind(name)
        .fetch_optional(&mut *connection)
        .await?;
    Ok(value)
}

async fn set_state(pool: &Pool, name: &str, value: &str) -> Result<(), SyncError> {
    let mut connection = pool.acquire().await?;
    set_state_on(&mut connection, name, value).await
}

async fn set_state_on(
    connection: &mut SqliteConnection,
    name: &str,
    value: &str,
) -> Result<(), SyncError> {
    sqlx::query(
        "INSERT INTO sync_state (name, value) VALUES (?, ?)
         ON CONFLICT (name) DO UPDATE SET value = excluded.value",
    )
    .bind(name)
    .bind(value)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

/// `synced_at` minus the overlap, as the cloud writes times.
fn with_overlap(synced_at: &str) -> String {
    match DateTime::parse_from_rfc3339(synced_at) {
        Ok(time) => (time - chrono::Duration::seconds(FETCH_OVERLAP_SECONDS))
            .with_timezone(&Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        Err(_) => synced_at.to_string(),
    }
}

/// True when `candidate` is later than `known` (or nothing is known yet).
fn newer_time(candidate: &str, known: Option<&str>) -> bool {
    let Some(known) = known else {
        return true;
    };
    match (
        DateTime::parse_from_rfc3339(candidate),
        DateTime::parse_from_rfc3339(known),
    ) {
        (Ok(candidate), Ok(known)) => candidate > known,
        _ => candidate > known,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_links_are_encrypted() {
        let key = crate::account::feed_key("secret123", "user-1");
        let url = "https://cloud.timeedit.net/ugent/web/ri6Q7XY.ics?key=secret";
        let sealed = encrypt_url(&key, url).unwrap();
        assert!(sealed.starts_with("enc1:"));
        assert!(!sealed.contains("timeedit"));
        assert_ne!(
            sealed,
            encrypt_url(&key, url).unwrap(),
            "a new nonce each time"
        );
        assert_eq!(decrypt_url(&key, &sealed).as_deref(), Some(url));

        // Another password: can't be opened, so it stays locked.
        let other = crate::account::feed_key("other-password", "user-1");
        assert_eq!(decrypt_url(&other, &sealed), None);
        assert!(is_locked(&sealed));
        // A plain link (never encrypted) passes as it is.
        assert_eq!(decrypt_url(&key, url).as_deref(), Some(url));
        assert!(!is_locked(url));
    }

    #[test]
    fn fetching_overlaps_a_minute() {
        assert_eq!(
            with_overlap("2026-11-18T18:05:12.123456+00:00"),
            "2026-11-18T18:04:12.123456Z"
        );
        assert!(newer_time(
            "2026-11-18T18:05:12.5+00:00",
            Some("2026-11-18T18:05:12.123456+00:00")
        ));
        assert!(!newer_time(
            "2026-11-18T18:05:12+00:00",
            Some("2026-11-18T18:05:12.1+00:00")
        ));
        assert!(newer_time("2026-11-18T18:05:12+00:00", None));
    }
}
