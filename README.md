<p align="center">
  <img src="assets/logo/svg/lockin-logo-on-light.svg#gh-light-mode-only" alt="Lockin" height="72">
  <img src="assets/logo/svg/lockin-logo-on-dark.svg#gh-dark-mode-only" alt="Lockin" height="72">
</p>

# Lockin

A personal desktop app that keeps track of my class schedule and helps me actually study:
it imports the schedule from TimeEdit, tracks what still needs doing per class, and runs
study sessions that block distracting apps and websites.

> **Status:** phases 1 to 3 of 6 are done: the TimeEdit import, the class tracking app
> (week view, activities, templates, semester overview) and deadlines (by hand and from
> Ufora, with the Urgent list) work. Phase 4 is under way: study sessions run, with the
> daemon as a service, and close the apps on your block profile; websites come next. Built first for CachyOS with
> KDE Plasma, with Windows and macOS support planned later.

## Features

**Working now**

- **Schedule import** from TimeEdit: paste the subscription link in Settings and the app
  syncs it, again whenever it's opened after an hour. Moved and cancelled classes are
  marked instead of silently changed or deleted, and earlier weeks can be imported from a
  downloaded file
- **Week view:** the week as a grid with every class in its course color, a status per
  class (done, partly done, behind, planned), a banner when the schedule changed,
  deadlines under the day headers, and Urgent and Deadlines panels
- **Activities** per class to check off, rename or delete (with undo), plus to-dos for the
  week that aren't tied to a class
- **Templates per course:** "after every WPO: do the exercises" adds the activity to every
  matching class, past ones included, so you can catch up when starting mid-semester
- **Semester overview:** weeks × courses with a status per cell and the share done per course
- **Urgent & deadlines:** one list of open activities and deadlines, most urgent first
  (how long something has been open, how close a deadline is), with deadlines you add
  yourself (D) and the ones from Ufora, "Coming 2 weeks" and the feeds' last sync
- **Deadlines from Ufora:** paste the Ufora calendar link next to the TimeEdit one; due
  dates are linked to their course by code, moved and cancelled ones are marked, and each
  opens its page in Ufora
- **Filter rules** to ignore blocks you don't attend (e.g. "zelfstandig werk"), with a
  manual exception per block
- **Dutch and English**, switchable without a restart; keyboard shortcuts throughout
- **Study sessions:** plan them in the week view (N), once or every week until the end of
  the semester. A background daemon (systemd user service) starts and ends them on time,
  also when the app is closed, and syncs the feeds every hour. The Session page shows the
  running session with its timer, break credit (5 min per 30 min studied, B takes a
  break), its log and an emergency stop you hold open for two minutes
- **Blocking apps:** your own block profiles (Settings → Block profiles, e.g. "Strict"
  and "Light"), with apps picked from your menu (Flatpaks too) or added by program name.
  While you study, the daemon closes them; breaks pause it

**Planned**

- **Blocking websites** during study sessions, penalty points, tray icon and
  notifications (phase 4)
- **Cloud sync** via Supabase, while staying fully usable offline (phase 5)
- **Points, streaks, statistics** and a cosmetic shop with themes and pets (phase 6)

## How it works

```
TimeEdit / Ufora feeds ──► lockin-daemon ──► SQLite ◄── Tauri app (GUI)
                              │                │
                         blocking         sync with Supabase
```

- The **daemon** runs in the background (a systemd user service), fetches the feeds and
  enforces study sessions, even when the GUI is closed.
- The **app** is the window for planning and looking back.
- Both use the shared **`lockin-core`** crate and the same local SQLite database, which is
  the source of truth.

## Tech stack

