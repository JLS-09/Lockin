// Everything the week page shows, and every change to it, in one place.
//
// Changes follow "feedback first": the screen changes at once, saving happens
// after. If saving fails, the change is rolled back and `error` says why.
// Both the shown week and today's week are kept in step, because the Today
// panel can show activities of another week than the grid.

import {
  api,
  type Activity,
  type ClassBlock,
  type Course,
  type FilterRule,
  type Week,
} from "$lib/api";
import { isoDate } from "$lib/i18n/index.svelte";

export class WeekData {
  /** Any day in the shown week. */
  date = $state(new Date());
  /** The shown week. */
  week = $state<Week | null>(null);
  /** The week with today in it, for the Today panel. */
  todayWeek = $state<Week | null>(null);
  courses = $state<Course[]>([]);
  rules = $state<FilterRule[]>([]);
  error = $state<string | null>(null);

  courseById = $derived(new Map(this.courses.map((c) => [c.id, c])));

  #latest = 0;

  async loadCoursesAndRules() {
    try {
      [this.courses, this.rules] = await Promise.all([api.courses(), api.rules()]);
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Load the week of `date`. A slow answer for an older request is dropped. */
  async loadWeek() {
    const request = ++this.#latest;
    try {
      const week = await api.week(isoDate(this.date));
      if (request !== this.#latest) return;
      this.week = week;
      this.error = null;
    } catch (e) {
      if (request === this.#latest) this.error = String(e);
    }
  }

  async loadToday() {
    try {
      this.todayWeek = await api.week(isoDate(new Date()));
    } catch (e) {
      this.error = String(e);
    }
  }

  // ---------- Activities ----------

  /** Check an activity off, or on. */
  async toggle(activity: Activity) {
    const done = !activity.done;
    this.#change(activity.id, { done });
    try {
      await api.setActivityDone(activity.id, done);
    } catch (e) {
      this.#change(activity.id, { done: !done });
      this.error = String(e);
    }
  }

  async rename(activity: Activity, title: string) {
    const old = activity.title;
    this.#change(activity.id, { title });
    try {
      await api.renameActivity(activity.id, title);
    } catch (e) {
      this.#change(activity.id, { title: old });
      this.error = String(e);
    }
  }

  /** Delete an activity. Returns true if it worked, so the page can offer "Undo". */
  async remove(activity: Activity): Promise<boolean> {
    this.#edit((list) => list.filter((a) => a.id !== activity.id));
    try {
      await api.removeActivity(activity.id);
      return true;
    } catch (e) {
      this.#insert(activity);
      this.error = String(e);
      return false;
    }
  }

  /** Undo `remove`. */
  async restore(activity: Activity) {
    this.#insert(activity);
    try {
      await api.restoreActivity(activity.id);
    } catch (e) {
      this.#edit((list) => list.filter((a) => a.id !== activity.id));
      this.error = String(e);
    }
  }

  /** Add an activity to a class block. */
  async addToBlock(block: ClassBlock, title: string) {
    if (!this.week) return;
    try {
      const id = await api.addBlockActivity(block.id, title);
      // You can only open blocks of the shown week, so it belongs to that week.
      this.#insert(newActivity(id, title, this.week.monday, block.id, block.courseId));
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Add an activity to the shown week, not tied to a class. */
  async addToWeek(courseId: string | null, title: string) {
    if (!this.week) return;
    try {
      const id = await api.addWeekActivity(courseId, this.week.monday, title);
      this.#insert(newActivity(id, title, this.week.monday, null, courseId));
    } catch (e) {
      this.error = String(e);
    }
  }

  // ---------- Blocks ----------

  /** "Got it" on the schedule changes banner. */
  async acknowledgeChanges(blocks: ClassBlock[]) {
    const ids = new Set(blocks.map((b) => b.id));
    const mark = (week: Week) => ({
      ...week,
      blocks: week.blocks.map((b) => (ids.has(b.id) ? { ...b, changeSeen: true } : b)),
    });
    if (this.week) this.week = mark(this.week);
    try {
      await api.acknowledgeChanges([...ids]);
    } catch (e) {
      this.error = String(e);
    }
  }

  /** "Show anyway": a manual exception that keeps this block, whatever the rules say. */
  async showAnyway(block: ClassBlock) {
    try {
      await api.setBlockIgnored(block.id, false);
      // Keeping a block can give it template activities: reload both weeks.
      await Promise.all([this.loadWeek(), this.loadToday()]);
    } catch (e) {
      this.error = String(e);
    }
  }

  // ---------- Helpers ----------

  /** Apply the same change to the activities of both weeks. */
  #edit(change: (list: Activity[]) => Activity[]) {
    if (this.week) this.week = { ...this.week, activities: change(this.week.activities) };
    if (this.todayWeek) {
      this.todayWeek = { ...this.todayWeek, activities: change(this.todayWeek.activities) };
    }
  }

  #change(id: string, fields: Partial<Activity>) {
    this.#edit((list) => list.map((a) => (a.id === id ? { ...a, ...fields } : a)));
  }

  /** Put an activity in every loaded week it belongs to. */
  #insert(activity: Activity) {
    for (const key of ["week", "todayWeek"] as const) {
      const week = this[key];
      if (!week || week.activities.some((a) => a.id === activity.id)) continue;
      const belongs = activity.blockId
        ? week.blocks.some((b) => b.id === activity.blockId)
        : activity.week === week.monday;
      if (belongs) this[key] = { ...week, activities: [...week.activities, activity] };
    }
  }
}

function newActivity(
  id: string,
  title: string,
  week: string,
  blockId: string | null,
  courseId: string | null,
): Activity {
  return { id, blockId, templateId: null, courseId, title, week, done: false, doneAt: null };
}
