//! For now the daemon is a small command-line tool, to test phase 1.
//! In phase 4 it becomes the systemd user service.

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use lockin_core::model::{FeedEvent, FeedSync};
use lockin_core::{db, deadline, import, paths, timeedit, urgent};

const USAGE: &str = "usage: lockin-daemon <command>

  sync                     fetch all saved feeds again
  sync <url>               save a new feed (TimeEdit or Ufora) and fetch it
  sync-file <file.ics>     same, from a saved file
  import-history <file>    add past blocks once, from a TimeEdit download
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
  urgent                   the Urgent list: open activities and deadlines";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str());
    let argument = args.get(2).map(|s| s.as_str());

    let pool = db::open(&paths::database_file()?).await?;

    match (command, argument) {
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
            let added = db::import_history(&pool, &events).await?;
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
