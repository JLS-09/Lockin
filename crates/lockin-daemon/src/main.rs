//! The daemon: `lockin-daemon run` is the background service (see service.rs);
//! every other command is a small command-line tool on the same database.

mod blocker;
mod service;

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use lockin_core::model::{FeedEvent, FeedSync, SessionDetails, StudySession};
use lockin_core::{block, db, deadline, import, paths, session, timeedit, urgent};

const USAGE: &str = "usage: lockin-daemon <command>

  run                      the background service: starts and ends study sessions
                           on time and syncs the feeds every hour (lockin.service
                           runs this; tools/install-daemon.sh installs it)
  status                   ask the running daemon whether it's up and when it last synced

  sync                     fetch all saved feeds again
  sync <url>               save a new feed (TimeEdit or Ufora) and fetch it
  sync-file <file.ics>     same, from a saved file
  import-history <file>    add past blocks once, from a TimeEdit download
  imports                  list the imported files
  remove-import <id>       remove an imported file's classes
  feeds                    list the saved feeds and what each one contains
  remove-feed <id>         remove a feed, then fetch the other feeds again
  week                     show this week's blocks
  courses                  list the courses found in the schedule
  ignore <text>            ignore blocks whose title contains <text>
  rules                    list the filter rules
  remove-rule <id>         remove a rule (the first characters of its id are enough)
  ignore-block <id>        always ignore this block, whatever the rules say
  keep-block <id>          never ignore this block, whatever the rules say
  reset-block <id>         let the rules decide again for this block
  add-deadline <when> <title> [course code]
                           add a deadline; <when> is 2026-11-16 (due 23:59)
                           or \"2026-11-16 14:00\"
  deadlines                list all deadlines
  done-deadline <id>       check a deadline off
  remove-deadline <id>     remove a deadline
  urgent                   the Urgent list: open activities and deadlines

  plan-session <when> <end> [title] [course code] [profile]
                           plan a study session; <when> is \"2026-11-18 13:00\",
                           <end> 15:30 (same day); use - for no title or course;
                           [profile] is the name of a block profile
  plan-weekly <when> <end> [title] [course code] [profile]
                           the same, every week until the end of the semester
  sessions                 list last week's and the coming study sessions
  session-status           the running session (study time, break credit) or the next one
  remove-session <id>      remove a session that hasn't started yet
  remove-following <id>    remove it and the later weeks of its repeat
  tick                     do once what the daemon will do every few seconds:
                           start, end or mark missed the sessions that are due
  break                    take a break in the running session (needs credit)
  resume                   end the break
  emergency-stop           end the running session now

  profiles                 the block profiles and their apps
  add-profile <name>       make a block profile, e.g. Strict
  block <profile> <app>    add an app to a profile: a desktop file id from
                           installed-apps (e.g. discord), or else a program name
  installed-apps           the apps in your menu, with their desktop file id";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str());
    let argument = args.get(2).map(|s| s.as_str());

    let pool = db::open(&paths::database_file()?).await?;

    match (command, argument) {
        (Some("run"), _) => service::run(pool).await?,
        (Some("status"), _) => {
            let response = lockin_core::ipc::send(&lockin_core::ipc::Request::Status).await?;
            if let Some(status) = response.status {
                let since = chrono::DateTime::parse_from_rfc3339(&status.started_at)?;
                print!(
                    "lockin-daemon {} running since {}",
                    status.version,
                    since.with_timezone(&Local).format("%a %d %b %H:%M")
                );
                match status.last_sync_at {
                    Some(last) => {
                        let last = chrono::DateTime::parse_from_rfc3339(&last)?;
                        println!(", last sync {}", last.with_timezone(&Local).format("%H:%M"));
                    }
                    None => println!(", no feeds yet"),
                }
            }
        }
        (Some("sync"), None) => sync_all(&pool).await?,
        (Some("sync"), Some(url)) => {
            let result = import::add_feed(&pool, url).await?;
            print_result(&result);
        }
        (Some("sync-file"), Some(file)) => {
            // Save the full path, so `sync` finds the file again from any folder.
            let path = std::fs::canonicalize(file)?;
            let url = format!("file://{}", path.display());
            let text = std::fs::read_to_string(&path)?;
            let result = import::add_feed_text(&pool, &url, &text).await?;
            print_result(&result);
        }
        (Some("import-history"), Some(file)) => {
            let text = std::fs::read_to_string(file)?;
            let events = timeedit::parse(&text)?;
            let name = std::path::Path::new(file)
                .file_name()
                .map(|n| n.to_string_lossy().to_string());
            let added = db::import_history(&pool, &events, name.as_deref()).await?;
            println!(
                "{added} past blocks added, {} skipped (already known or not in the past).",
                events.len() as u32 - added
            );
        }
        (Some("feeds"), _) => {
            for source in db::sources(&pool).await? {
                // Never print the full URL: it contains a personal key.
                print!("{}  {}  ", &source.id[..8], source.kind);
                if source.kind == "ufora" {
                    let count = import::feeds(&pool)
                        .await?
                        .into_iter()
                        .find(|feed| feed.id == source.id)
                        .map_or(0, |feed| feed.deadlines);
                    println!("{count} deadlines");
                    continue;
                }
                match import::fetch_events(&source.url).await {
                    Ok(events) => print_feed_contents(&events),
                    Err(error) => println!("failed: {error:#}"),
                }
            }
        }
        (Some("remove-feed"), Some(id)) => {
            let source_id = db::find_source_id(&pool, id).await?;
            let hidden = db::remove_source(&pool, &source_id).await?;
            println!("Feed removed, {hidden} blocks hidden. Fetching the other feeds again:");
            sync_all(&pool).await?;
        }
        (Some("week"), _) => show_week(&pool).await?,
        (Some("courses"), _) => {
            for course in db::courses(&pool).await? {
                println!("{}  {}", course.code.unwrap_or_default(), course.name);
            }
        }
        (Some("ignore"), Some(text)) => {
            let changed = db::add_title_rule(&pool, text).await?;
            println!("Rule added, {changed} blocks changed.");
        }
        (Some("rules"), _) => {
            for rule in db::filter_rules(&pool).await? {
                println!(
                    "{}  {} contains \"{}\"",
                    &rule.id[..8],
                    rule.field,
                    rule.pattern
                );
            }
        }
        (Some("remove-rule"), Some(id)) => {
            let rule_id = db::find_rule_id(&pool, id).await?;
            let changed = db::remove_rule(&pool, &rule_id).await?;
            println!("Rule removed, {changed} blocks changed.");
        }
        (Some("ignore-block"), Some(id)) => {
            let block_id = db::find_block_id(&pool, id).await?;
            db::set_ignored_override(&pool, &block_id, Some(true)).await?;
        }
        (Some("keep-block"), Some(id)) => {
            let block_id = db::find_block_id(&pool, id).await?;
            db::set_ignored_override(&pool, &block_id, Some(false)).await?;
        }
        (Some("reset-block"), Some(id)) => {
            let block_id = db::find_block_id(&pool, id).await?;
            db::set_ignored_override(&pool, &block_id, None).await?;
        }
        (Some("add-deadline"), Some(when)) => {
            let (Some(title), code) = (args.get(3), args.get(4)) else {
                bail!(USAGE);
            };
            let due = parse_when(when)?;
            let course_id = match code {
                Some(code) => Some(course_id_for_code(&pool, code).await?),
                None => None,
            };
            deadline::add(&pool, course_id.as_deref(), title, due).await?;
            println!(
                "Deadline added, due {}.",
                due.with_timezone(&Local).format("%a %d %b %Y %H:%M")
            );
        }
        (Some("deadlines"), _) => show_deadlines(&pool).await?,
        (Some("done-deadline"), Some(id)) => {
            let deadline_id = deadline::find_id(&pool, id).await?;
            deadline::set_done(&pool, &deadline_id, true).await?;
        }
        (Some("remove-deadline"), Some(id)) => {
            let deadline_id = deadline::find_id(&pool, id).await?;
            deadline::remove(&pool, &deadline_id).await?;
        }
        (Some("urgent"), _) => show_urgent(&pool).await?,
        (Some("imports"), _) => {
            let imports = db::history_imports(&pool).await?;
            if imports.is_empty() {
                println!("No imported files.");
            }
            for item in imports {
                println!(
                    "{}  {}  {} classes",
                    &item.id[..8],
                    item.name.as_deref().unwrap_or("(earlier import)"),
                    item.classes
                );
            }
        }
        (Some("remove-import"), Some(id)) => {
            let import_id = db::find_history_import_id(&pool, id).await?;
            let hidden = db::remove_history_import(&pool, &import_id).await?;
            println!("Removed, with {hidden} classes.");
        }
        (Some(command @ ("plan-session" | "plan-weekly")), Some(when)) => {
            let Some(end) = args.get(3) else {
                bail!(USAGE);
            };
            let title = args.get(4).filter(|t| *t != "-").cloned();
            let course_id = match args.get(5).filter(|c| *c != "-") {
                Some(code) => Some(course_id_for_code(&pool, code).await?),
                None => None,
            };
            let profile_id = match args.get(6) {
                Some(name) => Some(profile_id_for_name(&pool, name).await?),
                None => None,
            };
            let details = SessionDetails {
                course_id,
                title,
                profile_id,
            };
            plan_session(&pool, command == "plan-weekly", when, end, &details).await?;
        }
        (Some("profiles"), _) => {
            let profiles = block::profiles(&pool).await?;
            if profiles.is_empty() {
                println!("No block profiles. Make one with: lockin-daemon add-profile Strict");
            }
            for profile in profiles {
                let names: Vec<&str> = profile.apps.iter().map(|a| a.name.as_str()).collect();
                println!("{}: {}", profile.name, names.join(", "));
            }
        }
        (Some("add-profile"), Some(name)) => {
            block::add_profile(&pool, name).await?;
            println!("Profile {name} made.");
        }
        (Some("block"), Some(profile)) => {
            let Some(app) = args.get(3) else {
                bail!(USAGE);
            };
            let profile_id = profile_id_for_name(&pool, profile).await?;
            let installed = lockin_core::apps::installed_apps();
            match installed.iter().find(|a| a.id == *app) {
                Some(found) => {
                    block::add_app(&pool, &profile_id, found).await?;
                    println!("{} added to {profile}.", found.name);
                }
                None => {
                    block::add_program(&pool, &profile_id, app).await?;
                    println!("Program {app} added to {profile}.");
                }
            }
        }
        (Some("installed-apps"), _) => {
            for app in lockin_core::apps::installed_apps() {
                let flatpak = app
                    .flatpak_id
                    .map(|f| format!(" (Flatpak {f})"))
                    .unwrap_or_default();
                println!(
                    "{:<40} {}  [{}]{flatpak}",
                    app.id,
                    app.name,
                    app.exec_names.join(", ")
                );
            }
        }
        (Some("sessions"), _) => show_sessions(&pool).await?,
        (Some("session-status"), _) => show_session_status(&pool).await?,
        (Some("remove-session"), Some(id)) => {
            let session_id = session::find_id(&pool, id).await?;
            session::remove(&pool, &session_id, Utc::now()).await?;
            println!("Study session removed.");
        }
        (Some("remove-following"), Some(id)) => {
            let session_id = session::find_id(&pool, id).await?;
            let removed = session::remove_following(&pool, &session_id, Utc::now()).await?;
            println!("{removed} study sessions removed.");
        }
        (Some("tick"), _) => {
            let result = session::tick(&pool, Utc::now()).await?;
            println!(
                "{} started, {} ended, {} breaks over, {} missed",
                result.started.len(),
                result.ended.len(),
                result.resumed.len(),
                result.missed.len()
            );
        }
        (Some("break"), _) => {
            let running = running_session(&pool).await?;
            session::take_break(&pool, &running.id, Utc::now()).await?;
            show_session_status(&pool).await?;
        }
        (Some("resume"), _) => {
            let running = running_session(&pool).await?;
            session::resume(&pool, &running.id, Utc::now(), None).await?;
            show_session_status(&pool).await?;
        }
        (Some("emergency-stop"), _) => {
            let running = running_session(&pool).await?;
            session::emergency_stop(&pool, &running.id, Utc::now()).await?;
            println!("Study session stopped.");
        }
        _ => bail!(USAGE),
    }

    Ok(())
}

