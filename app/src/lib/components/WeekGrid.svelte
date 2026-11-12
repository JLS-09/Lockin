<script lang="ts">
  // The week as a CSS Grid, following the week view mockup: hour labels on the
  // left, a column per day, class blocks placed by their start and end time.
  // Built from scratch, no calendar library: everything can be styled with tokens.
  // Deadlines (not in the mockups) sit under the day headers as small chips,
  // like all-day events in a calendar: most are due at 23:59, after the grid ends.
  // Study sessions are blocks with a lock, as in the mockup: planned ones with a
  // border, the running one pulsing with "Live" and the time left.
  import type {
    Activity,
    BlockProfile,
    ClassBlock,
    Course,
    Deadline,
    FilterRule,
    StudySession,
  } from "$lib/api";
  import { hoursText, live } from "$lib/session/live.svelte";
  import { sessionAbout, sessionTimes } from "$lib/session/labels";
  import { isIgnored } from "$lib/api";
  import { formatDate, formatDay, formatTime, isoDate, t, tn } from "$lib/i18n/index.svelte";
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
    selectedId,
    deadlines,
    onSelect,
    onShowAnyway,
    onOpenDeadline,
    onOpenSession,
    profiles = [],
  }: {
    layout: WeekLayout;
    activities: Activity[];
    courses: Map<string, Course>;
    rules: FilterRule[];
    /** The current time, updated by the page every minute. */
    now: Date;
    /** The block whose panel is open. */
    selectedId: string | null;
    /** Deadlines; the ones due on a shown day get a chip under its header. */
    deadlines: Deadline[];
    /** Click (or Enter / Space) on a block. */
    onSelect: (block: ClassBlock) => void;
    /** "Show anyway" on an ignored block. */
    onShowAnyway: (block: ClassBlock) => void;
    /** Click on a deadline chip. */
    onOpenDeadline: (deadline: Deadline) => void;
    /** Click on a study session. */
    onOpenSession: (session: StudySession) => void;
    /** For the profile name on a session ("13:00–15:30 · Strict"). */
    profiles?: BlockProfile[];
  } = $props();

  /** "13:00–15:30 · Strict", as in the mockup. */
  function sessionLine(session: StudySession): string {
    const profile = profiles.find((p) => p.id === session.profileId);
    return profile ? `${sessionTimes(session)} · ${profile.name}` : sessionTimes(session);
  }

  /** The session's status, with the running one taken from `live` (it may have started since loading). */
  function sessionStatus(session: StudySession): string {
    if (live.current?.session.id === session.id) return "running";
    return session.status;
  }

  // The deadlines of each shown day ("2026-11-16" → […]), sorted by due time.
  const deadlinesByDay = $derived.by(() => {
    const days = new Map(layout.days.map((d) => [isoDate(d.date), [] as Deadline[]]));
    for (const deadline of [...deadlines].sort((a, b) => a.dueAt.localeCompare(b.dueAt))) {
      days.get(isoDate(new Date(deadline.dueAt)))?.push(deadline);
    }
    return days;
  });

  // The header row grows with the busiest day, so every column lines up.
  const chipRows = $derived(Math.max(0, ...[...deadlinesByDay.values()].map((list) => list.length)));

  /** "late", "done", "cancelled" or "" for a chip's look. */
  function deadlineState(deadline: Deadline): string {
    if (deadline.status === "cancelled") return "cancelled";
    if (deadline.done) return "done";
    return new Date(deadline.dueAt) < now ? "late" : "";
  }

  /** "Lab report 2 · Databanken · due 23:59 · moved, was Mon 16 Nov" for the tooltip. */
  function deadlineTooltip(deadline: Deadline): string {
    const course = deadline.courseId ? courses.get(deadline.courseId)?.name : undefined;
    const due = t("week.deadlineDue", { time: formatTime(new Date(deadline.dueAt)) });
    const moved =
      deadline.status === "moved" && deadline.movedFrom
        ? t("urgent.movedFrom", { day: formatDay(new Date(deadline.movedFrom)) })
        : undefined;
    return [deadline.title, course, due, moved].filter(Boolean).join(" · ");
  }

  /** The time shows on the chip only when it isn't the usual 23:59. */
  function chipTime(deadline: Deadline): string | null {
    const due = new Date(deadline.dueAt);
    return due.getHours() === 23 && due.getMinutes() === 59 ? null : formatTime(due);
  }

  const slotsPerHour = 60 / SLOT_MINUTES;
  const hours = $derived(
    Array.from({ length: layout.lastHour - layout.firstHour }, (_, i) => layout.firstHour + i),
  );
  const todayColumn = $derived(columnForDate(layout, now));
  const nowRow = $derived(todayColumn === null ? null : rowForTime(layout, now));

  function hourLabel(hour: number): string {
    return formatTime(new Date(2000, 0, 1, hour));
  }

  /** Blocks are buttons: Enter and Space open them too. */
  function onBlockKeydown(event: KeyboardEvent, block: ClassBlock) {
    if (event.target !== event.currentTarget) return; // e.g. "Show anyway" inside it
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect(block);
    }
  }

  /** "Fri 13:00" */
  function dayAndTime(text: string): string {
    const date = new Date(text);
    return `${formatDate(date, { weekday: "short" })} ${formatTime(date)}`;
  }
