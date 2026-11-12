//! The daemon on your desktop: the tray icon and notifications.
//!
//! - The tray icon is a StatusNotifierItem (KDE's tray protocol over D-Bus,
//!   through the `ksni` crate): "Studying · 34 min left" with the break credit,
//!   take or end a break, open Lockin, and the emergency stop (which opens the
//!   app's Session page: the stop has to be kept open there). Clicking the icon
//!   opens Lockin.
//! - Notifications go to the desktop's notification server over D-Bus
//!   (`notify-rust`). What to say comes from `lockin_core::notices`.
//!
//! Both need the session's D-Bus. The daemon starts at login, maybe before
//! Plasma's tray is there: the tray then waits for it, and a notification that
//! can't be shown is tried again on the next tick.

use std::time::Instant;

use chrono::{DateTime, Utc};
use ksni::TrayMethods;
use lockin_core::i18n::{self, Texts};
use lockin_core::notices::{self, TrayStatus};
use lockin_core::{apps, db};
use tokio::sync::mpsc;

/// What a click in the tray asks the daemon to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    TakeBreak,
    EndBreak,
    /// Open the app (or bring it to the front), optionally on the Session page.
    Open {
        session_page: bool,
    },
}

/// How often the tray icon's colours are checked against the Plasma theme,
/// and a tray that failed is tried again.
const RETRY_SECONDS: u64 = 60;

/// The tray icons, exported from the logo (assets/logo/png/tray): light ink
/// for a dark panel and dark ink for a light one, at 1x and 2x.
const ICONS_ON_DARK: [&[u8]; 6] = [
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-22-on-dark.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-22-on-dark@2x.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-32-on-dark.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-32-on-dark@2x.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-48-on-dark.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-48-on-dark@2x.png"),
];
const ICONS_ON_LIGHT: [&[u8]; 6] = [
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-22-on-light.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-22-on-light@2x.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-32-on-light.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-32-on-light@2x.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-48-on-light.png"),
    include_bytes!("../../../assets/logo/png/tray/lockin-tray-48-on-light@2x.png"),
];

/// The tray as ksni shows it: everything it needs to draw itself.
struct LockinTray {
    status: TrayStatus,
    /// "Open Lockin", "End the break", "Emergency stop…" in the current language.
    open_label: String,
    end_break_label: String,
    emergency_label: String,
    icons: Vec<ksni::Icon>,
    actions: mpsc::UnboundedSender<TrayAction>,
}

impl LockinTray {
    fn send(&self, action: TrayAction) {
        // The daemon's loop is gone only while it stops: nothing to do then.
        let _ = self.actions.send(action);
    }
}

impl ksni::Tray for LockinTray {
    fn id(&self) -> String {
        "lockin".into()
    }

    fn title(&self) -> String {
        "Lockin".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let mut description = self.status.headline.clone();
        if let Some(credit) = &self.status.credit {
            description = format!("{description}\n{credit}");
        }
        ksni::ToolTip {
            title: "Lockin".into(),
            description,
            ..Default::default()
        }
    }