/// Fetch every saved feed again. One failing feed doesn't stop the others.
async fn sync_all(pool: &db::Pool) -> Result<()> {
    let results = import::sync_all(pool).await?;
    if results.is_empty() {
        println!("No feeds saved yet. Add one with: lockin-daemon sync <url>");
    }
    for feed in results {
        // Never print the full URL: it contains a personal key.
        print!("feed {} ({}): ", &feed.source_id[..8], feed.kind);
        match &feed.error {
            Some(error) => println!("failed: {error}"),
            None => print_result(&feed),
        }
    }
    Ok(())
}

/// "219 events, Mon 09 Nov 2026 to Thu 20 May 2027"
fn print_feed_contents(events: &[FeedEvent]) {
    let first = events.iter().map(|e| e.start).min();
    let last = events.iter().map(|e| e.start).max();
    match (first, last) {
        (Some(first), Some(last)) => println!(
            "{} events, {} to {}",
            events.len(),
            first.with_timezone(&Local).format("%a %d %b %Y"),
            last.with_timezone(&Local).format("%a %d %b %Y")
        ),
        _ => println!("no events"),
    }
}

/// "3 new deadlines, 2 updated (1 moved), 0 cancelled"
fn print_result(result: &FeedSync) {
    let what = if result.kind == "ufora" {
        "deadlines"
    } else {
        "classes"
    };
    println!(
        "{} new {what}, {} updated ({} moved), {} cancelled",
        result.added, result.updated, result.moved, result.cancelled
    );
}

