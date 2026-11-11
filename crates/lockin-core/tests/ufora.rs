//! Tests for the Ufora feed, on a made-up sample in the format of a real
//! Ufora export ("Alle cursussen"), in 2099 like the TimeEdit sample.
//!
//! The sample holds, for courses the TimeEdit sample knows:
//! - "Verslag 1 - Verwacht" (Signalen en systemen, Mon 16 Nov, with an assignment link)
//! - "Quiz 3 - Beschikbaarheid eindigt" (Programmeren, a room before the course)
//! - "Checklist week 10 - verloopt" (Wiskunde I, a whole day: due 23:59)
//! - "Labo 0 - Verwacht" (Signalen en systemen, 1 Nov: already past)
//!
//! and four events that must be skipped: released content ("- Beschikbaar"),
//! an info session, and a deadline of a course from earlier studies.

use chrono::{DateTime, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::{deadline, import, timeedit, ufora, urgent};

const TIMEEDIT: &str = include_str!("fixtures/timeedit_sample.ics");
const UFORA: &str = include_str!("fixtures/ufora_sample.ics");

fn time(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

/// Thursday 12 November 2099, noon: "now" in these tests.
fn now() -> DateTime<Utc> {
    time("2099-11-12T12:00:00Z")
}

/// A database with the TimeEdit sample (it brings the courses) and a Ufora source.
async fn database() -> (Pool, String) {
    let pool = db::open_in_memory().await.unwrap();
    let timeedit_source = db::add_timeedit_source(&pool, "https://example.test/te.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &timeedit_source, &timeedit::parse(TIMEEDIT).unwrap())
        .await
        .unwrap();
    let ufora_source = db::add_source(&pool, "https://example.test/ufora.ics", "ufora")
        .await
        .unwrap();
    (pool, ufora_source)
}

async fn course_id(pool: &Pool, code: &str) -> String {
    db::courses(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.code.as_deref() == Some(code))
        .unwrap()
        .id
}

#[test]
fn only_deadlines_are_read() {
    let deadlines = ufora::parse(UFORA).unwrap();
    let titles: Vec<&str> = deadlines.iter().map(|d| d.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Verslag 1",
            "Quiz 3",
            "Checklist week 10",
            "Oud project",
            "Labo 0"
        ]
    );

    let report = &deadlines[0];
    assert_eq!(report.uid, "6606-9001@ufora.ugent.be");
    assert_eq!(report.course_code.as_deref(), Some("E702010A"));
    assert_eq!(report.due, time("2099-11-16T22:59:59Z"));
    // The assignment's page, not the calendar event.
    assert_eq!(
        report.url.as_deref(),
        Some("https://ufora.ugent.be/d2l/lms/dropbox/user/folder_submit_files.d2l?ou=100&db=200")
    );
    let description = report.description.as_deref().unwrap();
    assert!(description.starts_with("Dien je verslag in als pdf, met je naam"));
    assert!(!description.contains("Gebeurtenis weergeven"));

    // A room before the course; only the event link.
    let quiz = &deadlines[1];
    assert_eq!(quiz.course_code.as_deref(), Some("E761018A"));
    assert!(quiz.url.as_deref().unwrap().contains("/event/9002/"));
    assert_eq!(quiz.description, None);

    // A whole day: due at 23:59 Belgian time (22:59 UTC in winter).
    assert_eq!(deadlines[2].due, time("2099-11-18T22:59:00Z"));
}

#[tokio::test]
async fn deadlines_of_known_courses_come_in() {
    let (pool, source) = database().await;
    let result = deadline::save_ufora(&pool, &source, &ufora::parse(UFORA).unwrap(), now())
        .await
        .unwrap();
    // "Oud project" belongs to a course Lockin doesn't know.
    assert_eq!((result.added, result.updated, result.cancelled), (4, 0, 0));

    let all = deadline::all(&pool).await.unwrap();
    let titles: Vec<&str> = all.iter().map(|d| d.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Labo 0", "Verslag 1", "Checklist week 10", "Quiz 3"]
    );
    assert!(
        all.iter()
            .all(|d| d.source_id.as_deref() == Some(source.as_str()))
    );

    // Already past on the first import: counts as done.
    assert!(all[0].done);
    assert!(all[1..].iter().all(|d| !d.done));

    let signals = course_id(&pool, "E702010A").await;
    assert_eq!(all[1].course_id.as_deref(), Some(signals.as_str()));
    assert!(all[1].url.as_deref().unwrap().contains("dropbox"));

    // They are in the Urgent list, with their link.
    let list = urgent::list(&pool, now()).await.unwrap();
    let report = list.iter().find(|item| item.title == "Verslag 1").unwrap();
    assert_eq!(report.status, "approaching");
    assert!(report.url.is_some());
    assert!(list.iter().all(|item| item.title != "Labo 0"));
}

#[tokio::test]
async fn moved_cancelled_and_back() {
    let (pool, source) = database().await;
    let feed = ufora::parse(UFORA).unwrap();
    deadline::save_ufora(&pool, &source, &feed, now())
        .await
        .unwrap();

    // Check "Verslag 1" off, then the teacher moves it to Wednesday.
    let report_id = deadline::all(&pool).await.unwrap()[1].id.clone();
    deadline::set_done(&pool, &report_id, true).await.unwrap();
    let moved_text = UFORA.replace("20991116T225959Z", "20991118T225959Z");
    let result = deadline::save_ufora(&pool, &source, &ufora::parse(&moved_text).unwrap(), now())
        .await
        .unwrap();
    assert_eq!((result.added, result.updated, result.moved), (0, 4, 1));

    let report = deadline::all(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.id == report_id)
        .unwrap();
    assert_eq!(report.status, "moved");
    assert_eq!(report.due_at, "2099-11-18T22:59:59Z");
    assert_eq!(report.moved_from.as_deref(), Some("2099-11-16T22:59:59Z"));
    assert!(report.done, "checking off survives a sync");

    // The quiz leaves the feed: cancelled, not deleted, and out of the Urgent list.
    let without_quiz: Vec<_> = feed
        .iter()
        .filter(|d| d.title != "Quiz 3")
        .cloned()
        .collect();
    let result = deadline::save_ufora(&pool, &source, &without_quiz, now())
        .await
        .unwrap();
    assert_eq!(result.cancelled, 1);
    let quiz = deadline::all(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.title == "Quiz 3")
        .unwrap();
    assert_eq!(quiz.status, "cancelled");
    let list = urgent::list(&pool, now()).await.unwrap();
    assert!(list.iter().all(|item| item.title != "Quiz 3"));

    // It comes back: normal again.
    deadline::save_ufora(&pool, &source, &feed, now())
        .await
        .unwrap();
    let quiz = deadline::all(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|d| d.title == "Quiz 3")
        .unwrap();
    assert_eq!(quiz.status, "normal");
}

#[tokio::test]
async fn a_pasted_ufora_feed_is_recognised() {
    let pool = db::open_in_memory().await.unwrap();
    let timeedit_source = db::add_timeedit_source(&pool, "https://example.test/te.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &timeedit_source, &timeedit::parse(TIMEEDIT).unwrap())
        .await
        .unwrap();

    let result = import::add_feed_text(&pool, "https://ufora.example.test/feed.ics", UFORA)
        .await
        .unwrap();
    assert_eq!(result.kind, "ufora");
    assert_eq!(result.added, 4);

    let feeds = import::feeds(&pool).await.unwrap();
    let ufora_feed = feeds.iter().find(|f| f.kind == "ufora").unwrap();
    assert_eq!(ufora_feed.origin, "ufora.example.test");
    assert_eq!((ufora_feed.classes, ufora_feed.deadlines), (0, 4));

    // Removing the feed hides its deadlines.
    assert_eq!(db::remove_source(&pool, &ufora_feed.id).await.unwrap(), 4);
    assert!(deadline::all(&pool).await.unwrap().is_empty());

    // A Ufora file isn't a TimeEdit download.
    assert!(import::import_history_text(&pool, UFORA).await.is_err());
}
