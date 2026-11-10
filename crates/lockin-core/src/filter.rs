//! Filter rules: decide which blocks are ignored.

use crate::model::FilterRule;

/// True if at least one rule matches the block's title, location or description.
/// Matching is a case-insensitive "contains".
pub fn is_ignored(
    title: &str,
    location: Option<&str>,
    description: Option<&str>,
    rules: &[FilterRule],
) -> bool {
    for rule in rules {
        let text = match rule.field.as_str() {
            "title" => Some(title),
            "location" => location,
            "description" => description,
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
