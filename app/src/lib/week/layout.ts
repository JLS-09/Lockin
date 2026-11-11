// Where everything goes in the week grid. Plain functions, no Svelte,
// so the component only has to draw what these return.
//
// The grid: one column for the hour labels, then one column per day.
// Row 1 holds the day headers; after that, one row per SLOT_MINUTES.
// A block from 10:15 to 12:15 simply spans the rows of that time.

import type { ClassBlock, StudySession } from "$lib/api";
import { addDays, isoDate } from "$lib/i18n/index.svelte";

/** Minutes per grid row. TimeEdit times are on 5-minute steps at the finest. */
export const SLOT_MINUTES = 5;

/** The hours shown when the week has no earlier or later classes, as in the mockups. */
const DEFAULT_FIRST_HOUR = 8;
const DEFAULT_LAST_HOUR = 19;

export interface DayColumn {
  date: Date;
  /** Grid column, counted from 1; column 1 holds the hour labels. */
  column: number;
}

export interface PlacedBlock {
  block: ClassBlock;
  column: number;
  rowStart: number;
  rowEnd: number;
  /** Overlapping blocks sit side by side: this one is lane `lane` of `lanes`. */
  lane: number;
  lanes: number;
}

/** A study session in the grid; it shares the lanes with the classes it overlaps. */
export interface PlacedSession {
  session: StudySession;
  column: number;
  rowStart: number;
  rowEnd: number;
  lane: number;
  lanes: number;
}

/** What the lanes need of a block or a session. */
interface Placed {
  rowStart: number;
  rowEnd: number;
  lane: number;
  lanes: number;
}

export interface WeekLayout {
  days: DayColumn[];
  firstHour: number;
  lastHour: number;
  /** Number of time rows (without the header row). */
  slots: number;
  blocks: PlacedBlock[];
  sessions: PlacedSession[];
}

function minutesOfDay(date: Date): number {
  return date.getHours() * 60 + date.getMinutes();
}

/** Grid row for a time of day; row 1 is the header. */
function rowFor(minutes: number, firstHour: number): number {
  return 2 + Math.round((minutes - firstHour * 60) / SLOT_MINUTES);
}

/**
 * Give overlapping blocks of one day their own lane.
 * Blocks that overlap (directly or through others) form a group; every block
 * takes the first lane that is free at its start, and all blocks of a group
 * share the group's number of lanes, so they get the same width.
 */
function assignLanes(blocks: Placed[]) {
  const sorted = [...blocks].sort((a, b) => a.rowStart - b.rowStart || b.rowEnd - a.rowEnd);
  let group: Placed[] = [];
  let laneEnds: number[] = []; // per lane: the row where its last block ends
  let groupEnd = 0;

  const closeGroup = () => {
    for (const placed of group) placed.lanes = laneEnds.length;
    group = [];
    laneEnds = [];
  };

  for (const placed of sorted) {
    if (placed.rowStart >= groupEnd) closeGroup();
    let lane = laneEnds.findIndex((end) => end <= placed.rowStart);
    if (lane === -1) {
      lane = laneEnds.length;
      laneEnds.push(0);
    }
    laneEnds[lane] = placed.rowEnd;
    placed.lane = lane;
    group.push(placed);
    groupEnd = Math.max(groupEnd, placed.rowEnd);
  }
  closeGroup();
}

/** Lay out the week that starts on `monday`: its classes and its study sessions. */
export function layoutWeek(
  monday: Date,
  blocks: ClassBlock[],
  sessions: StudySession[] = [],
): WeekLayout {
  // Monday to Friday always; Saturday and Sunday only when they have classes or sessions.
  const dates = Array.from({ length: 7 }, (_, i) => addDays(monday, i));
  const timed = [...blocks, ...sessions];
  const busyDays = new Set(timed.map((b) => isoDate(new Date(b.startAt))));
  const shown = dates.filter((d, i) => i < 5 || busyDays.has(isoDate(d)));
  const days = shown.map((date, i) => ({ date, column: i + 2 }));

  // Widen the hours to fit the earliest start and the latest end.
  let firstHour = DEFAULT_FIRST_HOUR;
  let lastHour = DEFAULT_LAST_HOUR;
  for (const block of timed) {
    const start = new Date(block.startAt);
    const end = new Date(block.endAt);
    firstHour = Math.min(firstHour, start.getHours());
    // An end at 00:00 the next day counts as 24:00.
    const endMinutes = isoDate(end) === isoDate(start) ? minutesOfDay(end) : 24 * 60;
    lastHour = Math.max(lastHour, Math.ceil(endMinutes / 60));
  }
  const slots = ((lastHour - firstHour) * 60) / SLOT_MINUTES;

  const columnByDay = new Map(days.map((d) => [isoDate(d.date), d.column]));
  // Column and rows of anything with a start and an end; null on a day not shown.
  const place = (item: { startAt: string; endAt: string }) => {
    const start = new Date(item.startAt);
    const end = new Date(item.endAt);
    const column = columnByDay.get(isoDate(start));
    if (column === undefined) return null;
    const endMinutes = isoDate(end) === isoDate(start) ? minutesOfDay(end) : 24 * 60;
    const rowStart = rowFor(minutesOfDay(start), firstHour);
    // At least one row high, so even a zero-length block stays visible.
    const rowEnd = Math.max(rowFor(endMinutes, firstHour), rowStart + 1);
    return { column, rowStart, rowEnd, lane: 0, lanes: 1 };
  };

  const placed: PlacedBlock[] = [];
  for (const block of blocks) {
    const spot = place(block);
    if (spot) placed.push({ block, ...spot });
  }
  const placedSessions: PlacedSession[] = [];
  for (const session of sessions) {
    const spot = place(session);
    if (spot) placedSessions.push({ session, ...spot });
  }

  for (const day of days) {
    assignLanes([
      ...placed.filter((p) => p.column === day.column),
      ...placedSessions.filter((p) => p.column === day.column),
    ]);
  }

  return { days, firstHour, lastHour, slots, blocks: placed, sessions: placedSessions };
}

/** The grid row for a time on a given layout, e.g. for the "now" line. */
export function rowForTime(layout: WeekLayout, date: Date): number | null {
  const minutes = minutesOfDay(date);
  if (minutes < layout.firstHour * 60 || minutes >= layout.lastHour * 60) return null;
  return rowFor(minutes, layout.firstHour);
}

/** The column of `date`, if that day is shown. */
export function columnForDate(layout: WeekLayout, date: Date): number | null {
  return layout.days.find((d) => isoDate(d.date) === isoDate(date))?.column ?? null;
}

/** First day and last shown day, for the "9–13 Nov" range in the header. */
export function shownRange(layout: WeekLayout): [Date, Date] {
  return [layout.days[0].date, layout.days[layout.days.length - 1].date];
}

