//! Tests for managing feeds the way the app does (Settings → Schedule).
//! They use the sample file and a local address that refuses connections,
//! so they never touch the network.

use lockin_core::{db, import};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");

/// The sample feed as a `file://` URL.
fn sample_url() -> String {
    format!(
        "file://{}/tests/fixtures/timeedit_sample.ics",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// An address where nothing listens: fetching from it fails at once.
const DEAD_URL: &str = "http://127.0.0.1:9/feed.ics";

#[tokio::test]
async fn feeds_show_origin_classes_and_last_sync() {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, &sample_url()).await.unwrap();
    import::import_source(&pool, &source, &sample_url())
        .await
        .unwrap();

    let feeds = import::feeds(&pool).await.unwrap();
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].origin, "timeedit_sample.ics");
    assert_eq!(feeds[0].classes, 4);
    assert!(feeds[0].last_fetched_at.is_some());
}

#[tokio::test]
async fn one_failing_feed_does_not_stop_the_others() {
    let pool = db::open_in_memory().await.unwrap();
    db::add_timeedit_source(&pool, &sample_url()).await.unwrap();
    db::add_timeedit_source(&pool, DEAD_URL).await.unwrap();

    let results = import::sync_all(&pool).await.unwrap();
    assert_eq!(results.len(), 2);
    let good = results.iter().find(|r| r.error.is_none()).unwrap();
    assert_eq!(good.added, 4);
    assert!(results.iter().any(|r| r.error.is_some()));
}

#[tokio::test]
async fn a_new_feed_that_fails_is_not_kept() {
    let pool = db::open_in_memory().await.unwrap();
    assert!(import::add_feed(&pool, DEAD_URL).await.is_err());
    assert!(db::sources(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_saved_feed_that_fails_is_kept() {
    let pool = db::open_in_memory().await.unwrap();
    db::add_timeedit_source(&pool, DEAD_URL).await.unwrap();
    assert!(import::add_feed(&pool, DEAD_URL).await.is_err());
    assert_eq!(db::sources(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn history_from_text_adds_past_classes_only() {
    let pool = db::open_in_memory().await.unwrap();
    // The sample's classes are all in 2099: none is in the past.
    let (added, total) = import::import_history_text(&pool, SAMPLE, Some("sample.ics"))
        .await
        .unwrap();
    assert_eq!((added, total), (0, 4));
    // Nothing added: no empty import is kept in the list.
    assert!(db::history_imports(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn errors_never_show_the_link() {
    let pool = db::open_in_memory().await.unwrap();
    let secret = "http://127.0.0.1:9/ugent/web/s1t2u3v4-personal-key.ics";
    let error = import::add_feed(&pool, secret).await.unwrap_err();
    let text = format!("{error:#}");
    assert!(!text.contains("personal-key"), "{text}");
}

#[tokio::test]
async fn a_sync_is_due_after_an_hour() {
    let pool = db::open_in_memory().await.unwrap();
    let now = chrono::Utc::now();
    // No feeds: nothing to sync.
    assert!(!import::sync_due(&pool, now).await.unwrap());

    // A feed that was never fetched is due at once.
    let source = db::add_timeedit_source(&pool, &sample_url()).await.unwrap();
    assert!(import::sync_due(&pool, now).await.unwrap());

    // Just fetched: due again only once it's over an hour old.
    import::import_source(&pool, &source, &sample_url())
        .await
        .unwrap();
    let now = chrono::Utc::now();
    assert!(!import::sync_due(&pool, now).await.unwrap());
    let later = now + chrono::Duration::minutes(import::SYNC_EVERY_MINUTES + 1);
    assert!(import::sync_due(&pool, later).await.unwrap());
}
