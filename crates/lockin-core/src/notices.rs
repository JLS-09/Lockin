//! What the daemon tells you: the notifications that are due, and the lines of
//! the tray menu. The daemon only shows them (`lockin-daemon/src/desktop.rs`).
//!
//! Every notification has a key that names what it's about, e.g.
//! "session-soon:<id>:<start>". Once sent, the key goes into
//! `notification_sent`, so nothing is said twice, also not after a restart.
//! Everything is worked out from the database at `now`, so the tests can run
//! on any moment.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::SqlitePool;

use crate::i18n::Texts;
use crate::model::{SessionEvent, StudySession};
use crate::session::{self, BREAK_MINUTES, EMERGENCY_PENALTY_POINTS};
use crate::{block, db, deadline, settings};

/// "Study session in 5 min": this long before a session starts.
pub const SOON_MINUTES: i64 = 5;
/// Deadlines are announced once they're due within this many hours.
pub const DEADLINE_HOURS: i64 = 24;
/// A start, end or break that happened longer ago than this isn't announced
/// any more (the daemon wasn't running then, or the PC was asleep).
pub const FRESH_SECONDS: i64 = 10 * 60;

/// Which switch in Settings a notification falls under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Sessions,
    Breaks,
    Deadlines,
    ClosedApps,
}

impl Kind {
    /// The setting that turns this kind on or off.
    pub fn setting(self) -> &'static str {
        match self {
            Kind::Sessions => settings::NOTIFY_SESSIONS,
            Kind::Breaks => settings::NOTIFY_BREAKS,
            Kind::Deadlines => settings::NOTIFY_DEADLINES,
            Kind::ClosedApps => settings::NOTIFY_CLOSED_APPS,
        }
    }
}

/// One notification to show.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    /// What it's about, so it's sent once (see the module comment).
    pub key: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
}

/// The notifications that are due at `now`, not sent before, and whose kind
/// is switched on. Call `mark_sent` for each one shown.
pub async fn due(pool: &SqlitePool, texts: &Texts, now: DateTime<Utc>) -> Result<Vec<Notice>> {
    let mut found = Vec::new();
    session_notices(pool, texts, now, &mut found).await?;
    deadline_notices(pool, texts, now, &mut found).await?;
    closed_app_notices(pool, texts, now, &mut found).await?;

    let mut due = Vec::new();
    for notice in found {
        if settings::is_on(pool, notice.kind.setting()).await?
            && !was_sent(pool, &notice.key).await?
        {
            due.push(notice);
        }
    }
    Ok(due)
}

/// Write down that a notification was shown (or skipped on purpose).
pub async fn mark_sent(pool: &SqlitePool, key: &str, now: DateTime<Utc>) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO notification_sent (key, sent_at) VALUES (?, ?)")
        .bind(key)
        .bind(now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .execute(pool)
        .await?;
    Ok(())
}

async fn was_sent(pool: &SqlitePool, key: &str) -> Result<bool> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT key FROM notification_sent WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

fn parse(text: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(text)?.with_timezone(&Utc))
}

/// True when `at` lies in the last `FRESH_SECONDS` before `now`.
fn fresh(at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    at <= now && now - at <= Duration::seconds(FRESH_SECONDS)
}

/// What a session is: its title, else its course, else "Study session".
async fn session_name(pool: &SqlitePool, texts: &Texts, item: &StudySession) -> Result<String> {
    if let Some(title) = &item.title {
        return Ok(title.clone());
    }
    if let Some(course_id) = &item.course_id
        && let Some(course) = db::courses(pool)
            .await?
            .into_iter()
            .find(|course| &course.id == course_id)
    {
        return Ok(course.name);
    }
    Ok(texts.t("notify.session", &[]))
}

/// "blocks Strict", or None without a profile.
async fn blocking_line(
    pool: &SqlitePool,
    texts: &Texts,
    item: &StudySession,
) -> Result<Option<String>> {
    let Some(profile_id) = &item.profile_id else {
        return Ok(None);
    };
    let name = block::profiles(pool)
        .await?
        .into_iter()
        .find(|profile| &profile.id == profile_id)
        .map(|profile| profile.name);
    Ok(name.map(|name| texts.t("notify.blocking", &[("profile", &name)])))
}

