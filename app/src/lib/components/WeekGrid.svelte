<script lang="ts">
  // The week as a CSS Grid, following the week view mockup: hour labels on the
  // left, a column per day, class blocks placed by their start and end time.
  // Built from scratch, no calendar library: everything can be styled with tokens.
  import type { Activity, ClassBlock, Course, FilterRule } from "$lib/api";
  import { isIgnored } from "$lib/api";
  import { formatDate, formatTime, t, tn } from "$lib/i18n/index.svelte";
  import { ignoringRule, kindLabel } from "$lib/week/labels";
  import { SLOT_MINUTES, columnForDate, rowForTime, type WeekLayout } from "$lib/week/layout";
  import { blockProgress } from "$lib/week/status";
  import StatusIcon from "./StatusIcon.svelte";

  let {
    layout,
    activities,
    courses,
    rules,
    now,
    onShowAnyway,
  }: {
    layout: WeekLayout;
    activities: Activity[];
    courses: Map<string, Course>;
    rules: FilterRule[];
    /** The current time, updated by the page every minute. */
    now: Date;
    /** "Show anyway" on an ignored block. */
    onShowAnyway: (block: ClassBlock) => void;
  } = $props();

  const slotsPerHour = 60 / SLOT_MINUTES;
  const hours = $derived(
    Array.from({ length: layout.lastHour - layout.firstHour }, (_, i) => layout.firstHour + i),
  );
  const todayColumn = $derived(columnForDate(layout, now));
  const nowRow = $derived(todayColumn === null ? null : rowForTime(layout, now));

  function hourLabel(hour: number): string {
    return formatTime(new Date(2000, 0, 1, hour));
  }

  /** "Fri 13:00" */
  function dayAndTime(text: string): string {
    const date = new Date(text);
    return `${formatDate(date, { weekday: "short" })} ${formatTime(date)}`;
  }
</script>

<div
  class="grid"
  style:grid-template-columns="var(--hour-column) repeat({layout.days.length}, minmax(0, 1fr))"
  style:grid-template-rows="var(--header-height) repeat({layout.slots}, calc(var(--hour-height) / {slotsPerHour}))"
