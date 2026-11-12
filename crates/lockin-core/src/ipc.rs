//! Talking to the daemon: a Unix socket at `$XDG_RUNTIME_DIR/lockin/daemon.sock`.
//!
//! The app (and anything else) connects, writes one request as a line of JSON,
//! reads one response line, and closes. For example:
//!
//! ```text
//! → {"request":"take_break"}
//! ← {"ok":true,"error":null,"status":null}
//! ```
//!
//! Reading data (sessions, their log) doesn't go through the daemon: the app
//! reads the database itself. What changes a running session does, so the
//! daemon can switch blocking off and on at once (steps 4 and 5).
//! The daemon's side is in `lockin-daemon/src/service.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::model::EmergencyStatus;
use crate::paths;

/// How long a request may take before the app gives up on the daemon.
pub const TIMEOUT_SECONDS: u64 = 3;

/// The error text when nothing listens on the socket. The app shows it as
/// "Daemon not running", so it stays one fixed sentence.
pub const NOT_RUNNING: &str = "the Lockin daemon isn't running";

/// What can be asked of the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum Request {
    /// Is it running, since when, and when did it last sync?
    Status,
    /// Start a break in the running session (needs break credit).
    TakeBreak,
    /// End the break: back to work, blocking on.
    Resume,
    /// Open the emergency stop, or keep it open: sent when you press "Emergency
    /// stop" and every 2 seconds while it stays open. After 2 minutes the daemon
    /// ends the session (`emergency.rs`); the answer carries `emergency`.
    EmergencyKeepOpen,
    /// "Keep studying": close the emergency stop.
    EmergencyCancel,
    /// Show a test notification (Settings → Tray and notifications).
    TestNotification,
}

/// The daemon's answer. `ok` false means `error` says why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub error: Option<String>,
    /// Only for `Request::Status`.
    pub status: Option<DaemonStatus>,
    /// Only for `Request::EmergencyKeepOpen`: where the stop stands.
    #[serde(default)]
    pub emergency: Option<EmergencyStatus>,
}

impl Response {
    pub fn done() -> Response {
        Response {
            ok: true,
            error: None,
            status: None,
            emergency: None,
        }
    }

    pub fn failed(error: impl std::fmt::Display) -> Response {
        Response {
            ok: false,
            error: Some(format!("{error:#}")),
            status: None,
            emergency: None,
        }
    }
}

/// What `Request::Status` returns, for the sidebar ("Daemon running · synced 2 min ago").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonStatus {
    /// The daemon's version, e.g. "0.1.0".
    pub version: String,
    /// When it started, RFC 3339 in UTC.
    pub started_at: String,
    /// The most recent feed sync, RFC 3339 in UTC; None without feeds.
    pub last_sync_at: Option<String>,
    /// Why websites can't be blocked right now (the helper isn't set up, polkit
    /// refused, …); None when blocking works or nothing needs blocking.
    #[serde(default)]
    pub websites_problem: Option<String>,
}

/// Where the socket lives: in the runtime folder, which only you can open and
/// which is emptied at logout. Without one, next to the database.
pub fn socket_path() -> Result<PathBuf> {
    let folder = match paths::runtime_dir() {
        Some(folder) => folder,
        None => paths::data_dir()?,
    };
    Ok(folder.join("daemon.sock"))
}

/// Send one request to the running daemon. An `ok: false` answer becomes an error
/// with the daemon's message; no daemon becomes the error `NOT_RUNNING`.
pub async fn send(request: &Request) -> Result<Response> {
    send_to(&socket_path()?, request).await
}

/// `send`, to a socket at `path` (the tests use their own).
pub async fn send_to(path: &Path, request: &Request) -> Result<Response> {
    let exchange = exchange(path, request);
    let response = tokio::time::timeout(Duration::from_secs(TIMEOUT_SECONDS), exchange)
        .await
        .map_err(|_| anyhow!("the Lockin daemon didn't answer"))??;
    if !response.ok {
        bail!(
            "{}",
            response
                .error
                .unwrap_or_else(|| "the daemon refused".into())
        );
    }
    Ok(response)
}

#[cfg(unix)]
async fn exchange(path: &Path, request: &Request) -> Result<Response> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(path).await else {
        bail!(NOT_RUNNING);
    };
    let mut line = serde_json::to_string(request)?;
    line.push('\n');
    stream.write_all(line.as_bytes()).await?;

    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).await?;
    serde_json::from_str(&answer).context("the daemon's answer wasn't understood")
}

/// Other systems get their own way to reach the daemon later (a named pipe on Windows).
#[cfg(not(unix))]
async fn exchange(_path: &Path, _request: &Request) -> Result<Response> {
    bail!(NOT_RUNNING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_one_json_line() {
        let text = serde_json::to_string(&Request::TakeBreak).unwrap();
        assert_eq!(text, r#"{"request":"take_break"}"#);
        let back: Request = serde_json::from_str(r#"{"request":"emergency_keep_open"}"#).unwrap();
        assert_eq!(back, Request::EmergencyKeepOpen);
        // An answer from an older daemon, without `emergency`, still reads.
        let old: Response =
            serde_json::from_str(r#"{"ok":true,"error":null,"status":null}"#).unwrap();
        assert_eq!(old, Response::done());
    }

    #[tokio::test]
    async fn no_daemon_is_a_clear_error() {
        let path = std::env::temp_dir().join(format!("lockin-test-{}.sock", crate::new_id()));
        let error = send_to(&path, &Request::Status).await.unwrap_err();
        assert_eq!(error.to_string(), NOT_RUNNING);
    }
}