    /// A left click opens Lockin.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::Open {
            session_page: false,
        });
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;

        // Lines you read but can't click.
        let line = |label: &str| -> ksni::MenuItem<Self> {
            StandardItem {
                label: label.to_string(),
                enabled: false,
                ..Default::default()
            }
            .into()
        };

        let mut items = vec![line(&self.status.headline)];
        if let Some(credit) = &self.status.credit {
            items.push(line(credit));
        }
        if let Some(label) = &self.status.take_break {
            items.push(
                StandardItem {
                    label: label.clone(),
                    activate: Box::new(|tray: &mut Self| tray.send(TrayAction::TakeBreak)),
                    ..Default::default()
                }
                .into(),
            );
        }
        if self.status.on_break {
            items.push(
                StandardItem {
                    label: self.end_break_label.clone(),
                    activate: Box::new(|tray: &mut Self| tray.send(TrayAction::EndBreak)),
                    ..Default::default()
                }
                .into(),
            );
        }
        items.push(ksni::MenuItem::Separator);
        items.push(
            StandardItem {
                label: self.open_label.clone(),
                activate: Box::new(|tray: &mut Self| {
                    tray.send(TrayAction::Open {
                        session_page: false,
                    })
                }),
                ..Default::default()
            }
            .into(),
        );
        if self.status.running {
            items.push(
                StandardItem {
                    label: self.emergency_label.clone(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.send(TrayAction::Open { session_page: true })
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }
        items
    }
}

/// The tray and notifications, kept by the daemon's loop.
pub struct Desktop {
    tray: Option<ksni::Handle<LockinTray>>,
    actions: mpsc::UnboundedSender<TrayAction>,
    /// When the tray was last tried (it's tried again after `RETRY_SECONDS`).
    tray_tried: Option<Instant>,
    /// The theme check, every `RETRY_SECONDS`.
    dark_checked: Option<Instant>,
    dark_panel: bool,
    /// Texts of the current language, made again when it changes.
    texts: Texts,
    /// The last notification error, so it's logged once.
    notify_problem: Option<String>,
}

impl Desktop {
    /// A new desktop side, and where the tray's clicks arrive.
    pub fn new() -> (Desktop, mpsc::UnboundedReceiver<TrayAction>) {
        let (actions, clicks) = mpsc::unbounded_channel();
        let desktop = Desktop {
            tray: None,
            actions,
            tray_tried: None,
            dark_checked: None,
            dark_panel: true,
            texts: Texts::new(i18n::system_language()),
            notify_problem: None,
        };
        (desktop, clicks)
    }

    /// Bring the tray up to date and show the notifications that are due.
    /// Returns lines for the log.
    pub async fn update(&mut self, pool: &db::Pool, now: DateTime<Utc>) -> Vec<String> {
        let mut log = Vec::new();
        let language = i18n::language(pool).await;
        if language != self.texts.language {
            self.texts = Texts::new(language);
        }
        if let Some(line) = self.update_tray(pool, now).await {
            log.push(line);
        }
        log.extend(self.notify_due(pool, now).await);
        log
    }

    async fn update_tray(&mut self, pool: &db::Pool, now: DateTime<Utc>) -> Option<String> {
        let status = match notices::tray_status(pool, &self.texts, now).await {
            Ok(status) => status,
            Err(error) => return Some(format!("tray: {error:#}")),
        };
        let open_label = self.texts.t("tray.open", &[]);
        let end_break_label = self.texts.t("tray.endBreak", &[]);
        let emergency_label = self.texts.t("tray.emergency", &[]);

        // Follow the Plasma colours: light ink on a dark panel, and the reverse.
        let theme_changed = if self
            .dark_checked
            .is_none_or(|at| at.elapsed().as_secs() >= RETRY_SECONDS)
        {
            self.dark_checked = Some(Instant::now());
            let dark = dark_panel();
            let changed = dark != self.dark_panel;
            self.dark_panel = dark;
            changed
        } else {
            false
        };

        if let Some(handle) = &self.tray {
            let icons = theme_changed.then(|| icons(self.dark_panel));
            // Copies go into the tray; the originals are kept in case it has to start again.
            let (new_status, open, end_break, emergency) = (
                status.clone(),
                open_label.clone(),
                end_break_label.clone(),
                emergency_label.clone(),
            );
            let updated = handle
                .update(|tray: &mut LockinTray| {
                    tray.status = new_status;
                    tray.open_label = open;
                    tray.end_break_label = end_break;
                    tray.emergency_label = emergency;
                    if let Some(icons) = icons {
                        tray.icons = icons;
                    }
                })
                .await;
            if updated.is_some() {
                return None;
            }
            // The tray service stopped: start it again below.
            self.tray = None;
        }

        if self
            .tray_tried
            .is_some_and(|at| at.elapsed().as_secs() < RETRY_SECONDS)
        {
            return None;
        }
        self.tray_tried = Some(Instant::now());
        let tray = LockinTray {
            status,
            open_label,
            end_break_label,
            emergency_label,
            icons: icons(self.dark_panel),
            actions: self.actions.clone(),
        };
        // `assume_sni_available`: when Plasma's tray isn't there yet (the daemon
        // starts at login), keep waiting for it instead of failing.
        match tray.assume_sni_available(true).spawn().await {
            Ok(handle) => {
                self.tray = Some(handle);
                Some("tray icon shown".to_string())
            }
            Err(error) => Some(format!("tray: {error}")),
        }
    }

    async fn notify_due(&mut self, pool: &db::Pool, now: DateTime<Utc>) -> Vec<String> {
        let due = match notices::due(pool, &self.texts, now).await {
            Ok(due) => due,
            Err(error) => return vec![format!("notifications: {error:#}")],
        };
        let mut log = Vec::new();
        for notice in due {
            match show(&notice.title, &notice.body).await {
                Ok(()) => {
                    self.notify_problem = None;
                    log.push(format!("notified: {} ({})", notice.title, notice.body));
                    if let Err(error) = notices::mark_sent(pool, &notice.key, now).await {
                        log.push(format!("notifications: {error:#}"));
                    }
                }
                Err(error) => {
                    // Not marked as sent: tried again next tick. Logged once.
                    if self.notify_problem.as_deref() != Some(error.as_str()) {
                        log.push(format!("notifications: {error}"));
                        self.notify_problem = Some(error);
                    }
                    break;
                }
            }
        }
        log
    }
}

/// Show one notification, as Lockin (grouped under its desktop file in KDE).
pub async fn show(title: &str, body: &str) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("Lockin")
        .summary(title)
        .body(body)
        .icon(apps::LOCKIN_APP_ID)
        .hint(notify_rust::Hint::DesktopEntry(
            apps::LOCKIN_APP_ID.to_string(),
        ))
        .show_async()
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// The test notification of Settings, in the chosen language.
pub async fn show_test(pool: &db::Pool) -> Result<(), String> {
    let texts = Texts::new(i18n::language(pool).await);
    show(
        &texts.t("notify.testTitle", &[]),
        &texts.t("notify.testBody", &[]),
    )
    .await
}

/// Start the Lockin app, or bring it to the front when it's already open (it
/// lets only one copy run). Through `systemd-run --user`, so the app gets the
/// desktop's current environment (the daemon may have started before it).
pub fn open_app(session_page: bool) -> Result<(), String> {
    let Some(mut command) = apps::lockin_app_command() else {
        return Err(
            "Lockin's desktop file isn't installed: run tools/install-desktop-entry.sh".to_string(),
        );
    };
    if session_page {
        command.extend(["--page".to_string(), "session".to_string()]);
    }
    let through_systemd = std::process::Command::new("systemd-run")
        .args(["--user", "--collect", "--quiet", "--"])
        .args(&command)
        .stdin(std::process::Stdio::null())
        // Without a user systemd, it complains; the fallback below takes over.
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if through_systemd {
        return Ok(());
    }
    std::process::Command::new(&command[0])
        .args(&command[1..])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("can't start {}: {error}", command[0]))
}

