//! Blocking websites while you study, through Lockin's section of /etc/hosts.
//!
//! Only root may change /etc/hosts, so the daemon asks lockin-helper to do it,
//! through polkit: `pkexec /usr/local/lib/lockin/lockin-helper block youtube.com`.
//! tools/install-helper.sh installs the helper and a polkit rule that lets your
//! user run it without a password.
//!
//! Every couple of seconds the daemon says which websites should be blocked
//! (none on a break or without a session). The helper only runs when that
//! differs from what /etc/hosts blocks now, so normally it runs at the start and
//! end of a session and around breaks.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lockin_helper::hosts;

/// Where tools/install-helper.sh puts the helper (polkit names this exact path).
pub const HELPER_PATH: &str = "/usr/local/lib/lockin/lockin-helper";
/// After a failure, the same change is tried again after this long.
const RETRY_SECONDS: u64 = 60;
/// The helper is stopped if it takes longer than this (polkit could wait for a password).
const HELPER_TIMEOUT_SECONDS: u64 = 30;

#[derive(Default)]
pub struct SiteBlocker {
    /// The last change that failed, and when: not tried again right away.
    failed: Option<(Vec<String>, Instant)>,
    /// Why websites can't be blocked right now, for the app ("run install-helper.sh").
    pub problem: Option<String>,
}

/// The hosts file (LOCKIN_HOSTS_FILE points elsewhere for development).
fn hosts_file() -> PathBuf {
    std::env::var_os("LOCKIN_HOSTS_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/hosts"))
}

/// How to start the helper: through pkexec, or LOCKIN_HELPER_COMMAND
/// ("sudo -n /path/to/lockin-helper") for development.
fn helper_command() -> Result<Vec<String>, String> {
    if let Ok(command) = std::env::var("LOCKIN_HELPER_COMMAND") {
        return Ok(command.split_whitespace().map(String::from).collect());
    }
    if !Path::new(HELPER_PATH).exists() {
        return Err(
            "website blocking isn't set up yet: run tools/install-helper.sh once".to_string(),
        );
    }
    Ok(vec!["pkexec".to_string(), HELPER_PATH.to_string()])
}

impl SiteBlocker {
    /// Make /etc/hosts block exactly `wanted`. Returns a line for the log when
    /// something changed.
    pub async fn apply(&mut self, wanted: &[String]) -> Option<String> {
        let current = std::fs::read_to_string(hosts_file())
            .map(|text| hosts::managed_domains(&text))
            .unwrap_or_default();
        if current == wanted {
            self.problem = None;
            return None;
        }
        if let Some((tried, at)) = &self.failed
            && tried == wanted
            && at.elapsed() < Duration::from_secs(RETRY_SECONDS)
        {
            return None;
        }

        match run_helper(wanted).await {
            Ok(()) => {
                self.failed = None;
                self.problem = None;
                Some(if wanted.is_empty() {
                    "websites unblocked".to_string()
                } else {
                    format!("blocking websites: {}", wanted.join(", "))
                })
            }
            Err(error) => {
                self.failed = Some((wanted.to_vec(), Instant::now()));
                let first_time = self.problem.as_deref() != Some(error.as_str());
                self.problem = Some(error.clone());
                // Said once, not every minute.
                first_time.then(|| format!("websites: {error}"))
            }
        }
    }
}

async fn run_helper(wanted: &[String]) -> Result<(), String> {
    let mut command = helper_command()?;
    if wanted.is_empty() {
        command.push("unblock".to_string());
    } else {
        command.push("block".to_string());
        command.extend(wanted.iter().cloned());
    }
    let child = tokio::process::Command::new(&command[0])
        .args(&command[1..])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = match tokio::time::timeout(Duration::from_secs(HELPER_TIMEOUT_SECONDS), child)
        .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(format!("can't start {}: {error}", command[0])),
        Err(_) => {
            return Err("lockin-helper took too long (is polkit asking for a password?)".into());
        }
    };
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    // pkexec exits with 126/127 when polkit refuses or the dialog is closed.
    Err(match output.status.code() {
        Some(126) | Some(127) => format!(
            "polkit didn't allow lockin-helper ({stderr}); run tools/install-helper.sh again"
        ),
        _ if stderr.is_empty() => "lockin-helper failed".to_string(),
        _ => stderr,
    })
}
