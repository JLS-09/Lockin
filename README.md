<p align="center">
  <img src="assets/logo/svg/lockin-logo-on-light.svg#gh-light-mode-only" alt="Lockin" height="72">
  <img src="assets/logo/svg/lockin-logo-on-dark.svg#gh-dark-mode-only" alt="Lockin" height="72">
</p>

# Lockin

A personal desktop app that keeps track of my class schedule and helps me actually study:
it imports the schedule from TimeEdit, tracks what still needs doing per class, and runs
study sessions that block distracting apps and websites.

> **Status:** early development (phase 1 of 6). The TimeEdit import and the database work;
> the GUI is still an empty Tauri window. Built first for CachyOS with KDE Plasma, with
> Windows and macOS support planned later.

## Features

Planned, roughly in this order:

- **Schedule import** from TimeEdit and deadlines from Ufora, via their iCal feeds
- **Week and semester overview** with activities to check off per class
- **Filter rules** to ignore blocks you don't attend (e.g. "zelfstandig werk")
- **Study sessions** started by a background service, with app and website blocking
- **Break credit**, points, streaks and a cosmetic shop (themes, pets)
- **Statistics:** planned versus actual study time per course and per week
- **Cloud sync** via Supabase, while staying fully usable offline

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
  lockin-core/     data model, database, migrations, TimeEdit import
  lockin-daemon/   background service (for now a small CLI)
  lockin-helper/   root helper for browser block policies (planned)
app/
  src/             Svelte frontend
  src-tauri/       Tauri (Rust) side of the app
assets/logo/       logo and icon sources (SVG, PNG)
tools/             scripts, e.g. regenerating the logo exports
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

## Trying the schedule import

Until the GUI is ready, the daemon works as a command-line tool:

```bash
cargo run -p lockin-daemon -- ignore "zelfstandig werk"   # add a filter rule
cargo run -p lockin-daemon -- sync "<TimeEdit iCal URL>"  # import the schedule
cargo run -p lockin-daemon -- week                        # show this week's classes
```

Get the URL in TimeEdit via **Subscribe**. It's personal, so don't commit it.

Running `sync` again updates classes by their iCal UID: a class whose time changed is marked
`moved`, and a future class that disappears from the feed is marked `cancelled` instead of
being deleted.

## Development

```bash
cargo test                              # tests use a saved sample .ics, no network
cargo fmt && cargo clippy --all-targets # before committing
```

Database changes go in a new file in `crates/lockin-core/migrations/`; they run automatically
when the database is opened.

## Roadmap

1. **Foundation:** workspace, database, TimeEdit import, filter rules *(in progress)*
2. **Tracking classes:** week view, activities, semester overview, NL/EN translations
3. **Deadlines:** manual deadlines and the Ufora feed
4. **Study mode:** daemon as a service, blocking, breaks, tray and notifications
5. **Cloud sync** with Supabase
6. **Motivation and insight:** points, streaks, statistics and the shop

## License

[MIT](LICENSE)