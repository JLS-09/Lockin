//! The Tauri side of the Lockin app.
//!
//! Every `#[tauri::command]` here is a function the Svelte frontend can call
//! with `invoke("name", { args })` (https://tauri.app/develop/calling-rust/).
//! They are thin: the real work happens in `lockin-core`, so the daemon can
//! use the same functions later. The frontend wrappers are in `src/lib/api.ts`.
//!
//! Tauri turns snake_case argument names into camelCase on the JavaScript side:
//! `block_id` here is `blockId` in `invoke`.

use chrono::{DateTime, Local, NaiveDate, Utc};
use lockin_core::apps::{self, InstalledApp};
use lockin_core::ipc::{self, DaemonStatus, Request};
use lockin_core::model::{
    Activity, ActivityTemplate, AppCloses, BlockProfile, ClassBlock, Course, CourseKind, Deadline,
    FeedInfo, FeedSync, FilterRule, HistoryImport, PlanResult, SessionDetails, SessionEvent,
    SessionState, StudySession, UrgentItem, WeekClasses, WeekProgress,
};
use lockin_core::{activity, block, db, deadline, import, paths, session, urgent};
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
/// the file and sends its text and name, so no file access is needed on this side.
#[tauri::command]
async fn import_history(
    pool: State<'_, db::Pool>,
    text: String,
    name: Option<String>,
) -> CommandResult<HistoryResult> {
    let (added, total) = import::import_history_text(&pool, &text, name.as_deref())
        .await
        .map_err(message)?;
    Ok(HistoryResult { added, total })
}

/// The file imports that still hold classes, newest first.
#[tauri::command]
async fn history_imports(pool: State<'_, db::Pool>) -> CommandResult<Vec<HistoryImport>> {
    db::history_imports(&pool).await.map_err(message)
}

/// Remove a file import: its classes are hidden, except the ones the live feed has.
/// Returns how many classes were hidden.
#[tauri::command]
async fn remove_history_import(pool: State<'_, db::Pool>, import_id: String) -> CommandResult<u32> {
    db::remove_history_import(&pool, &import_id)
        .await
        .map_err(message)
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

// ---------- Study sessions ----------
// Planning and changing sessions that haven't started is done here, on the
// database. What happens to a running session goes through the daemon
// (`lockin_core::ipc`), so it can switch blocking off and on at once.

/// The sessions that start between `from` and `to` (RFC 3339), sorted by start.
#[tauri::command]
async fn sessions(
    pool: State<'_, db::Pool>,
    from: String,
    to: String,
) -> CommandResult<Vec<StudySession>> {
    session::between(&pool, parse_time(&from)?, parse_time(&to)?)
        .await
        .map_err(message)
}

/// The running session with where it stands and its log, for the Session page
/// and the sidebar.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentSession {
    session: StudySession,
    state: SessionState,
    events: Vec<SessionEvent>,
    /// The break rules (`session.rs`), so the page shows "5 min per 30 min".
    credit_minutes: i64,
    credit_every_minutes: i64,
    break_minutes: i64,
    /// The emergency button: hold this long, costs these points (step 6 charges them).
    emergency_wait_seconds: i64,
    emergency_penalty_points: i64,
}

/// The running session, or null.
#[tauri::command]
async fn current_session(pool: State<'_, db::Pool>) -> CommandResult<Option<CurrentSession>> {
    let Some(running) = session::running(&pool).await.map_err(message)? else {
        return Ok(None);
    };
    let events = session::events(&pool, &running.id).await.map_err(message)?;
    let state = session::state(&running, &events, Utc::now()).map_err(message)?;
    Ok(Some(CurrentSession {
        session: running,
        state,
        events,
        credit_minutes: session::CREDIT_MINUTES,
        credit_every_minutes: session::CREDIT_EVERY_MINUTES,
        break_minutes: session::BREAK_MINUTES,
        emergency_wait_seconds: session::EMERGENCY_WAIT_SECONDS,
        emergency_penalty_points: session::EMERGENCY_PENALTY_POINTS,
    }))
}

/// Plan one session; course, title and block profile are optional. Returns its id.
#[tauri::command]
async fn plan_session(
    pool: State<'_, db::Pool>,
    course_id: Option<String>,
    title: Option<String>,
    profile_id: Option<String>,
    start: String,
    end: String,
) -> CommandResult<String> {
    let (start, end) = (parse_time(&start)?, parse_time(&end)?);
    let details = SessionDetails {
        course_id,
        title,
        profile_id,
    };
    session::plan(&pool, &details, start, end, Utc::now())
        .await
        .map_err(message)
}

/// Plan a session every week from the first one (`start` to `end`), on the
/// same weekday and local time, until `until` (YYYY-MM-DD) or the end of the semester.
#[tauri::command]
async fn plan_weekly(
    pool: State<'_, db::Pool>,
    course_id: Option<String>,
    title: Option<String>,
    profile_id: Option<String>,
    start: String,
    end: String,
    until: Option<String>,
) -> CommandResult<PlanResult> {
    let start = parse_time(&start)?.with_timezone(&Local);
    let end = parse_time(&end)?.with_timezone(&Local);
    let until = match until {
        Some(text) => Some(parse_date(&text)?),
        None => None,
    };
    let details = SessionDetails {
        course_id,
        title,
        profile_id,
    };
    session::plan_weekly(
        &pool,
        &details,
        start.date_naive(),
        start.time(),
        end.time(),
        until,
        Utc::now(),
    )
    .await
    .map_err(message)
}

