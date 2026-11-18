//! The cloud sync (`lockin_core::sync`).
//!
//! - Without a network: the columns match the cloud schema, which rows go out
//!   and how they look, and which version of a row is kept when one comes in.
//! - Against a local Supabase (Auth + REST + Postgres with Lockin's schema), run
//!   by `tools/test-cloud.sh`: two computers with the same account end up with
//!   the same data. A plain `cargo test` skips those (no LOCKIN_TEST_REST_URL).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use lockin_core::account::{self, Cloud, Session};
use lockin_core::db::{self, Pool};
use lockin_core::sync::{self, SyncError};
use lockin_core::{activity, timeedit};
use serde_json::{Value, json};

const SAMPLE: &str = include_str!("fixtures/timeedit_sample.ics");
const FEED_URL: &str = "https://cloud.timeedit.net/ugent/web/ri6Q7XY.ics?key=secret";

/// The sample's classes are in the week of Monday 9 November 2099.
fn sample_week() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2099, 11, 11).unwrap()
}

async fn database_with_sample() -> Pool {
    let pool = db::open_in_memory().await.unwrap();
    let source = db::add_timeedit_source(&pool, FEED_URL).await.unwrap();
    db::save_feed(&pool, &source, &timeedit::parse(SAMPLE).unwrap())
        .await
        .unwrap();
    pool
}

/// A login that never talks to a server, for the tests without a network.
fn offline_session() -> Session {
    Session {
        user_id: "00000000-0000-0000-0000-00000000000a".to_string(),
        email: "a@example.test".to_string(),
        access_token: "token".to_string(),
        refresh_token: "refresh".to_string(),
        expires_at: 4_000_000_000,
        feed_key: account::feed_key("secret123", "00000000-0000-0000-0000-00000000000a"),
    }
}

// ---------- Without a network ----------

/// Every column of every synced table exists in the cloud (supabase/migrations),
/// so a new column in a SQLite migration can't silently break the sync.
#[tokio::test]
async fn columns_match_the_cloud() {
    let pool = db::open_in_memory().await.unwrap();
    let cloud = cloud_columns();
    for table in sync::TABLES {
        let local: BTreeSet<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
            .bind(table)
            .fetch_all(&pool)
            .await
            .unwrap()
            .into_iter()
            .collect();
        let mut expected = local.clone();
        expected.insert("user_id".to_string());
        expected.insert("synced_at".to_string());
        assert_eq!(
            cloud.get(*table),
            Some(&expected),
            "{table}: the cloud schema needs the same columns (add a file to supabase/migrations)"
        );
    }
}

