//! Tests for activities, templates and the semester overview.

use chrono::NaiveDate;
use lockin_core::db::{self, Pool};
use lockin_core::{activity, timeedit};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");

/// The sample feed's blocks are in the week of Monday 9 November 2099.
fn sample_week() -> NaiveDate {
    NaiveDate::from_ymd_opt(2099, 11, 11).unwrap()
}

async fn database_with_sample() -> (Pool, String) {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, "https://example.test/feed.ics")
        .await
        .unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    (pool, source)
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
fn week_starts_on_monday() {
    assert_eq!(activity::week_of(sample_week()), "2099-11-09");
    assert_eq!(
        activity::week_of(NaiveDate::from_ymd_opt(2099, 11, 15).unwrap()),
        "2099-11-09"
    );
}

#[tokio::test]
async fn courses_get_a_color() {
    let (pool, _) = database_with_sample().await;
    for course in db::courses(&pool).await.unwrap() {
        assert!(course.color.is_some(), "{} has no color", course.name);
    }
}

#[tokio::test]
async fn template_per_kind() {
    let (pool, _) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;

    // Signalen en systemen has one hoorcollege and one werkcollege in the sample.
    let created = activity::add_template(&pool, &signals, Some("werkcollege"), "Exercises")
        .await
        .unwrap();
    assert_eq!(created, 1);

    let created = activity::add_template(&pool, &signals, None, "Review the class")
        .await
        .unwrap();
    assert_eq!(created, 2);

    let week = activity::in_week(&pool, sample_week()).await.unwrap();
    assert_eq!(week.len(), 3);
    assert!(week.iter().all(|a| a.week == "2099-11-09"));
}