| Part | Technology |
| --- | --- |
| Desktop app | [Tauri 2](https://tauri.app), Svelte 5, TypeScript, custom CSS |
| Backend and daemon | Rust (Cargo workspace), tokio |
| Database | SQLite via [sqlx](https://github.com/launchbadge/sqlx) |
| Calendar parsing | [icalendar](https://crates.io/crates/icalendar) |
| Cloud (planned) | Supabase (Postgres) |

## Project structure

```
crates/
  lockin-core/       data model, database, migrations, TimeEdit import, activities
  lockin-daemon/     background service (`run`) and a command-line tool
  lockin-helper/     root helper for browser block policies (planned)
app/
  src/
    routes/          pages: week (/), semester, settings, and placeholders for later phases
    lib/api.ts       typed wrappers around the Tauri commands
    lib/components/  week grid, panels, activity rows, …
    lib/week/        layout and status logic for the week and semester views
    lib/session/     the running study session and the daemon's status (live.svelte.ts)
    lib/i18n/        translations (en.json, nl.json) and date formatting
    lib/styles/      design tokens (tokens.css) and shared styles
  src-tauri/         Tauri (Rust) side of the app: thin commands over lockin-core
assets/logo/         logo and icon sources (SVG, PNG)
tools/               scripts: daemon install (lockin.service), logo exports, desktop entry
```

Data is stored in `~/.local/share/lockin/lockin.db`, config in `~/.config/lockin/`.

## Building

Requirements: Rust (stable), Node.js with pnpm, and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/JLS-09/Lockin.git
cd Lockin
cargo build
```

Run the app in development mode:

```bash
cd app
pnpm install
pnpm tauri dev
```

On first start the week is empty: open **Settings → Schedule** and paste your TimeEdit
link, and your Ufora calendar link for deadlines.

### The daemon

The daemon starts and ends study sessions on time and syncs the feeds every hour, also
when the app is closed. Install it as a systemd user service (Linux):

```bash
tools/install-daemon.sh            # build, install to ~/.local/bin and start lockin.service
journalctl --user -u lockin -f     # what it's doing
lockin-daemon status               # is it running, and when did it last sync
tools/install-daemon.sh --remove   # stop and remove it again (your data stays)
```

Run the script again after pulling changes. Without installing, `cargo run -p
lockin-daemon -- run` runs it in the terminal until Ctrl+C; only one daemon runs at a time.

## Importing the schedule

In TimeEdit, choose **Subscribe**, pick the longest period and copy the link. Paste it in
**Settings → Schedule** (a `webcal://` link works too). The link holds a personal key, so
Lockin never shows it in full, not even in error messages, and it never goes in git.

Under **Filter rules** in Settings you can ignore classes you don't attend, e.g. everything
with "zelfstandig werk" in the title.

A TimeEdit subscription never includes classes older than 14 days. When you start
mid-semester, download the earlier weeks once in TimeEdit (**Download → iCal**) and import
the file with **Settings → Schedule → Import file…**. Only past classes Lockin doesn't know
yet are added, and the file isn't saved as a feed. Each import is listed under the button
and can be removed again: its classes disappear with their activities (classes the live
feed also has stay), and importing the file again brings them back.

Running a sync again updates classes by their iCal UID: a class whose time changed is marked
`moved`, and a future class that disappears from the feed is marked `cancelled` instead of
being deleted. Past classes stay, even after they drop out of the feed.

## Deadlines from Ufora

In Ufora, open the calendar, turn on **Enable calendar feeds** in its settings, choose
**Subscribe** and copy the link. Paste it in **Settings → Schedule**, in the same field as
the TimeEdit link: Lockin recognises it.

The feed holds every event of every Ufora course, so Lockin keeps only due dates
("… - Verwacht" / "… - Due") and ends of availability ("… - Beschikbaarheid eindigt",
"… - verloopt" / "… - Availability Ends"); Ufora in Dutch and in English both work,
and only for courses it knows from TimeEdit (matched on the course code). A deadline that
is already past the first time it comes in counts as done. Like classes, deadlines are
matched on their UID: a changed due date is marked moved, one that leaves the feed is
marked cancelled.

### From the command line

The daemon is also a small command-line tool on the same database:

```bash
cargo run -p lockin-daemon -- sync "<TimeEdit or Ufora URL>"  # save the feed and import it
cargo run -p lockin-daemon -- sync                        # later: fetch all saved feeds again
cargo run -p lockin-daemon -- ignore "zelfstandig werk"   # add a filter rule (applies right away)
cargo run -p lockin-daemon -- week                        # this week's classes, with short ids
cargo run -p lockin-daemon -- courses                     # the courses found in the schedule
cargo run -p lockin-daemon -- feeds                       # what each saved feed contains
cargo run -p lockin-daemon -- import-history <file>.ics   # earlier weeks from a download
cargo run -p lockin-daemon -- imports                     # the imported files; remove one with remove-import <id>
cargo run -p lockin-daemon -- add-deadline 2026-11-16 "Lab report 2" E702010A  # due 23:59
cargo run -p lockin-daemon -- urgent                      # open activities and deadlines, most urgent first
cargo run -p lockin-daemon -- plan-session "2026-11-18 13:00" 15:30 "Exercises"  # a study session
cargo run -p lockin-daemon -- plan-weekly "2026-11-18 13:00" 15:30 "Exercises"   # every week, to the end of the semester
cargo run -p lockin-daemon -- sessions                    # last week's and the coming study sessions
cargo run -p lockin-daemon -- session-status              # the running session: study time, break credit
cargo run -p lockin-daemon -- add-profile Strict          # a block profile
cargo run -p lockin-daemon -- block Strict discord        # add an app (see installed-apps) or a program name
```

Run the daemon without arguments to see all commands, including `rules`, `remove-rule`,
`remove-feed`, `keep-block` / `ignore-block` / `reset-block` for a manual exception per block,
`deadlines` / `done-deadline` / `remove-deadline`, and for study sessions `remove-session`,
`remove-following`, `break`, `resume`, `emergency-stop` and `tick` (what the running daemon
does every few seconds), and `profiles`, `installed-apps` and `find-process <name>` (what
the daemon sees of a running app, and whether a profile blocks it) for blocking.

## Development

```bash
cargo fmt && cargo clippy --all-targets && cargo test   # Rust: before committing
cd app && pnpm check                                     # Svelte and TypeScript
```

Tests use a saved sample `.ics`, so they never hit TimeEdit. GitHub Actions runs the same
checks on every push.

- **Database changes** go in a new file in `crates/lockin-core/migrations/`; they run
  automatically when the database is opened. Never edit a migration that has been pushed.
- **Text in the UI** always comes from `app/src/lib/i18n/en.json` and `nl.json`;
  `pnpm check` fails when a key is missing from `nl.json`.
- **Colors, sizes and durations** come from the CSS variables in
  `app/src/lib/styles/tokens.css`, never raw values in components.
- **Smoothness:** in a dev build, switching weeks logs frame times to the console
  (right-click → Inspect).
- **Task bar icon on KDE Wayland:** `tools/install-desktop-entry.sh` installs a desktop
  entry and icon for the dev build (`--remove` takes them out again).


## Roadmap

1. **Foundation:** workspace, database, TimeEdit import, filter rules *(done)*
2. **Tracking classes:** week view, activities, templates, semester overview, NL/EN *(done)*
3. **Deadlines:** manual deadlines, the Ufora feed and the "urgent" list *(done)*
4. **Study mode:** daemon as a service, blocking, breaks, tray and notifications *(in progress)*
5. **Cloud sync** with Supabase
6. **Motivation and insight:** points, streaks, statistics and the shop

## License

[MIT](LICENSE)