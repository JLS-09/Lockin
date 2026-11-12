//! The data types used across Lockin.

use chrono::{DateTime, Utc};
use serde::Serialize;

/// One event read from an iCal feed, before it goes into the database.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedEvent {
    pub uid: String,
    pub title: String,
    pub location: Option<String>,
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// A class block as stored in the database.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassBlock {
    pub id: String,
    pub ical_uid: Option<String>,
    pub course_id: Option<String>,
    pub kind: Option<String>,
    pub title: String,
    pub location: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub ignored_by_rule: bool,
    pub ignored_override: Option<bool>,
    pub status: String,
    /// Start time before the block was first moved, for "Moved · was Fri 13:00".
    pub moved_from: Option<String>,
    /// True once you've dismissed this change in the week view.
    pub change_seen: bool,
}

impl ClassBlock {
    /// A manual exception wins; otherwise the filter rules decide.
    pub fn is_ignored(&self) -> bool {
        self.ignored_override.unwrap_or(self.ignored_by_rule)
    }
}

/// A rule like "ignore everything with 'zelfstandig werk' in the title".
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRule {
    pub id: String,
    pub field: String,
    pub pattern: String,
}

/// A course, e.g. "Signalen en systemen" with code E702010A.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub id: String,
    pub code: Option<String>,
    pub name: String,
    /// Hex color, e.g. "#6b9bd1": class blocks, dots and charts of this course.
    pub color: Option<String>,
}

/// A saved iCal feed: TimeEdit (classes) or Ufora (deadlines).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CalendarSource {
    pub id: String,
    pub kind: String,
    pub url: String,
    pub last_fetched_at: Option<String>,
}

/// A saved feed as the app shows it: everything except the URL, which holds a
/// personal key and never leaves the Rust side.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedInfo {
    pub id: String,
    /// "timeedit" or "ufora".
    pub kind: String,
    /// Where it comes from without the key: "cloud.timeedit.net", or the file name.
    pub origin: String,
    pub last_fetched_at: Option<String>,
    /// How many classes this feed holds now (TimeEdit).
    pub classes: i64,
    /// How many deadlines this feed holds now (Ufora).
    pub deadlines: i64,
}

/// Earlier weeks imported from a downloaded TimeEdit file, as Settings lists them.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryImport {
    pub id: String,
    /// The file name; None for imports from before Lockin remembered them.
    pub name: Option<String>,
    pub imported_at: String,
    /// Classes it still holds (not taken over by the live feed).
    pub classes: i64,
    /// Start of its first and last class, for "26 Sep – 27 Oct".
    pub first_start: Option<String>,
    pub last_start: Option<String>,
}

/// The result of syncing one feed, for the app: what changed, or why it failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedSync {
    pub source_id: String,
    /// "timeedit" (the counts are classes) or "ufora" (deadlines).
    pub kind: String,
    pub added: u32,
    pub updated: u32,
    /// Classes whose time changed.
    pub moved: u32,
    pub cancelled: u32,
    /// Set when this feed couldn't be fetched; the others still sync.
    pub error: Option<String>,
}

/// Something to check off, e.g. "Do the exercises".
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    /// The class block it belongs to; None for an activity on its own.
    pub block_id: Option<String>,
    /// The template that created it; None if it was added by hand.
    pub template_id: Option<String>,
    pub course_id: Option<String>,
    pub title: String,
    /// Monday of the week it belongs to, e.g. "2026-11-09".
    pub week: String,
    pub done: bool,
    pub done_at: Option<String>,
}

/// "After every werkcollege of this course: do the exercises."
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTemplate {
    pub id: String,
    pub course_id: String,
    /// The kind of class it applies to; None = every block of the course.
    pub kind: Option<String>,
    pub title: String,
}

/// How many activities of one course in one week are done.
/// One cell of the semester overview.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekProgress {
    /// Monday of the week, e.g. "2026-11-09".
    pub week: String,
    pub course_id: String,
    pub total: i64,
    pub done: i64,
    /// Not done, on a class that has already ended.
    pub overdue: i64,
}

/// One kind of class a course has, e.g. Signalen en systemen has "werkcollege".
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseKind {
    pub course_id: String,
    pub kind: String,
}

/// How many classes one course has in one week.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekClasses {
    /// Monday of the week, e.g. "2026-11-09".
    pub week: String,
    pub course_id: String,
    pub classes: i64,
}

/// Something due at a set time: added by hand, or from the Ufora feed.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deadline {
    pub id: String,
    /// The feed it came from; None if it was added by hand.
    pub source_id: Option<String>,
    pub course_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    /// When it's due, RFC 3339 in UTC.
    pub due_at: String,
    pub done: bool,
    pub done_at: Option<String>,
    /// "normal", "moved" or "cancelled" (the last two only for feed deadlines).
    pub status: String,
    /// Due time before its first move, for "Moved · was Fri 23:59".
    pub moved_from: Option<String>,
    /// True once you've dismissed the change.
    pub change_seen: bool,
    /// Feed deadlines: where to open it in Ufora.
    pub url: Option<String>,
}

