// Planning, changing and removing study sessions, shared by the week page and
// the Session page. Sessions that haven't started are changed in the database
// directly; a running one only through the daemon (`live`).

import { api, type StudySession } from "$lib/api";

/** What the session dialog gives back. Times are RFC 3339. */
export interface SessionFields {
  courseId: string | null;
  title: string | null;
  start: string;
  end: string;
  /** New sessions only: every week until the end of the semester. */
  weekly: boolean;
}

/** What saving did, for the toast. */
export interface SaveResult {
  /** Sessions made: 1, or the weeks of a repeat. */
  planned: number;
  /** Weeks of a repeat left out because they overlap another session. */
  overlapping: number;
}

/** Plan a new session (`id` null) or change one. Throws with the reason when it can't. */
export async function saveSession(id: string | null, fields: SessionFields): Promise<SaveResult> {
  const { courseId, title, start, end } = fields;
  if (id) {
    await api.updateSession(id, courseId, title, start, end);
    return { planned: 1, overlapping: 0 };
  }
  if (fields.weekly) {
    const result = await api.planWeekly(courseId, title, start, end, null);
    return { planned: result.planned, overlapping: result.overlapping };
  }
  await api.planSession(courseId, title, start, end);
  return { planned: 1, overlapping: 0 };
}

/** The sessions of the week starting on `monday` (local midnight). */
export function sessionsOfWeek(monday: Date): Promise<StudySession[]> {
  const end = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + 7);
  return api.sessions(monday.toISOString(), end.toISOString());
}

/** True for a session you can still change: planned and not started yet. */
export function isEditable(session: StudySession, now: Date): boolean {
  return session.status === "planned" && new Date(session.startAt) > now;
}
