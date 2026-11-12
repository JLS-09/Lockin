//! Study sessions: planned blocks of study time, like "Wed 13:00–15:30, Databanken".
//!
//! Planning (once or every week) and changing sessions happens here, and so do
//! the things the daemon does while one runs: start it, take a break, resume,
//! end it, or stop it with the emergency button. Every one of those is written to
//! the session's log (`session_event`), and `state` works out from that log where
//! a session stands: how long you studied and how much break credit is left.
//!
//! Functions that depend on the current time take `now`, so the tests can run at
//! any moment.

use anyhow::{Result, bail};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use sqlx::sqlite::SqlitePool;

use crate::model::{
    PlanResult, SessionDetails, SessionEvent, SessionState, StudySession, TickResult,
};
use crate::{new_id, now_text};

/// Every this many minutes studied...
pub const CREDIT_EVERY_MINUTES: i64 = 30;
/// ...earns this many minutes of break credit.
pub const CREDIT_MINUTES: i64 = 5;
/// One break lasts at most this long (less when there's less credit left).
pub const BREAK_MINUTES: i64 = 5;
/// The emergency button has to be held open this long before the session ends.
/// The daemon enforces the wait (step 6); this is the number it uses.
pub const EMERGENCY_WAIT_SECONDS: i64 = 120;
/// Penalty points for the emergency button (recorded from step 6 on).
pub const EMERGENCY_PENALTY_POINTS: i64 = 50;
/// A session the daemon starts more than this long after its planned start is
/// logged as a late start ("late"): the daemon wasn't running at its start.
pub const START_GRACE_SECONDS: i64 = 60;
/// The longest session you can plan.
pub const MAX_SESSION_HOURS: i64 = 12;

/// The columns every session query reads, in the order of `StudySession`.
const COLUMNS: &str = "id, series_id, course_id, title, start_at, end_at, status, profile_id";

/// RFC 3339 text in UTC with seconds, the format every time is stored in.
fn time_text(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Read a stored time back.
fn parse_time(text: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(text)?.with_timezone(&Utc))
}

/// A local date and time (as on the clock in Gent) as a UTC time.
pub fn local_time(date: NaiveDate, time: NaiveTime) -> Result<DateTime<Utc>> {
    // `earliest` picks the first one when the clock goes back in autumn.
    let Some(local) = Local.from_local_datetime(&date.and_time(time)).earliest() else {
        bail!("{date} {time} doesn't exist in local time (the clock skips it)");
    };
    Ok(local.with_timezone(&Utc))
}

/// The last day of the semester `date` falls in: 31 January for semester 1
/// (August to January), 31 July for semester 2 (February to July).
/// The same split the semester view uses; UGent's official dates aren't known.
pub fn semester_end(date: NaiveDate) -> NaiveDate {
    let (year, month) = match date.month() {
        8..=12 => (date.year() + 1, 1),
        1 => (date.year(), 1),
        _ => (date.year(), 7),
    };
    // January and July both have 31 days.
    NaiveDate::from_ymd_opt(year, month, 31).unwrap()
}

/// Empty titles become None, others lose the spaces around them.
fn clean_title(title: Option<&str>) -> Option<String> {
    title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

/// Check that a session has a sensible length and lies in the future.
fn check_times(start: DateTime<Utc>, end: DateTime<Utc>, now: DateTime<Utc>) -> Result<()> {
    if end <= start {
        bail!("a study session has to end after it starts");
    }
    if end - start > Duration::hours(MAX_SESSION_HOURS) {
        bail!("a study session can last at most {MAX_SESSION_HOURS} hours");
    }
    if start <= now {
        bail!("a study session has to start in the future");
    }
    Ok(())
}

/// The planned or running session that overlaps this time, if any.
/// `except` leaves one session out (the one being changed).
async fn overlapping(
    pool: &SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    except: Option<&str>,
) -> Result<Option<StudySession>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND status IN ('planned', 'running')
           AND start_at < ? AND end_at > ? AND id != ?
         ORDER BY start_at LIMIT 1"
    );
    let session = sqlx::query_as(&sql)
        .bind(time_text(end))
        .bind(time_text(start))
        .bind(except.unwrap_or(""))
        .fetch_optional(pool)
        .await?;
    Ok(session)
}

