//! Fetching calendar sources and saving their events.
//! Used by the app (Settings → Schedule), by `lockin-daemon sync`, and by the
//! daemon's hourly fetch in phase 4.
//!
//! Two kinds of feed: TimeEdit (classes) and Ufora (deadlines). You paste
//! either link in the same place; the feed's text tells which one it is
//! (`ufora::is_ufora_feed`).

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};

use crate::db::{self, Pool, SyncResult};
use crate::model::{FeedEvent, FeedInfo, FeedSync};
use crate::{deadline, timeedit, ufora};

/// The text of a feed. A `file://` source is read from disk, anything else is downloaded.
pub async fn fetch_text(url: &str) -> Result<String> {
    match url.strip_prefix("file://") {
        Some(path) => Ok(std::fs::read_to_string(path)?),
        None => timeedit::download(url).await,
    }
}

/// Fetch the events of a TimeEdit feed without saving them.
pub async fn fetch_events(url: &str) -> Result<Vec<FeedEvent>> {
    timeedit::parse(&fetch_text(url).await?)
}

/// "ufora" or "timeedit", from the feed's text.
fn kind_of(text: &str) -> &'static str {
    if ufora::is_ufora_feed(text) {
        "ufora"
    } else {
        "timeedit"
    }
}

/// Save a feed's text: classes for TimeEdit, deadlines for Ufora.
async fn save_text(pool: &Pool, source_id: &str, text: &str) -> Result<SyncResult> {
    if ufora::is_ufora_feed(text) {
        let deadlines = ufora::parse(text)?;
        deadline::save_ufora(pool, source_id, &deadlines, Utc::now()).await
    } else {
        let events = timeedit::parse(text)?;
        db::save_feed(pool, source_id, &events).await
    }
}

/// Fetch one saved source and save what's in it.
pub async fn import_source(pool: &Pool, source_id: &str, url: &str) -> Result<SyncResult> {
    let text = fetch_text(url).await?;
    save_text(pool, source_id, &text).await
}

/// Clean up a pasted link: trims spaces, turns `webcal://` into `https://`
/// (TimeEdit and Ufora offer both), and refuses anything that isn't a web link.
pub fn clean_feed_url(pasted: &str) -> Result<String> {
    let url = pasted.trim();
    let url = match url.strip_prefix("webcal://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    };
    if !url.starts_with("https://") && !url.starts_with("http://") {
        bail!(
            "this is not a web link; copy the link from TimeEdit → Subscribe, or Ufora → Calendar → Subscribe"
        );
    }
    Ok(url)
}

/// Save a new feed from a pasted link (TimeEdit or Ufora) and import it right away.
/// The link is downloaded first, so a typo or a dead link is never saved.
/// A link that was already saved just syncs again.
pub async fn add_feed(pool: &Pool, pasted_url: &str) -> Result<FeedSync> {
    let url = clean_feed_url(pasted_url)?;
    add_feed_text(pool, &url, &fetch_text(&url).await?).await
}

/// Save a feed whose text is already fetched (from `add_feed`, or a file for the CLI).
pub async fn add_feed_text(pool: &Pool, url: &str, text: &str) -> Result<FeedSync> {
    let already_saved = db::sources(pool).await?.iter().any(|s| s.url == url);
    let kind = kind_of(text);
    let source_id = db::add_source(pool, url, kind).await?;
    match save_text(pool, &source_id, text).await {
        Ok(result) => Ok(feed_sync(source_id, kind, result)),
        Err(error) => {
            if !already_saved {
                db::remove_source(pool, &source_id).await?;
            }
            Err(error)
        }
    }
}

fn feed_sync(source_id: String, kind: &str, result: SyncResult) -> FeedSync {
    FeedSync {
        source_id,
        kind: kind.to_string(),
        added: result.added,
        updated: result.updated,
        moved: result.moved,
        cancelled: result.cancelled,
        error: None,
    }
}

/// Fetch every saved feed again. One failing feed doesn't stop the others:
/// its error is in its `FeedSync`.
pub async fn sync_all(pool: &Pool) -> Result<Vec<FeedSync>> {
    let mut results = Vec::new();
    for source in db::sources(pool).await? {
        let outcome = import_source(pool, &source.id, &source.url).await;
        results.push(match outcome {
            Ok(result) => feed_sync(source.id, &source.kind, result),
            Err(error) => FeedSync {
                error: Some(format!("{error:#}")),
                ..feed_sync(source.id, &source.kind, SyncResult::default())
            },
        });
    }
    Ok(results)
}

/// Feeds are fetched again once their last sync is older than this.
pub const SYNC_EVERY_MINUTES: i64 = 60;

/// True when a saved feed was never fetched, or not in the last
/// `SYNC_EVERY_MINUTES` before `now`. The daemon then calls `sync_all`.
pub async fn sync_due(pool: &Pool, now: DateTime<Utc>) -> Result<bool> {
    let limit = now - chrono::Duration::minutes(SYNC_EVERY_MINUTES);
    let limit = limit.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let stale: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM calendar_source
         WHERE deleted = 0 AND (last_fetched_at IS NULL OR last_fetched_at < ?)",
    )
    .bind(limit)
    .fetch_one(pool)
    .await?;
    Ok(stale > 0)
}

