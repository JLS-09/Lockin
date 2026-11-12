//! Dutch and English for the daemon (tray and notifications), from the same
//! translation files as the app: app/src/lib/i18n/en.json and nl.json. Keys
//! are flat ("tray.open"), placeholders look like {name}, and a key with
//! `.one` and `.other` forms is a plural (`tn`), exactly as in the app.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Local, Utc};
use sqlx::sqlite::SqlitePool;

use crate::settings;

/// The translation files, built into the binary.
const EN_JSON: &str = include_str!("../../../app/src/lib/i18n/en.json");
const NL_JSON: &str = include_str!("../../../app/src/lib/i18n/nl.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Nl,
}

impl Language {
    /// "en" or "nl", as stored in the settings.
    pub fn from_code(code: &str) -> Option<Language> {
        match code {
            "en" => Some(Language::En),
            "nl" => Some(Language::Nl),
            _ => None,
        }
    }
}

/// The language chosen in the app, else the system's (Dutch when LANG, or
/// LC_ALL / LC_MESSAGES, starts with "nl"; English otherwise).
pub async fn language(pool: &SqlitePool) -> Language {
    if let Ok(Some(code)) = settings::get(pool, settings::LANGUAGE).await
        && let Some(language) = Language::from_code(&code)
    {
        return language;
    }
    system_language()
}

/// The system language from the environment, as the app follows it too.
pub fn system_language() -> Language {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name)
            && !value.is_empty()
        {
            return if value.starts_with("nl") {
                Language::Nl
            } else {
                Language::En
            };
        }
    }
    Language::En
}

/// The texts of one language.
pub struct Texts {
    pub language: Language,
    messages: HashMap<String, String>,
}

impl Texts {
    pub fn new(language: Language) -> Texts {
        let json = match language {
            Language::En => EN_JSON,
            Language::Nl => NL_JSON,
        };
        // The files are checked by `pnpm check` and a test below, so this can't fail.
        let messages = serde_json::from_str(json).unwrap_or_default();
        Texts { language, messages }
    }

    /// The text for `key` with its {placeholders} filled in; the key itself
    /// when it's missing, so a mistake shows instead of an empty line.
    pub fn t(&self, key: &str, params: &[(&str, &str)]) -> String {
        let Some(text) = self.messages.get(key) else {
            return key.to_string();
        };
        let mut text = text.clone();
        for (name, value) in params {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    /// A plural: `key.one` for 1, `key.other` otherwise (the same in Dutch and
    /// English), with {count} filled in.
    pub fn tn(&self, key: &str, count: i64, params: &[(&str, &str)]) -> String {
        let form = if count == 1 { "one" } else { "other" };
        let count = count.to_string();
        let mut all = vec![("count", count.as_str())];
        all.extend_from_slice(params);
        self.t(&format!("{key}.{form}"), &all)
    }

    /// "13:00" in local time.
    pub fn clock(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&Local).format("%H:%M").to_string()
    }

    /// When something happens, seen from `now`: "today 13:00", "tomorrow
    /// 13:00", "Thu 13:00" within a week, else "Thu 19/11 13:00".
    pub fn when(&self, at: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let local = at.with_timezone(&Local);
        let days = (local.date_naive() - now.with_timezone(&Local).date_naive()).num_days();
        let time = self.clock(at);
        let day = self.t(
            &format!("weekday.{}", local.weekday().number_from_monday()),
            &[],
        );
        match days {
            0 => self.t("when.today", &[("time", &time)]),
            1 => self.t("when.tomorrow", &[("time", &time)]),
            2..=6 => self.t("when.day", &[("day", &day), ("time", &time)]),
            _ => {
                let date = format!("{}/{}", local.day(), local.month());
                self.t(
                    "when.date",
                    &[("day", &day), ("date", &date), ("time", &time)],
                )
            }
        }
    }

    /// "2 h 30 min", "2 h" or "25 min".
    pub fn duration(&self, seconds: i64) -> String {
        let minutes = (seconds.max(0) + 30) / 60;
        let (hours, minutes) = (minutes / 60, minutes % 60);
        let (h, m) = (hours.to_string(), minutes.to_string());
        match (hours, minutes) {
            (0, _) => self.t("duration.minutes", &[("minutes", &m)]),
            (_, 0) => self.t("duration.hours", &[("hours", &h)]),
            _ => self.t("duration.hoursMinutes", &[("hours", &h), ("minutes", &m)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_files_have_the_same_keys() {
        let en: HashMap<String, String> = serde_json::from_str(EN_JSON).unwrap();
        let nl: HashMap<String, String> = serde_json::from_str(NL_JSON).unwrap();
        let mut missing: Vec<_> = en.keys().filter(|key| !nl.contains_key(*key)).collect();
        missing.sort();
        assert!(missing.is_empty(), "missing in nl.json: {missing:?}");
    }

    #[test]
    fn fills_placeholders_and_plurals() {
        let en = Texts::new(Language::En);
        let nl = Texts::new(Language::Nl);
        assert_eq!(en.t("tray.open", &[]), "Open Lockin");
        assert_eq!(nl.t("tray.open", &[]), "Lockin openen");
        assert_eq!(
            en.t("tray.credit", &[("minutes", "10")]),
            "Break credit: 10 min"
        );
        assert_eq!(
            en.tn("notify.sessionSoon", 1, &[]),
            "Study session in 1 minute"
        );
        assert_eq!(
            nl.tn("notify.sessionSoon", 5, &[]),
            "Studiesessie over 5 minuten"
        );
        assert_eq!(en.t("no.such.key", &[]), "no.such.key");
    }

    #[test]
    fn durations() {
        let en = Texts::new(Language::En);
        assert_eq!(en.duration(25 * 60), "25 min");
        assert_eq!(en.duration(2 * 3600), "2 h");
        assert_eq!(en.duration(2 * 3600 + 30 * 60 + 10), "2 h 30 min");
        let nl = Texts::new(Language::Nl);
        assert_eq!(nl.duration(90 * 60), "1 u 30 min");
    }
}
