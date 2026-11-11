// The semester overview's calculations: which semesters there are, which
// weeks and courses they show, and the status of every cell. Plain functions,
// no Svelte, so the page only has to draw what these return.
//
// Lockin doesn't know the official semester dates, so it follows the
// academic calendar: August to January is semester 1, February to July is
// semester 2 (at UGent semester 1 ends with the January exams). A semester's
// columns run from its first week with classes to its last one; weeks in
// between without classes (holidays) are kept, so the weeks stay in order.

import type { WeekClasses, WeekProgress } from "$lib/api";
import { addDays, isoDate, parseIsoDate } from "$lib/i18n/index.svelte";
import type { Status } from "./status";

export interface Semester {
  /** "2026-1": academic year 2026–2027, semester 1. */
  key: string;
  number: 1 | 2;
  /** 2026 for 2026–2027. */
  startYear: number;
  /** Mondays of its weeks, e.g. "2026-09-21". */
  weeks: string[];
}

/** A cell: a status, "none" when the course has no classes that week, "empty" with classes but no activities. */
export type CellStatus = Status | "none" | "empty";

/** The semester a week belongs to, judged by its Thursday (like ISO weeks). */
function semesterOf(monday: string): { key: string; number: 1 | 2; startYear: number } {
  const thursday = addDays(parseIsoDate(monday), 3);
  const month = thursday.getMonth(); // 0 = January
  const year = thursday.getFullYear();
  if (month >= 7) return { key: `${year}-1`, number: 1, startYear: year }; // Aug–Dec
  if (month === 0) return { key: `${year - 1}-1`, number: 1, startYear: year - 1 }; // January
  return { key: `${year - 1}-2`, number: 2, startYear: year - 1 }; // Feb–Jul
}

/** All semesters with classes, oldest first. */
export function findSemesters(classWeeks: WeekClasses[]): Semester[] {
  const byKey = new Map<string, { info: ReturnType<typeof semesterOf>; first: string; last: string }>();
  for (const { week } of classWeeks) {
    const info = semesterOf(week);
    const found = byKey.get(info.key);
    if (!found) byKey.set(info.key, { info, first: week, last: week });
    else {
      if (week < found.first) found.first = week;
      if (week > found.last) found.last = week;
    }
  }

  return [...byKey.values()]
    .map(({ info, first, last }) => {
      const weeks: string[] = [];
      for (let day = parseIsoDate(first); isoDate(day) <= last; day = addDays(day, 7)) {
        weeks.push(isoDate(day));
      }
      return { ...info, weeks };
    })
    .sort((a, b) => a.key.localeCompare(b.key));
}

/** The semester `today` is in, else the last one before it, else the first. */
export function currentSemester(semesters: Semester[], today: Date): Semester | undefined {
  const week = isoDate(addDays(today, -((today.getDay() + 6) % 7)));
  return (
    semesters.find((s) => s.weeks[0] <= week && week <= s.weeks[s.weeks.length - 1]) ??
    [...semesters].reverse().find((s) => s.weeks[0] <= week) ??
    semesters[0]
  );
}

function weekIsOver(monday: string, now: Date): boolean {
  return addDays(parseIsoDate(monday), 7) <= now;
}

/**
 * The status of one cell, from its activities, like a class block in the week view:
 * all done = done, some = partly done, none done and a class already ended (or the
 * week is over) = behind, none done and nothing due yet = planned.
 */
export function cellStatus(
  classes: number,
  progress: WeekProgress | undefined,
  monday: string,
  now: Date,
): CellStatus {
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  if (total === 0) return classes > 0 ? "empty" : "none";
  if (done === total) return "done";
  if (done > 0) return "partial";
  if ((progress?.overdue ?? 0) > 0 || weekIsOver(monday, now)) return "behind";
  return "planned";
}

/**
 * "Done" column: activities done out of the ones that were due, as a whole
 * percentage. Due = everything in weeks that are over, plus this week's done and
 * overdue ones, so work that's still to come doesn't count against you.
 * null when nothing was due yet.
 */
export function donePercent(progress: WeekProgress[], weeks: string[], now: Date): number | null {
  const shown = new Set(weeks);
  let done = 0;
  let due = 0;
  for (const p of progress) {
    if (!shown.has(p.week) || parseIsoDate(p.week) > now) continue;
    done += p.done;
    due += weekIsOver(p.week, now) ? p.total : p.done + p.overdue;
  }
  return due === 0 ? null : Math.round((100 * done) / due);
}