/// The tray icons for a dark or a light panel.
fn icons(dark_panel: bool) -> Vec<ksni::Icon> {
    let files = if dark_panel {
        ICONS_ON_DARK
    } else {
        ICONS_ON_LIGHT
    };
    files.iter().filter_map(|png| decode(png)).collect()
}

/// A PNG as the ARGB32 pixels (big-endian) the tray protocol wants.
fn decode(png_bytes: &[u8]) -> Option<ksni::Icon> {
    let mut decoder = png::Decoder::new(png_bytes);
    // Expand palettes and low bit depths to 8-bit channels.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).ok()?;
    let pixels = &pixels[..frame.buffer_size()];
    let mut data = Vec::with_capacity(frame.width as usize * frame.height as usize * 4);
    match frame.color_type {
        png::ColorType::Rgba => {
            for p in pixels.chunks_exact(4) {
                data.extend_from_slice(&[p[3], p[0], p[1], p[2]]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in pixels.chunks_exact(2) {
                data.extend_from_slice(&[p[1], p[0], p[0], p[0]]);
            }
        }
        png::ColorType::Rgb => {
            for p in pixels.chunks_exact(3) {
                data.extend_from_slice(&[255, p[0], p[1], p[2]]);
            }
        }
        _ => return None,
    }
    Some(ksni::Icon {
        width: frame.width as i32,
        height: frame.height as i32,
        data,
    })
}

/// True when the Plasma colours are dark: the window background in
/// ~/.config/kdeglobals ([Colors:Window] BackgroundNormal=r,g,b) is dark.
/// Without that file (another desktop), a dark panel is assumed.
fn dark_panel() -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return true;
    };
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(&home).join(".config"));
    let Ok(text) = std::fs::read_to_string(config.join("kdeglobals")) else {
        return true;
    };
    is_dark(&text).unwrap_or(true)
}

/// Read the window background from kdeglobals' text.
fn is_dark(kdeglobals: &str) -> Option<bool> {
    let mut in_window = false;
    for line in kdeglobals.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_window = line == "[Colors:Window]";
        } else if in_window && let Some(value) = line.strip_prefix("BackgroundNormal=") {
            let parts: Vec<f64> = value
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if parts.len() >= 3 {
                // Perceived brightness, 0 (black) to 255 (white).
                let light = 0.299 * parts[0] + 0.587 * parts[1] + 0.114 * parts[2];
                return Some(light < 128.0);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_plasma_colours() {
        let breeze_dark =
            "[General]\nColorScheme=BreezeDark\n\n[Colors:Window]\nBackgroundNormal=32,35,38\n";
        let breeze = "[Colors:View]\nBackgroundNormal=1,1,1\n[Colors:Window]\nBackgroundNormal=239,240,241\n";
        assert_eq!(is_dark(breeze_dark), Some(true));
        assert_eq!(is_dark(breeze), Some(false));
        assert_eq!(is_dark("[General]\n"), None);
    }

    #[test]
    fn the_tray_icons_decode() {
        let dark = icons(true);
        assert_eq!(dark.len(), 6);
        assert_eq!((dark[0].width, dark[0].height), (22, 22));
        assert_eq!(dark[1].width, 44);
        assert!(
            dark.iter()
                .all(|icon| icon.data.len() == (icon.width * icon.height * 4) as usize)
        );
        // Some pixels are drawn (not fully transparent).
        assert!(dark[0].data.chunks_exact(4).any(|p| p[0] > 0));
        assert_eq!(icons(false).len(), 6);
    }
}
