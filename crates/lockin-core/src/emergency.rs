//! The emergency stop, as the daemon runs it.
//!
//! Ending a session early is possible, but never quick: you open the stop and
//! keep it open for `session::EMERGENCY_WAIT_SECONDS` (2 minutes). Only then does
//! the daemon end the session, and it costs `session::EMERGENCY_PENALTY_POINTS`.
//!
//! "Keeping it open" means the app (or the CLI) checks in with the daemon every
//! couple of seconds (`EmergencyStop::keep_open`). When the check-ins stop for
//! `KEEP_OPEN_SECONDS` (the page was left, the app closed), the stop is dropped
//! and the next one starts again from 2:00. The wait lives in the daemon, so it
//! can't be skipped by changing the app.
//!
//! The daemon keeps this in memory only: if it restarts, an open stop is gone,
//! which makes stopping harder, never easier.

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::SqlitePool;

use crate::model::EmergencyStatus;
use crate::session::{self, EMERGENCY_PENALTY_POINTS, EMERGENCY_WAIT_SECONDS};

/// The app checks in every 2 seconds while the stop is open. Without a check-in
/// for this long, the stop counts as closed.
pub const KEEP_OPEN_SECONDS: i64 = 10;

/// One stop being held open.
#[derive(Debug, Clone, PartialEq)]
struct Hold {
    session_id: String,
    started_at: DateTime<Utc>,
    /// The last check-in.
    last_seen: DateTime<Utc>,
}

impl Hold {
    /// When the session ends if the stop stays open.
    fn stops_at(&self) -> DateTime<Utc> {
        self.started_at + Duration::seconds(EMERGENCY_WAIT_SECONDS)
    }

    /// True when nobody checked in for too long: the stop was closed.
    fn lapsed(&self, now: DateTime<Utc>) -> bool {
        now - self.last_seen > Duration::seconds(KEEP_OPEN_SECONDS)
    }

    fn status(&self, stopped: bool) -> EmergencyStatus {
        let text = |time: DateTime<Utc>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        EmergencyStatus {
            started_at: text(self.started_at),
            stops_at: text(self.stops_at()),
            stopped,
            penalty_points: EMERGENCY_PENALTY_POINTS,
        }
    }
}

/// What `check` did.
#[derive(Debug, Clone, PartialEq)]
pub enum Checked {
    /// Nothing open, or still waiting.
    Nothing,
    /// The stop was closed without a "Keep studying" (no check-ins any more).
    Lapsed,
    /// The wait is over: this session was stopped and the points taken.
    Stopped(String),
}

/// The daemon's emergency stop: at most one held open at a time.
#[derive(Debug, Default)]
pub struct EmergencyStop {
    hold: Option<Hold>,
}

impl EmergencyStop {
    /// Open the stop, or keep it open: the app sends this when you press
    /// "Emergency stop" and every 2 seconds after. Once the wait is over, this
    /// stops the running session and answers with `stopped: true`.
    pub async fn keep_open(
        &mut self,
        pool: &SqlitePool,
        now: DateTime<Utc>,
    ) -> Result<EmergencyStatus> {
        let Some(running) = session::running(pool).await? else {
            self.hold = None;
            bail!("no study session is running");
        };
        // A new stop when none is open, the last one lapsed, or it was for
        // another session.
        let start_again = match &self.hold {
            Some(hold) => hold.session_id != running.id || hold.lapsed(now),
            None => true,
        };
        if start_again {
            self.hold = Some(Hold {
                session_id: running.id,
                started_at: now,
                last_seen: now,
            });
        }
        let Some(hold) = self.hold.as_mut() else {
            bail!("the emergency stop isn't open");
        };
        hold.last_seen = now;
        let hold = hold.clone();

        if now >= hold.stops_at() {
            self.hold = None;
            session::emergency_stop(pool, &hold.session_id, now).await?;
            return Ok(hold.status(true));
        }
        Ok(hold.status(false))
    }

    /// "Keep studying": close the stop. Returns true when one was open.
    pub fn cancel(&mut self) -> bool {
        self.hold.take().is_some()
    }

    /// Where the open stop stands, if one is open.
    pub fn status(&self, now: DateTime<Utc>) -> Option<EmergencyStatus> {
        self.hold
            .as_ref()
            .filter(|hold| !hold.lapsed(now))
            .map(|hold| hold.status(false))
    }

    /// What the daemon does every couple of seconds: drop a stop nobody keeps
    /// open any more, and stop the session once the wait is over (also when
    /// the app's next check-in hasn't come yet).
    pub async fn check(&mut self, pool: &SqlitePool, now: DateTime<Utc>) -> Result<Checked> {
        let Some(hold) = self.hold.clone() else {
            return Ok(Checked::Nothing);
        };
        if hold.lapsed(now) {
            self.hold = None;
            return Ok(Checked::Lapsed);
        }
        // The session ended by itself (its planned end) while the stop was open.
        let still_running = session::running(pool)
            .await?
            .is_some_and(|running| running.id == hold.session_id);
        if !still_running {
            self.hold = None;
            return Ok(Checked::Nothing);
        }
        if now >= hold.stops_at() {
            self.hold = None;
            session::emergency_stop(pool, &hold.session_id, now).await?;
            return Ok(Checked::Stopped(hold.session_id));
        }
        Ok(Checked::Nothing)
    }
}
