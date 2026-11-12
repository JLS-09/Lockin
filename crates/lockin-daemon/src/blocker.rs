//! Closing blocked apps while you study.
//!
//! Every few seconds `service.rs` asks the blocker to close the running
//! processes of the apps on the session's profile. How to find and close a
//! process differs per operating system, so that sits behind `AppBlocker`:
//! Linux reads `/proc` (below); Windows and macOS get their own later.
//!
//! A blocked app is asked to quit first (SIGTERM), so it can save. If it's
//! still there after `KILL_AFTER_SECONDS`, it's closed for good (SIGKILL).
//! Only your own processes are touched, never Lockin or the desktop
//! (`apps::PROTECTED`).

use lockin_core::model::BlockedApp;

/// An app that ignores the request to quit is closed after this long.
pub const KILL_AFTER_SECONDS: u64 = 5;

pub trait AppBlocker: Send {
    /// Close what runs of `apps`. Returns the apps that had something to close
    /// just now (once per app, however many processes it had). Called with no
    /// apps when nothing is blocked (no session, or a break).
    fn close(&mut self, apps: &[BlockedApp]) -> Vec<BlockedApp>;
}

/// The blocker for the system the daemon runs on.
pub fn for_this_system() -> Box<dyn AppBlocker> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::ProcBlocker::default())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(NoBlocker)
    }
}

/// Until another system has its own blocker: blocks nothing.
#[cfg(not(target_os = "linux"))]
struct NoBlocker;

#[cfg(not(target_os = "linux"))]
impl AppBlocker for NoBlocker {
    fn close(&mut self, _apps: &[BlockedApp]) -> Vec<BlockedApp> {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    use lockin_core::apps::{self, ProcessInfo};
    use lockin_core::model::BlockedApp;

    use super::{AppBlocker, KILL_AFTER_SECONDS};

    /// Finds processes in /proc and closes them with signals.
    #[derive(Default)]
    pub struct ProcBlocker {
        /// Processes asked to quit, and when: closed for good if they're still there later.
        asked: HashMap<u32, Instant>,
    }

    impl AppBlocker for ProcBlocker {
        fn close(&mut self, blocked: &[BlockedApp]) -> Vec<BlockedApp> {
            if blocked.is_empty() {
                self.asked.clear();
                return Vec::new();
            }
            let Ok(me) = std::fs::metadata("/proc/self").map(|m| m.uid()) else {
                return Vec::new();
            };
            let own_pid = std::process::id();

            let mut closed: Vec<BlockedApp> = Vec::new();
            let mut seen = Vec::new();
            for pid in process_ids() {
                if pid == own_pid {
                    continue;
                }
                // Only your own processes: never another user's, never the system's.
                let folder = format!("/proc/{pid}");
                if std::fs::metadata(&folder).map(|m| m.uid()).ok() != Some(me) {
                    continue;
                }
                let Some(process) = read_process(pid) else {
                    continue;
                };
                if is_protected(&process) {
                    continue;
                }
                let Some(app) = blocked.iter().find(|app| apps::matches(app, &process)) else {
                    continue;
                };
                seen.push(pid);
                match self.asked.get(&pid) {
                    None => {
                        // First time: ask it to quit. That's one "close" of this app.
                        signal(pid, libc::SIGTERM);
                        self.asked.insert(pid, Instant::now());
                        if !closed.iter().any(|c| c.app_id == app.app_id) {
                            closed.push(app.clone());
                        }
                    }
                    Some(asked) if asked.elapsed() > Duration::from_secs(KILL_AFTER_SECONDS) => {
                        signal(pid, libc::SIGKILL);
                    }
                    Some(_) => {}
                }
            }
            // Forget processes that are gone.
            self.asked.retain(|pid, _| seen.contains(pid));
            closed
        }
    }

    fn signal(pid: u32, which: libc::c_int) {
        // SAFETY: kill() only sends a signal; a pid that's gone gives an error we ignore.
        unsafe {
            libc::kill(pid as libc::pid_t, which);
        }
    }

    /// The process ids in /proc (its folders with a number for a name).
    fn process_ids() -> Vec<u32> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
            .collect()
    }

    fn file_name(path: &str) -> Option<String> {
        Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    /// What we need to know about one process; None when it's gone (or a kernel thread).
    fn read_process(pid: u32) -> Option<ProcessInfo> {
        let folder = format!("/proc/{pid}");
        let comm = std::fs::read_to_string(format!("{folder}/comm")).ok()?;
        let exe = std::fs::read_link(format!("{folder}/exe")).ok().map(|p| {
            p.to_string_lossy()
                .trim_end_matches(" (deleted)")
                .to_string()
        });
        // The command line can still be empty for a moment while a program starts
        // (the kernel sets its name first), so that alone isn't a reason to skip it.
        let cmdline = std::fs::read(format!("{folder}/cmdline")).unwrap_or_default();
        if exe.is_none() && cmdline.is_empty() {
            return None; // a kernel thread: no program file, no command line
        }
        let argv0 = String::from_utf8_lossy(cmdline.split(|b| *b == 0).next().unwrap_or_default())
            .to_string();
        let cgroup = std::fs::read_to_string(format!("{folder}/cgroup")).unwrap_or_default();
        let (unit_app_id, flatpak_id) = apps::unit_app_ids(&cgroup);
        Some(ProcessInfo {
            pid,
            comm: comm.trim().to_string(),
            exe_name: exe.as_deref().and_then(file_name),
            argv0_name: Some(argv0)
                .filter(|a| !a.is_empty())
                .and_then(|a| file_name(&a)),
            unit_app_id,
            flatpak_id,
        })
    }

    fn is_protected(process: &ProcessInfo) -> bool {
        apps::is_protected(&process.comm)
            || process.exe_name.as_deref().is_some_and(apps::is_protected)
            || process
                .argv0_name
                .as_deref()
                .is_some_and(apps::is_protected)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_its_own_process() {
            let me = read_process(std::process::id()).unwrap();
            assert!(!me.comm.is_empty());
            assert!(me.exe_name.is_some());
        }

        #[test]
        fn closes_a_blocked_program() {
            // A copy of `sleep` under a name nothing else uses, so the test only
            // ever sees its own process.
            let name = format!("lktest{}", std::process::id());
            let folder = std::env::temp_dir().join(lockin_core::new_id());
            std::fs::create_dir_all(&folder).unwrap();
            let program = folder.join(&name);
            std::fs::copy("/bin/sleep", &program).unwrap();
            let mut child = std::process::Command::new(&program)
                .arg("60")
                .spawn()
                .unwrap();
            // Right after spawning, the child can still carry the test's name for a
            // moment (between fork and exec); wait until it runs as the program.
            let comm = format!("/proc/{}/comm", child.id());
            for _ in 0..200 {
                if std::fs::read_to_string(&comm).is_ok_and(|c| c.trim() == name) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            let app = BlockedApp {
                id: "x".into(),
                profile_id: "p".into(),
                app_id: format!("program:{name}"),
                name: name.clone(),
                exec_names: name.clone(),
                flatpak_id: None,
            };
            let mut blocker = ProcBlocker::default();
            let closed = blocker.close(std::slice::from_ref(&app));
            assert_eq!(closed.len(), 1);
            let status = child.wait().unwrap();
            assert!(!status.success()); // ended by the signal
            // Nothing left to close: not reported again.
            assert!(blocker.close(&[app]).is_empty());
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}