async fn show_week(pool: &db::Pool) -> Result<()> {
    // Monday 00:00 of this week, in local time.
    let today = Local::now().date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let start = Local
        .from_local_datetime(&monday.and_hms_opt(0, 0, 0).unwrap())
        .unwrap()
        .with_timezone(&Utc);
    let end = start + Duration::days(7);

    let blocks = db::blocks_between(pool, start, end).await?;
    if blocks.is_empty() {
        println!("No blocks this week.");
    }

    for block in blocks {
        let start_local =
            chrono::DateTime::parse_from_rfc3339(&block.start_at)?.with_timezone(&Local);
        let end_local = chrono::DateTime::parse_from_rfc3339(&block.end_at)?.with_timezone(&Local);

        let mut marks = Vec::new();
        if block.is_ignored() {
            marks.push("ignored");
        }
        if block.ignored_override.is_some() {
            marks.push("manual");
        }
        if block.status != "normal" {
            marks.push(block.status.as_str());
        }
        let marks = if marks.is_empty() {
            String::new()
        } else {
            format!("  [{}]", marks.join(", "))
        };

        println!(
            "{}  {} {}-{}  {}{}",
            &block.id[..8],
            start_local.format("%a %d %b"),
            start_local.format("%H:%M"),
            end_local.format("%H:%M"),
            block.title,
            marks
        );
    }
    Ok(())
}

