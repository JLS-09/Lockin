//! Tests for deadlines and the Urgent list.

use chrono::{DateTime, Local, NaiveDate, NaiveTime, Utc};
use lockin_core::db::{self, Pool};
use lockin_core::{activity, deadline, timeedit, urgent};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");

async fn database_with_sample() -> Pool {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    pool
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

fn time(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
fn a_date_alone_is_due_at_the_end_of_the_day() {
    let due = deadline::due_time(date(2099, 11, 16), None).unwrap();
    let local = due.with_timezone(&Local);
    assert_eq!(local.date_naive(), date(2099, 11, 16));
    assert_eq!(local.time(), NaiveTime::from_hms_opt(23, 59, 0).unwrap());
}

#[tokio::test]
async fn add_edit_check_off_and_remove() {
    let pool = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;

    let id = deadline::add(
        &pool,
        Some(&signals),
        "  Lab report 2 ",
        time("2099-11-16T22:59:00Z"),
    )
    .await
    .unwrap();
    let all = deadline::all(&pool).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].title, "Lab report 2");
    assert_eq!(all[0].due_at, "2099-11-16T22:59:00Z");
    assert_eq!(all[0].source_id, None);
    assert_eq!(all[0].status, "normal");

    deadline::update(
        &pool,
        &id,
        None,
        "Lab report 3",
        time("2099-11-20T22:59:00Z"),
    )
    .await
    .unwrap();
    deadline::set_done(&pool, &id, true).await.unwrap();
    let all = deadline::all(&pool).await.unwrap();
    assert_eq!(all[0].title, "Lab report 3");
    assert_eq!(all[0].course_id, None);
    assert!(all[0].done && all[0].done_at.is_some());

    deadline::remove(&pool, &id).await.unwrap();
    assert!(deadline::all(&pool).await.unwrap().is_empty());
    deadline::restore(&pool, &id).await.unwrap();
    assert_eq!(deadline::all(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_deadline_needs_a_title() {
    let pool = database_with_sample().await;
    assert!(
        deadline::add(&pool, None, "   ", time("2099-11-16T22:59:00Z"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn deadlines_between_two_times() {
    let pool = database_with_sample().await;
    deadline::add(&pool, None, "This week", time("2099-11-13T22:59:00Z"))
        .await
        .unwrap();
    deadline::add(&pool, None, "Later", time("2099-12-01T22:59:00Z"))
        .await
        .unwrap();

    let coming = deadline::between(
        &pool,
        time("2099-11-09T00:00:00Z"),
        time("2099-11-23T00:00:00Z"),
    )
    .await
    .unwrap();
    assert_eq!(coming.len(), 1);
    assert_eq!(coming[0].title, "This week");
}

#[tokio::test]
async fn urgent_list() {
    let pool = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    let c_course = course_id(&pool, "E761018A").await;
    // Thursday 12 November 2099, noon.
    let now = time("2099-11-12T12:00:00Z");

    // Signalen en systemen: a class on Monday 18:30 (2.7 days ago) and on
    // Tuesday 14:30 (1.9 days ago). The template gives each one an activity.
    activity::add_template(&pool, &signals, None, "Review")
        .await
        .unwrap();

    // A class on Friday: its activity isn't open yet.
    let blocks = db::blocks_between(
        &pool,
        time("2099-11-13T00:00:00Z"),
        time("2099-11-14T00:00:00Z"),
    )
    .await
    .unwrap();
    activity::add_to_block(&pool, &blocks[0].id, "Prepare the test")
        .await
        .unwrap();

    // An activity without a class: open since Monday.
    activity::add_to_week(&pool, Some(&c_course), date(2099, 11, 11), "Read chapter 6")
        .await
        .unwrap();
    // A done activity isn't urgent.
    let done = activity::add_to_week(&pool, None, date(2099, 11, 11), "Done already")
        .await
        .unwrap();
    activity::set_done(&pool, &done, true).await.unwrap();

    // Due in 1.5 days, due in 18 days, and one that's done.
    deadline::add(
        &pool,
        Some(&signals),
        "Milestone 1",
        time("2099-11-13T23:59:00Z"),
    )
    .await
    .unwrap();
    deadline::add(&pool, None, "Problem set 9", time("2099-11-30T23:59:00Z"))
        .await
        .unwrap();
    let handed_in = deadline::add(&pool, None, "Handed in", time("2099-11-14T23:59:00Z"))
        .await
        .unwrap();
    deadline::set_done(&pool, &handed_in, true).await.unwrap();

    let list = urgent::list(&pool, now).await.unwrap();
    let titles: Vec<&str> = list.iter().map(|item| item.title.as_str()).collect();

    // The deadline in 1.5 days (score 11) first, then the activities by how
    // long they've been open, the far deadline last. "Read chapter 6" has been
    // open since Monday 00:00 local time, before the Monday class ended.
    assert_eq!(
        titles,
        [
            "Milestone 1",
            "Read chapter 6",
            "Review",
            "Review",
            "Problem set 9"
        ]
    );

    let statuses: Vec<&str> = list.iter().map(|item| item.status.as_str()).collect();
    assert_eq!(
        statuses,
        ["approaching", "behind", "behind", "planned", "planned"]
    );

    let milestone = &list[0];
    assert_eq!(milestone.kind, "deadline");
    assert_eq!(milestone.course_id.as_deref(), Some(signals.as_str()));
    assert!(milestone.due_at.is_some() && milestone.open_since.is_none());

    let review = &list[2];
    assert_eq!(review.kind, "activity");
    assert_eq!(review.open_since.as_deref(), Some("2099-11-09T18:30:00Z"));
    assert_eq!(review.week.as_deref(), Some("2099-11-09"));
    assert!(review.block_id.is_some());
}

#[tokio::test]
async fn ignored_classes_leave_the_urgent_list() {
    let pool = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    activity::add_template(&pool, &signals, None, "Review")
        .await
        .unwrap();
    let now = time("2099-11-12T12:00:00Z");
    assert_eq!(urgent::list(&pool, now).await.unwrap().len(), 2);

    // Ignoring the werkcollege hides its open template activity everywhere.
    db::add_title_rule(&pool, "werkcollege").await.unwrap();
    assert_eq!(urgent::list(&pool, now).await.unwrap().len(), 1);
}