/// The columns of each `create table public.<name> (…)` in supabase/migrations.
/// Later migrations that add columns (`alter table … add column`) count too.
fn cloud_columns() -> BTreeMap<String, BTreeSet<String>> {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../supabase/migrations");
    let mut files: Vec<_> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    let mut tables: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(file).unwrap();
        let mut current: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("create table public.") {
                current = Some(rest.split_whitespace().next().unwrap().to_string());
                tables.entry(current.clone().unwrap()).or_default();
            } else if line.starts_with(");") {
                current = None;
            } else if let Some(table) = &current {
                let word = line.split_whitespace().next().unwrap_or("");
                if !word.is_empty() && !word.starts_with("--") && word != "primary" {
                    tables.get_mut(table).unwrap().insert(word.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("alter table public.") {
                let words: Vec<&str> = rest.split_whitespace().collect();
                if let [table, "add", "column", column, ..] = words.as_slice() {
                    tables
                        .entry(table.to_string())
                        .or_default()
                        .insert(column.to_string());
                }
            }
        }
    }
    tables
}

#[tokio::test]
async fn rows_go_out_with_the_account_and_an_encrypted_link() {
    let pool = database_with_sample().await;
    let session = offline_session();

    let sources = sync::rows_to_send(&pool, "calendar_source", "", &session)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    let source = &sources[0];
    assert_eq!(source["user_id"], json!(session.user_id));
    let sent_url = source["url"].as_str().unwrap();
    assert!(sent_url.starts_with("enc1:") && !sent_url.contains("timeedit"));
    assert_eq!(
        sync::decrypt_url(&session.feed_key, sent_url).as_deref(),
        Some(FEED_URL)
    );
    assert!(
        source.get("last_fetched_at").is_none(),
        "when this computer fetched stays here"
    );

    // Numbers stay numbers, empty stays null.
    let blocks = sync::rows_to_send(&pool, "class_block", "", &session)
        .await
        .unwrap();
    assert!(!blocks.is_empty());
    assert_eq!(blocks[0]["deleted"], json!(0));
    assert_eq!(blocks[0]["ignored_override"], Value::Null);

    // Only what changed since a time goes out.
    let later = "2999-01-01T00:00:00Z";
    assert!(
        sync::rows_to_send(&pool, "class_block", later, &session)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_newer_version_of_a_row_is_kept() {
    let pool = database_with_sample().await;
    let session = offline_session();
    let course = db::courses(&pool).await.unwrap().remove(0);
    let added = activity::add_to_week(&pool, Some(&course.id), sample_week(), "Read chapter 3")
        .await
        .unwrap();
    let row = |title: &str, updated_at: &str| {
        json!({
            "id": added, "block_id": null, "template_id": null, "course_id": course.id,
            "title": title, "week": "2099-11-09", "done": 1, "done_at": updated_at,
            "updated_at": updated_at, "deleted": 0,
            "user_id": session.user_id, "synced_at": "2099-11-09T12:00:00.000000+00:00"
        })
    };
    let title = |pool: Pool| async move {
        sqlx::query_scalar::<_, String>("SELECT title FROM activity WHERE id = ?")
            .bind(&added_id(&pool).await)
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    let mut connection = pool.acquire().await.unwrap();

    // Older than the row here: kept as it is.
    let older = sync::apply_rows(
        &mut connection,
        "activity",
        &[row("Old title", "2000-01-01T00:00:00Z")],
        "",
    )
    .await
    .unwrap();
    assert_eq!(older.written, 0);

    // Newer: written.
    let newer = sync::apply_rows(
        &mut connection,
        "activity",
        &[row("Read chapter 4", "2999-01-01T00:00:00Z")],
        "",
    )
    .await
    .unwrap();
    assert_eq!(newer.written, 1);
    drop(connection);
    assert_eq!(title(pool.clone()).await, "Read chapter 4");

    // A row this computer doesn't have yet: added.
    let mut connection = pool.acquire().await.unwrap();
    let mut new_row = row("From the laptop", "2099-11-09T10:00:00Z");
    new_row["id"] = json!("a-new-id");
    let added_row = sync::apply_rows(&mut connection, "activity", &[new_row], "")
        .await
        .unwrap();
    assert_eq!(added_row.written, 1);
}

async fn added_id(pool: &Pool) -> String {
    sqlx::query_scalar("SELECT id FROM activity WHERE week = '2099-11-09' ORDER BY rowid LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_class_this_computer_has_under_another_id_is_skipped() {
    let pool = database_with_sample().await;
    let session = offline_session();
    // The same class from another computer, under its own id.
    let mut blocks = sync::rows_to_send(&pool, "class_block", "", &session)
        .await
        .unwrap();
    let mut copy = blocks.remove(0);
    copy["id"] = json!("another-computers-id");
    copy["updated_at"] = json!("2999-01-01T00:00:00Z");

    let mut connection = pool.acquire().await.unwrap();
    let applied = sync::apply_rows(&mut connection, "class_block", &[copy], "")
        .await
        .unwrap();
    assert_eq!(applied.written, 0);
    assert_eq!(applied.skipped.len(), 1);
    assert!(applied.skipped[0].starts_with("class_block another-computers-id"));
}

// ---------- Against a local Supabase ----------

fn local() -> Option<(Cloud, String)> {
    let auth_url = std::env::var("LOCKIN_TEST_AUTH_URL").ok()?;
    let rest_url = std::env::var("LOCKIN_TEST_REST_URL").ok()?;
    let database = std::env::var("LOCKIN_TEST_DATABASE_URL").ok()?;
    Some((Cloud::new(&auth_url, &rest_url, "test-key"), database))
}

macro_rules! local_or_skip {
    () => {
        match local() {
            Some(found) => found,
            None => {
                eprintln!("skipped: run tools/test-cloud.sh for the sync tests");
                return;
            }
        }
    };
}

fn sql(database: &str, query: &str) -> String {
    let output = std::process::Command::new("psql")
        .args([
            database,
            "-X",
            "-q",
            "-A",
            "-t",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            query,
        ])
        .output()
        .expect("psql runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

async fn new_account(cloud: &Cloud, database: &str) -> Session {
    let code = sql(
        database,
        "insert into public.invite_code (note) values ('sync test') returning code",
    );
    let email = format!("{}@example.test", lockin_core::new_id());
    account::sign_up(cloud, &email, "secret123", &code)
        .await
        .unwrap()
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

/// What a computer shows: every activity's title and whether it's done.
async fn activities(pool: &Pool) -> Vec<(String, bool)> {
    sqlx::query_as("SELECT title, done FROM activity WHERE deleted = 0 ORDER BY title")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn two_computers_end_up_the_same() {
    let (cloud, database) = local_or_skip!();
    let session = new_account(&cloud, &database).await;

    // The desktop has the schedule and an activity; the laptop is new.
    let desktop = database_with_sample().await;
    let course = db::courses(&desktop).await.unwrap().remove(0);
    let read = activity::add_to_week(&desktop, Some(&course.id), sample_week(), "Read chapter 3")
        .await
        .unwrap();
    let laptop = db::open_in_memory().await.unwrap();

    let sent = sync::sync(&desktop, &cloud, &session, now()).await.unwrap();
    assert!(sent.sent > 0);
    let received = sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    assert_eq!(received.received, sent.sent);
    assert!(received.skipped.is_empty(), "{:?}", received.skipped);
    assert_eq!(activities(&laptop).await, activities(&desktop).await);
    assert_eq!(
        db::courses(&laptop).await.unwrap().len(),
        db::courses(&desktop).await.unwrap().len()
    );

    // The feed link arrived, readable; the cloud only has it encrypted.
    let link: String = sqlx::query_scalar("SELECT url FROM calendar_source")
        .fetch_one(&laptop)
        .await
        .unwrap();
    assert_eq!(link, FEED_URL);
    let stored = sql(
        &database,
        &format!(
            "select url from public.calendar_source where user_id = '{}'",
            session.user_id
        ),
    );
    assert!(stored.starts_with("enc1:"), "{stored}");

    // Checked off on the laptop: done on the desktop after both sync.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    activity::set_done(&laptop, &read, true).await.unwrap();
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    sync::sync(&desktop, &cloud, &session, now()).await.unwrap();
    assert_eq!(
        activities(&desktop).await,
        vec![("Read chapter 3".to_string(), true)]
    );

    // Removed on the desktop: gone on the laptop (a soft delete syncs too).
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    activity::remove(&desktop, &read).await.unwrap();
    sync::sync(&desktop, &cloud, &session, now()).await.unwrap();
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    assert!(activities(&laptop).await.is_empty());

    // Nothing changed: after a sync or two, nothing goes either way. (A row that
    // came in during the same second as the last send may go back once: times
    // have whole seconds. The cloud then keeps the same version: harmless.)
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    let quiet = sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    assert_eq!((quiet.sent, quiet.received), (0, 0), "{quiet:?}");
}

#[tokio::test]
async fn the_later_change_wins_on_both_computers() {
    let (cloud, database) = local_or_skip!();
    let session = new_account(&cloud, &database).await;
    let desktop = database_with_sample().await;
    let course = db::courses(&desktop).await.unwrap().remove(0);
    let item = activity::add_to_week(&desktop, Some(&course.id), sample_week(), "Exercises")
        .await
        .unwrap();
    let laptop = db::open_in_memory().await.unwrap();
    sync::sync(&desktop, &cloud, &session, now()).await.unwrap();
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();

    // Both rename it while offline; the laptop does it later.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    activity::rename(&desktop, &item, "Exercises (desktop)")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    activity::rename(&laptop, &item, "Exercises (laptop)")
        .await
        .unwrap();

    // The laptop comes online first, the desktop after: its older change loses.
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    sync::sync(&desktop, &cloud, &session, now()).await.unwrap();
    sync::sync(&laptop, &cloud, &session, now()).await.unwrap();
    let expected = vec![("Exercises (laptop)".to_string(), false)];
    assert_eq!(activities(&desktop).await, expected);
    assert_eq!(activities(&laptop).await, expected);
}

#[tokio::test]
async fn accounts_stay_apart() {
    let (cloud, database) = local_or_skip!();
    let alice = new_account(&cloud, &database).await;
    let bob = new_account(&cloud, &database).await;

    let alices = database_with_sample().await;
    sync::sync(&alices, &cloud, &alice, now()).await.unwrap();

    // Bob's new computer gets nothing of Alice's.
    let bobs = db::open_in_memory().await.unwrap();
    let report = sync::sync(&bobs, &cloud, &bob, now()).await.unwrap();
    assert_eq!(report.received, 0);

    // Bob logging in on Alice's computer doesn't upload her data to his account.
    assert!(matches!(
        sync::sync(&alices, &cloud, &bob, now()).await,
        Err(SyncError::OtherAccount)
    ));
    assert_eq!(
        sql(
            &database,
            &format!(
                "select count(*) from public.class_block where user_id = '{}'",
                bob.user_id
            )
        ),
        "0"
    );
    assert_eq!(
        sync::state(&alices, "last_error").await.unwrap().as_deref(),
        Some("sync.error.otherAccount")
    );
}

#[tokio::test]
async fn another_password_locks_the_feed_link() {
    let (cloud, database) = local_or_skip!();
    let session = new_account(&cloud, &database).await;
    let desktop = database_with_sample().await;
    sync::sync(&desktop, &cloud, &session, now()).await.unwrap();

    // The password was reset by hand: a new login, a new feed key.
    sql(&database, &format!(
        "update auth.users set encrypted_password = extensions.crypt('new-secret-456', extensions.gen_salt('bf'))
         where id = '{}'", session.user_id));
    let after_reset = account::log_in(&cloud, &session.email, "new-secret-456")
        .await
        .unwrap();

    let laptop = db::open_in_memory().await.unwrap();
    let report = sync::sync(&laptop, &cloud, &after_reset, now())
        .await
        .unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    let link: String = sqlx::query_scalar("SELECT url FROM calendar_source")
        .fetch_one(&laptop)
        .await
        .unwrap();
    assert!(sync::is_locked(&link), "kept encrypted: {link}");

    // The classes came anyway; the feed asks for its link again.
    assert!(!db::courses(&laptop).await.unwrap().is_empty());
    let results = lockin_core::import::sync_all(&laptop).await.unwrap();
    assert!(
        results[0]
            .error
            .as_deref()
            .unwrap()
            .contains("paste it again")
    );
}
