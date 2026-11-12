//! Apps: which ones are installed, and whether a running process is one of them.
//!
//! Installed apps come from their `.desktop` files (the entries in the app
//! menu), Flatpaks included. When you add one to a block profile, what to look
//! for is copied into the database (`block_app`): its program names and its
//! Flatpak id. The daemon then matches running processes against that, with
//! three signals, any one of which is enough:
//!
//! 1. the program: its name (the process's executable, its first argument, or
//!    its `comm`, the kernel's name cut to 15 characters), or the app's own
//!    install folder (`/opt/google/chrome/`): a process whose executable lives
//!    there belongs to the app. Many apps start through a script with another
//!    name (Chrome's `google-chrome-stable` runs `/opt/google/chrome/chrome`), so
//!    such scripts are read for the program they start;
//! 2. the Flatpak id, for apps running in a Flatpak sandbox;
//! 3. the app id in the process's systemd scope: KDE and GNOME start apps from
//!    the menu in a scope named after their desktop file
//!    (`app-flatpak-com.discordapp.Discord-1234.scope`, `app-steam@a1b2.service`).
//!    That catches apps whose program has another name than their launcher,
//!    like Discord (`/usr/bin/discord` starts `/opt/discord/Discord`).
//!
//! Reading `/proc` happens in the daemon (it differs per OS); everything here is
//! plain text handling, so the tests can run anywhere.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::BlockedApp;

/// The kernel keeps only this many characters of a process name in `comm`.
pub const COMM_LENGTH: usize = 15;

/// Lockin's own desktop file id: it can never be blocked.
pub const LOCKIN_APP_ID: &str = "io.github.jls_09.lockin";

/// Programs that are never closed, whatever a profile says: Lockin itself and
/// the parts of the desktop you need to keep working.
pub const PROTECTED: &[&str] = &[
    "lockin-app",
    "lockin-daemon",
    "plasmashell",
    "kwin_wayland",
    "kwin_x11",
    "kwin_wayland_wrapper",
    "Xwayland",
    "Xorg",
    "ksmserver",
    "kded6",
    "kded5",
    "systemd",
    "dbus-daemon",
    "dbus-broker",
    "pipewire",
    "wireplumber",
    "pulseaudio",
    "gnome-shell",
    "sddm",
    "polkitd",
];

/// An app from the menu, as the profile editor lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// The desktop file id, e.g. "discord" or "com.discordapp.Discord".
    pub id: String,
    /// The name in the menu, e.g. "Discord".
    pub name: String,
    /// Program names to match, e.g. ["discord", "Discord"].
    pub exec_names: Vec<String>,
    /// The Flatpak app id, for Flatpak apps.
    pub flatpak_id: Option<String>,
}

/// What the daemon reads about one running process (`/proc/<pid>/...`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// `/proc/<pid>/comm`: the name, at most 15 characters.
    pub comm: String,
    /// The file name of `/proc/<pid>/exe`.
    pub exe_name: Option<String>,
    /// The full path of `/proc/<pid>/exe`, for matching an app's install folder.
    pub exe_path: Option<String>,
    /// The file name of the first argument in `/proc/<pid>/cmdline`.
    pub argv0_name: Option<String>,
    /// The app id from its systemd scope (`unit_app_ids`).
    pub unit_app_id: Option<String>,
    /// The Flatpak id from its systemd scope.
    pub flatpak_id: Option<String>,
}

/// True when the name is a program Lockin never closes.
pub fn is_protected(name: &str) -> bool {
    PROTECTED.contains(&name)
}

/// True when `process` belongs to the blocked `app`.
pub fn matches(app: &BlockedApp, process: &ProcessInfo) -> bool {
    if let Some(flatpak) = &app.flatpak_id
        && process.flatpak_id.as_deref() == Some(flatpak.as_str())
    {
        return true;
    }
    // A program added by name has no desktop file, so no scope to match.
    if !app.app_id.starts_with("program:") && process.unit_app_id.as_deref() == Some(&app.app_id) {
        return true;
    }
    app.exec_names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .any(|name| {
            if name.starts_with('/') {
                // An install folder, e.g. "/opt/google/chrome/".
                return process
                    .exe_path
                    .as_deref()
                    .is_some_and(|path| path.starts_with(name));
            }
            process.exe_name.as_deref() == Some(name)
                || process.argv0_name.as_deref() == Some(name)
                || process.comm == short_name(name)
        })
}

/// The first 15 characters, as the kernel keeps them in `comm`.
fn short_name(name: &str) -> String {
    name.chars().take(COMM_LENGTH).collect()
}

