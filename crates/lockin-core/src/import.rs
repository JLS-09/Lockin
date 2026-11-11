//! Fetching calendar sources and saving their events.
//! Used by the app (Settings → Schedule), by `lockin-daemon sync`, and by the
//! daemon's hourly fetch in phase 4.

use anyhow::{Result, bail};

use crate::db::{self, Pool, SyncResult};
use crate::model::{FeedEvent, FeedInfo, FeedSync};
use crate::timeedit;

/// Fetch the events of a feed without saving them.
/// A `file://` source is read from disk, anything else is downloaded.
pub async fn fetch_events(url: &str) -> Result<Vec<FeedEvent>> {
    let text = match url.strip_prefix("file://") {
        Some(path) => std::fs::read_to_string(path)?,
        None => timeedit::download(url).await?,
    };
    timeedit::parse(&text)
}

/// Fetch one saved source and save its events.
pub async fn import_source(pool: &Pool, source_id: &str, url: &str) -> Result<SyncResult> {
    let events = fetch_events(url).await?;
    db::save_feed(pool, source_id, &events).await
}

/// Clean up a pasted TimeEdit link: trims spaces, turns `webcal://` into
/// `https://` (TimeEdit offers both), and refuses anything that isn't a web link.
pub fn clean_feed_url(pasted: &str) -> Result<String> {
    let url = pasted.trim();
    let url = match url.strip_prefix("webcal://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    };
    if !url.starts_with("https://") && !url.starts_with("http://") {
        bail!("this is not a web link; copy the link from TimeEdit → Subscribe");
    }
    Ok(url)
}

/// Save a new TimeEdit feed from a pasted link and import it right away.
/// If the import of a new feed fails, it isn't kept, so a typo doesn't stay behind.
/// A link that was already saved just syncs again.
pub async fn add_feed(pool: &Pool, pasted_url: &str) -> Result<SyncResult> {
    let url = clean_feed_url(pasted_url)?;
    let already_saved = db::sources(pool).await?.iter().any(|s| s.url == url);
    let source_id = db::add_timeedit_source(pool, &url).await?;
    match import_source(pool, &source_id, &url).await {
        Ok(result) => Ok(result),
        Err(error) => {
            if !already_saved {
                db::remove_source(pool, &source_id).await?;
            }
            Err(error)
        }
    }
}

/// Fetch every saved feed again. One failing feed doesn't stop the others:
/// its error is in its `FeedSync`.
pub async fn sync_all(pool: &Pool) -> Result<Vec<FeedSync>> {
    let mut results = Vec::new();
    for source in db::sources(pool).await? {
        let outcome = import_source(pool, &source.id, &source.url).await;
        results.push(match outcome {
            Ok(result) => FeedSync {
                source_id: source.id,
                added: result.added,
                updated: result.updated,
                moved: result.moved,
                cancelled: result.cancelled,
                error: None,
            },
            Err(error) => FeedSync {
                source_id: source.id,
                added: 0,
                updated: 0,
                moved: 0,
                cancelled: 0,
                error: Some(format!("{error:#}")),
            },
        });
    }
    Ok(results)
}

/// The saved feeds as the app shows them, without their URLs.
pub async fn feeds(pool: &Pool) -> Result<Vec<FeedInfo>> {
    let counts = db::classes_per_source(pool).await?;
    let feeds = db::sources(pool)
        .await?
        .into_iter()
        .map(|source| FeedInfo {
            origin: origin(&source.url),
            classes: counts
                .iter()
                .find(|(id, _)| *id == source.id)
                .map_or(0, |(_, count)| *count),
            id: source.id,
            kind: source.kind,
            last_fetched_at: source.last_fetched_at,
        })
        .collect();
    Ok(feeds)
}

/// Add the past classes from a downloaded TimeEdit file (its text), once.
/// Returns (added, events in the file).
pub async fn import_history_text(pool: &Pool, ics_text: &str) -> Result<(u32, u32)> {
    let events = timeedit::parse(ics_text)?;
    let added = db::import_history(pool, &events).await?;
    Ok((added, events.len() as u32))
}

/// "cloud.timeedit.net" for a web link, the file name for a `file://` source.
/// Never the path or query, where TimeEdit puts the personal key.
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
            origin("file:///home/jls/Downloads/schedule.ics"),
            "schedule.ics"
        );
    }
}
