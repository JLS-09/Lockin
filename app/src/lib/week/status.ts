// The status of a class block, from its activities. It decides the color
// and the icon, following the status table in the project plan.

import type { Activity, ClassBlock } from "$lib/api";

export type Status = "done" | "partial" | "behind" | "planned";

export interface BlockProgress {
  done: number;
  total: number;
  /** null when the block has no activities: then there's nothing to show. */
  status: Status | null;
}

/**
 * - all activities done: done
 * - some done: partial
 * - none done and the class is over: behind
 * - none done and the class is still to come: planned
 */
export function blockProgress(block: ClassBlock, activities: Activity[], now: Date): BlockProgress {
  const own = activities.filter((a) => a.blockId === block.id);
  const done = own.filter((a) => a.done).length;
  const total = own.length;

  let status: Status | null = null;
  if (total > 0) {
    if (done === total) status = "done";
    else if (done > 0) status = "partial";
    else if (new Date(block.endAt) < now) status = "behind";
    else status = "planned";
  }
  return { done, total, status };
}
