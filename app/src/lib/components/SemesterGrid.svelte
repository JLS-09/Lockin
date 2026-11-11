<script lang="ts" module>
  import type { Course } from "$lib/api";
  import type { CellStatus } from "$lib/week/semester";

  /** One course: its cells, one per week, and its "Done" percentage. */
  export interface Row {
    course: Course;
    cells: CellStatus[];
    percent: number | null;
  }
</script>

<script lang="ts">
  // The semester mockup's grid: a row per course, a column per week, a status
  // per cell and "Done" per course at the end.
  // Click or arrow keys select a cell; Enter or a double click opens that week.
  import { addDays, formatRange, parseIsoDate, t } from "$lib/i18n/index.svelte";
  import StatusIcon from "./StatusIcon.svelte";

  let {
    weeks,
    currentIndex,
    rows,
    selected,
    onSelect,
    onOpen,
  }: {
    /** Mondays of the shown weeks. */
    weeks: string[];
    /** Column of this week, or -1. */
    currentIndex: number;
    rows: Row[];
    selected: { row: number; week: number } | null;
    onSelect: (row: number, week: number) => void;
    /** Open this week in the week view. */
    onOpen: (week: number) => void;
  } = $props();

  let grid: HTMLDivElement;

  const labels: Record<CellStatus, string> = $derived({
    done: t("status.done"),
    partial: t("status.partial"),
    behind: t("semester.notDone"),
    planned: t("status.planned"),
    none: t("semester.noClasses"),
    empty: t("semester.noActivities"),
  });

  function weekRange(week: string): string {
    const monday = parseIsoDate(week);
    return formatRange(monday, addDays(monday, 4));
  }

  // Arrow keys move the selection through the grid, Enter opens the week.
  function onKeydown(event: KeyboardEvent, row: number, week: number) {
    let next: [number, number] | null = null;
    if (event.key === "ArrowLeft") next = [row, Math.max(0, week - 1)];
    else if (event.key === "ArrowRight") next = [row, Math.min(weeks.length - 1, week + 1)];
    else if (event.key === "ArrowUp") next = [Math.max(0, row - 1), week];
    else if (event.key === "ArrowDown") next = [Math.min(rows.length - 1, row + 1), week];
    else if (event.key === "Enter") {
      event.preventDefault();
      onOpen(week);
      return;
    }
    if (!next) return;
    event.preventDefault();
    onSelect(...next);
    grid.querySelector<HTMLButtonElement>(`[data-cell="${next[0]}-${next[1]}"]`)?.focus();
  }
</script>

<div
  class="grid"
  bind:this={grid}
  style:grid-template-columns="210px repeat({weeks.length}, minmax(0, 1fr)) 76px"
  style:min-width="calc(286px + {weeks.length} * 48px)"
>
  <span class="head course-head">{t("semester.course")}</span>
  {#each weeks as week, i (week)}
    <span class="head week" class:current={i === currentIndex} title={weekRange(week)}>
      {t("semester.weekShort", { number: i + 1 })}
    </span>
  {/each}
  <span class="head done-head">{t("semester.done")}</span>

  {#each rows as row, r (row.course.id)}
    <span class="course" title={row.course.name}>
      <span class="dot" style:background={row.course.color}></span>
      <span class="ellipsis">{row.course.name}</span>
    </span>
    {#each row.cells as cell, w (weeks[w])}
      {@const isSelected = selected?.row === r && selected?.week === w}
      <button
        class="cell"
        class:sel={isSelected}
        data-cell="{r}-{w}"
        tabindex={isSelected || (!selected && r === 0 && w === 0) ? 0 : -1}
        aria-label={t("semester.cellLabel", { course: row.course.name, number: w + 1, status: labels[cell] })}
        title="{weekRange(weeks[w])} · {labels[cell]}"
        onclick={() => onSelect(r, w)}
        ondblclick={() => onOpen(w)}
        onkeydown={(e) => onKeydown(e, r, w)}
      >
        {#if cell === "none"}
          <span class="none"></span>
        {:else if cell !== "empty"}
          <StatusIcon status={cell} />
        {/if}
      </button>
    {/each}
    <span class="percent">{row.percent === null ? "–" : `${row.percent}%`}</span>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    align-items: center;
  }

  .head {
    font-size: var(--text-sm);
    color: var(--text-faint);
  }

  .course-head {
    padding: 0 var(--space-2);
  }

  .week {
    text-align: center;
    padding: 6px 0;
    font-weight: var(--weight-medium);
    color: var(--text-muted);
  }

  .week.current {
    font-weight: var(--weight-heavy);
    color: var(--text);
  }

  .done-head {
    text-align: right;
    padding: 0 var(--space-2);
  }

  .course {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    padding: 0 var(--space-2);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  .cell {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 44px;
    margin: 2px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: var(--surface-2);
    cursor: pointer;
    transition:
      background-color var(--duration-micro) var(--ease-out),
      transform var(--duration-micro) var(--ease-out);
  }

  .cell:hover {
    background: var(--surface-active);
  }

  .cell:active {
    transform: scale(0.95);
  }

  .cell.sel {
    border-color: var(--control);
    background: var(--surface-active);
  }

  .none {
    width: 14px;
    height: 2px;
    background: var(--text-none);
  }

  .percent {
    text-align: right;
    padding: 0 var(--space-2);
    font-size: var(--text-md);
    font-variant-numeric: tabular-nums;
  }
</style>
