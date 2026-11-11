//! Reading a Ufora (Brightspace) calendar feed.
//!
//! The feed ("Alle cursussen") holds every calendar event of every Ufora course
//! since you started, most of which aren't deadlines: released content, info
//! sessions, Zoom classes. Brightspace names its events after what happens:
//!
//! - "Sprint 1 - Verwacht": the due date of an assignment.
//! - "Verslag - Beschikbaarheid eindigt", "Checklist - verloopt": the last moment
//!   something can be handed in or done.
//! - "Werkcollege 8: oplossingen - Beschikbaar": content released. Not a deadline.
//!
//! Only the first two kinds become deadlines, titled without the suffix. The
//! suffixes follow the Ufora interface language. In English they are "Due" and
//! "Availability Ends" ("Available" for released content), and the English
//! export sometimes uses an en dash: "Quiz 3 – Availability Ends".
//!
//! LOCATION holds the course: "E761080A - Machinaal leren", or a room followed
//! by "(E761080A - Machinaal leren)". The code links the deadline to the course
//! Lockin knows from TimeEdit (see `deadline::save_ufora`).

use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::Brussels;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, DatePerhapsTime, EventLike,
};

/// The labels after the title that mark a deadline, in Dutch and English.
/// Checked against real exports of 11 Nov 2026: "Verwacht" is "Due",
/// "Beschikbaarheid eindigt" is "Availability Ends", "verloopt" is "Due" too.
pub const DEADLINE_LABELS: &[&str] = &[
    "Verwacht",
    "Beschikbaarheid eindigt",
    "verloopt",
    "Due",
    "Availability Ends",
];

/// Between the title and the label: a hyphen, or an en dash in some English events.
const SEPARATORS: &[&str] = &[" - ", " – "];

/// The line at the end of every description, followed by a link to the event.
const EVENT_LINK_LABELS: &[&str] = &["Gebeurtenis weergeven - ", "View event – ", "View event - "];

/// One deadline from the Ufora feed.
#[derive(Debug, Clone, PartialEq)]
pub struct UforaDeadline {
    pub uid: String,
    /// The title without " - Verwacht".
    pub title: String,
    /// "E761080A"; None for pages that aren't a course ("Wel in je vel").
    pub course_code: Option<String>,
    pub due: DateTime<Utc>,
    /// The description without the "Gebeurtenis weergeven" line.
    pub description: Option<String>,
    /// The assignment's page when the description links to one, else the event.
    pub url: Option<String>,
}

/// True when this feed text comes from Brightspace (Ufora) rather than TimeEdit.
pub fn is_ufora_feed(ics_text: &str) -> bool {
    ics_text
        .lines()
        .take(20)
        .any(|line| line.starts_with("PRODID:") && line.contains("D2L"))
}

/// The deadlines in a Ufora feed. Everything else in it is skipped.
pub fn parse(ics_text: &str) -> Result<Vec<UforaDeadline>> {
    let calendar: Calendar = ics_text
        .parse()
        .map_err(|e| anyhow!("could not parse the Ufora feed: {e}"))?;

    let mut deadlines = Vec::new();
    for component in &calendar.components {
        let CalendarComponent::Event(event) = component else {
            continue;
        };
        let (Some(uid), Some(summary)) = (event.get_uid(), event.get_summary()) else {
            continue;
        };
        let Some(title) = deadline_title(summary) else {
            continue;
        };
        let Some(due) = event.get_start().and_then(due_time) else {
            continue;
        };
        let (description, url) = match event.get_description() {
            Some(text) => split_description(text),
            None => (None, None),
        };

        deadlines.push(UforaDeadline {
            uid: uid.to_string(),
            title: title.to_string(),
            course_code: event.get_location().and_then(course_code),
            due,
            description,
            url,
        });
    }
    Ok(deadlines)
}

/// "Sprint 1 - Verwacht" or "Sprint 1 - Due" → "Sprint 1"; None when the title
/// isn't a deadline.
pub fn deadline_title(summary: &str) -> Option<&str> {
    let summary = summary.trim();
    for label in DEADLINE_LABELS {
        for separator in SEPARATORS {
            if let Some(title) = summary.strip_suffix(&format!("{separator}{label}")) {
                let title = title.trim();
                return (!title.is_empty()).then_some(title);
            }
        }
    }
    None
}

/// The course code in a LOCATION: "E761080A - Machinaal leren" → "E761080A",
/// "GSCHB.3.019 (E761080A - Machinaal leren)" → "E761080A".
/// None when there is no code ("Wel in je vel", "Monitoraat W&B").
pub fn course_code(location: &str) -> Option<String> {
    // With a room in front, the course is between the last brackets.
    let course = match location.rfind('(') {
        Some(open) => location[open + 1..].trim_end_matches(')'),
        None => location,
    };
    let (code, _name) = course.split_once(" - ")?;
    let code = code.trim();
    // A code is one word of letters and digits with at least one digit, e.g. E761080A or C003781.
    let looks_like_code = !code.is_empty()
        && code.chars().all(|c| c.is_ascii_alphanumeric())
        && code.chars().any(|c| c.is_ascii_digit());
    looks_like_code.then(|| code.to_string())
}

