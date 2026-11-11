//! The Tauri side of the Lockin app.
//!
//! Every `#[tauri::command]` here is a function the Svelte frontend can call
//! with `invoke("name", { args })` (https://tauri.app/develop/calling-rust/).
//! They are thin: the real work happens in `lockin-core`, so the daemon can
//! use the same functions later. The frontend wrappers are in `src/lib/api.ts`.
//!
//! Tauri turns snake_case argument names into camelCase on the JavaScript side:
//! `block_id` here is `blockId` in `invoke`.

use chrono::{DateTime, NaiveDate, Utc};
use lockin_core::model::{
    Activity, ActivityTemplate, ClassBlock, Course, CourseKind, Deadline, FeedInfo, FeedSync,
    FilterRule, UrgentItem, WeekClasses, WeekProgress,
};
use lockin_core::{activity, db, deadline, import, paths, urgent};
use serde::Serialize;
use tauri::{Manager, State};

/// Commands return errors as text: the frontend shows them as they are.
type CommandResult<T> = Result<T, String>;

/// Turn any error into its message, including the causes ("{:#}" with anyhow).
fn message(error: impl std::fmt::Display) -> String {
    format!("{error:#}")
}

/// Dates come from the frontend as "2026-11-11".
fn parse_date(text: &str) -> CommandResult<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| format!("not a date: {text}"))
}

/// Times come from the frontend as RFC 3339, e.g. "2026-11-16T22:59:00.000Z"
/// (what `Date.toISOString()` gives).
fn parse_time(text: &str) -> CommandResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| format!("not a time: {text}"))
}

/// Everything the week view needs in one call.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Week {
    /// Monday of the week, e.g. "2026-11-09".
    monday: String,
    blocks: Vec<ClassBlock>,
    activities: Vec<Activity>,
}

// ---------- Reading ----------

#[tauri::command]
async fn courses(pool: State<'_, db::Pool>) -> CommandResult<Vec<Course>> {
    db::courses(&pool).await.map_err(message)
}

/// The blocks and activities of the week `date` is in.
#[tauri::command]
async fn week(pool: State<'_, db::Pool>, date: String) -> CommandResult<Week> {
    let date = parse_date(&date)?;
    let (start, end) = activity::week_range(date);
    Ok(Week {
        monday: activity::week_of(date),
        blocks: db::blocks_between(&pool, start, end)
            .await
            .map_err(message)?,
        activities: activity::in_week(&pool, date).await.map_err(message)?,
    })
}

/// The kinds of class per course, to choose from when making a template.
#[tauri::command]
async fn course_kinds(pool: State<'_, db::Pool>) -> CommandResult<Vec<CourseKind>> {
    db::course_kinds(&pool).await.map_err(message)
}

#[tauri::command]
async fn templates(pool: State<'_, db::Pool>) -> CommandResult<Vec<ActivityTemplate>> {
    activity::templates(&pool).await.map_err(message)
}

/// Classes per course per week, for the semester overview.
#[tauri::command]
async fn class_weeks(pool: State<'_, db::Pool>) -> CommandResult<Vec<WeekClasses>> {
    activity::class_weeks(&pool).await.map_err(message)
}

/// Done and total activities per course per week, for the semester overview.
#[tauri::command]
async fn progress(pool: State<'_, db::Pool>) -> CommandResult<Vec<WeekProgress>> {
    activity::progress(&pool).await.map_err(message)
}

/// The Urgent list right now: open activities and deadlines, most urgent first.
#[tauri::command]
async fn urgent(pool: State<'_, db::Pool>) -> CommandResult<Vec<UrgentItem>> {
    urgent::list(&pool, Utc::now()).await.map_err(message)
}

/// Every deadline, done and cancelled ones too, sorted by due time.
#[tauri::command]
async fn deadlines(pool: State<'_, db::Pool>) -> CommandResult<Vec<Deadline>> {
    deadline::all(&pool).await.map_err(message)
}

// ---------- Activities ----------

/// Returns the id of the new activity.
#[tauri::command]
async fn add_block_activity(
    pool: State<'_, db::Pool>,
    block_id: String,
    title: String,
) -> CommandResult<String> {
    activity::add_to_block(&pool, &block_id, &title)
        .await
        .map_err(message)
}