/// The app id and Flatpak id from a process's `/proc/<pid>/cgroup` text.
///
/// KDE and GNOME start an app from the menu in its own systemd unit, named
/// after its desktop file (the systemd "app-" naming convention):
/// - `app-flatpak-com.discordapp.Discord-12345.scope` (launcher "flatpak")
/// - `app-org.kde.dolphin@a1b2c3.service`, `app-gnome-firefox-4567.scope`
///
/// A dash inside the id is written `\x2d`. Returns (app id, Flatpak id).
pub fn unit_app_ids(cgroup: &str) -> (Option<String>, Option<String>) {
    // The unit is the last path part that starts with "app-".
    let Some(unit) = cgroup
        .lines()
        .flat_map(|line| line.rsplit('/'))
        .find(|part| {
            part.starts_with("app-") && (part.ends_with(".scope") || part.ends_with(".service"))
        })
    else {
        return (None, None);
    };

    let mut name = unit.trim_start_matches("app-");
    if let Some(rest) = name.strip_suffix(".service") {
        // "<launcher>-<id>@<random>.service" or "<launcher>-<id>.service"
        name = rest.split('@').next().unwrap_or(rest);
    } else if let Some(rest) = name.strip_suffix(".scope") {
        // "<launcher>-<id>-<random>.scope": drop the random part
        name = match rest.rsplit_once('-') {
            Some((before, _random)) => before,
            None => rest,
        };
    }

    // What's left is "<id>" or "<launcher>-<id>"; dashes inside the id are escaped.
    let (launcher, id) = match name.rsplit_once('-') {
        Some((launcher, id)) => (Some(launcher), id),
        None => (None, name),
    };
    let id = id.replace("\\x2d", "-");
    let id = id.trim_end_matches(".desktop").to_string();
    if id.is_empty() {
        return (None, None);
    }
    let flatpak = (launcher == Some("flatpak")).then(|| id.clone());
    (Some(id), flatpak)
}

/// Split an `Exec=` line into words, with "double quotes" kept together.
fn exec_words(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => quoted = !quoted,
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    // "%U", "%f" and the like are filled in by the launcher; they aren't part of the program.
    words.retain(|w| !(w.starts_with('%') && w.len() == 2));
    words
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// What the file system says about a program from an `Exec=` line.
pub struct ProgramFile {
    /// The file it really runs, links followed (/usr/bin/discord → /opt/discord/Discord).
    pub real_path: PathBuf,
    /// Its text, when it's a script ("#!..."): the program it starts is in there.
    pub script: Option<String>,
}

/// Folders that hold programs of many apps: never taken as one app's install folder.
const SHARED_FOLDERS: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/usr/games",
    "/usr/lib",
    "/usr/lib64",
    "/usr/libexec",
    "/usr/share",
    "/usr/local/bin",
    "/usr/local/sbin",
    "/usr/local/lib",
    "/opt",
    "/snap/bin",
];

/// The app's own folder for a program file, written with a slash at the end
/// ("/opt/google/chrome/"), or None for a folder many apps share (/usr/bin) or
/// one too close to the root to be an app's own.
fn app_folder(program: &Path) -> Option<String> {
    let folder = program.parent()?;
    let text = folder.to_str()?;
    let depth = folder.components().count().saturating_sub(1); // without the root
    let shared = SHARED_FOLDERS.contains(&text) || text.ends_with("/.local/bin");
    (!shared && depth >= 3).then(|| format!("{text}/"))
}

/// The programs a launcher script starts: its `exec` lines, e.g.
/// `exec -a "$0" "$HERE/chrome" "$@"` → "chrome", or
/// `exec /opt/onlyoffice/desktopeditors/DesktopEditors "$@"` → that path.
fn script_targets(script: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in script.lines() {
        let Some(rest) = line.trim().strip_prefix("exec ") else {
            continue;
        };
        let words = exec_words(rest);
        let mut words = words.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                // "-a NAME" sets the name the program sees; skip both.
                "-a" => {
                    words.next();
                }
                "-c" | "-l" | "--" => {}
                // "exec < /dev/null", "exec > >(cat)", "exec 2> …": redirections, no program.
                w if w.starts_with('<') || w.starts_with('>') || w.contains('(') => break,
                w if w.starts_with(|c: char| c.is_ascii_digit()) && w.contains('>') => break,
                w => {
                    if let Some(after) = w.strip_prefix('$') {
                        // "$HERE/chrome" → "chrome"; "$@" or "$BINARY" alone → nothing.
                        if let Some((_, name)) = after.split_once('/') {
                            targets.push(file_name(name));
                        }
                    } else {
                        targets.push(w.to_string());
                    }
                    break;
                }
            }
        }
    }
    targets.retain(|t| !t.is_empty());
    targets
}

