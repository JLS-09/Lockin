//! `lockin-daemon run`: the background service (the systemd user unit `lockin.service`).
//!
//! It loops until it's stopped:
//! - every few seconds, `session::tick`: start, end or mark missed the study
//!   sessions that are due, and end breaks that ran out;
//! - when a feed's last sync is over an hour old, fetch all feeds again
//!   (`import::sync_all`), in the background so a slow download never holds
//!   up a session.
//!
//! - every 2 seconds while you study (not on a break), close the apps on the
//!   session's block profile (`blocker.rs`) and block its websites
//!   (`sites.rs`); on a break and after the session, unblock them;
//! - answer the app on the socket (`lockin_core::ipc`): its status, and taking
//!   a break, resuming or the emergency stop in the running session;
//! - every 2 seconds, look at the emergency stop: after 2 minutes kept open it
//!   ends the session (−50 points), and it's dropped when the app stops checking
//!   in (`lockin_core::emergency`).
//!
//! Only one daemon runs at a time: it holds a lock on a file next to the database.
//! Its messages go to the terminal, or to the journal under systemd
//! (`journalctl --user -u lockin`).

use std::fs::{File, TryLockError};
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{DateTime, Local, Utc};
use lockin_core::emergency::{Checked, EmergencyStop};
use lockin_core::ipc::{self, DaemonStatus, Request, Response};
use lockin_core::model::TickResult;
use lockin_core::{block, db, import, now_text, paths, session};

use crate::blocker::{self, AppBlocker};
use crate::sites::SiteBlocker;

/// What every connection to the socket may read: since when the daemon runs,
/// why websites can't be blocked (if so), and the emergency stop.
/// Cloning shares the same values.
#[derive(Clone)]
struct Shared {
    started_at: String,
    websites_problem: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// A tokio Mutex: it stays locked while the stop talks to the database.
    emergency: std::sync::Arc<tokio::sync::Mutex<EmergencyStop>>,
}

impl Shared {
    fn new(started_at: String) -> Shared {
        Shared {
            started_at,
            websites_problem: Default::default(),
            emergency: Default::default(),
        }
    }