>
  {#if todayColumn !== null}
    <div class="today" style:grid-column={todayColumn} style:grid-row="1 / -1"></div>
  {/if}

  {#each layout.days as day (day.column)}
    {@const isToday = day.column === todayColumn}
    <div class="day" class:is-today={isToday} style:grid-column={day.column} style:grid-row="1">
      <span class="weekday">{formatDate(day.date, { weekday: "short" })}</span>
      {day.date.getDate()}
      {#if isToday}<span class="pill">{t("week.today")}</span>{/if}
    </div>
  {/each}

  {#each hours as hour (hour)}
    <span class="hour" style:grid-row={2 + (hour - layout.firstHour) * slotsPerHour}>
      {hourLabel(hour)}
    </span>
  {/each}

  {#each layout.blocks as placed (placed.block.id)}
    {@const block = placed.block}
    {@const course = block.courseId ? courses.get(block.courseId) : undefined}
    {@const name = course?.name ?? block.title}
    {@const kind = kindLabel(block.kind)}
    {@const times = `${formatTime(new Date(block.startAt))}–${formatTime(new Date(block.endAt))}`}
    {@const position = `grid-column: ${placed.column}; grid-row: ${placed.rowStart} / ${placed.rowEnd}; --lane: ${placed.lane}; --lanes: ${placed.lanes}; --course: ${course?.color ?? "var(--text-faint)"}`}

    {#if isIgnored(block)}
      {@const rule = ignoringRule(block, rules)}
      <div class="blk ignored" style={position}>
        <div class="blk-title">
          <span class="ellipsis">{name}</span>
          <svg class="icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M3 3l18 18M10.6 6.1A9 9 0 0 1 21 12a10 10 0 0 1-2.3 3M6.6 6.6A10 10 0 0 0 3 12c2 4 5.5 6 9 6 1.6 0 3.1-.4 4.4-1.1" /></svg>
        </div>
        <span>{times}</span>
        <span>
          {#if rule}{t("block.ignoredByRule", { rule: rule.pattern })}
          {:else if block.ignoredOverride}{t("block.ignoredByHand")}
          {:else}{t("block.ignored")}{/if}
        </span>
        <button class="btn btn-small" onclick={() => onShowAnyway(block)}>{t("block.showAnyway")}</button>
      </div>
    {:else if block.status === "cancelled"}
      <div class="blk cancelled" style={position}>
        <div class="blk-title">
          <span class="name"><span class="dot" aria-hidden="true"></span><s class="ellipsis">{name}</s></span>
          <span class="cancelled-label">{t("block.cancelled")}</span>
        </div>
        <span>{kind ? `${kind} · ${times}` : times}</span>
      </div>
    {:else}
      {@const progress = blockProgress(block, activities, now)}
      {@const showCount = progress.status === "partial" || progress.status === "behind"}
      <div class="blk" style={position}>
        <div class="blk-title">
          <span class="name"><span class="dot" aria-hidden="true"></span><span class="ellipsis">{name}</span></span>
          {#if progress.status}<StatusIcon status={progress.status} />{/if}
        </div>
        <span>{kind ? `${kind} · ${times}` : times}</span>
        {#if showCount}
          <span>{tn("block.activities", progress.total, { done: progress.done, total: progress.total })}</span>
        {:else if block.location}
          <span>{block.location}</span>
        {/if}
        {#if block.status === "moved"}
          <span class="moved">
            <svg class="icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
            <span class="ellipsis">
              {block.movedFrom
                ? t("block.movedFrom", { from: dayAndTime(block.movedFrom) })
                : t("block.moved")}
            </span>
          </span>
        {/if}
      </div>
    {/if}
  {/each}

  {#if todayColumn !== null && nowRow !== null}
    <div class="now" style:grid-column={todayColumn} style:grid-row={nowRow} aria-hidden="true"></div>
  {/if}
</div>

<style>
  .grid {
    /* Sizes from the mockup: 52 px per hour, a 40 px header row. */
    --hour-column: 52px;
    --header-height: 40px;
    --hour-height: 52px;

    display: grid;
    min-width: 680px;
    /* A line every hour, starting under the header row. */
    background-image: repeating-linear-gradient(
      to bottom,
      var(--grid-line) 0 1px,
      transparent 1px var(--hour-height)
    );
    background-position: 0 var(--header-height);
    background-repeat: repeat-x;
    background-size: 100% 100%;
  }

  .today {
    background: var(--today-column);
    border-radius: var(--radius);
  }

  .day {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    font-size: var(--text-base);
    color: var(--text-muted);
  }

  .weekday {
    font-weight: var(--weight-bold);
    color: var(--text);
    text-transform: capitalize;
  }

  .is-today {
    color: var(--text);
  }

  .is-today .weekday {
    font-weight: var(--weight-heavy);
  }

  .pill {
    margin-left: auto;
    font-size: var(--text-sm);
    padding: 2px var(--space-2);
    border-radius: 99px;
    background: var(--pill);
  }

  .hour {
    grid-column: 1;
    margin-top: -7px;
    padding-left: 6px;
    font-size: var(--text-sm);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  .blk {
    /* Overlapping blocks share the day column: each gets 1/lanes of it. */
    width: calc((100% - 8px) / var(--lanes));
    margin: 2px 4px 2px calc(4px + (100% - 8px) * var(--lane) / var(--lanes));
    padding: 6px 10px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--course) var(--course-tint), var(--bg));
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    position: relative;
    z-index: 1;
    font-size: var(--text-sm);
    color: var(--text-muted);
    transition: transform var(--duration-micro) var(--ease-out);
  }

  .blk:hover {
    transform: translateY(-1px);
  }

  .blk > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .blk-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    font-size: var(--text-base);
    font-weight: var(--weight-bold);
    color: var(--text);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  /* Long course names end in "…" instead of being cut off. */
  .ellipsis {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .icon {
    flex: none;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
    background: var(--course);
  }

  .moved {
    align-self: flex-start;
    max-width: 100%;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    margin-top: 2px;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: var(--tag);
    color: var(--text);
  }

  .cancelled {
    background: var(--cancelled-surface);
    border: 1px solid var(--border-strong);
  }

  .cancelled .name {
    color: var(--text-muted);
  }

  .cancelled-label {
    flex: none;
    font-size: var(--text-sm);
    font-weight: var(--weight-bold);
    color: var(--text-cancelled);
  }

  .ignored {
    background: transparent;
    border: 1px dashed var(--border-dashed);
    color: var(--text-faint);
  }

  .ignored .blk-title {
    color: var(--text-ignored);
    font-weight: var(--weight-medium);
  }

  .btn-small {
    white-space: nowrap;
    margin-top: 6px;
    align-self: flex-start;
    min-height: 28px;
    padding: 0 10px;
    font-size: var(--text-sm);
  }

  .now {
    align-self: start;
    height: 2px;
    background: var(--text);
    border-radius: 1px;
    position: relative;
    z-index: 2;
    pointer-events: none;
  }

  .now::before {
    content: "";
    position: absolute;
    left: -4px;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text);
  }
</style>
