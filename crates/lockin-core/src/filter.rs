//! Filter rules: decide which feed events are ignored.

use crate::model::{FeedEvent, FilterRule};

/// True if at least one rule matches the event.
/// Matching is a case-insensitive "contains".
pub fn is_ignored(event: &FeedEvent, rules: &[FilterRule]) -> bool {
    for rule in rules {
        let text = match rule.field.as_str() {
            "title" => Some(event.title.as_str()),
            "location" => event.location.as_deref(),
            "description" => event.description.as_deref(),
            _ => None,
        };

        if let Some(text) = text
            && text.to_lowercase().contains(&rule.pattern.to_lowercase())
        {
            return true;
        }
    }
    false
}