#[tokio::test]
async fn sync_again_makes_no_duplicates() {
    let (pool, source) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    activity::add_template(&pool, &signals, None, "Review")
        .await
        .unwrap();

    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    assert_eq!(
        activity::in_week(&pool, sample_week()).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn deleted_activity_does_not_come_back() {
    let (pool, source) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    activity::add_template(&pool, &signals, Some("werkcollege"), "Exercises")
        .await
        .unwrap();

    let first = &activity::in_week(&pool, sample_week()).await.unwrap()[0];
    activity::remove(&pool, &first.id).await.unwrap();

    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    assert!(
        activity::in_week(&pool, sample_week())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn open_template_activities_on_ignored_blocks_are_hidden() {
    let (pool, _) = database_with_sample().await;
    let math = course_id(&pool, "E701033A").await;
    activity::add_template(&pool, &math, None, "Watch the clips")
        .await
        .unwrap();
    assert_eq!(
        activity::in_week(&pool, sample_week()).await.unwrap().len(),
        1
    );

    // The only Wiskunde block is "zelfstandig werk".
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();
    assert!(
        activity::in_week(&pool, sample_week())
            .await
            .unwrap()
            .is_empty()
    );

    // Remove the rule and the activity is back, still the same one.
    let rule = &db::filter_rules(&pool).await.unwrap()[0];
    db::remove_rule(&pool, &rule.id).await.unwrap();
    assert_eq!(
        activity::in_week(&pool, sample_week()).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ignored_blocks_get_no_template_activities_until_kept() {
    let (pool, _) = database_with_sample().await;
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();

    let math = course_id(&pool, "E701033A").await;
    let created = activity::add_template(&pool, &math, None, "Watch the clips")
        .await
        .unwrap();
    assert_eq!(created, 0);

    // A manual exception "keep this block" gives it its activity.
    let week = sample_week().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let blocks = db::blocks_between(&pool, week, week + chrono::Duration::days(7))
        .await
        .unwrap();
    let clips = blocks
        .iter()
        .find(|b| b.course_id.as_deref() == Some(&math))
        .unwrap();
    db::set_ignored_override(&pool, &clips.id, Some(false))
        .await
        .unwrap();
    assert_eq!(
        activity::in_week(&pool, sample_week()).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn own_and_done_activities_stay_visible_on_ignored_blocks() {
    let (pool, _) = database_with_sample().await;
    let math = course_id(&pool, "E701033A").await;
    activity::add_template(&pool, &math, None, "Watch the clips")
        .await
        .unwrap();
    let from_template = activity::in_week(&pool, sample_week()).await.unwrap()[0].clone();
    activity::set_done(&pool, &from_template.id, true)
        .await
        .unwrap();

    let block_id = from_template.block_id.clone().unwrap();
    activity::add_to_block(&pool, &block_id, "Make a summary")
        .await
        .unwrap();

    // Ignoring the block hides neither: one is done, the other you added yourself.
    db::add_title_rule(&pool, "zelfstandig werk").await.unwrap();
    assert_eq!(
        activity::in_week(&pool, sample_week()).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn removing_a_template_keeps_done_activities() {
    let (pool, _) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    activity::add_template(&pool, &signals, None, "Review")
        .await
        .unwrap();

    let week = activity::in_week(&pool, sample_week()).await.unwrap();
    activity::set_done(&pool, &week[0].id, true).await.unwrap();

    let template = &activity::templates(&pool).await.unwrap()[0];
    let removed = activity::remove_template(&pool, &template.id)
        .await
        .unwrap();
    assert_eq!(removed, 1);

    let week = activity::in_week(&pool, sample_week()).await.unwrap();
    assert_eq!(week.len(), 1);
    assert!(week[0].done);
    assert!(week[0].done_at.is_some());
}

#[tokio::test]
async fn standalone_activity_and_progress() {
    let (pool, _) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    activity::add_template(&pool, &signals, None, "Review")
        .await
        .unwrap();

    // Added on a Thursday, it belongs to the week of that Monday.
    let date = NaiveDate::from_ymd_opt(2099, 11, 12).unwrap();
    let id = activity::add_to_week(&pool, Some(&signals), date, "Read chapter 3")
        .await
        .unwrap();
    activity::set_done(&pool, &id, true).await.unwrap();

    let progress = activity::progress(&pool).await.unwrap();
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].week, "2099-11-09");
    assert_eq!(progress[0].course_id, signals);
    assert_eq!((progress[0].done, progress[0].total), (1, 3));
}

#[test]
fn week_range_is_seven_local_days() {
    let (start, end) = activity::week_range(sample_week());
    assert!(start < end);
    // 7 days, give or take an hour for a summer/winter time change.
    let hours = (end - start).num_hours();
    assert!((167..=169).contains(&hours), "{hours} hours");
}

#[tokio::test]
async fn removed_activity_can_be_restored() {
    let (pool, _) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    let id = activity::add_to_week(&pool, Some(&signals), sample_week(), "Read chapter 3")
        .await
        .unwrap();
    activity::set_done(&pool, &id, true).await.unwrap();

    activity::remove(&pool, &id).await.unwrap();
    assert!(
        activity::in_week(&pool, sample_week())
            .await
            .unwrap()
            .is_empty()
    );

    // Back exactly as it was, done included.
    activity::restore(&pool, &id).await.unwrap();
    let week = activity::in_week(&pool, sample_week()).await.unwrap();
    assert_eq!(week.len(), 1);
    assert_eq!(week[0].id, id);
    assert!(week[0].done);
}

#[tokio::test]
async fn activities_keep_the_order_they_were_added_in() {
    let (pool, _) = database_with_sample().await;
    for title in ["Zebra", "Apple", "Mango"] {
        activity::add_to_week(&pool, None, sample_week(), title)
            .await
            .unwrap();
    }
    let titles: Vec<String> = activity::in_week(&pool, sample_week())
        .await
        .unwrap()
        .into_iter()
        .map(|a| a.title)
        .collect();
    assert_eq!(titles, ["Zebra", "Apple", "Mango"]);
}

#[tokio::test]
async fn course_kinds_lists_each_kind_once() {
    let (pool, _) = database_with_sample().await;
    let signals = course_id(&pool, "E702010A").await;
    let kinds: Vec<String> = db::course_kinds(&pool)
        .await
        .unwrap()
        .into_iter()
        .filter(|k| k.course_id == signals)
        .map(|k| k.kind)
        .collect();
    assert_eq!(kinds, ["hoorcollege", "werkcollege"]);
}
