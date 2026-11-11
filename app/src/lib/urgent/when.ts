// The "when" texts of the Urgent list: "Open 8 days", "Due Fri 23:59",
// "Due Mon 16 Nov", "2 days late". Worked out from the times in an
// UrgentItem and the current time, so they stay right while the page is open.

import type { UrgentItem, UrgentStatus } from "$lib/api";
import { addDays, formatDate, formatDay, formatTime, isoDate, t, tn } from "$lib/i18n/index.svelte";
import type { Status } from "$lib/week/status";

/** Whole calendar days from `from` to `to`, in local time: Fri → Mon is 3. */
export function calendarDays(from: Date, to: Date): number {
  const start = new Date(from.getFullYear(), from.getMonth(), from.getDate());
  const end = new Date(to.getFullYear(), to.getMonth(), to.getDate());
  // Round, because a day with a clock change is 23 or 25 hours long.
  return Math.round((end.getTime() - start.getTime()) / 86_400_000);
}

/** Monday 00:00 of the week `date` is in. */
export function mondayOf(date: Date): Date {
  return addDays(date, -((date.getDay() + 6) % 7));
}

/** "23:59" is the usual end of a day: left out when the date is shown. */
function isEndOfDay(date: Date): boolean {
  return date.getHours() === 23 && date.getMinutes() === 59;
}

/**
 * "Open 8 days" for an activity. Counts whole 24-hour days, like the status
 * in `urgent.rs` (behind after 2 days), so "Open 2 days" is always red.
 */
export function openText(openSince: Date, now: Date): string {
  const days = Math.floor((now.getTime() - openSince.getTime()) / 86_400_000);
  return days <= 0 ? t("urgent.openToday") : tn("urgent.open", days);
}

/** "Due Fri 23:59", "Due Mon 16 Nov", "Due today 14:00", "2 days late". */
export function dueText(due: Date, now: Date): string {
  const days = calendarDays(now, due);
  const time = formatTime(due);
  if (due < now) {
    return days === 0 ? t("urgent.lateToday", { time }) : tn("urgent.late", -days);
  }
  if (days === 0) return t("urgent.dueToday", { time });
  if (days === 1) return t("urgent.dueTomorrow", { time });
  // Still this week: the weekday is enough, as in the mockup ("Due Fri 23:59").
  if (isoDate(mondayOf(due)) === isoDate(mondayOf(now))) {
    return t("urgent.dueDay", { day: `${formatDate(due, { weekday: "short" })} ${time}` });
  }
  // Later: the date, and the time only when it isn't the end of the day.
  const day = formatDay(due);
  return t("urgent.dueDay", { day: isEndOfDay(due) ? day : `${day} ${time}` });
}

/** The "when" text of a line in the Urgent list. */
export function whenText(item: UrgentItem, now: Date): string {
  if (item.openSince) return openText(new Date(item.openSince), now);
  if (item.dueAt) return dueText(new Date(item.dueAt), now);
  return "";
}

/**
 * Where an Urgent line leads in the week view: an activity opens its week with
 * its class selected ("/?date=2026-11-09&block=…"), a deadline the week it's due.
 */
export function weekHref(item: UrgentItem): string {
  if (item.kind === "activity") {
    const block = item.blockId ? `&block=${item.blockId}` : "";
    return `/?date=${item.week}${block}`;
  }
  return `/?date=${isoDate(new Date(item.dueAt ?? Date.now()))}`;
}

/** The status icon of an Urgent line: "approaching" uses the half-filled dot. */
export function urgentIcon(status: UrgentStatus): Status {
  return status === "approaching" ? "partial" : status;
}
