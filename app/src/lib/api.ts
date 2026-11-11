// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
// Components call these instead of invoke() directly, so the command names
// and argument names live in one place.
// Dates are "YYYY-MM-DD" text; times are RFC 3339 text in UTC.

import { invoke } from "@tauri-apps/api/core";

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
  kind: string;
  /** "cloud.timeedit.net", or the file name. */
  origin: string;
  lastFetchedAt: string | null;
  classes: number;
}

/** What syncing one feed changed, or why it failed. */
export interface FeedSync {
  sourceId: string;
  added: number;
  /** Known classes seen again, changed or not. */
  updated: number;
  /** Classes whose time changed. */
  moved: number;
  cancelled: number;
  error: string | null;
}

export interface SyncResult {
  added: number;
  updated: number;
  moved: number;
  cancelled: number;
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

  addTemplate: (courseId: string, kind: string | null, title: string) =>
    invoke<number>("add_template", { courseId, kind, title }),
  removeTemplate: (templateId: string) => invoke<number>("remove_template", { templateId }),

  setBlockIgnored: (blockId: string, ignored: boolean | null) =>
    invoke<void>("set_block_ignored", { blockId, ignored }),
  acknowledgeChanges: (blockIds: string[]) => invoke<void>("acknowledge_changes", { blockIds }),

  feeds: () => invoke<FeedInfo[]>("feeds"),
  addFeed: (url: string) => invoke<SyncResult>("add_feed", { url }),
  syncFeeds: () => invoke<FeedSync[]>("sync_feeds"),
  /** Returns how many classes the removed feed held. */
  removeFeed: (sourceId: string) => invoke<number>("remove_feed", { sourceId }),
  importHistory: (text: string) =>
    invoke<{ added: number; total: number }>("import_history", { text }),
  /** Returns how many classes changed. */
  addRule: (pattern: string) => invoke<number>("add_rule", { pattern }),
  removeRule: (ruleId: string) => invoke<number>("remove_rule", { ruleId }),
};