/// The first event of this kind in the log.
fn first<'a>(events: &'a [SessionEvent], kind: &str) -> Option<&'a SessionEvent> {
    events.iter().find(|event| event.kind == kind)
}

async fn session_notices(
    pool: &SqlitePool,
    texts: &Texts,
    now: DateTime<Utc>,
    found: &mut Vec<Notice>,
) -> Result<()> {
    // Sessions can last at most a day, so this covers every running one too.
    let sessions = session::between(
        pool,
        now - Duration::days(1),
        now + Duration::minutes(SOON_MINUTES) + Duration::seconds(1),
    )
    .await?;

    for item in sessions {
        let start = parse(&item.start_at)?;
        let end = parse(&item.end_at)?;
        let name = session_name(pool, texts, &item).await?;
        let times = format!("{}–{}", texts.clock(start), texts.clock(end));
        let blocking = blocking_line(pool, texts, &item).await?;

        // 5 minutes before.
        if item.status == "planned" && start > now && start - now <= Duration::minutes(SOON_MINUTES)
        {
            let minutes = ((start - now).num_seconds() + 59) / 60;
            let mut body = vec![name.clone(), times.clone()];
            body.extend(blocking.clone());
            found.push(Notice {
                key: format!("session-soon:{}:{}", item.id, item.start_at),
                kind: Kind::Sessions,
                title: texts.tn("notify.sessionSoon", minutes, &[]),
                body: body.join(" · "),
            });
        }

        let events = session::events(pool, &item.id).await?;

        // Started (late, when the PC was off at its start).
        if let Some(started) = first(&events, "start")
            && fresh(parse(&started.at)?, now)
        {
            let title = if started.detail.as_deref() == Some("late") {
                texts.t("notify.sessionStartedLate", &[])
            } else {
                texts.t("notify.sessionStarted", &[])
            };
            let mut body = vec![
                name.clone(),
                texts.t("notify.until", &[("time", &texts.clock(end))]),
            ];
            body.extend(blocking.clone());
            found.push(Notice {
                key: format!("session-start:{}", item.id),
                kind: Kind::Sessions,
                title,
                body: body.join(" · "),
            });
        }

        // Done, or stopped with the emergency button.
        if let Some(ended) = events
            .iter()
            .find(|event| event.kind == "end" || event.kind == "emergency")
            && fresh(parse(&ended.at)?, now)
        {
            let (title, line) = if ended.kind == "emergency" {
                let points = EMERGENCY_PENALTY_POINTS.to_string();
                (
                    texts.t("notify.sessionStopped", &[]),
                    texts.t("notify.penalty", &[("points", &points)]),
                )
            } else {
                let worked = session::state(&item, &events, now)?.worked_seconds;
                (
                    texts.t("notify.sessionDone", &[]),
                    texts.t("notify.studied", &[("duration", &texts.duration(worked))]),
                )
            };
            found.push(Notice {
                key: format!("session-end:{}", item.id),
                kind: Kind::Sessions,
                title,
                body: format!("{name} · {line}"),
            });
        }

        // A break that ran out by itself (not one you ended).
        for event in &events {
            if event.kind == "resume"
                && event.detail.as_deref() == Some("auto")
                && item.status == "running"
                && fresh(parse(&event.at)?, now)
            {
                found.push(Notice {
                    key: format!("break-over:{}", event.id),
                    kind: Kind::Breaks,
                    title: texts.t("notify.breakOver", &[]),
                    body: texts.t("notify.backToWork", &[("time", &texts.clock(end))]),
                });
            }
        }

        // New break credit while studying: once per 5 minutes earned.
        if item.status == "running" {
            let state = session::state(&item, &events, now)?;
            if state.phase == "working"
                && state.credit_earned_seconds > 0
                && state.credit_left_seconds > 0
            {
                let minutes = (state.credit_left_seconds / 60).to_string();
                found.push(Notice {
                    key: format!("break-earned:{}:{}", item.id, state.credit_earned_seconds),
                    kind: Kind::Breaks,
                    title: texts.t("notify.breakEarned", &[]),
                    body: texts.t("notify.breakCredit", &[("minutes", &minutes)]),
                });
            }
        }
    }
    Ok(())
}