/// Change a session that hasn't started yet.
#[tauri::command]
async fn update_session(
    pool: State<'_, db::Pool>,
    session_id: String,
    course_id: Option<String>,
    title: Option<String>,
    profile_id: Option<String>,
    start: String,
    end: String,
) -> CommandResult<()> {
    let (start, end) = (parse_time(&start)?, parse_time(&end)?);
    let details = SessionDetails {
        course_id,
        title,
        profile_id,
    };
    session::update(&pool, &session_id, &details, start, end, Utc::now())
        .await
        .map_err(message)
}

/// The profile of the last planned session, for the next one's default.
#[tauri::command]
async fn last_profile(pool: State<'_, db::Pool>) -> CommandResult<Option<String>> {
    session::last_profile(&pool).await.map_err(message)
}

// ---------- Block profiles ----------

#[tauri::command]
async fn block_profiles(pool: State<'_, db::Pool>) -> CommandResult<Vec<BlockProfile>> {
    block::profiles(&pool).await.map_err(message)
}

/// Make a profile; returns its id.
#[tauri::command]
async fn add_block_profile(pool: State<'_, db::Pool>, name: String) -> CommandResult<String> {
    block::add_profile(&pool, &name).await.map_err(message)
}

#[tauri::command]
async fn rename_block_profile(
    pool: State<'_, db::Pool>,
    profile_id: String,
    name: String,
) -> CommandResult<()> {
    block::rename_profile(&pool, &profile_id, &name)
        .await
        .map_err(message)
}

/// How many planned sessions use a profile, to ask before deleting it.
#[tauri::command]
async fn sessions_using_profile(
    pool: State<'_, db::Pool>,
    profile_id: String,
) -> CommandResult<u32> {
    block::planned_sessions_using(&pool, &profile_id)
        .await
        .map_err(message)
}

/// Delete a profile; returns how many planned sessions now block nothing.
#[tauri::command]
async fn remove_block_profile(pool: State<'_, db::Pool>, profile_id: String) -> CommandResult<u32> {
    block::remove_profile(&pool, &profile_id)
        .await
        .map_err(message)
}

/// The apps in the menu, for the picker. Reading the desktop files takes a
/// moment, so it runs off the async threads.
#[tauri::command]
async fn installed_apps() -> CommandResult<Vec<InstalledApp>> {
    tauri::async_runtime::spawn_blocking(apps::installed_apps)
        .await
        .map_err(message)
}

#[tauri::command]
async fn add_blocked_app(
    pool: State<'_, db::Pool>,
    profile_id: String,
    app: InstalledApp,
) -> CommandResult<String> {
    block::add_app(&pool, &profile_id, &app)
        .await
        .map_err(message)
}

/// A program without a menu entry, by its process name.
#[tauri::command]
async fn add_blocked_program(
    pool: State<'_, db::Pool>,
    profile_id: String,
    program: String,
) -> CommandResult<String> {
    block::add_program(&pool, &profile_id, &program)
        .await
        .map_err(message)
}

#[tauri::command]
async fn remove_blocked_app(pool: State<'_, db::Pool>, entry_id: String) -> CommandResult<()> {
    block::remove_app(&pool, &entry_id).await.map_err(message)
}

/// How often each app was closed during a session.
#[tauri::command]
async fn session_closes(
    pool: State<'_, db::Pool>,
    session_id: String,
) -> CommandResult<Vec<AppCloses>> {
    block::closes(&pool, &session_id).await.map_err(message)
}

/// Remove a session that hasn't started yet; `restore_session` undoes it.
#[tauri::command]
async fn remove_session(pool: State<'_, db::Pool>, session_id: String) -> CommandResult<()> {
    session::remove(&pool, &session_id, Utc::now())
        .await
        .map_err(message)
}

#[tauri::command]
async fn restore_session(pool: State<'_, db::Pool>, session_id: String) -> CommandResult<()> {
    session::restore(&pool, &session_id, Utc::now())
        .await
        .map_err(message)
}

/// Remove this session and the later weeks of its repeat. Returns how many.
#[tauri::command]
async fn remove_following_sessions(
    pool: State<'_, db::Pool>,
    session_id: String,
) -> CommandResult<u32> {
    session::remove_following(&pool, &session_id, Utc::now())
        .await
        .map_err(message)
}

/// The daemon's status, or null when it isn't running.
#[tauri::command]
async fn daemon_status() -> CommandResult<Option<DaemonStatus>> {
    match ipc::send(&Request::Status).await {
        Ok(response) => Ok(response.status),
        Err(error) if error.to_string() == ipc::NOT_RUNNING => Ok(None),
        Err(error) => Err(message(error)),
    }
}

/// Ask the daemon to start a break in the running session.
#[tauri::command]
async fn take_break() -> CommandResult<()> {
    ipc::send(&Request::TakeBreak)
        .await
        .map(|_| ())
        .map_err(message)
}

/// Ask the daemon to end the break.
#[tauri::command]
async fn resume_session() -> CommandResult<()> {
    ipc::send(&Request::Resume)
        .await
        .map(|_| ())
        .map_err(message)
}

/// Ask the daemon to end the running session now (the emergency button).
#[tauri::command]
async fn emergency_stop() -> CommandResult<()> {
    ipc::send(&Request::EmergencyStop)
        .await
        .map(|_| ())
        .map_err(message)
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
            history_imports,
            remove_history_import,
            add_rule,
            remove_rule,
            sessions,
            current_session,
            plan_session,
            plan_weekly,
            update_session,
            remove_session,
            restore_session,
            remove_following_sessions,
            daemon_status,
            take_break,
            resume_session,
            emergency_stop,
            last_profile,
            block_profiles,
            add_block_profile,
            rename_block_profile,
            sessions_using_profile,
            remove_block_profile,
            installed_apps,
            add_blocked_app,
            add_blocked_program,
            remove_blocked_app,
            session_closes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Lockin app");
}