</script>

<div
  class="grid"
  style:--chip-rows={chipRows}
  style:grid-template-columns="var(--hour-column) repeat({layout.days.length}, minmax(0, 1fr))"
  style:grid-template-rows="var(--header-height) repeat({layout.slots}, calc(var(--hour-height) / {slotsPerHour}))"
>
  {#if todayColumn !== null}
    <div class="today" style:grid-column={todayColumn} style:grid-row="1 / -1"></div>
  {/if}

  {#each layout.days as day (day.column)}
    {@const isToday = day.column === todayColumn}
    {@const dayDeadlines = deadlinesByDay.get(isoDate(day.date)) ?? []}
    <div class="day" class:is-today={isToday} style:grid-column={day.column} style:grid-row="1">
      <div class="day-top">
        <span class="weekday">{formatDate(day.date, { weekday: "short" })}</span>
        {day.date.getDate()}
        {#if isToday}<span class="pill">{t("week.today")}</span>{/if}
      </div>
      {#each dayDeadlines as deadline (deadline.id)}
        {@const course = deadline.courseId ? courses.get(deadline.courseId) : undefined}
        {@const time = chipTime(deadline)}
        {@const state = deadlineState(deadline)}
        <button
          class="deadline {state}"
          style:--course={course?.color}
          title={deadlineTooltip(deadline)}
          onclick={() => onOpenDeadline(deadline)}
        >
          <svg class="icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 21V4h11l-2 4 2 4H5" /></svg>
          {#if course}<span class="dot" aria-hidden="true"></span>{/if}
          {#if time}<span class="chip-time">{time}</span>{/if}
          <span class="ellipsis chip-title">{deadline.title}</span>
          {#if state === "done"}<span class="visually-hidden">{t("status.done")}</span>{/if}
          {#if state === "late"}<span class="visually-hidden">{t("status.behind")}</span>{/if}
        </button>
      {/each}
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
    {@const selected = block.id === selectedId}
    {@const position = `grid-column: ${placed.column}; grid-row: ${placed.rowStart} / ${placed.rowEnd}; --lane: ${placed.lane}; --lanes: ${placed.lanes}; --course: ${course?.color ?? "var(--text-faint)"}`}

    {#if isIgnored(block)}
      {@const rule = ignoringRule(block, rules)}
      {@const reason = rule
        ? t("block.ignoredByRule", { rule: rule.pattern })
        : block.ignoredOverride
          ? t("block.ignoredByHand")
          : t("block.ignored")}
      <div
        class="blk ignored"
        class:selected
        style={position}
        role="button"
        tabindex="0"
        aria-pressed={selected}
        onclick={() => onSelect(block)}
        onkeydown={(e) => onBlockKeydown(e, block)}
      >
        <div class="blk-title">
          <span class="ellipsis" title={name}>{name}</span>
          <svg class="icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M3 3l18 18M10.6 6.1A9 9 0 0 1 21 12a10 10 0 0 1-2.3 3M6.6 6.6A10 10 0 0 0 3 12c2 4 5.5 6 9 6 1.6 0 3.1-.4 4.4-1.1" /></svg>
        </div>
        <span title={times}>{times}</span>
        <span title={reason}>{reason}</span>
        <button
          class="btn btn-small"
          onclick={(e) => {
            e.stopPropagation(); // not also open the block
            onShowAnyway(block);
          }}
        >
          {t("block.showAnyway")}
        </button>
      </div>
    {:else if block.status === "cancelled"}
      {@const line = kind ? `${kind} · ${times}` : times}
      <div
        class="blk cancelled"
        class:selected
        style={position}
        role="button"
        tabindex="0"
        aria-pressed={selected}
        onclick={() => onSelect(block)}
        onkeydown={(e) => onBlockKeydown(e, block)}
      >
        <div class="blk-title">
          <span class="name"><span class="dot" aria-hidden="true"></span><s class="ellipsis" title={name}>{name}</s></span>
          <span class="cancelled-label">{t("block.cancelled")}</span>
        </div>
        <span title={line}>{line}</span>
      </div>
    {:else}
      {@const progress = blockProgress(block, activities, now)}
      {@const showCount = progress.status === "partial" || progress.status === "behind"}
      {@const line = kind ? `${kind} · ${times}` : times}
      {@const count = tn("block.activities", progress.total, { done: progress.done, total: progress.total })}
      {@const movedText = block.movedFrom
        ? t("block.movedFrom", { from: dayAndTime(block.movedFrom) })
        : t("block.moved")}
      <div
        class="blk"
        class:selected
        style={position}
        role="button"
        tabindex="0"
        aria-pressed={selected}
        onclick={() => onSelect(block)}
        onkeydown={(e) => onBlockKeydown(e, block)}
      >
        <div class="blk-title">
          <span class="name"><span class="dot" aria-hidden="true"></span><span class="ellipsis" title={name}>{name}</span></span>
          {#if progress.status}<StatusIcon status={progress.status} />{/if}
        </div>
        <span title={line}>{line}</span>
        {#if showCount}
          <span title={count}>{count}</span>
        {:else if block.location}
          <span title={block.location}>{block.location}</span>
        {/if}
        {#if block.status === "moved"}
          <span class="moved" title={movedText}>
            <svg class="icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
            <span class="ellipsis">{movedText}</span>
          </span>
        {/if}
      </div>
    {/if}
  {/each}

  {#each layout.sessions as placed (placed.session.id)}
    {@const session = placed.session}
    {@const status = sessionStatus(session)}
    {@const times = sessionLine(session)}
    {@const about = sessionAbout(session, courses)}
    {@const position = `grid-column: ${placed.column}; grid-row: ${placed.rowStart} / ${placed.rowEnd}; --lane: ${placed.lane}; --lanes: ${placed.lanes}`}
    <div
      class="blk session {status}"
      style={position}
      role="button"
      tabindex="0"
      onclick={() => onOpenSession(session)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpenSession(session);
        }
      }}
    >
      <div class="blk-title">
        <span class="name">
          <svg class="icon lock" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
          <span class="ellipsis">{t("session.untitled")}</span>
        </span>
        {#if status === "running"}
          <span class="live-label">{t("session.live")}</span>
        {:else if status === "done"}
          <StatusIcon status="done" />
        {:else if status === "stopped"}
          <StatusIcon status="behind" label={t("session.status.stopped")} />
        {:else if status === "missed"}
          <span class="missed-label">{t("session.status.missed")}</span>
        {:else}
          <StatusIcon status="planned" />
        {/if}
      </div>
      <span class="session-times" title={times}>{times}</span>
      {#if about}<span title={about}>{about}</span>{/if}
      {#if status === "running"}
        <span>{t("live.left", { time: hoursText(live.secondsLeft + 59) })}</span>
      {/if}
    </div>
  {/each}

  {#if todayColumn !== null && nowRow !== null}
    <div class="now" style:grid-column={todayColumn} style:grid-row={nowRow} aria-hidden="true"></div>
  {/if}
</div>

<style>
  .grid {
    /* Sizes from the mockup: 52 px per hour, a 40 px header row,
       plus a row of chips per deadline on the busiest day. */
    --hour-column: 52px;
    --header-height: calc(40px + var(--chip-rows) * 26px);
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
    flex-direction: column;
    min-width: 0;
    padding: 0 var(--space-1) var(--space-1);
    font-size: var(--text-base);
    color: var(--text-muted);
  }

  .day-top {
    height: 40px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 6px;
  }

  /* A deadline chip: flag, course dot, time (if not 23:59) and title. */
  .deadline {
    height: 22px;
    margin-bottom: 4px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    padding: 0 6px;
    border: 0;
    border-radius: var(--radius-sm);
    background: var(--tag);
    color: var(--text);
    font-size: var(--text-sm);
    text-align: left;
    cursor: pointer;
    position: relative;
    z-index: 1;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .deadline:hover {
    background: var(--surface-active);
  }

  .deadline .dot {
    background: var(--course);
  }

  .chip-time {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .chip-title {
    flex: 1;
  }

  /* Late: red flag and text, as "behind" everywhere. */
  .deadline.late {
    color: var(--status-behind-text);
  }

  /* Done: faded and struck through; cancelled: struck through. */
  .deadline.done,
  .deadline.cancelled {
    color: var(--text-done);
  }

  .deadline.done .chip-title,
  .deadline.cancelled .chip-title {
    text-decoration: line-through;
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

  .blk {
    cursor: pointer;
  }

  .blk:hover {
    transform: translateY(-1px);
  }

  /* The block whose panel is open. */
  .blk.selected {
    outline: 2px solid var(--control);
    outline-offset: -1px;
  }

  /* Lines keep their height: a short block cuts off the last ones instead of squashing them. */
  .blk > span {
    flex: none;
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

  /* Study sessions: grey with a lock, no course tint (the course is in the text). */
  .session {
    background: var(--session-surface);
  }

  .session .lock {
    color: var(--control);
  }

  .session.planned {
    border: 1px solid var(--session-border);
  }

  .session.running {
    background: var(--session-live-surface);
  }

  .session.running .session-times {
    color: var(--text);
  }

  /* The running session pulses, as "study session active" does everywhere. */
  .session.running::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: var(--radius);
    border: 1.5px solid var(--status-active);
    animation: pulse 2s ease-in-out infinite;
    pointer-events: none;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
    }
    50% {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .session.running::after {
      animation: none;
      opacity: 0.7;
    }
  }

  .live-label {
    flex: none;
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--text);
  }

  /* Missed costs nothing: a quiet grey label, not a red one. */
  .session.missed {
    color: var(--text-faint);
  }

  .missed-label {
    flex: none;
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    color: var(--text-muted);
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