async fn deadline_notices(
    pool: &SqlitePool,
    texts: &Texts,
    now: DateTime<Utc>,
    found: &mut Vec<Notice>,
) -> Result<()> {
    let courses = db::courses(pool).await?;
    for item in deadline::between(pool, now, now + Duration::hours(DEADLINE_HOURS)).await? {
        if item.done || item.status == "cancelled" {
            continue;
        }
        let due = parse(&item.due_at)?;
        let mut body = item.title.clone();
        if let Some(course) = courses
            .iter()
            .find(|c| Some(&c.id) == item.course_id.as_ref())
        {
            body = format!("{body} · {}", course.name);
        }
        found.push(Notice {
            key: format!("deadline-soon:{}:{}", item.id, item.due_at),
            kind: Kind::Deadlines,
            title: texts.t("notify.deadline", &[("when", &texts.when(due, now))]),
            body,
        });
    }
    Ok(())
}

async fn closed_app_notices(
    pool: &SqlitePool,
    texts: &Texts,
    now: DateTime<Utc>,
    found: &mut Vec<Notice>,
) -> Result<()> {
    let Some(running) = session::running(pool).await? else {
        return Ok(());
    };
    let closes: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT app_id, name, at FROM app_closed
         WHERE session_id = ? AND deleted = 0 ORDER BY at",
    )
    .bind(&running.id)
    .fetch_all(pool)
    .await?;
    for (app_id, name, at) in closes {
        if fresh(parse(&at)?, now) {
            found.push(Notice {
                key: format!("app-closed:{}:{app_id}", running.id),
                kind: Kind::ClosedApps,
                title: texts.t("notify.appClosed", &[("app", &name)]),
                body: texts.t("notify.appClosedBody", &[]),
            });
        }
    }
    Ok(())
}

/// What the tray shows and offers right now.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrayStatus {
    /// "Studying · 34 min left", "On a break until 13:45", "Next session tomorrow 13:00".
    pub headline: String,
    /// While a session runs: "Break credit: 10 min".
    pub credit: Option<String>,
    /// A session is running (studying or on a break).
    pub running: bool,
    pub on_break: bool,
    /// "Take a 5-min break" when you can take one now.
    pub take_break: Option<String>,
}

pub async fn tray_status(
    pool: &SqlitePool,
    texts: &Texts,
    now: DateTime<Utc>,
) -> Result<TrayStatus> {
    let Some(running) = session::running(pool).await? else {
        let headline = match session::next(pool, now).await? {
            Some(next) => texts.t(
                "tray.next",
                &[("when", &texts.when(parse(&next.start_at)?, now))],
            ),
            None => texts.t("tray.noSession", &[]),
        };
        return Ok(TrayStatus {
            headline,
            ..Default::default()
        });
    };

    let state = session::state_now(pool, &running, now).await?;
    let on_break = state.phase == "break";
    let headline = match &state.break_ends_at {
        Some(ends) if on_break => texts.t("tray.onBreak", &[("time", &texts.clock(parse(ends)?))]),
        _ => {
            let left = (parse(&running.end_at)? - now).num_seconds();
            texts.t("tray.studying", &[("left", &texts.duration(left))])
        }
    };
    let credit_minutes = state.credit_left_seconds / 60;
    let take_break = state.can_take_break.then(|| {
        let minutes = credit_minutes.clamp(1, BREAK_MINUTES).to_string();
        texts.t("tray.takeBreak", &[("minutes", &minutes)])
    });
    Ok(TrayStatus {
        headline,
        credit: Some(texts.t("tray.credit", &[("minutes", &credit_minutes.to_string())])),
        running: true,
        on_break,
        take_break,
    })
}