    fn websites_problem(&self) -> Option<String> {
        self.websites_problem
            .lock()
            .map(|p| p.clone())
            .unwrap_or(None)
    }
}
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// How often the daemon looks at the study sessions.
pub const TICK_SECONDS: u64 = 5;
/// How often the daemon looks for blocked apps while you study.
pub const BLOCK_SECONDS: u64 = 2;
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

    let shared = Shared::new(now_text());
    let socket = ipc::socket_path()?;
    let listener = listen(&socket)?;

    let mut ticker = tokio::time::interval(Duration::from_secs(TICK_SECONDS));
    // After a suspend, tick once and carry on; don't catch up on every missed tick.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut block_ticker = tokio::time::interval(Duration::from_secs(BLOCK_SECONDS));
    block_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut blocker = blocker::for_this_system();
    let mut sites = SiteBlocker::default();
    refresh_blocked_apps(&pool).await;

    let mut syncing: Option<JoinHandle<()>> = None;
    let mut last_sync_attempt: Option<DateTime<Utc>> = None;

    let stop = stop_signal();
    tokio::pin!(stop);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let now = Utc::now();

                match session::tick(&pool, now).await {
                    Ok(result) => {
                        report(&pool, &result).await;
                        // A session starts: block with what's installed right now.
                        if !result.started.is_empty() {
                            refresh_blocked_apps(&pool).await;
                        }
                    }
                    // Keep going: the next tick tries again.
                    Err(error) => say(&format!("study sessions: {error:#}")),
                }

                let busy = syncing.as_ref().is_some_and(|task| !task.is_finished());
                if !busy && sync_wanted(&pool, now, last_sync_attempt).await {
                    last_sync_attempt = Some(now);
                    syncing = Some(tokio::spawn(sync(pool.clone())));
                }
            }
            _ = block_ticker.tick() => {
                check_emergency(&pool, &shared).await;
                enforce_blocking(&pool, blocker.as_mut(), &mut sites).await;
                if let Ok(mut problem) = shared.websites_problem.lock() {
                    problem.clone_from(&sites.problem);
                }
            }
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    // Each connection gets its own task, so a slow client never holds up the loop.
                    tokio::spawn(answer_connection(stream, pool.clone(), shared.clone()));
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
async fn answer_connection(stream: UnixStream, pool: db::Pool, shared: Shared) {
    let (reader, mut writer) = stream.into_split();
    let mut line = String::new();
    // A request is one short line; more than 4 KB isn't one of ours.
    let mut reader = BufReader::new(reader).take(4096);
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) => answer(&pool, &shared, request, Utc::now()).await,
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
    shared: &Shared,
    request: Request,
    now: DateTime<Utc>,
) -> Response {
    let result = match request {
        Request::Status => {
            return match import::last_sync(pool).await {
                Ok(last_sync_at) => Response {
                    status: Some(DaemonStatus {
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        started_at: shared.started_at.clone(),
                        last_sync_at,
                        websites_problem: shared.websites_problem(),
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
        Request::EmergencyKeepOpen => {
            let mut stop = shared.emergency.lock().await;
            let opening = stop.status(now).is_none();
            return match stop.keep_open(pool, now).await {
                Ok(status) => {
                    if status.stopped {
                        say(&format!(
                            "study session: stopped with the emergency button, -{} points",
                            status.penalty_points
                        ));
                    } else if opening {
                        say("study session: emergency stop opened (2 minutes to go)");
                    }
                    Response {
                        emergency: Some(status),
                        ..Response::done()
                    }
                }
                Err(error) => Response::failed(error),
            };
        }
        Request::EmergencyCancel => {
            if shared.emergency.lock().await.cancel() {
                say("study session: emergency stop closed, keep studying");
            }
            return Response::done();
        }
    };
    match result {
        Ok(what) => {
            say(&format!("study session: {what} (from the app)"));
            Response::done()
        }
        Err(error) => Response::failed(error),
    }
}

/// Every 2 seconds: end the session once the emergency stop was kept open for
/// 2 minutes, or drop the stop when the app stopped checking in.
async fn check_emergency(pool: &db::Pool, shared: &Shared) {
    let mut stop = shared.emergency.lock().await;
    match stop.check(pool, Utc::now()).await {
        Ok(Checked::Nothing) => {}
        Ok(Checked::Lapsed) => say("study session: emergency stop closed (the app stopped asking)"),
        Ok(Checked::Stopped(_)) => say(&format!(
            "study session: stopped with the emergency button, -{} points",
            session::EMERGENCY_PENALTY_POINTS
        )),
        Err(error) => say(&format!("emergency stop: {error:#}")),
    }
}

/// The id of the running session, or an error that says there is none.
async fn running(pool: &db::Pool) -> Result<String> {
    match session::running(pool).await? {
        Some(running) => Ok(running.id),
        None => bail!("no study session is running"),
    }
}

/// Update what the profiles' apps look for from the apps installed now
/// (`block::refresh_apps`). Reading the desktop files runs off the async threads.
async fn refresh_blocked_apps(pool: &db::Pool) {
    let Ok(installed) = tokio::task::spawn_blocking(lockin_core::apps::installed_apps).await else {
        return;
    };
    match block::refresh_apps(pool, &installed).await {
        Ok(0) => {}
        Ok(changed) => say(&format!(
            "updated {changed} apps on block profiles from the installed apps"
        )),
        Err(error) => say(&format!("blocking: {error:#}")),
    }
}

/// Close the blocked apps and block the websites of the running session
/// (nothing on a break), and write down each app close for the Session page.
async fn enforce_blocking(pool: &db::Pool, blocker: &mut dyn AppBlocker, sites: &mut SiteBlocker) {
    let now = Utc::now();
    let blocked = match block::blocked_now(pool, now).await {
        Ok(found) => found,
        Err(error) => {
            say(&format!("blocking: {error:#}"));
            return;
        }
    };
    if let Some(change) = sites.apply(&blocked.sites).await {
        say(&change);
    }
    let closed = blocker.close(&blocked.apps);
    let Some(session_id) = blocked.session_id else {
        return;
    };
    for app in closed {
        say(&format!(
            "closed {} (blocked during this session)",
            app.name
        ));
        if let Err(error) = block::record_close(pool, &session_id, &app, now).await {
            say(&format!("blocking: {error:#}"));
        }
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
    use lockin_core::model::SessionDetails;

    /// A daemon's socket on a test database, answering until the test ends.
    async fn test_daemon() -> (db::Pool, std::path::PathBuf) {
        let pool = db::open_in_memory().await.unwrap();
        let path = std::env::temp_dir().join(format!("lockin-{}.sock", lockin_core::new_id()));
        let listener = listen(&path).unwrap();
        let serving = pool.clone();
        // One Shared for every connection, as in `run`: the emergency stop lives there.
        let shared = Shared::new("2099-11-04T12:00:00Z".into());
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(answer_connection(stream, serving.clone(), shared.clone()));
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
            &SessionDetails::default(),
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

        // The emergency stop opens with 2 minutes to go and doesn't stop at once.
        let response = ipc::send_to(&path, &Request::EmergencyKeepOpen)
            .await
            .unwrap();
        let first = response.emergency.unwrap();
        assert!(!first.stopped);
        assert_eq!(first.penalty_points, 50);
        // Checking in again keeps the same stop.
        let again = ipc::send_to(&path, &Request::EmergencyKeepOpen)
            .await
            .unwrap()
            .emergency
            .unwrap();
        assert_eq!(again.stops_at, first.stops_at);
        ipc::send_to(&path, &Request::EmergencyCancel)
            .await
            .unwrap();
        assert_eq!(session::get(&pool, &id).await.unwrap().status, "running");
        let _ = std::fs::remove_file(path);
    }
}
