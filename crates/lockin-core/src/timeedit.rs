//! Reading a TimeEdit iCal feed.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Europe::Brussels;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventLike,
};

use crate::model::FeedEvent;

/// A feed download that takes longer than this fails.
pub const DOWNLOAD_TIMEOUT_SECONDS: u64 = 60;

/// Download the feed text from a TimeEdit subscription URL.
/// Errors never contain the URL: it holds a personal key, and errors end up
/// on screen (Settings, the CLI).
pub async fn download(url: &str) -> Result<String> {
    // Give up after a minute, so a feed that hangs never blocks the daemon's next sync.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECONDS))
        .build()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!(e.without_url()))
        .context("could not reach the feed")?;
    let response = response
        .error_for_status()
        .map_err(|e| anyhow!(e.without_url()))
        .context("the feed returned an error")?;
    let text = response
        .text()
        .await
        .map_err(|e| anyhow!(e.without_url()))?;
    Ok(text)
}

/// Turn the feed text into a list of events.
/// Events without a UID or without start/end time are skipped.
pub fn parse(ics_text: &str) -> Result<Vec<FeedEvent>> {
    let calendar: Calendar = ics_text
        .parse()
        .map_err(|e| anyhow!("could not parse the iCal feed: {e}"))?;

    let mut events = Vec::new();

    for component in &calendar.components {
        let CalendarComponent::Event(event) = component else {
            continue;
        };

        let Some(uid) = event.get_uid() else { continue };
        let Some(start) = event.get_start().and_then(to_utc) else {
            continue;
        };
        let Some(end) = event.get_end().and_then(to_utc) else {
            continue;
        };

        let description = event.get_description().map(|s| s.to_string());

        // Some classes have no LOCATION but a "Lokaal: ..." line in the description.
        let location = match event.get_location() {
            Some(location) if !location.trim().is_empty() => Some(location.to_string()),
            _ => description.as_deref().and_then(room_from_description),
        };

        events.push(FeedEvent {
            uid: uid.to_string(),
            title: event.get_summary().unwrap_or("(no title)").to_string(),
            location,
            description,
            start,
            end,
        });
    }

    Ok(events)
}

/// The room from a "Lokaal: GSCHD.1.106" line in the description, if there is one.
fn room_from_description(description: &str) -> Option<String> {
    description
        .lines()
        .find_map(|line| line.trim().strip_prefix("Lokaal:"))
        .map(|room| room.trim().to_string())
}

/// Convert an iCal time to UTC.
/// TimeEdit normally sends UTC times; times without a zone are read as Belgian time.
/// All-day events (a date without a time) are skipped: classes always have a time.
fn to_utc(value: DatePerhapsTime) -> Option<DateTime<Utc>> {
    match value {
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(time)) => Some(time),
        DatePerhapsTime::DateTime(CalendarDateTime::Floating(time)) => Brussels
            .from_local_datetime(&time)
            .earliest()
            .map(|t| t.with_timezone(&Utc)),
        DatePerhapsTime::DateTime(with_zone) => with_zone.try_into_utc(),
        DatePerhapsTime::Date(_) => None,
    }
}
