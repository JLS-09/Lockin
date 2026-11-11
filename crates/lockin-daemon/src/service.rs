//! `lockin-daemon run`: the background service (the systemd user unit `lockin.service`).
//!
//! It loops until it's stopped:
//! - every few seconds, `session::tick`: start, end or mark missed the study
//!   sessions that are due, and end breaks that ran out;
//! - when a feed's last sync is over an hour old, fetch all feeds again
//!   (`import::sync_all`), in the background so a slow download never holds
//!   up a session.
//!
//! - answer the app on the socket (`lockin_core::ipc`): its status, and taking
//!   a break, resuming or the emergency stop in the running session.
//!
//! Only one daemon runs at a time: it holds a lock on a file next to the database.
//! Its messages go to the terminal, or to the journal under systemd
//! (`journalctl --user -u lockin`).

use std::fs::{File, TryLockError};
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{DateTime, Local, Utc};
use lockin_core::ipc::{self, DaemonStatus, Request, Response};
use lockin_core::model::TickResult;
use lockin_core::{db, import, now_text, paths, session};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// How often the daemon looks at the study sessions.
pub const TICK_SECONDS: u64 = 5;
/// After a failed sync (offline, say), wait this long before trying again.
pub const SYNC_RETRY_MINUTES: i64 = 10;

/// Run until Ctrl+C or a stop from systemd.
pub async fn run(pool: db::Pool) -> Result<()> {
    // Kept until the end of `run`: the lock lasts as long as the file is open.
    let _lock = lock_single_instance()?;
    say(&format!(
        "lockin-daemon running, database {}",
        paths::database_file()?.display()
    ));

    let started_at = now_text();
    let socket = ipc::socket_path()?;
    let listener = listen(&socket)?;

    let mut ticker = tokio::time::interval(Duration::from_secs(TICK_SECONDS));
    // After a suspend, tick once and carry on; don't catch up on every missed tick.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut syncing: Option<JoinHandle<()>> = None;
    let mut last_sync_attempt: Option<DateTime<Utc>> = None;

    let stop = stop_signal();
    tokio::pin!(stop);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let now = Utc::now();

                match session::tick(&pool, now).await {
                    Ok(result) => report(&pool, &result).await,
                    // Keep going: the next tick tries again.
                    Err(error) => say(&format!("study sessions: {error:#}")),
                }

                let busy = syncing.as_ref().is_some_and(|task| !task.is_finished());
                if !busy && sync_wanted(&pool, now, last_sync_attempt).await {
                    last_sync_attempt = Some(now);
                    syncing = Some(tokio::spawn(sync(pool.clone())));
                }
            }
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    // Each connection gets its own task, so a slow client never holds up the loop.
                    tokio::spawn(answer_connection(stream, pool.clone(), started_at.clone()));
                }
            }
            _ = &mut stop => {
                say("stopping");
                if let Some(task) = syncing {
                    task.abort();
                }
                break;
            }
        }
    }
    let _ = std::fs::remove_file(&socket);
    pool.close().await;
    Ok(())
}

/// Open the socket the app talks to. A file left by a daemon that crashed is
/// removed first: we hold the lock, so no other daemon uses it.
fn listen(socket: &Path) -> Result<UnixListener> {
    if let Some(folder) = socket.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket)?;
    // Only you may connect (the runtime folder is private already).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(listener)
}

/// One connection: read one request line, write one response line.
async fn answer_connection(stream: UnixStream, pool: db::Pool, started_at: String) {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    // A request is one short line; more than 4 KB isn't one of ours.
    let mut reader = BufReader::new(reader).take(4096);
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) => answer(&pool, &started_at, request, Utc::now()).await,
        Err(_) => Response::failed("unknown request"),
    };
    if let Ok(mut text) = serde_json::to_string(&response) {
        text.push('\n');
        let _ = writer.write_all(text.as_bytes()).await;
    }
}

/// What the daemon answers to a request, at `now`.
async fn answer(
    pool: &db::Pool,
    started_at: &str,
    request: Request,
    now: DateTime<Utc>,
) -> Response {
    let result = match request {
        Request::Status => {
            return match import::last_sync(pool).await {
                Ok(last_sync_at) => Response {
                    status: Some(DaemonStatus {
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        started_at: started_at.to_string(),
                        last_sync_at,
                    }),
                    ..Response::done()
                },
                Err(error) => Response::failed(error),
            };
        }
        Request::TakeBreak => match running(pool).await {
            Ok(id) => session::take_break(pool, &id, now)
                .await
                .map(|_| "break started"),
            Err(error) => Err(error),
        },
        Request::Resume => match running(pool).await {
            Ok(id) => session::resume(pool, &id, now, None)
                .await
                .map(|_| "break ended"),
            Err(error) => Err(error),
        },
        Request::EmergencyStop => match running(pool).await {
            Ok(id) => session::emergency_stop(pool, &id, now)
                .await
                .map(|_| "stopped with the emergency button"),
            Err(error) => Err(error),
        },
    };
    match result {
        Ok(what) => {
            say(&format!("study session: {what} (from the app)"));
            Response::done()
        }
        Err(error) => Response::failed(error),
    }
}

/// The id of the running session, or an error that says there is none.
async fn running(pool: &db::Pool) -> Result<String> {
    match session::running(pool).await? {
        Some(running) => Ok(running.id),
        None => bail!("no study session is running"),
    }
}

