// Everything the Urgent page shows, and every change to it, in one place.
//
// "Feedback first", as on the week page: a change shows at once and saves
// after; if saving fails it rolls back and `error` says why.
// A line you check off stays where it is ("Done just now") until the list
// reloads (you leave the page, or add or edit a deadline), so a slip is one
// click to undo and the list doesn't jump.

import { SvelteSet } from "svelte/reactivity";

import {
  api,
  type ClassBlock,
  type Course,
  type Deadline,
  type FeedInfo,
  type UrgentItem,
} from "$lib/api";
import { addDays, isoDate } from "$lib/i18n/index.svelte";

/** "activity:<id>" or "deadline:<id>": an activity and a deadline can't share an id then. */
export function itemKey(item: { kind: string; id: string }): string {
  return `${item.kind}:${item.id}`;
}

/** What the deadline dialog hands back. */
export interface DeadlineFields {
  title: string;
  courseId: string | null;
  /** RFC 3339, from `Date.toISOString()`. */
  due: string;
}

export class UrgentData {
  items = $state<UrgentItem[]>([]);
  /** Every deadline, done ones too: for "Coming 2 weeks" and the dialog. */
  deadlines = $state<Deadline[]>([]);
  courses = $state<Course[]>([]);
  feeds = $state<FeedInfo[]>([]);
  /** The classes of this week and next week, for the schedule changes in "Coming 2 weeks". */
  blocks = $state<ClassBlock[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  /** Lines checked off during this visit (by `itemKey`). */
  checked = new SvelteSet<string>();

  courseById = $derived(new Map(this.courses.map((c) => [c.id, c])));

  async load() {
    try {
      const today = new Date();
      const [items, deadlines, courses, feeds, thisWeek, nextWeek] = await Promise.all([
        api.urgent(),
        api.deadlines(),
        api.courses(),
        api.feeds(),
        api.week(isoDate(today)),
        api.week(isoDate(addDays(today, 7))),
      ]);
      this.items = items;
      this.deadlines = deadlines;
      this.courses = courses;
      this.feeds = feeds;
      this.blocks = [...thisWeek.blocks, ...nextWeek.blocks];
      this.loaded = true;
      this.error = null;
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Reload the list and the deadlines after an add or an edit: the order may change. */
  async #reloadLists() {
    try {
      [this.items, this.deadlines] = await Promise.all([api.urgent(), api.deadlines()]);
    } catch (e) {
      this.error = String(e);
    }
  }

  isDone(item: UrgentItem): boolean {
    return this.checked.has(itemKey(item));
  }

  /** Check a line off, or on again. */
  async toggle(item: UrgentItem) {
    const key = itemKey(item);
    const done = !this.checked.has(key);
    const show = (value: boolean) => (value ? this.checked.add(key) : this.checked.delete(key));
    show(done);
    this.#setDeadlineDone(item, done);
    try {
      if (item.kind === "activity") await api.setActivityDone(item.id, done);
      else await api.setDeadlineDone(item.id, done);
    } catch (e) {
      show(!done);
      this.#setDeadlineDone(item, !done);
      this.error = String(e);
    }
  }

  /** Keep "Coming 2 weeks" in step with a checked-off deadline. */
  #setDeadlineDone(item: UrgentItem, done: boolean) {
    if (item.kind !== "deadline") return;
    this.deadlines = this.deadlines.map((d) => (d.id === item.id ? { ...d, done } : d));
  }

  // ---------- Deadlines ----------

  deadline(id: string): Deadline | undefined {
    return this.deadlines.find((d) => d.id === id);
  }

  /** Returns false when saving failed, so the dialog stays open. */
  async addDeadline(fields: DeadlineFields): Promise<boolean> {
    try {
      await api.addDeadline(fields.courseId, fields.title, fields.due);
      await this.#reloadLists();
      return true;
    } catch (e) {
      this.error = String(e);
      return false;
    }
  }

  async updateDeadline(id: string, fields: DeadlineFields): Promise<boolean> {
    try {
      await api.updateDeadline(id, fields.courseId, fields.title, fields.due);
      await this.#reloadLists();
      return true;
    } catch (e) {
      this.error = String(e);
      return false;
    }
  }

  /** Delete a deadline. Returns true if it worked, so the page can offer "Undo". */
  async removeDeadline(deadline: Deadline): Promise<boolean> {
    const items = this.items;
    const deadlines = this.deadlines;
    this.items = items.filter((i) => !(i.kind === "deadline" && i.id === deadline.id));
    this.deadlines = deadlines.filter((d) => d.id !== deadline.id);
    try {
      await api.removeDeadline(deadline.id);
      return true;
    } catch (e) {
      this.items = items;
      this.deadlines = deadlines;
      this.error = String(e);
      return false;
    }
  }

  /** Undo `removeDeadline`. */
  async restoreDeadline(deadline: Deadline) {
    try {
      await api.restoreDeadline(deadline.id);
      await this.#reloadLists();
    } catch (e) {
      this.error = String(e);
    }
  }
}