/// "2026-11-16" (due 23:59) or "2026-11-16 14:00", in local time.
fn parse_when(text: &str) -> Result<chrono::DateTime<Utc>> {
    let (date_text, time_text) = match text.split_once([' ', 'T']) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
        bail!("not a date: {date_text} (write it as 2026-11-16)");
    };
    let time = match time_text {
        Some(time_text) => match NaiveTime::parse_from_str(time_text, "%H:%M") {
            Ok(time) => Some(time),
            Err(_) => bail!("not a time: {time_text} (write it as 14:00)"),
        },
        None => None,
    };
    deadline::due_time(date, time)
}

/// The id of the course with this code, e.g. E702010A.
async fn course_id_for_code(pool: &db::Pool, code: &str) -> Result<String> {
    let courses = db::courses(pool).await?;
    match courses
        .into_iter()
        .find(|c| c.code.as_deref() == Some(code))
    {
        Some(course) => Ok(course.id),
        None => bail!("no course with code {code}; `courses` lists them"),
    }
}

/// The name of a course by id, or an empty string.
fn course_name(courses: &[lockin_core::model::Course], id: Option<&str>) -> String {
    courses
        .iter()
        .find(|c| Some(c.id.as_str()) == id)
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

async fn show_deadlines(pool: &db::Pool) -> Result<()> {
    let courses = db::courses(pool).await?;
    let deadlines = deadline::all(pool).await?;
    if deadlines.is_empty() {
        println!(
            "No deadlines yet. Add one with: lockin-daemon add-deadline 2026-11-16 \"Lab report\""
        );
    }
    for item in deadlines {
        let due = chrono::DateTime::parse_from_rfc3339(&item.due_at)?.with_timezone(&Local);
        let mark = if item.done { "  [done]" } else { "" };
        println!(
            "{}  {}  {}  {}{}",
            &item.id[..8],
            due.format("%a %d %b %H:%M"),
            item.title,
            course_name(&courses, item.course_id.as_deref()),
            mark
        );
    }
    Ok(())
}

async fn show_urgent(pool: &db::Pool) -> Result<()> {
    let courses = db::courses(pool).await?;
    let items = urgent::list(pool, Utc::now()).await?;
    if items.is_empty() {
        println!("Nothing urgent.");
    }
    for item in items {
        // "open 3 days" for an activity, "due Fri 13 Nov 23:59" for a deadline.
        let when = match (&item.open_since, &item.due_at) {
            (Some(since), _) => {
                let since = chrono::DateTime::parse_from_rfc3339(since)?.with_timezone(&Utc);
                format!("open {} days", (Utc::now() - since).num_days())
            }
            (None, Some(due)) => {
                let due = chrono::DateTime::parse_from_rfc3339(due)?.with_timezone(&Local);
                format!("due {}", due.format("%a %d %b %H:%M"))
            }
            (None, None) => String::new(),
        };
        println!(
            "{:>6.1}  {:<11}  {}  {}  ({})",
            item.score,
            item.status,
            item.title,
            course_name(&courses, item.course_id.as_deref()),
            when
        );
    }
    Ok(())
}

/// The id of the block profile with this name (case doesn't matter).
async fn profile_id_for_name(pool: &db::Pool, name: &str) -> Result<String> {
    let profiles = block::profiles(pool).await?;
    match profiles.iter().find(|p| p.name.eq_ignore_ascii_case(name)) {
        Some(profile) => Ok(profile.id.clone()),
        None => bail!("no block profile called {name} (see: lockin-daemon profiles)"),
    }
}

/// "2026-11-18 13:00" plus "15:30": a session that day, in local time.
async fn plan_session(
    pool: &db::Pool,
    weekly: bool,
    when: &str,
    end: &str,
    details: &SessionDetails,
) -> Result<()> {
    let Some((date_text, start_text)) = when.split_once([' ', 'T']) else {
        bail!("give a date and a start time, like \"2026-11-18 13:00\"");
    };
    let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
        bail!("not a date: {date_text} (write it as 2026-11-18)");
    };
    let clock = |text: &str| match NaiveTime::parse_from_str(text, "%H:%M") {
        Ok(time) => Ok(time),
        Err(_) => Err(anyhow::anyhow!("not a time: {text} (write it as 13:00)")),
    };
    let (start_time, end_time) = (clock(start_text)?, clock(end)?);

    if weekly {
        let result =
            session::plan_weekly(pool, details, date, start_time, end_time, None, Utc::now())
                .await?;
        print!("{} weekly study sessions planned", result.planned);
        if result.overlapping > 0 {
            print!(
                ", {} weeks left out (they overlap another session)",
                result.overlapping
            );
        }
        println!(".");
    } else {
        let start = session::local_time(date, start_time)?;
        let end = session::local_time(date, end_time)?;
        session::plan(pool, details, start, end, Utc::now()).await?;
        println!(
            "Study session planned, {}.",
            session_times(&StudySession {
                id: String::new(),
                series_id: None,
                course_id: None,
                title: None,
                start_at: start.to_rfc3339(),
                end_at: end.to_rfc3339(),
                status: String::new(),
                profile_id: None,
            })?
        );
    }
    Ok(())
}