/// Split a description into its text and the best link to open in Ufora.
/// The event link ("Gebeurtenis weergeven - https://…", in English "View event –
/// https://…") is always last; an
/// assignment link ("Sprint 1 - https://…/dropbox/…") is preferred when there is one.
pub fn split_description(description: &str) -> (Option<String>, Option<String>) {
    // Where the event link line starts, and how long its label is.
    let found = EVENT_LINK_LABELS
        .iter()
        .find_map(|label| description.rfind(label).map(|at| (at, label.len())));
    let (text, event_link) = match found {
        Some((at, length)) => (&description[..at], Some(description[at + length..].trim())),
        None => (description, None),
    };

    // The first other Ufora link in the text, e.g. the assignment's dropbox.
    let other_link = text
        .split_whitespace()
        .find(|word| word.starts_with("https://ufora.ugent.be/"));

    let url = other_link
        .or(event_link)
        .filter(|link| link.starts_with("https://"))
        .map(str::to_string);
    let text = text.trim();
    let text = (!text.is_empty()).then(|| text.to_string());
    (text, url)
}

/// The due time in UTC. A date without a time (an all-day event) is due at
/// 23:59 Belgian time; a time without a zone is Belgian time too.
fn due_time(value: DatePerhapsTime) -> Option<DateTime<Utc>> {
    match value {
        DatePerhapsTime::DateTime(CalendarDateTime::Utc(time)) => Some(time),
        DatePerhapsTime::DateTime(CalendarDateTime::Floating(time)) => Brussels
            .from_local_datetime(&time)
            .earliest()
            .map(|t| t.with_timezone(&Utc)),
        DatePerhapsTime::DateTime(with_zone) => with_zone.try_into_utc(),
        DatePerhapsTime::Date(date) => {
            let end_of_day = date.and_time(NaiveTime::from_hms_opt(23, 59, 0)?);
            Brussels
                .from_local_datetime(&end_of_day)
                .earliest()
                .map(|t| t.with_timezone(&Utc))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_titles() {
        assert_eq!(deadline_title("Sprint 1 - Verwacht"), Some("Sprint 1"));
        assert_eq!(
            deadline_title("Verslag DIY experiment - Beschikbaarheid eindigt"),
            Some("Verslag DIY experiment")
        );
        assert_eq!(
            deadline_title("Checklist week 1 - verloopt"),
            Some("Checklist week 1")
        );
        // Released content and plain events aren't deadlines.
        assert_eq!(
            deadline_title("Werkcollege 8: oplossingen - Beschikbaar"),
            None
        );
        assert_eq!(deadline_title("Infosessie Erasmus"), None);
        assert_eq!(deadline_title(" - Verwacht"), None);
    }

    #[test]
    fn english_deadline_titles() {
        assert_eq!(deadline_title("Sprint 1 - Due"), Some("Sprint 1"));
        assert_eq!(deadline_title("Quiz 3 - Availability Ends"), Some("Quiz 3"));
        // The English export sometimes uses an en dash.
        assert_eq!(
            deadline_title("Verslag – Availability Ends"),
            Some("Verslag")
        );
        assert_eq!(
            deadline_title("Rotaties en reflecties [oplossingen] – Available"),
            None
        );
        assert_eq!(deadline_title("Werkcollege 8 - Available"), None);
    }

    #[test]
    fn course_codes() {
        assert_eq!(
            course_code("E761080A - Machinaal leren").as_deref(),
            Some("E761080A")
        );
        assert_eq!(
            course_code("GSCHB.3.019 (E702010A - Signalen en systemen)").as_deref(),
            Some("E702010A")
        );
        assert_eq!(course_code("Wel in je vel"), None);
        assert_eq!(course_code("Monitoraat W&B - Faculteit"), None);
    }

    #[test]
    fn links_in_the_description() {
        let text = "Upload hier.\n\nOpdrachten:\nSprint 1 - https://ufora.ugent.be/d2l/lms/dropbox/x?ou=1\n\n\nGebeurtenis weergeven - https://ufora.ugent.be/d2l/le/calendar/1/event/2/detailsview";
        let (description, url) = split_description(text);
        assert_eq!(
            url.as_deref(),
            Some("https://ufora.ugent.be/d2l/lms/dropbox/x?ou=1")
        );
        let description = description.unwrap();
        assert!(description.starts_with("Upload hier."));
        assert!(!description.contains("Gebeurtenis weergeven"));

        // Without an assignment link, the event itself.
        let (_, url) = split_description("Gebeurtenis weergeven - https://ufora.ugent.be/e/3");
        assert_eq!(url.as_deref(), Some("https://ufora.ugent.be/e/3"));

        // English: "View event – " with an en dash.
        let (text, url) = split_description("Hand in.\n\nView event – https://ufora.ugent.be/e/4");
        assert_eq!(url.as_deref(), Some("https://ufora.ugent.be/e/4"));
        assert_eq!(text.as_deref(), Some("Hand in."));
    }

    #[test]
    fn recognises_a_ufora_feed() {
        assert!(is_ufora_feed(
            "BEGIN:VCALENDAR\r\nPRODID:-//D2L//NONSGML v1.0//EN\r\n"
        ));
        assert!(!is_ufora_feed(
            "BEGIN:VCALENDAR\r\nPRODID:-//TimeEdit//EN\r\n"
        ));
    }
}
