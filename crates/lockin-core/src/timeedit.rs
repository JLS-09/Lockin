//! Reading a TimeEdit iCal feed.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Europe::Brussels;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventLike,
};

use crate::model::FeedEvent;

/// Download the feed text from a TimeEdit subscription URL.
pub async fn download(url: &str) -> Result<String> {
    let response = reqwest::get(url)
        .await
        .context("could not reach the feed")?;
    let response = response
        .error_for_status()
        .context("the feed returned an error")?;
    let text = response.text().await?;
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

        events.push(FeedEvent {
            uid: uid.to_string(),
            title: event.get_summary().unwrap_or("(no title)").to_string(),
            location: event.get_location().map(|s| s.to_string()),
            description: event.get_description().map(|s| s.to_string()),
            start,
            end,
        });
    }

    Ok(events)
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