/// "overlaps the study session of Wed 11 Nov 13:00–15:30"
fn overlap_message(other: &StudySession) -> String {
    let start = parse_time(&other.start_at).map(|t| t.with_timezone(&Local));
    let end = parse_time(&other.end_at).map(|t| t.with_timezone(&Local));
    match (start, end) {
        (Ok(start), Ok(end)) => format!(
            "overlaps the study session of {}–{}",
            start.format("%a %d %b %H:%M"),
            end.format("%H:%M")
        ),
        _ => "overlaps another study session".to_string(),
    }
}

/// Plan one session. Course, title and block profile are all optional. Returns its id.
pub async fn plan(
    pool: &SqlitePool,
    details: &SessionDetails,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<String> {
    check_times(start, end, now)?;
    if let Some(other) = overlapping(pool, start, end, None).await? {
        bail!("This {}", overlap_message(&other));
    }
    let id = new_id();
    insert(pool, &id, None, details, start, end).await?;
    Ok(id)
}

async fn insert(
    pool: &SqlitePool,
    id: &str,
    series_id: Option<&str>,
    details: &SessionDetails,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO study_session
           (id, series_id, course_id, title, profile_id, start_at, end_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(series_id)
    .bind(&details.course_id)
    .bind(clean_title(details.title.as_deref()))
    .bind(&details.profile_id)
    .bind(time_text(start))
    .bind(time_text(end))
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// Plan a session every week on the weekday of `first_date`, from `start_time`
/// to `end_time` (local time, so it stays 13:00 when the clock changes), up to
/// and including `until` (by default the end of the semester).
///
/// Weeks that overlap another session are left out and counted; the others are
/// still planned. Each week is its own session, so one can be changed alone.
#[allow(clippy::too_many_arguments)]
pub async fn plan_weekly(
    pool: &SqlitePool,
    details: &SessionDetails,
    first_date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    until: Option<NaiveDate>,
    now: DateTime<Utc>,
) -> Result<PlanResult> {
    if end_time <= start_time {
        bail!("a study session has to end after it starts");
    }
    let until = until.unwrap_or_else(|| semester_end(first_date));
    if until < first_date {
        bail!("the repeat has to end on or after the first session");
    }
    let first_start = local_time(first_date, start_time)?;
    check_times(first_start, local_time(first_date, end_time)?, now)?;

    let series_id = new_id();
    sqlx::query(
        "INSERT INTO session_series (id, course_id, title, profile_id, until_date, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&series_id)
    .bind(&details.course_id)
    .bind(clean_title(details.title.as_deref()))
    .bind(&details.profile_id)
    .bind(until.format("%Y-%m-%d").to_string())
    .bind(now_text())
    .execute(pool)
    .await?;

    let mut result = PlanResult {
        series_id: series_id.clone(),
        planned: 0,
        overlapping: 0,
    };
    let mut date = first_date;
    while date <= until {
        let start = local_time(date, start_time)?;
        let end = local_time(date, end_time)?;
        if overlapping(pool, start, end, None).await?.is_some() {
            result.overlapping += 1;
        } else {
            insert(pool, &new_id(), Some(&series_id), details, start, end).await?;
            result.planned += 1;
        }
        date += Duration::days(7);
    }

    if result.planned == 0 {
        sqlx::query("UPDATE session_series SET deleted = 1, updated_at = ? WHERE id = ?")
            .bind(now_text())
            .bind(&series_id)
            .execute(pool)
            .await?;
        bail!("every week overlaps another study session, so nothing was planned");
    }
    Ok(result)
}

/// One session by id (deleted ones too, so `restore` can check it).
pub async fn get(pool: &SqlitePool, session_id: &str) -> Result<StudySession> {
    let sql = format!("SELECT {COLUMNS} FROM study_session WHERE id = ?");
    let Some(session) = sqlx::query_as(&sql)
        .bind(session_id)
        .fetch_optional(pool)
        .await?
    else {
        bail!("no study session with id {session_id}");
    };
    Ok(session)
}

/// A session can only be changed or removed while it is still to come:
/// "a running study session can't be changed".
fn check_editable(session: &StudySession, now: DateTime<Utc>) -> Result<()> {
    if session.status != "planned" || parse_time(&session.start_at)? <= now {
        bail!("only study sessions that haven't started yet can be changed");
    }
    Ok(())
}

/// Change the course, title, profile and times of a session that hasn't started
/// yet. A session of a weekly repeat stays part of it.
pub async fn update(
    pool: &SqlitePool,
    session_id: &str,
    details: &SessionDetails,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<()> {
    check_editable(&get(pool, session_id).await?, now)?;
    check_times(start, end, now)?;
    if let Some(other) = overlapping(pool, start, end, Some(session_id)).await? {
        bail!("This {}", overlap_message(&other));
    }
    sqlx::query(
        "UPDATE study_session
         SET course_id = ?, title = ?, profile_id = ?, start_at = ?, end_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&details.course_id)
    .bind(clean_title(details.title.as_deref()))
    .bind(&details.profile_id)
    .bind(time_text(start))
    .bind(time_text(end))
    .bind(now_text())
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a session that hasn't started yet (soft delete, so `restore` can
/// bring it back). Removing a planned session costs nothing.
pub async fn remove(pool: &SqlitePool, session_id: &str, now: DateTime<Utc>) -> Result<()> {
    check_editable(&get(pool, session_id).await?, now)?;
    sqlx::query("UPDATE study_session SET deleted = 1, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Undo `remove`, unless the session has started by now or another one took its time.
pub async fn restore(pool: &SqlitePool, session_id: &str, now: DateTime<Utc>) -> Result<()> {
    let session = get(pool, session_id).await?;
    check_editable(&session, now)?;
    let start = parse_time(&session.start_at)?;
    let end = parse_time(&session.end_at)?;
    if let Some(other) = overlapping(pool, start, end, Some(session_id)).await? {
        bail!("Can't bring it back: it {}", overlap_message(&other));
    }
    sqlx::query("UPDATE study_session SET deleted = 0, updated_at = ? WHERE id = ?")
        .bind(now_text())
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remove this session and every later one of its weekly repeat that hasn't
/// started yet ("this and following weeks"). Returns how many were removed.
pub async fn remove_following(
    pool: &SqlitePool,
    session_id: &str,
    now: DateTime<Utc>,
) -> Result<u32> {
    let session = get(pool, session_id).await?;
    check_editable(&session, now)?;
    let Some(series_id) = session.series_id else {
        remove(pool, session_id, now).await?;
        return Ok(1);
    };
    let removed = sqlx::query(
        "UPDATE study_session SET deleted = 1, updated_at = ?
         WHERE series_id = ? AND deleted = 0 AND status = 'planned'
           AND start_at >= ? AND start_at > ?",
    )
    .bind(now_text())
    .bind(&series_id)
    .bind(&session.start_at)
    .bind(time_text(now))
    .execute(pool)
    .await?
    .rows_affected();

    // The repeat now ends the day before this session.
    let day_before = parse_time(&session.start_at)?
        .with_timezone(&Local)
        .date_naive()
        - Duration::days(1);
    sqlx::query("UPDATE session_series SET until_date = ?, updated_at = ? WHERE id = ?")
        .bind(day_before.format("%Y-%m-%d").to_string())
        .bind(now_text())
        .bind(&series_id)
        .execute(pool)
        .await?;
    Ok(removed as u32)
}

/// The sessions that start between `from` and `to`, sorted by start.
pub async fn between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<StudySession>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND start_at >= ? AND start_at < ?
         ORDER BY start_at"
    );
    let sessions = sqlx::query_as(&sql)
        .bind(time_text(from))
        .bind(time_text(to))
        .fetch_all(pool)
        .await?;
    Ok(sessions)
}

/// The block profile of the most recently planned or changed session, so a new
/// one can start with the same.
pub async fn last_profile(pool: &SqlitePool) -> Result<Option<String>> {
    let profile: Option<Option<String>> = sqlx::query_scalar(
        "SELECT s.profile_id FROM study_session s
         JOIN block_profile p ON p.id = s.profile_id AND p.deleted = 0
         WHERE s.deleted = 0 ORDER BY s.updated_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(profile.flatten())
}

/// The session that is running right now, if any.
pub async fn running(pool: &SqlitePool) -> Result<Option<StudySession>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND status = 'running' ORDER BY start_at LIMIT 1"
    );
    Ok(sqlx::query_as(&sql).fetch_optional(pool).await?)
}

/// The next session still to come after `now`, if any.
pub async fn next(pool: &SqlitePool, now: DateTime<Utc>) -> Result<Option<StudySession>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND status = 'planned' AND start_at > ?
         ORDER BY start_at LIMIT 1"
    );
    Ok(sqlx::query_as(&sql)
        .bind(time_text(now))
        .fetch_optional(pool)
        .await?)
}

/// A session's log, oldest first.
pub async fn events(pool: &SqlitePool, session_id: &str) -> Result<Vec<SessionEvent>> {
    let events = sqlx::query_as(
        "SELECT id, session_id, kind, at, detail FROM session_event
         WHERE session_id = ? AND deleted = 0
         ORDER BY at, rowid",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

/// Write one line to a session's log.
async fn log(
    pool: &SqlitePool,
    session_id: &str,
    kind: &str,
    at: DateTime<Utc>,
    detail: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO session_event (id, session_id, kind, at, detail, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(session_id)
    .bind(kind)
    .bind(time_text(at))
    .bind(detail)
    .bind(now_text())
    .execute(pool)
    .await?;
    Ok(())
}

/// Move a session from status `from` to `to`. Returns false when it wasn't in
/// `from` (any more): then someone else, like a second daemon, got there first.
async fn change_status(pool: &SqlitePool, session_id: &str, from: &str, to: &str) -> Result<bool> {
    let changed = sqlx::query(
        "UPDATE study_session SET status = ?, updated_at = ? WHERE id = ? AND status = ?",
    )
    .bind(to)
    .bind(now_text())
    .bind(session_id)
    .bind(from)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed == 1)
}

/// Break credit earned by studying this many seconds: 5 minutes for every
/// whole 30 minutes (see the constants at the top).
pub fn credit_for(worked_seconds: i64) -> i64 {
    (worked_seconds / (CREDIT_EVERY_MINUTES * 60)) * CREDIT_MINUTES * 60
}

/// Where a session stands at `now`, worked out from its log alone.
///
/// - Study time runs from the start to now (or the end), minus breaks.
/// - A break lasts until the "resume" in the log, but never longer than
///   `BREAK_MINUTES` or the credit that was left when it began: then it is over
///   by itself, even if the daemon hasn't written the "resume" yet.
/// - Credit used is the time actually spent on breaks, so a break you end
///   early leaves the rest as credit.
pub fn state(
    session: &StudySession,
    events: &[SessionEvent],
    now: DateTime<Utc>,
) -> Result<SessionState> {
    let mut result = SessionState {
        phase: "planned".to_string(),
        worked_seconds: 0,
        credit_earned_seconds: 0,
        credit_used_seconds: 0,
        credit_left_seconds: 0,
        break_ends_at: None,
        can_take_break: false,
    };

    // Seconds since the Unix epoch keep the arithmetic simple.
    let mut times = Vec::new();
    for event in events {
        times.push((event.kind.as_str(), parse_time(&event.at)?.timestamp()));
    }
    let Some(&(_, start)) = times.iter().find(|(kind, _)| *kind == "start") else {
        if session.status != "planned" {
            result.phase = "over".to_string();
        }
        return Ok(result);
    };

    // The session counts until now, its planned end, or an end in the log.
    let mut until = now
        .timestamp()
        .min(parse_time(&session.end_at)?.timestamp());
    if let Some(&(_, end)) = times
        .iter()
        .find(|(kind, _)| *kind == "end" || *kind == "emergency")
    {
        until = until.min(end);
    }
    let over = until < now.timestamp() || session.status != "running";

    let mut worked = 0;
    let mut used = 0;
    // Where study time was last counted up to.
    let mut counted_to = start;
    // During a break: (when it began, when it ends by itself).
    let mut on_break: Option<(i64, i64)> = None;

    for &(kind, at) in &times {
        if at <= start || at > until {
            continue;
        }
        // A break whose time ran out before this event is over by itself.
        if let Some((began, ends)) = on_break
            && ends <= at
        {
            used += ends - began;
            counted_to = ends;
            on_break = None;
        }
        match (kind, on_break) {
            ("break", None) => {
                worked += at - counted_to;
                let left = credit_for(worked) - used;
                let length = left.min(BREAK_MINUTES * 60);
                on_break = Some((at, at + length));
            }
            ("resume", Some((began, _))) => {
                used += at - began;
                counted_to = at;
                on_break = None;
            }
            _ => {}
        }
    }

    match on_break {
        Some((began, ends)) if ends <= until => {
            used += ends - began;
            worked += until - ends;
        }
        Some((began, ends)) => {
            used += until - began;
            result.break_ends_at = DateTime::from_timestamp(ends, 0).map(time_text);
        }
        None => worked += until - counted_to,
    }

    result.worked_seconds = worked;
    result.credit_earned_seconds = credit_for(worked);
    result.credit_used_seconds = used;
    result.credit_left_seconds = (result.credit_earned_seconds - used).max(0);
    result.phase = if over {
        "over"
    } else if result.break_ends_at.is_some() {
        "break"
    } else {
        "working"
    }
    .to_string();
    if over {
        result.break_ends_at = None;
    }
    result.can_take_break = result.phase == "working" && result.credit_left_seconds > 0;
    Ok(result)
}

/// The state of a session read from the database.
pub async fn state_now(
    pool: &SqlitePool,
    session: &StudySession,
    now: DateTime<Utc>,
) -> Result<SessionState> {
    state(session, &events(pool, &session.id).await?, now)
}

/// Start a planned session at `now`. `detail` says who started it ("daemon").
pub async fn start(
    pool: &SqlitePool,
    session_id: &str,
    now: DateTime<Utc>,
    detail: Option<&str>,
) -> Result<()> {
    // The status changes first, and only from "planned": a session never starts twice.
    if !change_status(pool, session_id, "planned", "running").await? {
        bail!("this study session isn't waiting to start");
    }
    log(pool, session_id, "start", now, detail).await
}

/// Start a break in the running session. It ends by itself after
/// `BREAK_MINUTES`, or sooner when the credit runs out.
pub async fn take_break(pool: &SqlitePool, session_id: &str, now: DateTime<Utc>) -> Result<()> {
    let session = get(pool, session_id).await?;
    let current = state_now(pool, &session, now).await?;
    if current.phase != "working" {
        bail!("a break can only start while you're studying");
    }
    if current.credit_left_seconds <= 0 {
        bail!("no break credit left yet");
    }
    log(pool, session_id, "break", now, None).await
}

/// End the current break: back to work, blocking on again.
/// `detail` is "auto" when the daemon ends it because its time ran out.
pub async fn resume(
    pool: &SqlitePool,
    session_id: &str,
    now: DateTime<Utc>,
    detail: Option<&str>,
) -> Result<()> {
    let session = get(pool, session_id).await?;
    if state_now(pool, &session, now).await?.phase != "break" {
        bail!("there's no break to end");
    }
    log(pool, session_id, "resume", now, detail).await
}

/// End a running session at its planned end (`at`): status "done".
pub async fn finish(pool: &SqlitePool, session_id: &str, at: DateTime<Utc>) -> Result<()> {
    if !change_status(pool, session_id, "running", "done").await? {
        bail!("this study session isn't running");
    }
    log(pool, session_id, "end", at, None).await
}

/// End a running session early with the emergency button: status "stopped".
/// The two-minute wait and the penalty points come with step 6.
pub async fn emergency_stop(pool: &SqlitePool, session_id: &str, now: DateTime<Utc>) -> Result<()> {
    if !change_status(pool, session_id, "running", "stopped").await? {
        bail!("this study session isn't running");
    }
    log(pool, session_id, "emergency", now, None).await
}

/// What the daemon does every few seconds:
/// 1. A running session past its end is finished (at its planned end).
/// 2. A break whose time ran out gets its "resume", so blocking comes back on.
/// 3. A planned session whose start has come is started. If the daemon wasn't
///    running at its start (the PC was off), it starts late, logged as "late":
///    no penalty, and study time (so points) only counts from now. One whose
///    end passed without it ever starting is "missed": no points, no penalty.
pub async fn tick(pool: &SqlitePool, now: DateTime<Utc>) -> Result<TickResult> {
    let mut result = TickResult::default();

    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND status = 'running' ORDER BY start_at"
    );
    let running: Vec<StudySession> = sqlx::query_as(&sql).fetch_all(pool).await?;
    for session in running {
        let end = parse_time(&session.end_at)?;
        if end <= now {
            finish(pool, &session.id, end).await?;
            result.ended.push(session.id);
            continue;
        }
        let events = events(pool, &session.id).await?;
        let on_break_in_log = events.last().is_some_and(|event| event.kind == "break");
        if on_break_in_log && state(&session, &events, now)?.phase == "working" {
            // Log the resume at the moment the break really ended.
            let ended_at = break_end(&session, &events, now)?.unwrap_or(now);
            log(pool, &session.id, "resume", ended_at, Some("auto")).await?;
            result.resumed.push(session.id);
        }
    }

    let sql = format!(
        "SELECT {COLUMNS} FROM study_session
         WHERE deleted = 0 AND status = 'planned' AND start_at <= ?
         ORDER BY start_at"
    );
    let due: Vec<StudySession> = sqlx::query_as(&sql)
        .bind(time_text(now))
        .fetch_all(pool)
        .await?;
    for session in due {
        let starts = parse_time(&session.start_at)?;
        let ends = parse_time(&session.end_at)?;
        if now >= ends {
            if change_status(pool, &session.id, "planned", "missed").await? {
                result.missed.push(session.id);
            }
        } else {
            let on_time = now - starts <= Duration::seconds(START_GRACE_SECONDS);
            let detail = if on_time { "daemon" } else { "late" };
            // Skip quietly if another process started it a moment ago.
            if start(pool, &session.id, now, Some(detail)).await.is_ok() {
                result.started.push(session.id);
            }
        }
    }
    Ok(result)
}

/// When the last break ended by itself: just before `now`, find the moment
/// the state switched from "break" to "working".
fn break_end(
    session: &StudySession,
    events: &[SessionEvent],
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>> {
    let Some(last) = events.last() else {
        return Ok(None);
    };
    let began = parse_time(&last.at)?;
    let before = state(session, events, began + Duration::seconds(1))?;
    // The break's own end time, as `state` saw it right after it began.
    let ends = match before.break_ends_at {
        Some(text) => parse_time(&text)?,
        None => began,
    };
    Ok(Some(ends.min(now)))
}

/// Find a session by the first characters of its id, as the `sessions` command shows them.
pub async fn find_id(pool: &SqlitePool, start_of_id: &str) -> Result<String> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM study_session WHERE deleted = 0 AND id LIKE ?")
            .bind(format!("{start_of_id}%"))
            .fetch_all(pool)
            .await?;
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => bail!("no study session with an id starting with {start_of_id}"),
        _ => bail!("more than one study session starts with {start_of_id}, type more characters"),
    }
}