/// What to match for an `Exec=` line, one entry per line of `block_app.exec_names`:
/// program names, and install folders (ending in "/"). Also a Flatpak id if it
/// starts one. `look` tells what a program file really is (see `ProgramFile`).
fn exec_names(
    exec: &str,
    look: &dyn Fn(&str) -> Option<ProgramFile>,
) -> (Vec<String>, Option<String>) {
    let words = exec_words(exec);
    // Skip "env" and "NAME=value" in front of the program.
    let mut rest = words
        .iter()
        .skip_while(|w| file_name(w) == "env" || (w.contains('=') && !w.starts_with('-')));
    let Some(program) = rest.next() else {
        return (Vec::new(), None);
    };

    if file_name(program) == "flatpak" {
        // "flatpak run [--options] [--command=name] <app id> [args]"
        let mut names = Vec::new();
        let mut flatpak = None;
        for word in rest {
            if let Some(command) = word.strip_prefix("--command=") {
                names.push(file_name(command));
            } else if word != "run" && !word.starts_with('-') && flatpak.is_none() {
                flatpak = Some(word.clone());
            }
        }
        return (names, flatpak);
    }

    let mut names = vec![file_name(program)];
    if let Some(file) = look(program) {
        names.push(file_name(&file.real_path.to_string_lossy()));
        names.extend(app_folder(&file.real_path));
        // A launcher script: what it starts, and that program's own folder.
        for target in script_targets(file.script.as_deref().unwrap_or_default()) {
            names.push(file_name(&target));
            if target.starts_with('/') {
                names.extend(app_folder(Path::new(&target)));
            }
        }
    }
    (names, None)
}

/// Follow a program to its real file (through PATH and links), and read it
/// when it's a script.
fn look_up_program(program: &str) -> Option<ProgramFile> {
    let path = if program.contains('/') {
        PathBuf::from(program)
    } else {
        let paths = std::env::var_os("PATH")?;
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())?
    };
    let real_path = std::fs::canonicalize(path).ok()?;
    // Scripts are small; a big file is a real program, not worth reading.
    let script = std::fs::metadata(&real_path)
        .ok()
        .filter(|m| m.len() < 64 * 1024)
        .and_then(|_| std::fs::read(&real_path).ok())
        .filter(|bytes| bytes.starts_with(b"#!"))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    Some(ProgramFile { real_path, script })
}

/// Read one `.desktop` file. `id` is its desktop file id. None for files that
/// aren't apps you'd see in the menu (hidden, no program, links).
pub fn parse_desktop_entry(id: &str, text: &str) -> Option<InstalledApp> {
    parse_desktop_entry_with(id, text, &look_up_program)
}

fn parse_desktop_entry_with(
    id: &str,
    text: &str,
    look: &dyn Fn(&str) -> Option<ProgramFile>,
) -> Option<InstalledApp> {
    // Only the [Desktop Entry] group counts; actions come after it.
    let mut fields = HashMap::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            fields.entry(key.trim()).or_insert(value.trim());
        }
    }

    let yes = |key: &str| fields.get(key).is_some_and(|v| *v == "true");
    if fields.get("Type") != Some(&"Application") || yes("NoDisplay") || yes("Hidden") {
        return None;
    }
    let name = fields.get("Name")?.to_string();
    let exec = fields.get("Exec")?;

    let (mut names, flatpak_from_exec) = exec_names(exec, look);
    if let Some(try_exec) = fields.get("TryExec") {
        names.push(file_name(try_exec));
    }
    names.retain(|n| !n.is_empty());
    // Each name once, in order.
    let mut unique = Vec::new();
    for n in names {
        if !unique.contains(&n) {
            unique.push(n);
        }
    }
    let flatpak_id = fields
        .get("X-Flatpak")
        .map(|v| v.to_string())
        .or(flatpak_from_exec);

    Some(InstalledApp {
        id: id.to_string(),
        name,
        exec_names: unique,
        flatpak_id,
    })
}

