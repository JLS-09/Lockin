// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
// Components call these instead of invoke() directly, so the command names
// and argument names live in one place.
// Dates are "YYYY-MM-DD" text; times are RFC 3339 text in UTC.

import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

export interface Course {
  id: string;
  code: string | null;
  name: string;
  /** Hex color, e.g. "#6b9bd1". */
  color: string | null;
}

export type BlockStatus = "normal" | "moved" | "cancelled";

export interface ClassBlock {
  id: string;
  icalUid: string | null;
  courseId: string | null;
  /** hoorcollege, werkcollege, … */
  kind: string | null;
  title: string;
  location: string | null;
  startAt: string;
  endAt: string;
  ignoredByRule: boolean;
  /** null = the rules decide; true/false = manual exception. */
  ignoredOverride: boolean | null;
  status: BlockStatus;
  /** Start time before its first move, for "Moved · was Fri 13:00". */
  movedFrom: string | null;
  /** True once the change was dismissed with "Got it". */
  changeSeen: boolean;
}

export interface FilterRule {
  id: string;
  /** "title", "location" or "description" */
  field: string;
  /** Case-insensitive "contains". */
  pattern: string;
}

/** A manual exception wins; otherwise the filter rules decide. */
export function isIgnored(block: ClassBlock): boolean {
  return block.ignoredOverride ?? block.ignoredByRule;
}

export interface Activity {
  id: string;
  blockId: string | null;
  templateId: string | null;
  courseId: string | null;
  title: string;
  /** Monday of its week, e.g. "2026-11-09". */
  week: string;
  done: boolean;
  doneAt: string | null;
}

export interface ActivityTemplate {
  id: string;
  courseId: string;
  /** null = every block of the course. */
  kind: string | null;
  title: string;
}

export interface CourseKind {
  courseId: string;
  /** "hoorcollege", "werkcollege", … */
  kind: string;
}

export interface WeekProgress {
  week: string;
  courseId: string;
  total: number;
  done: number;
  /** Not done, on a class that has already ended. */
  overdue: number;
}

export interface WeekClasses {
  week: string;
  courseId: string;
  classes: number;
}

/** A saved feed, without its URL (it holds a personal key). */
export interface FeedInfo {
  id: string;
  kind: "timeedit" | "ufora";
  /** "cloud.timeedit.net", "ufora.ugent.be", or the file name. */
  origin: string;
  lastFetchedAt: string | null;
  /** Classes in this feed (TimeEdit). */
  classes: number;
  /** Deadlines in this feed (Ufora). */
  deadlines: number;
}

/** What syncing one feed changed, or why it failed. */
export interface FeedSync {
  sourceId: string;
  /** "timeedit": the counts are classes; "ufora": deadlines. */
  kind: "timeedit" | "ufora";
  added: number;
  /** Known classes or deadlines seen again, changed or not. */
  updated: number;
  /** Ones whose time changed. */
  moved: number;
  cancelled: number;
  error: string | null;
}

export type DeadlineStatus = "normal" | "moved" | "cancelled";

/** Something due at a set time: added by hand, or from the Ufora feed. */
export interface Deadline {
  id: string;
  /** The feed it came from; null if it was added by hand. */
  sourceId: string | null;
  courseId: string | null;
  title: string;
  description: string | null;
  /** When it's due, RFC 3339 in UTC. */
  dueAt: string;
  done: boolean;
  doneAt: string | null;
  /** "moved" and "cancelled" only happen to feed deadlines. */
  status: DeadlineStatus;
  /** Due time before its first move. */
  movedFrom: string | null;
  changeSeen: boolean;
  /** Feed deadlines: where to open it in Ufora. */
  url: string | null;
}

/** Status of an Urgent line: the icon and color it gets. */
export type UrgentStatus = "behind" | "approaching" | "planned";