/// An activity without a block, in the week of `date`. Returns its id.
#[tauri::command]
async fn add_week_activity(
    pool: State<'_, db::Pool>,
    course_id: Option<String>,
    date: String,
    title: String,
) -> CommandResult<String> {
    let date = parse_date(&date)?;
    activity::add_to_week(&pool, course_id.as_deref(), date, &title)
        .await
        .map_err(message)
}

#[tauri::command]
async fn set_activity_done(
    pool: State<'_, db::Pool>,
    activity_id: String,
    done: bool,
) -> CommandResult<()> {
    activity::set_done(&pool, &activity_id, done)
        .await
        .map_err(message)
}

#[tauri::command]
async fn rename_activity(
    pool: State<'_, db::Pool>,
    activity_id: String,
    title: String,
) -> CommandResult<()> {
    activity::rename(&pool, &activity_id, &title)
        .await
        .map_err(message)
}

#[tauri::command]
async fn remove_activity(pool: State<'_, db::Pool>, activity_id: String) -> CommandResult<()> {
    activity::remove(&pool, &activity_id).await.map_err(message)
}

/// Undo a removal.
#[tauri::command]
async fn restore_activity(pool: State<'_, db::Pool>, activity_id: String) -> CommandResult<()> {
    activity::restore(&pool, &activity_id)
        .await
        .map_err(message)
}

// ---------- Deadlines ----------

/// A deadline added by hand; the course is optional. Returns its id.
#[tauri::command]
async fn add_deadline(
    pool: State<'_, db::Pool>,
    course_id: Option<String>,
    title: String,
    due: String,
) -> CommandResult<String> {
    let due = parse_time(&due)?;
    deadline::add(&pool, course_id.as_deref(), &title, due)
        .await
        .map_err(message)
}

#[tauri::command]
async fn update_deadline(
    pool: State<'_, db::Pool>,
    deadline_id: String,
    course_id: Option<String>,
    title: String,
    due: String,
) -> CommandResult<()> {
    let due = parse_time(&due)?;
    deadline::update(&pool, &deadline_id, course_id.as_deref(), &title, due)
        .await
        .map_err(message)
}

#[tauri::command]
async fn set_deadline_done(
    pool: State<'_, db::Pool>,
    deadline_id: String,
    done: bool,
) -> CommandResult<()> {
    deadline::set_done(&pool, &deadline_id, done)
        .await
        .map_err(message)
}

#[tauri::command]
async fn remove_deadline(pool: State<'_, db::Pool>, deadline_id: String) -> CommandResult<()> {
    deadline::remove(&pool, &deadline_id).await.map_err(message)
}

/// Undo a removal.
#[tauri::command]
async fn restore_deadline(pool: State<'_, db::Pool>, deadline_id: String) -> CommandResult<()> {
    deadline::restore(&pool, &deadline_id)
        .await
        .map_err(message)
}

// ---------- Templates ----------

/// `kind` None = every block of the course. Returns how many activities it made.
#[tauri::command]
async fn add_template(
    pool: State<'_, db::Pool>,
    course_id: String,
    kind: Option<String>,
    title: String,
) -> CommandResult<u32> {
    activity::add_template(&pool, &course_id, kind.as_deref(), &title)
        .await
        .map_err(message)
}

/// Returns how many open activities were removed with it.
#[tauri::command]
async fn remove_template(pool: State<'_, db::Pool>, template_id: String) -> CommandResult<u32> {
    activity::remove_template(&pool, &template_id)
        .await
        .map_err(message)
}

/// The filter rules, to say which rule ignores a block.
#[tauri::command]
async fn rules(pool: State<'_, db::Pool>) -> CommandResult<Vec<FilterRule>> {
    db::filter_rules(&pool).await.map_err(message)
}

// ---------- Schedule: feeds and rules ----------

/// The saved feeds, without their URLs (they hold a personal key).
#[tauri::command]
async fn feeds(pool: State<'_, db::Pool>) -> CommandResult<Vec<FeedInfo>> {
    import::feeds(&pool).await.map_err(message)
}