/// Where `.desktop` files live: your own first, then the system's, then Flatpak's.
fn application_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());

    let mut dirs = Vec::new();
    if let Some(data_home) = &data_home {
        dirs.push(data_home.join("applications"));
        dirs.push(data_home.join("flatpak/exports/share/applications"));
    }
    for dir in data_dirs.split(':').filter(|d| !d.is_empty()) {
        dirs.push(Path::new(dir).join("applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs
}

/// Every app in the menu, sorted by name. The first file with an id wins, so
/// your own copy of a desktop file hides the system's.
pub fn installed_apps() -> Vec<InstalledApp> {
    let mut found: HashMap<String, Option<InstalledApp>> = HashMap::new();
    for dir in application_dirs() {
        collect(&dir, &dir, &mut found);
    }
    let mut apps: Vec<InstalledApp> = found
        .into_values()
        .flatten()
        .filter(|app| app.id != LOCKIN_APP_ID)
        .filter(|app| !app.exec_names.iter().any(|n| is_protected(n)))
        .collect();
    apps.sort_by_key(|app| (app.name.to_lowercase(), app.id.clone()));
    apps
}

/// Read the .desktop files under `dir` (subfolders too: their id gets the
/// folder name, "kde4/okular.desktop" → "kde4-okular").
fn collect(root: &Path, dir: &Path, found: &mut HashMap<String, Option<InstalledApp>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, found);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let id = relative
            .with_extension("")
            .to_string_lossy()
            .replace('/', "-");
        if found.contains_key(&id) {
            continue;
        }
        // An unreadable or hidden file still takes its id, so it hides later copies.
        let app = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse_desktop_entry(&id, &text));
        found.insert(id, app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_links(_: &str) -> Option<ProgramFile> {
        None
    }

    fn parse(id: &str, text: &str) -> Option<InstalledApp> {
        parse_desktop_entry_with(id, text, &no_links)
    }

    #[test]
    fn reads_a_desktop_file() {
        let text = "[Desktop Entry]\nType=Application\nName=Discord\nName[nl]=Discord NL\n\
                    Exec=/usr/bin/discord --start-minimized %U\nTryExec=discord\n\
                    [Desktop Action new]\nName=New window\nExec=other";
        let app = parse("discord", text).unwrap();
        assert_eq!(app.name, "Discord");
        assert_eq!(app.exec_names, vec!["discord"]);
        assert_eq!(app.flatpak_id, None);
    }

    #[test]
    fn follows_the_link_to_the_real_program() {
        let look = |p: &str| {
            (p == "/usr/bin/discord").then(|| ProgramFile {
                real_path: PathBuf::from("/opt/discord/Discord"),
                script: None,
            })
        };
        let text = "[Desktop Entry]\nType=Application\nName=Discord\nExec=/usr/bin/discord";
        let app = parse_desktop_entry_with("discord", text, &look).unwrap();
        // "/opt/discord" is only two folders deep: too close to the root to be the app's own.
        assert_eq!(app.exec_names, vec!["discord", "Discord"]);
    }

    #[test]
    fn reads_chromes_launcher_script() {
        // /usr/bin/google-chrome-stable links to this script, which starts ./chrome.
        let script = "#!/bin/bash\nHERE=\"`dirname \"$0\"`\"\nexec < /dev/null\n\
                      exec > >(exec cat)\nexec 2> >(exec cat >&2)\n\
                      exec -a \"$0\" \"$HERE/chrome\" \"$@\"\n";
        let look = |p: &str| {
            (p == "/usr/bin/google-chrome-stable").then(|| ProgramFile {
                real_path: PathBuf::from("/opt/google/chrome/google-chrome"),
                script: Some(script.to_string()),
            })
        };
        let text = "[Desktop Entry]\nType=Application\nName=Google Chrome\n\
                    Exec=/usr/bin/google-chrome-stable %U";
        let app = parse_desktop_entry_with("google-chrome", text, &look).unwrap();
        assert_eq!(
            app.exec_names,
            vec![
                "google-chrome-stable",
                "google-chrome",
                "/opt/google/chrome/",
                "chrome"
            ]
        );

        // Any process running from Chrome's folder is Chrome (its helpers too).
        let chrome = blocked("google-chrome", &app.exec_names.join("\n"), None);
        let helper = ProcessInfo {
            comm: "chrome_crashpad".into(),
            exe_path: Some("/opt/google/chrome/chrome_crashpad_handler".into()),
            ..Default::default()
        };
        assert!(matches(&chrome, &helper));
        let other = ProcessInfo {
            comm: "chromium".into(),
            exe_path: Some("/usr/lib/chromium/chromium".into()),
            ..Default::default()
        };
        assert!(!matches(&chrome, &other));
    }

    #[test]
    fn reads_a_script_that_starts_a_program_elsewhere() {
        let script = "#!/bin/sh\nexport LD_LIBRARY_PATH=/opt/onlyoffice/desktopeditors\n\
                      exec /opt/onlyoffice/desktopeditors/DesktopEditors \"$@\"\n";
        let look = |p: &str| {
            (p == "/usr/bin/onlyoffice-desktopeditors").then(|| ProgramFile {
                real_path: PathBuf::from("/usr/bin/onlyoffice-desktopeditors"),
                script: Some(script.to_string()),
            })
        };
        let text = "[Desktop Entry]\nType=Application\nName=ONLYOFFICE\n\
                    Exec=/usr/bin/onlyoffice-desktopeditors %U";
        let app = parse_desktop_entry_with("onlyoffice-desktopeditors", text, &look).unwrap();
        // /usr/bin is shared by every app: not taken as this one's folder.
        assert_eq!(
            app.exec_names,
            vec![
                "onlyoffice-desktopeditors",
                "DesktopEditors",
                "/opt/onlyoffice/desktopeditors/"
            ]
        );
    }

    #[test]
    fn reads_a_flatpak() {
        let text = "[Desktop Entry]\nType=Application\nName=Discord\n\
                    Exec=/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=com.discordapp.Discord com.discordapp.Discord @@u %U @@\n\
                    X-Flatpak=com.discordapp.Discord";
        let app = parse("com.discordapp.Discord", text).unwrap();
        assert_eq!(app.flatpak_id.as_deref(), Some("com.discordapp.Discord"));
        assert!(!app.exec_names.contains(&"flatpak".to_string()));
    }

    #[test]
    fn skips_env_and_quotes() {
        let text = "[Desktop Entry]\nType=Application\nName=Game\n\
                    Exec=env WINEPREFIX=\"/home/me/.wine\" \"/opt/My Game/game\" %f";
        assert_eq!(parse("game", text).unwrap().exec_names, vec!["game"]);
    }

    #[test]
    fn hidden_entries_are_no_apps() {
        let hidden = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true";
        assert!(parse("x", hidden).is_none());
        let link = "[Desktop Entry]\nType=Link\nName=X\nURL=https://example.com";
        assert!(parse("x", link).is_none());
    }

    #[test]
    fn reads_the_app_from_the_systemd_scope() {
        let flatpak = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-flatpak-com.discordapp.Discord-12345.scope";
        assert_eq!(
            unit_app_ids(flatpak),
            (
                Some("com.discordapp.Discord".into()),
                Some("com.discordapp.Discord".into())
            )
        );
        let kde =
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-steam@a1b2c3.service";
        assert_eq!(unit_app_ids(kde), (Some("steam".into()), None));
        let gnome =
            "0::/user.slice/user@1000.service/app.slice/app-gnome-google\\x2dchrome-4567.scope";
        assert_eq!(unit_app_ids(gnome), (Some("google-chrome".into()), None));
        let terminal = "0::/user.slice/user-1000.slice/session-2.scope";
        assert_eq!(unit_app_ids(terminal), (None, None));
    }

    fn blocked(app_id: &str, names: &str, flatpak: Option<&str>) -> BlockedApp {
        BlockedApp {
            id: "x".into(),
            profile_id: "p".into(),
            app_id: app_id.into(),
            name: "App".into(),
            exec_names: names.into(),
            flatpak_id: flatpak.map(String::from),
        }
    }

    #[test]
    fn matches_by_program_scope_or_flatpak() {
        let discord = blocked("discord", "discord\nDiscord", None);
        let by_exe = ProcessInfo {
            exe_name: Some("Discord".into()),
            comm: "Discord".into(),
            ..Default::default()
        };
        assert!(matches(&discord, &by_exe));

        let by_scope = ProcessInfo {
            comm: "electron".into(),
            unit_app_id: Some("discord".into()),
            ..Default::default()
        };
        assert!(matches(&discord, &by_scope));

        let flatpak = blocked("com.spotify.Client", "", Some("com.spotify.Client"));
        let sandboxed = ProcessInfo {
            comm: "spotify".into(),
            flatpak_id: Some("com.spotify.Client".into()),
            ..Default::default()
        };
        assert!(matches(&flatpak, &sandboxed));

        let other = ProcessInfo {
            comm: "firefox".into(),
            exe_name: Some("firefox".into()),
            ..Default::default()
        };
        assert!(!matches(&discord, &other));
    }

    #[test]
    fn long_names_match_the_short_comm() {
        let app = blocked("program:steamwebhelper-extra", "steamwebhelper-extra", None);
        let process = ProcessInfo {
            comm: "steamwebhelper-".into(),
            ..Default::default()
        };
        assert!(matches(&app, &process));
    }
}