/// One line of the Urgent list: an open activity or a deadline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrgentItem {
    /// "activity" or "deadline".
    pub kind: String,
    /// The id of the activity or the deadline.
    pub id: String,
    pub title: String,
    pub course_id: Option<String>,
    /// Activities: the class block it belongs to, if any.
    pub block_id: Option<String>,
    /// Activities: Monday of its week, to open the week view there.
    pub week: Option<String>,
    /// Activities: since when it's open (the end of its class, or the
    /// Monday of its week). RFC 3339 in UTC.
    pub open_since: Option<String>,
    /// Deadlines: when it's due. RFC 3339 in UTC.
    pub due_at: Option<String>,
    /// Deadlines: None if added by hand, else the feed it came from.
    pub source_id: Option<String>,
    /// Feed deadlines: where to open it in Ufora.
    pub url: Option<String>,
    /// Feed deadlines that moved: the due time before the first move.
    pub moved_from: Option<String>,
    /// "behind", "approaching" or "planned": the status icon and color.
    pub status: String,
    /// Higher = more urgent; the list is sorted on it. See `urgent::score`.
    pub score: f64,
}

/// A planned block of study time. The daemon starts and ends it on time.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySession {
    pub id: String,
    /// The weekly repeat it belongs to; None if it was planned once.
    pub series_id: Option<String>,
    pub course_id: Option<String>,
    /// What you plan to do, e.g. "Exercises WPO 4"; None for a plain session.
    pub title: Option<String>,
    /// RFC 3339 in UTC.
    pub start_at: String,
    pub end_at: String,
    /// "planned", "running", "done", "stopped", "missed" or "skipped".
    pub status: String,
    /// The block profile it closes apps with; None = blocks nothing.
    pub profile_id: Option<String>,
}

/// What a session is about, apart from its times: given when planning or changing one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionDetails {
    pub course_id: Option<String>,
    pub title: Option<String>,
    pub profile_id: Option<String>,
}

/// One line of a session's log: start, break, resume, emergency or end.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub id: String,
    pub session_id: String,
    /// "start", "break", "resume", "emergency" or "end".
    pub kind: String,
    /// RFC 3339 in UTC.
    pub at: String,
    /// Extra information, e.g. "daemon" on a start or "auto" on a resume.
    pub detail: Option<String>,
}

/// Where a session stands at a moment, worked out from its log (`session::state`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// "planned" (not started), "working", "break" or "over".
    pub phase: String,
    /// Time studied so far, breaks not counted.
    pub worked_seconds: i64,
    /// Break credit built up by studying.
    pub credit_earned_seconds: i64,
    /// Break credit already spent on breaks.
    pub credit_used_seconds: i64,
    /// What's left to spend: earned minus used.
    pub credit_left_seconds: i64,
    /// During a break: when blocking turns back on. RFC 3339 in UTC.
    pub break_ends_at: Option<String>,
    /// True while working with some credit left.
    pub can_take_break: bool,
}

/// What planning a weekly repeat made.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanResult {
    pub series_id: String,
    /// Sessions made.
    pub planned: u32,
    /// Weeks left out because they overlap another session.
    pub overlapping: u32,
}

/// What one tick of the daemon changed (`session::tick`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickResult {
    /// Sessions that started now.
    pub started: Vec<String>,
    /// Sessions that reached their end.
    pub ended: Vec<String>,
    /// Sessions whose break ran out, so blocking is back on.
    pub resumed: Vec<String>,
    /// Sessions whose start passed while the daemon wasn't running.
    pub missed: Vec<String>,
}

/// An app on a block profile. `exec_names` holds one program name per line.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedApp {
    pub id: String,
    pub profile_id: String,
    /// The desktop file id, or "program:<name>" for a program added by name.
    pub app_id: String,
    /// As shown, e.g. "Discord".
    pub name: String,
    pub exec_names: String,
    pub flatpak_id: Option<String>,
}

/// A block profile with its apps, e.g. "Strict": Discord, Steam, Spotify.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockProfile {
    pub id: String,
    pub name: String,
    pub apps: Vec<BlockedApp>,
    /// True while the running session blocks with it: then apps can be added, not removed.
    pub in_use: bool,
}

/// How often the daemon closed one app during a session.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCloses {
    pub app_id: String,
    pub name: String,
    pub count: i64,
    /// The last time, RFC 3339 in UTC.
    pub last_at: String,
}
