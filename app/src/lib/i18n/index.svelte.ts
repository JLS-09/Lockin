// Translations and date formatting, for Dutch and English.
//
// All UI text lives in en.json and nl.json, never in components. Keys are flat
// ("week.title") so the daemon can read the same files later for notifications.
// Placeholders look like {name}. A key with .one and .other forms is a plural:
// use tn() for those.
//
// The current language is reactive ($state), so every t() in a component
// updates as soon as you switch, without a restart.

import en from "./en.json";
import nl from "./nl.json";

export type Locale = "en" | "nl";
export const locales: Locale[] = ["en", "nl"];

type Key = keyof typeof en;
type PluralKey = { [K in Key]: K extends `${infer Base}.one` ? Base : never }[Key];
type Params = Record<string, string | number>;

// nl.json must have every key of en.json: svelte-check fails here if one is missing.
const dutch: Record<Key, string> = nl;
const messages: Record<Locale, Record<Key, string>> = { en, nl: dutch };

// Used by Intl for dates and plurals: Belgian Dutch, British English (24-hour clock).
const intlTags: Record<Locale, string> = { en: "en-GB", nl: "nl-BE" };

const STORAGE_KEY = "lockin.locale";

function systemLocale(): Locale {
  return navigator.language.toLowerCase().startsWith("nl") ? "nl" : "en";
}

function savedLocale(): Locale | null {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return value === "en" || value === "nl" ? value : null;
  } catch {
    return null;
  }
}

class I18n {
  // Saved choice first, else the system language.
  locale = $state<Locale>(savedLocale() ?? systemLocale());

  /** True when a language was chosen in Settings (not just the system's). */
  get chosen(): boolean {
    return savedLocale() !== null;
  }

  get intlTag(): string {
    return intlTags[this.locale];
  }

  setLocale(locale: Locale) {
    this.locale = locale;
    document.documentElement.lang = locale;
    try {
      localStorage.setItem(STORAGE_KEY, locale);
    } catch {
      // Not saved; the choice still holds until the app closes.
    }
  }
}

export const i18n = new I18n();

function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/** The text for `key` in the current language, with {placeholders} filled in. */
export function t(key: Key, params?: Params): string {
  return fill(messages[i18n.locale][key] ?? en[key], params);
}

/** A plural: picks "<key>.one" or "<key>.other" by `count`. */
export function tn(key: PluralKey, count: number, params?: Params): string {
  const form = new Intl.PluralRules(i18n.intlTag).select(count) === "one" ? "one" : "other";
  return t(`${key}.${form}` as Key, { count, ...params });
}

// ---------- Dates ----------

/** "2026-11-11" for a local date: the format the Rust commands take. */
export function isoDate(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/** A local Date at midnight from "2026-11-11". */
export function parseIsoDate(text: string): Date {
  const [year, month, day] = text.split("-").map(Number);
  return new Date(year, month - 1, day);
}

/** The same day `days` later (or earlier, if negative). */
export function addDays(date: Date, days: number): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days);
}

/** ISO week number: weeks start on Monday, week 1 holds the first Thursday. */
export function isoWeek(date: Date): number {
  const thursday = addDays(date, 3 - ((date.getDay() + 6) % 7));
  const firstThursday = new Date(thursday.getFullYear(), 0, 4);
  return 1 + Math.round((thursday.getTime() - firstThursday.getTime()) / (7 * 86400000));
}

/** Format with Intl in the current language, e.g. { weekday: "short" } → "Mon" / "ma". */
export function formatDate(date: Date, options: Intl.DateTimeFormatOptions): string {
  return new Intl.DateTimeFormat(i18n.intlTag, options).format(date);
}

/** "Wed 11 Nov" / "wo 11 nov.": without the comma Intl puts after the weekday in English. */
export function formatDay(date: Date): string {
  const weekday = formatDate(date, { weekday: "short" });
  return `${weekday} ${formatDate(date, { day: "numeric", month: "short" })}`;
}

/** "08:30" */
export function formatTime(date: Date): string {
  return formatDate(date, { hour: "2-digit", minute: "2-digit" });
}

/** "9–13 Nov 2026" / "9–13 nov. 2026" */
export function formatRange(start: Date, end: Date): string {
  return new Intl.DateTimeFormat(i18n.intlTag, {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).formatRange(start, end);
}

/** "5 minutes ago" / "5 minuten geleden", "yesterday" / "gisteren". */
export function formatAgo(date: Date, now = new Date()): string {
  const seconds = (date.getTime() - now.getTime()) / 1000;
  const format = new Intl.RelativeTimeFormat(i18n.intlTag, { numeric: "auto" });
  if (Math.abs(seconds) < 60) return format.format(0, "minute");
  if (Math.abs(seconds) < 3600) return format.format(Math.round(seconds / 60), "minute");
  if (Math.abs(seconds) < 86400) return format.format(Math.round(seconds / 3600), "hour");
  return format.format(Math.round(seconds / 86400), "day");
}
