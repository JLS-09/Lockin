//! Reading the parts of a UGent TimeEdit title.
//!
//! A title looks like `E702010A. Signalen en systemen GR02, werkcollege`:
//! the course code, the course name with an optional group, the kind of class,
//! and sometimes an extra note (`..., zelfstandig werk, Kennisclips`).

/// The parts of a class title.
#[derive(Debug, PartialEq)]
pub struct ClassTitle {
    pub code: String,
    pub course: String,
    pub kind: Option<String>,
}

/// Split a title into its parts. Returns None if it doesn't start with a course code.
pub fn parse(title: &str) -> Option<ClassTitle> {
    let (code, rest) = title.split_once(". ")?;
    if !is_course_code(code) {
        return None;
    }

    // "Signalen en systemen GR02, werkcollege" -> name part and kind.
    let mut parts = rest.split(", ");
    let name_part = parts.next()?;
    let kind = parts.next().map(|k| k.to_string());

    // Drop the group at the end of the name ("... GR02").
    let course = match name_part.rsplit_once(' ') {
        Some((name, last)) if is_group(last) => name,
        _ => name_part,
    };

    Some(ClassTitle {
        code: code.to_string(),
        course: course.to_string(),
        kind,
    })
}

/// UGent course codes are 8 letters and digits, e.g. E702010A.
fn is_course_code(text: &str) -> bool {
    text.len() == 8 && text.chars().all(|c| c.is_ascii_alphanumeric())
}

/// A group looks like GR02.
fn is_group(word: &str) -> bool {
    match word.strip_prefix("GR") {
        Some(number) => !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_with_group() {
        let title = parse("E702010A. Signalen en systemen GR02, werkcollege").unwrap();
        assert_eq!(title.code, "E702010A");
        assert_eq!(title.course, "Signalen en systemen");
        assert_eq!(title.kind.as_deref(), Some("werkcollege"));
    }

    #[test]
    fn title_with_note() {
        let title = parse("E701033A. Wiskunde I, zelfstandig werk, Kennisclips").unwrap();
        assert_eq!(title.course, "Wiskunde I");
        assert_eq!(title.kind.as_deref(), Some("zelfstandig werk"));
    }

    #[test]
    fn title_without_code() {
        assert_eq!(parse("Vergadering studentenraad"), None);
    }
}
