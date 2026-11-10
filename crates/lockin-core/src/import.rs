//! Fetching a calendar source and saving its events.
//! Used by `lockin-daemon sync` now, and by the daemon's hourly fetch in phase 4.

use anyhow::Result;

use crate::db::{self, Pool, SyncResult};
use crate::model::FeedEvent;
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