/** One line of the Urgent list: an open activity or a deadline. */
export interface UrgentItem {
  kind: "activity" | "deadline";
  /** The id of the activity or the deadline. */
  id: string;
  title: string;
  courseId: string | null;
  /** Activities: the class it belongs to, if any. */
  blockId: string | null;
  /** Activities: Monday of its week, to open the week view there. */
  week: string | null;
  /** Activities: since when it's open (end of its class, or Monday of its week). */
  openSince: string | null;
  /** Deadlines: when it's due. */
  dueAt: string | null;
  /** Deadlines: null if added by hand, else the feed it came from. */
  sourceId: string | null;
  /** Feed deadlines: where to open it in Ufora. */
  url: string | null;
  /** Feed deadlines that moved: the due time before the first move. */
  movedFrom: string | null;
  status: UrgentStatus;
  /** Higher = more urgent; the list comes sorted on it. */
  score: number;
}

export interface Week {
  monday: string;
  blocks: ClassBlock[];
  activities: Activity[];
}

export const api = {
  courses: () => invoke<Course[]>("courses"),
  courseKinds: () => invoke<CourseKind[]>("course_kinds"),
  week: (date: string) => invoke<Week>("week", { date }),
  templates: () => invoke<ActivityTemplate[]>("templates"),
  progress: () => invoke<WeekProgress[]>("progress"),
  classWeeks: () => invoke<WeekClasses[]>("class_weeks"),
  rules: () => invoke<FilterRule[]>("rules"),
  urgent: () => invoke<UrgentItem[]>("urgent"),
  deadlines: () => invoke<Deadline[]>("deadlines"),

  addBlockActivity: (blockId: string, title: string) =>
    invoke<string>("add_block_activity", { blockId, title }),
  addWeekActivity: (courseId: string | null, date: string, title: string) =>
    invoke<string>("add_week_activity", { courseId, date, title }),
  setActivityDone: (activityId: string, done: boolean) =>
    invoke<void>("set_activity_done", { activityId, done }),
  renameActivity: (activityId: string, title: string) =>
    invoke<void>("rename_activity", { activityId, title }),
  removeActivity: (activityId: string) => invoke<void>("remove_activity", { activityId }),
  restoreActivity: (activityId: string) => invoke<void>("restore_activity", { activityId }),

  /** `due` is RFC 3339, e.g. `date.toISOString()`. Returns the new id. */
  addDeadline: (courseId: string | null, title: string, due: string) =>
    invoke<string>("add_deadline", { courseId, title, due }),
  updateDeadline: (deadlineId: string, courseId: string | null, title: string, due: string) =>
    invoke<void>("update_deadline", { deadlineId, courseId, title, due }),
  setDeadlineDone: (deadlineId: string, done: boolean) =>
    invoke<void>("set_deadline_done", { deadlineId, done }),
  removeDeadline: (deadlineId: string) => invoke<void>("remove_deadline", { deadlineId }),
  restoreDeadline: (deadlineId: string) => invoke<void>("restore_deadline", { deadlineId }),

  /** Open a Ufora page in the browser. Only ufora.ugent.be links are allowed
   *  (src-tauri/capabilities/default.json). */
  openInBrowser: (url: string) => openUrl(url),

  addTemplate: (courseId: string, kind: string | null, title: string) =>
    invoke<number>("add_template", { courseId, kind, title }),
  removeTemplate: (templateId: string) => invoke<number>("remove_template", { templateId }),

  setBlockIgnored: (blockId: string, ignored: boolean | null) =>
    invoke<void>("set_block_ignored", { blockId, ignored }),
  acknowledgeChanges: (blockIds: string[]) => invoke<void>("acknowledge_changes", { blockIds }),

  feeds: () => invoke<FeedInfo[]>("feeds"),
  /** A TimeEdit or Ufora link; `kind` in the result says which it was. */
  addFeed: (url: string) => invoke<FeedSync>("add_feed", { url }),
  syncFeeds: () => invoke<FeedSync[]>("sync_feeds"),
  /** Returns how many classes the removed feed held. */
  removeFeed: (sourceId: string) => invoke<number>("remove_feed", { sourceId }),
  importHistory: (text: string) =>
    invoke<{ added: number; total: number }>("import_history", { text }),
  /** Returns how many classes changed. */
  addRule: (pattern: string) => invoke<number>("add_rule", { pattern }),
  removeRule: (ruleId: string) => invoke<number>("remove_rule", { ruleId }),
};
