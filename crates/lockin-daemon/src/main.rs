//! For now the daemon is a small command-line tool, to test phase 1.
//! In phase 4 it becomes the systemd user service.

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, Local, TimeZone, Utc};
use lockin_core::model::FeedEvent;
use lockin_core::{db, import, paths, timeedit};

const USAGE: &str = "usage: lockin-daemon <command>

  sync                     fetch all saved feeds again
  sync <timeedit-url>      save a new feed and fetch it
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
  reset-block <id>         let the rules decide again for this block";

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
            let source_id = db::add_timeedit_source(&pool, &url).await?;
            let result = import::import_source(&pool, &source_id, &url).await?;
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
        print!("feed {}: ", &feed.source_id[..8]);
        match feed.error {
            Some(error) => println!("failed: {error}"),
            None => println!(
                "{} new, {} updated ({} moved), {} cancelled",
                feed.added, feed.updated, feed.moved, feed.cancelled
            ),
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

fn print_result(result: &db::SyncResult) {
    println!(
        "{} new, {} updated ({} moved), {} cancelled",
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