/// Take the lock on ~/.local/share/lockin/daemon.lock, or fail when another
/// daemon holds it. The operating system releases it when the daemon exits,
/// also after a crash, so it never needs cleaning up.
fn lock_single_instance() -> Result<File> {
    let file_path = paths::data_dir()?.join("daemon.lock");
    let file = File::create(&file_path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => {
            bail!("another lockin-daemon is already running (is lockin.service started?)")
        }
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

/// Resolves on Ctrl+C, or when systemd stops the service (SIGTERM).
async fn stop_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// A sync is wanted when a feed is over an hour old, but not more often than
/// every `SYNC_RETRY_MINUTES`, so being offline doesn't mean trying every tick.
async fn sync_wanted(
    pool: &db::Pool,
    now: DateTime<Utc>,
    last_attempt: Option<DateTime<Utc>>,
) -> bool {
    if let Some(last) = last_attempt
        && now - last < chrono::Duration::minutes(SYNC_RETRY_MINUTES)
    {
        return false;
    }
    match import::sync_due(pool, now).await {
        Ok(due) => due,
        Err(error) => {
            say(&format!("checking the feeds: {error:#}"));
            false
        }
    }
}

/// Fetch every feed again and say what changed. Runs as its own task.
async fn sync(pool: db::Pool) {
    let results = match import::sync_all(&pool).await {
        Ok(results) => results,
        Err(error) => {
            say(&format!("sync failed: {error:#}"));
            return;
        }
    };
    for feed in results {
        // Never print the URL: it contains a personal key.
        let name = format!("feed {} ({})", &feed.source_id[..8], feed.kind);
        match feed.error {
            Some(error) => say(&format!("{name}: sync failed: {error}")),
            None => say(&format!(
                "{name}: synced, {} new, {} moved, {} cancelled",
                feed.added, feed.moved, feed.cancelled
            )),
        }
    }
}

/// Say what a tick changed; nothing when nothing happened.
async fn report(pool: &db::Pool, result: &TickResult) {
    let lines = [
        (&result.started, "started"),
        (&result.ended, "ended"),
        (&result.resumed, "break over, blocking back on"),
        (&result.missed, "missed (the daemon wasn't running)"),
    ];
    for (ids, what) in lines {
        for id in ids {
            let when = match session::get(pool, id).await {
                Ok(item) => session_times(&item.start_at, &item.end_at),
                Err(_) => String::new(),
            };
            // A start logged as "late": the daemon wasn't running at its start.
            let late = what == "started"
                && session::events(pool, id).await.is_ok_and(|events| {
                    events.first().and_then(|e| e.detail.as_deref()) == Some("late")
                });
            let what = if late {
                "started late (the daemon wasn't running at its start)"
            } else {
                what
            };
            say(&format!("study session {} {when}: {what}", &id[..8]));
        }
    }
}

/// "Wed 18 Nov 13:00-15:30"
fn session_times(start: &str, end: &str) -> String {
    let local = |text: &str| DateTime::parse_from_rfc3339(text).map(|t| t.with_timezone(&Local));
    match (local(start), local(end)) {
        (Ok(start), Ok(end)) => format!(
            "{} {}-{}",
            start.format("%a %d %b"),
            start.format("%H:%M"),
            end.format("%H:%M")
        ),
        _ => String::new(),
    }
}

/// One line of output. The journal adds its own timestamps, but a terminal
/// doesn't, so every line starts with the time.
fn say(message: &str) {
    println!("{} {message}", Local::now().format("%H:%M:%S"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use lockin_core::ipc;

    /// A daemon's socket on a test database, answering until the test ends.
    async fn test_daemon() -> (db::Pool, std::path::PathBuf) {
        let pool = db::open_in_memory().await.unwrap();
        let path = std::env::temp_dir().join(format!("lockin-{}.sock", lockin_core::new_id()));
        let listener = listen(&path).unwrap();
        let serving = pool.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(answer_connection(
                    stream,
                    serving.clone(),
                    "2099-11-04T12:00:00Z".into(),
                ));
            }
        });
        (pool, path)
    }

    #[tokio::test]
    async fn status_over_the_socket() {
        let (_pool, path) = test_daemon().await;
        let response = ipc::send_to(&path, &Request::Status).await.unwrap();
        let status = response.status.unwrap();
        assert_eq!(status.started_at, "2099-11-04T12:00:00Z");
        assert_eq!(status.last_sync_at, None);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn breaks_over_the_socket() {
        let (pool, path) = test_daemon().await;
        // Nothing running: a clear error comes back.
        let error = ipc::send_to(&path, &Request::TakeBreak).await.unwrap_err();
        assert_eq!(error.to_string(), "no study session is running");

        // A session that started 40 minutes ago has 5 minutes of credit.
        let now = Utc::now();
        let id = session::plan(
            &pool,
            None,
            None,
            now + chrono::Duration::minutes(1),
            now + chrono::Duration::hours(2),
            now,
        )
        .await
        .unwrap();
        session::start(&pool, &id, now - chrono::Duration::minutes(40), None)
            .await
            .unwrap();

        ipc::send_to(&path, &Request::TakeBreak).await.unwrap();
        let item = session::get(&pool, &id).await.unwrap();
        assert_eq!(
            session::state_now(&pool, &item, Utc::now())
                .await
                .unwrap()
                .phase,
            "break"
        );
        ipc::send_to(&path, &Request::Resume).await.unwrap();
        ipc::send_to(&path, &Request::EmergencyStop).await.unwrap();
        assert_eq!(session::get(&pool, &id).await.unwrap().status, "stopped");
        let _ = std::fs::remove_file(path);
    }
}