/// Save a pasted TimeEdit or Ufora link and import it right away.
/// `kind` in the result says which one it was.
#[tauri::command]
async fn add_feed(pool: State<'_, db::Pool>, url: String) -> CommandResult<FeedSync> {
    import::add_feed(&pool, &url).await.map_err(message)
}

/// Fetch every saved feed again.
#[tauri::command]
async fn sync_feeds(pool: State<'_, db::Pool>) -> CommandResult<Vec<FeedSync>> {
    import::sync_all(&pool).await.map_err(message)
}

/// Remove a feed, then sync the others, so classes they also have come back.
/// Returns how many classes (TimeEdit) or deadlines (Ufora) the removed feed held.
#[tauri::command]
async fn remove_feed(pool: State<'_, db::Pool>, source_id: String) -> CommandResult<u32> {
    let hidden = db::remove_source(&pool, &source_id)
        .await
        .map_err(message)?;
    import::sync_all(&pool).await.map_err(message)?;
    Ok(hidden)
}

/// How many classes a TimeEdit download added, out of how many in the file.
#[derive(Serialize)]
struct HistoryResult {
    added: u32,
    total: u32,
}

/// Add the past classes from a downloaded TimeEdit file. The frontend reads
/// the file and sends its text, so no file access is needed on this side.
#[tauri::command]
async fn import_history(pool: State<'_, db::Pool>, text: String) -> CommandResult<HistoryResult> {
    let (added, total) = import::import_history_text(&pool, &text)
        .await
        .map_err(message)?;
    Ok(HistoryResult { added, total })
}

/// "Ignore classes whose title contains …". Returns how many classes changed.
#[tauri::command]
async fn add_rule(pool: State<'_, db::Pool>, pattern: String) -> CommandResult<u32> {
    db::add_title_rule(&pool, pattern.trim())
        .await
        .map_err(message)
}

/// Returns how many classes changed.
#[tauri::command]
async fn remove_rule(pool: State<'_, db::Pool>, rule_id: String) -> CommandResult<u32> {
    db::remove_rule(&pool, &rule_id).await.map_err(message)
}

// ---------- Blocks ----------

/// "Got it" on the schedule changes banner.
#[tauri::command]
async fn acknowledge_changes(
    pool: State<'_, db::Pool>,
    block_ids: Vec<String>,
) -> CommandResult<()> {
    db::acknowledge_changes(&pool, &block_ids)
        .await
        .map_err(message)
}

/// The manual exception for one block: true = always ignore,
/// false = never ignore, null = let the rules decide again.
#[tauri::command]
async fn set_block_ignored(
    pool: State<'_, db::Pool>,
    block_id: String,
    ignored: Option<bool>,
) -> CommandResult<()> {
    db::set_ignored_override(&pool, &block_id, ignored)
        .await
        .map_err(message)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // On Wayland, KDE finds the window's icon through its app id: it shows the icon
    // of `<app id>.desktop`. GTK takes that id from the program name, which is
    // otherwise the binary's file name (`lockin-app` in dev, `lockin` when bundled).
    // Setting it before Tauri starts GTK gives one fixed id everywhere:
    // the `identifier` from tauri.conf.json.
    #[cfg(target_os = "linux")]
    glib::set_prgname(Some("io.github.jls_09.lockin"));

    tauri::Builder::default()
        // "Open in Ufora" opens links in the browser; only Ufora links are
        // allowed (capabilities/default.json).
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // The same database file the daemon uses, see lockin_core::paths.
            let file = paths::database_file()?;
            let pool = tauri::async_runtime::block_on(db::open(&file))?;
            app.manage(pool);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            courses,
            course_kinds,
            week,
            templates,
            progress,
            class_weeks,
            rules,
            urgent,
            deadlines,
            add_block_activity,
            add_week_activity,
            set_activity_done,
            rename_activity,
            remove_activity,
            restore_activity,
            add_deadline,
            update_deadline,
            set_deadline_done,
            remove_deadline,
            restore_deadline,
            add_template,
            remove_template,
            set_block_ignored,
            acknowledge_changes,
            feeds,
            add_feed,
            sync_feeds,
            remove_feed,
            import_history,
            add_rule,
            remove_rule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Lockin app");
}
