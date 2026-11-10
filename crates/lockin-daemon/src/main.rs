//! For now the daemon is a small command-line tool, to test phase 1.
//! In phase 4 it becomes the systemd user service.
//!
//! Usage:
//!   lockin-daemon sync <timeedit-url>     fetch the feed and save it
//!   lockin-daemon sync-file <file.ics>    same, from a saved file
//!   lockin-daemon ignore <text>           ignore blocks whose title contains <text>
//!   lockin-daemon week                    show this week's blocks
//!   lockin-daemon courses                 list the courses found in the schedule

use anyhow::{Result, bail};
use chrono::{Datelike, Duration, Local, TimeZone, Utc};
use lockin_core::{db, paths, timeedit};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str());
    let argument = args.get(2);

    let pool = db::open(&paths::database_file()?).await?;

    match (command, argument) {
        (Some("sync"), Some(url)) => {
            let source_id = db::add_timeedit_source(&pool, url).await?;
            let text = timeedit::download(url).await?;
            let events = timeedit::parse(&text)?;
            let result = db::save_feed(&pool, &source_id, &events).await?;
            println!(
                "{} new, {} updated, {} cancelled",
                result.added, result.updated, result.cancelled
            );
        }
        (Some("sync-file"), Some(file)) => {
            // Use the file path as the source "url", so it gets its own source.
            let source_id = db::add_timeedit_source(&pool, &format!("file://{file}")).await?;
            let text = std::fs::read_to_string(file)?;
            let events = timeedit::parse(&text)?;
            let result = db::save_feed(&pool, &source_id, &events).await?;
            println!(
                "{} new, {} updated, {} cancelled",
                result.added, result.updated, result.cancelled
            );
        }
        (Some("ignore"), Some(text)) => {
            db::add_title_rule(&pool, text).await?;
            println!("Rule added. Run sync again to apply it.");
        }
        (Some("week"), _) => show_week(&pool).await?,
        (Some("courses"), _) => {
            for course in db::courses(&pool).await? {
                println!("{}  {}", course.code.unwrap_or_default(), course.name);
            }
        }
        _ => bail!("usage: lockin-daemon sync <url> | sync-file <file.ics> | ignore <text> | week"),
    }

    Ok(())
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
        if block.status != "normal" {
            marks.push(block.status.as_str());
        }
        let marks = if marks.is_empty() {
            String::new()
        } else {
            format!("  [{}]", marks.join(", "))
        };

        println!(
            "{} {}-{}  {}{}",
            start_local.format("%a %d %b"),
            start_local.format("%H:%M"),
            end_local.format("%H:%M"),
            block.title,
            marks
        );
    }
    Ok(())
}
