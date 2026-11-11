<script lang="ts">
  // "Coming 2 weeks" from the Urgent mockup: from today to the end of next week,
  // grouped as "This week" and "Week 47". It lists deadlines ("Lab report 2 due")
  // and classes that were moved or cancelled; regular classes are in the week view.
  import { isIgnored, type ClassBlock, type Course, type Deadline } from "$lib/api";
  import { addDays, formatDate, isoDate, isoWeek, t } from "$lib/i18n/index.svelte";
  import { mondayOf } from "$lib/urgent/when";
  import { kindLabel } from "$lib/week/labels";

  let {
    deadlines,
    blocks,
    courses,
    now,
  }: {
    deadlines: Deadline[];
    /** The classes of this week and next week. */
    blocks: ClassBlock[];
    courses: Map<string, Course>;
    now: Date;
  } = $props();

  interface Line {
    key: string;
    time: Date;
    /** Cut off with an ellipsis when too long. */
    text: string;
    /** "moved", "cancelled", "done": always visible, after the text. */
    note: string | null;
    color: string | null;
    /** A done deadline or a cancelled class: drawn fainter. */
    faded: boolean;
  }

  const groups = $derived.by(() => {
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const nextMonday = addDays(mondayOf(today), 7);
    const end = addDays(nextMonday, 7);
    const inRange = (time: Date) => time >= today && time < end;

    const lines: Line[] = [];
    for (const deadline of deadlines) {
      const due = new Date(deadline.dueAt);
      if (deadline.status === "cancelled" || !inRange(due)) continue;
      const course = deadline.courseId ? courses.get(deadline.courseId) : undefined;
      lines.push({
        key: `deadline:${deadline.id}`,
        time: due,
        text: t("coming.due", { title: deadline.title }),
        note: deadline.done ? t("coming.done") : null,
        color: course?.color ?? null,
        faded: deadline.done,
      });
    }
    for (const block of blocks) {
      const start = new Date(block.startAt);
      if (block.status === "normal" || isIgnored(block) || !inRange(start)) continue;
      const course = block.courseId ? courses.get(block.courseId) : undefined;
      const name = [course?.name ?? block.title, kindLabel(block.kind)].filter(Boolean).join(" ");
      lines.push({
        key: `block:${block.id}`,
        time: start,
        text: name,
        note: t(block.status === "cancelled" ? "coming.cancelled" : "coming.moved"),
        color: course?.color ?? null,
        faded: block.status === "cancelled",
      });
    }
    lines.sort((a, b) => a.time.getTime() - b.time.getTime());

    return [
      { label: t("coming.thisWeek"), lines: lines.filter((l) => l.time < nextMonday) },
      {
        label: t("coming.week", { number: isoWeek(nextMonday) }),
        lines: lines.filter((l) => l.time >= nextMonday),
      },
    ].filter((group) => group.lines.length > 0);
  });

  /** "Thu 12" / "do 12" */
  function dayLabel(time: Date): string {
    return formatDate(time, { weekday: "short", day: "numeric" });
  }
</script>

<section class="card" aria-labelledby="coming-heading">
  <h2 id="coming-heading">{t("coming.title")}</h2>

  {#if groups.length === 0}
    <p class="empty">{t("coming.empty")}</p>
  {/if}

  {#each groups as group (group.label)}
    <div class="group">
      <h3>{group.label}</h3>
      {#each group.lines as line (line.key)}
        <div class="line" class:faded={line.faded}>
          <span class="day">{dayLabel(line.time)}</span>
          <span class="text" title={line.note ? `${line.text} · ${line.note}` : line.text}>
            <!-- No course: an empty dot, so the text still lines up. -->
            <span class="dot" style:background={line.color ?? "transparent"}></span>
            <span class="ellipsis">{line.text}</span>
            {#if line.note}<span class="note">· {line.note}</span>{/if}
          </span>
        </div>
      {/each}
    </div>
  {/each}

  <a href="/?date={isoDate(now)}">{t("coming.openWeek")}</a>
</section>

<style>
  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  h3 {
    font-size: var(--text-sm);
    font-weight: var(--weight-bold);
    color: var(--text-faint);
    text-transform: uppercase;
  }

  .line {
    display: flex;
    gap: var(--space-3);
    font-size: var(--text-md);
  }

  .line.faded .text {
    color: var(--text-done);
  }

  .day {
    flex: none;
    width: 52px;
    color: var(--text-muted);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .note {
    flex: none;
    color: var(--text-muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  .empty {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  a {
    align-self: flex-start;
    font-size: var(--text-base);
    color: var(--text-muted);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  a:hover {
    color: var(--control-hover);
  }
</style>
