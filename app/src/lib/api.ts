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

/** Earlier weeks imported from a downloaded TimeEdit file. */
export interface HistoryImport {
  id: string;
  /** The file name; null for imports from before Lockin remembered them. */
  name: string | null;
  importedAt: string;
  /** Classes it still holds (not taken over by the live feed). */
  classes: number;
  /** Start of its first and last class. */
  firstStart: string | null;
  lastStart: string | null;
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

export type SessionStatus = "planned" | "running" | "done" | "stopped" | "missed" | "skipped";

/** A planned block of study time; the daemon starts and ends it on time. */
export interface StudySession {
  id: string;
  /** The weekly repeat it belongs to; null if it was planned once. */
  seriesId: string | null;
  courseId: string | null;
  /** What you plan to do, e.g. "Exercises WPO 4"; null for a plain session. */
  title: string | null;
  startAt: string;
  endAt: string;
  status: SessionStatus;
  /** The block profile it closes apps with; null = blocks nothing. */
  profileId: string | null;
}

export type SessionEventKind = "start" | "break" | "resume" | "emergency" | "end";

/** One line of a session's log. */
export interface SessionEvent {
  id: string;
  sessionId: string;
  kind: SessionEventKind;
  at: string;
  /** "daemon" or "late" on a start, "auto" on a resume when the break ran out. */
  detail: string | null;
}

export type SessionPhase = "planned" | "working" | "break" | "over";

/** Where a session stands, worked out from its log (`session::state`). */
export interface SessionState {
  phase: SessionPhase;
  workedSeconds: number;
  creditEarnedSeconds: number;
  creditUsedSeconds: number;
  creditLeftSeconds: number;
  /** During a break: when blocking turns back on. */
  breakEndsAt: string | null;
  canTakeBreak: boolean;
}

/** The running session with its state and log. */
export interface CurrentSession {
  session: StudySession;
  state: SessionState;
  events: SessionEvent[];
  /** The break rules from session.rs: `creditMinutes` per `creditEveryMinutes` studied,
   *  one break at most `breakMinutes`. */
  creditMinutes: number;
  creditEveryMinutes: number;
  breakMinutes: number;
  /** The emergency stop: keep it open this long; it costs these points. */
  emergencyWaitSeconds: number;
  emergencyPenaltyPoints: number;
}

/** Where the emergency stop stands, from the daemon. */
export interface EmergencyStatus {
  /** RFC 3339 in UTC. */
  startedAt: string;
  /** The session ends then, if the stop stays open. */
  stopsAt: string;
  /** True once the daemon stopped the session (and took the points). */
  stopped: boolean;
  penaltyPoints: number;
}

export interface PlanResult {
  seriesId: string;
  /** Sessions made. */
  planned: number;
  /** Weeks left out because they overlap another session. */
  overlapping: number;
}

/** An app on a block profile. */
export interface BlockedApp {
  id: string;
  profileId: string;
  /** The desktop file id, or "program:<name>" for a program added by name. */
  appId: string;
  name: string;
  /** Program names to match, one per line. */
  execNames: string;
  flatpakId: string | null;
}

/** A website on a block profile, e.g. "youtube.com" (www. and m. are blocked with it). */
export interface BlockedSite {
  id: string;
  profileId: string;
  domain: string;
}

/** A block profile with its apps and websites, e.g. "Strict". */
export interface BlockProfile {
  id: string;
  name: string;
  apps: BlockedApp[];
  sites: BlockedSite[];
  /** True while the running session blocks with it: apps can be added, not removed. */
  inUse: boolean;
}

/** An app from the menu, for the picker. */
export interface InstalledApp {
  id: string;
  name: string;
  execNames: string[];
  flatpakId: string | null;
}

/** How often the daemon closed an app during a session. */
export interface AppCloses {
  appId: string;
  name: string;
  count: number;
  lastAt: string;
}

/** The daemon's answer to "are you there?". */
export interface DaemonStatus {
  version: string;
  startedAt: string;
  /** The most recent feed sync; null without feeds. */
  lastSyncAt: string | null;
  /** Why websites can't be blocked right now (e.g. the helper isn't set up). */
  websitesProblem: string | null;
}

export const api = {
  /** Settings shared with the daemon (language, notify.*); missing keys = default. */
  settings: () => invoke<Record<string, string>>("settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  /** Ask the daemon to show a test notification. */
  testNotification: () => invoke<void>("test_notification"),
  /** The page asked for with `--page` when the app started (the tray's emergency stop), or null. */
  startupPage: () => invoke<string | null>("startup_page"),
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
  /** `name` is the file name, shown in Settings to remove the import again. */
  importHistory: (text: string, name: string | null) =>
    invoke<{ added: number; total: number }>("import_history", { text, name }),
  historyImports: () => invoke<HistoryImport[]>("history_imports"),
  /** Returns how many classes were hidden. */
  removeHistoryImport: (importId: string) =>
    invoke<number>("remove_history_import", { importId }),
  /** Returns how many classes changed. */
  addRule: (pattern: string) => invoke<number>("add_rule", { pattern }),
  removeRule: (ruleId: string) => invoke<number>("remove_rule", { ruleId }),

  /** Sessions that start between `from` and `to` (RFC 3339). */
  sessions: (from: string, to: string) => invoke<StudySession[]>("sessions", { from, to }),
  /** The running session, or null. */
  currentSession: () => invoke<CurrentSession | null>("current_session"),
  /** Returns the new id. Times are RFC 3339. */
  planSession: (
    courseId: string | null,
    title: string | null,
    profileId: string | null,
    start: string,
    end: string,
  ) => invoke<string>("plan_session", { courseId, title, profileId, start, end }),
  /** Every week from this first one, until `until` (YYYY-MM-DD) or the end of the semester. */
  planWeekly: (
    courseId: string | null,
    title: string | null,
    profileId: string | null,
    start: string,
    end: string,
    until: string | null,
  ) => invoke<PlanResult>("plan_weekly", { courseId, title, profileId, start, end, until }),
  updateSession: (
    sessionId: string,
    courseId: string | null,
    title: string | null,
    profileId: string | null,
    start: string,
    end: string,
  ) => invoke<void>("update_session", { sessionId, courseId, title, profileId, start, end }),
  /** The profile of the last planned session, the default for a new one. */
  lastProfile: () => invoke<string | null>("last_profile"),
  removeSession: (sessionId: string) => invoke<void>("remove_session", { sessionId }),
  restoreSession: (sessionId: string) => invoke<void>("restore_session", { sessionId }),
  /** This session and the later weeks of its repeat. Returns how many were removed. */
  removeFollowingSessions: (sessionId: string) =>
    invoke<number>("remove_following_sessions", { sessionId }),

  /** The daemon's status, or null when it isn't running. */
  daemonStatus: () => invoke<DaemonStatus | null>("daemon_status"),
  /** These go through the daemon; they fail when it isn't running. */
  takeBreak: () => invoke<void>("take_break"),
  resumeSession: () => invoke<void>("resume_session"),
  /** Open the emergency stop, or keep it open: call it every 2 seconds while
   *  it's open. After 2 minutes the daemon ends the session (`stopped: true`). */
  emergencyKeepOpen: () => invoke<EmergencyStatus>("emergency_keep_open"),
  /** "Keep studying": close the emergency stop. */
  emergencyCancel: () => invoke<void>("emergency_cancel"),
  /** Points per session ("−50" on a stopped one); sessions without points are missing. */
  sessionPoints: (sessionIds: string[]) =>
    invoke<Record<string, number>>("session_points", { sessionIds }),

  blockProfiles: () => invoke<BlockProfile[]>("block_profiles"),
  /** Returns the new profile's id. */
  addBlockProfile: (name: string) => invoke<string>("add_block_profile", { name }),
  renameBlockProfile: (profileId: string, name: string) =>
    invoke<void>("rename_block_profile", { profileId, name }),
  /** Planned sessions that use the profile, to ask before deleting it. */
  sessionsUsingProfile: (profileId: string) =>
    invoke<number>("sessions_using_profile", { profileId }),
  /** Returns how many planned sessions now block nothing. */
  removeBlockProfile: (profileId: string) => invoke<number>("remove_block_profile", { profileId }),
  installedApps: () => invoke<InstalledApp[]>("installed_apps"),
  addBlockedApp: (profileId: string, app: InstalledApp) =>
    invoke<string>("add_blocked_app", { profileId, app }),
  /** A program without a menu entry, by its process name. */
  addBlockedProgram: (profileId: string, program: string) =>
    invoke<string>("add_blocked_program", { profileId, program }),
  removeBlockedApp: (entryId: string) => invoke<void>("remove_blocked_app", { entryId }),
  /** Typed any way ("https://www.youtube.com/watch"); stored as the domain. */
  addBlockedSite: (profileId: string, website: string) =>
    invoke<string>("add_blocked_site", { profileId, website }),
  removeBlockedSite: (entryId: string) => invoke<void>("remove_blocked_site", { entryId }),
  sessionCloses: (sessionId: string) => invoke<AppCloses[]>("session_closes", { sessionId }),
};
