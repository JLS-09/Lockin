// Names and lines for a study session, the same everywhere it shows.

import type { BlockProfile, Course, StudySession } from "$lib/api";
import { formatTime, t, tn } from "$lib/i18n/index.svelte";

/** The session's main name: its course, else its title, else "Study session". */
export function sessionName(session: StudySession, courses: Map<string, Course>): string {
  const course = session.courseId ? courses.get(session.courseId) : undefined;
  return course?.name ?? session.title ?? t("session.untitled");
}

/** "13:00–15:30" */
export function sessionTimes(session: StudySession): string {
  return `${formatTime(new Date(session.startAt))}–${formatTime(new Date(session.endAt))}`;
}

/** "Databanken · Exercises WPO 4", leaving out what's empty. */
export function sessionAbout(session: StudySession, courses: Map<string, Course>): string {
  const course = session.courseId ? courses.get(session.courseId)?.name : undefined;
  return [course, session.title].filter(Boolean).join(" · ");
}

/** "12 apps · 9 sites", as in the Session mockup; only the apps without websites. */
export function profileCount(profile: BlockProfile): string {
  const apps = tn("profiles.apps", profile.apps.length);
  return profile.sites.length > 0 ? `${apps} · ${tn("profiles.sites", profile.sites.length)}` : apps;
}