/// When a feed was last fetched (the most recent one), RFC 3339 in UTC; None
/// without feeds. For "synced 2 min ago" in the sidebar.
pub async fn last_sync(pool: &Pool) -> Result<Option<String>> {
    let last =
        sqlx::query_scalar("SELECT MAX(last_fetched_at) FROM calendar_source WHERE deleted = 0")
            .fetch_one(pool)
            .await?;
    Ok(last)
}

/// The saved feeds as the app shows them, without their URLs.
pub async fn feeds(pool: &Pool) -> Result<Vec<FeedInfo>> {
    let classes = db::classes_per_source(pool).await?;
    let deadlines = deadline::per_source(pool).await?;
    let count = |counts: &[(String, i64)], id: &str| {
        counts
            .iter()
            .find(|(source, _)| source == id)
            .map_or(0, |(_, count)| *count)
    };
    let feeds = db::sources(pool)
        .await?
        .into_iter()
        .map(|source| FeedInfo {
            origin: origin(&source.url),
            classes: count(&classes, &source.id),
            deadlines: count(&deadlines, &source.id),
            id: source.id,
            kind: source.kind,
            last_fetched_at: source.last_fetched_at,
        })
        .collect();
    Ok(feeds)
}

/// Add the past classes from a downloaded TimeEdit file (its text), once.
/// `name` is the file name, shown in Settings to remove the import again.
/// Returns (added, events in the file).
pub async fn import_history_text(
    pool: &Pool,
    ics_text: &str,
    name: Option<&str>,
) -> Result<(u32, u32)> {
    if ufora::is_ufora_feed(ics_text) {
        bail!("this is a Ufora file; paste the Ufora link under Schedule instead");
    }
    let events = timeedit::parse(ics_text)?;
    let added = db::import_history(pool, &events, name).await?;
    Ok((added, events.len() as u32))
}

/// "cloud.timeedit.net" for a web link, the file name for a `file://` source.
/// Never the path or query, where TimeEdit and Ufora put the personal key.
fn origin(url: &str) -> String {
    if let Some(path) = url.strip_prefix("file://") {
        return path.rsplit('/').next().unwrap_or(path).to_string();
    }
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    without_scheme
        .split('/')
        .next()
        .unwrap_or(without_scheme)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webcal_becomes_https() {
        assert_eq!(
            clean_feed_url("  webcal://cloud.timeedit.net/ugent/web/x.ics ").unwrap(),
            "https://cloud.timeedit.net/ugent/web/x.ics"
        );
    }

    #[test]
    fn rejects_text_that_is_no_link() {
        assert!(clean_feed_url("my schedule").is_err());
    }

    #[test]
    fn origin_hides_the_key() {
        assert_eq!(
            origin("https://cloud.timeedit.net/ugent/web/ri6Q7XY.ics?key=secret"),
            "cloud.timeedit.net"
        );
        assert_eq!(
            origin("https://ufora.ugent.be/d2l/le/calendar/feed/user/feed.ics?token=secret"),
            "ufora.ugent.be"
        );
        assert_eq!(
            origin("file:///home/jls/Downloads/schedule.ics"),
            "schedule.ics"
        );
    }
}