/// "Wed 18 Nov 13:00-15:30"
fn session_times(item: &StudySession) -> Result<String> {
    let start = chrono::DateTime::parse_from_rfc3339(&item.start_at)?.with_timezone(&Local);
    let end = chrono::DateTime::parse_from_rfc3339(&item.end_at)?.with_timezone(&Local);
    Ok(format!(
        "{} {}-{}",
        start.format("%a %d %b"),
        start.format("%H:%M"),
        end.format("%H:%M")
    ))
}

/// "13:05" as minutes and seconds: "1:10:24" or "4:58".
fn duration_text(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let (hours, minutes, rest) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}

/// The running session, or an error that says there is none.
async fn running_session(pool: &db::Pool) -> Result<StudySession> {
    match session::running(pool).await? {
        Some(running) => Ok(running),
        None => bail!("no study session is running"),
    }
}

async fn show_sessions(pool: &db::Pool) -> Result<()> {
    let courses = db::courses(pool).await?;
    let now = Utc::now();
    let sessions =
        session::between(pool, now - Duration::days(7), now + Duration::days(60)).await?;
    if sessions.is_empty() {
        println!(
            "No study sessions. Plan one with: lockin-daemon plan-session \"2026-11-18 13:00\" 15:30"
        );
    }
    for item in sessions {
        // "Databanken · Exercises · weekly", leaving out what's empty.
        let course = course_name(&courses, item.course_id.as_deref());
        let repeat = item.series_id.as_ref().map(|_| "weekly".to_string());
        let about: Vec<String> = [Some(course), item.title.clone(), repeat]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .collect();
        println!(
            "{}  {}  {:<8} {}",
            &item.id[..8],
            session_times(&item)?,
            item.status,
            about.join(" · ")
        );
    }
    Ok(())
}

async fn show_session_status(pool: &db::Pool) -> Result<()> {
    let now = Utc::now();
    let Some(running) = session::running(pool).await? else {
        match session::next(pool, now).await? {
            Some(next) => println!("No study session running. Next: {}.", session_times(&next)?),
            None => println!("No study session running, none planned."),
        }
        return Ok(());
    };
    let state = session::state_now(pool, &running, now).await?;
    let end = chrono::DateTime::parse_from_rfc3339(&running.end_at)?.with_timezone(&Utc);
    println!(
        "Study session {} ({})",
        session_times(&running)?,
        state.phase
    );
    println!(
        "  studied {}, {} left",
        duration_text(state.worked_seconds),
        duration_text((end - now).num_seconds())
    );
    println!(
        "  break credit {} left ({} earned, {} used)",
        duration_text(state.credit_left_seconds),
        duration_text(state.credit_earned_seconds),
        duration_text(state.credit_used_seconds)
    );
    if let Some(ends) = state.break_ends_at {
        let ends = chrono::DateTime::parse_from_rfc3339(&ends)?.with_timezone(&Local);
        println!("  on a break until {}", ends.format("%H:%M:%S"));
    }
    Ok(())
}
