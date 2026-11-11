<script lang="ts">
  // The week page, following the week view mockup: header with week switching,
  // the schedule changes banner, the week grid and today's activities beside it.
  // The Urgent panel and the study session button follow in phases 3 and 4.
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import {
    api,
    isIgnored,
    type Activity,
    type ClassBlock,
    type Course,
    type FilterRule,
    type Week,
  } from "$lib/api";
  import ChangesBanner from "$lib/components/ChangesBanner.svelte";
  import TodayPanel from "$lib/components/TodayPanel.svelte";
  import WeekGrid from "$lib/components/WeekGrid.svelte";
  import { addDays, formatRange, isoDate, isoWeek, parseIsoDate, t } from "$lib/i18n/index.svelte";
  import { measureFrames } from "$lib/perf";
  import { layoutWeek, shownRange } from "$lib/week/layout";

  // Any day in the shown week; starts at today.
  let date = $state(new Date());
  let week = $state<Week | null>(null);
  // The week with today in it, for the Today panel, whichever week is shown.
  let todayWeek = $state<Week | null>(null);
  let courses = $state<Course[]>([]);
  let rules = $state<FilterRule[]>([]);
  let error = $state<string | null>(null);
  let now = $state(new Date());
  // -1 = the new week slides in from the left, 1 = from the right.
  let direction = $state(0);
  let showIgnored = $state(loadShowIgnored());

  const courseById = $derived(new Map(courses.map((c) => [c.id, c])));
  const ignoredCount = $derived(week?.blocks.filter(isIgnored).length ?? 0);
  const layout = $derived.by(() => {
    if (!week) return null;
    const blocks = showIgnored ? week.blocks : week.blocks.filter((b) => !isIgnored(b));
    return layoutWeek(parseIsoDate(week.monday), blocks);
  });

  // Moved or cancelled blocks of this week you haven't dismissed yet.
  const changes = $derived(
    week?.blocks.filter((b) => b.status !== "normal" && !b.changeSeen && !isIgnored(b)) ?? [],
  );

  // Today's activities: the ones on today's classes, in the order of the classes.
  const todayActivities = $derived.by(() => {
    if (!todayWeek) return [];
    const today = isoDate(now);
    const order = new Map(
      todayWeek.blocks
        .filter((b) => isoDate(new Date(b.startAt)) === today && !isIgnored(b))
        .map((b, i) => [b.id, i]),
    );
    return todayWeek.activities
      .filter((a) => a.blockId !== null && order.has(a.blockId))
      .sort((a, b) => order.get(a.blockId!)! - order.get(b.blockId!)!);
  });

  Promise.all([api.courses(), api.rules()])
    .then(([courseList, ruleList]) => {
      courses = courseList;
      rules = ruleList;
    })
    .catch((e) => (error = String(e)));

  // Reload whenever the date changes. A slow answer for an older week is dropped.
  // The old week stays on screen until the new one is there, so nothing flashes.
  let latest = 0;
  async function loadWeek() {
    const request = ++latest;
    try {
      const result = await api.week(isoDate(date));
      if (request !== latest) return;
      week = result;
      error = null;
      if (import.meta.env.DEV && direction !== 0) measureFrames("week switch");
    } catch (e) {
      if (request === latest) error = String(e);
    }
  }

  async function loadToday() {
    try {
      todayWeek = await api.week(isoDate(new Date()));
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    isoDate(date); // reload when the date changes
    loadWeek();
  });
  loadToday();

  // Keep "now" fresh for the today column, the now line and the statuses.
  $effect(() => {
    const timer = setInterval(() => {
      const previous = isoDate(now);
      now = new Date();
      if (isoDate(now) !== previous) loadToday(); // past midnight: a new today
    }, 60_000);
    return () => clearInterval(timer);
  });

  function goTo(newDate: Date, newDirection: number) {
    direction = newDirection;
    date = newDate;
  }

  const previousWeek = () => goTo(addDays(date, -7), -1);
  const nextWeek = () => goTo(addDays(date, 7), 1);
  const thisWeek = () => {
    const today = new Date();
    goTo(today, Math.sign(today.getTime() - date.getTime()));
  };

  // ← and → switch weeks, Home jumps to this week. Not while typing.
  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable]")) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "ArrowLeft") previousWeek();
    else if (event.key === "ArrowRight") nextWeek();
    else if (event.key === "Home") thisWeek();
    else return;
    event.preventDefault();
  }

  /**
   * Check an activity off or on. The UI changes at once; saving happens after.
   * If saving fails, it flips back and the error shows.
   */
  async function toggle(activity: Activity) {
    const done = !activity.done;
    setDone(activity.id, done);
    try {
      await api.setActivityDone(activity.id, done);
    } catch (e) {
      setDone(activity.id, !done);
      error = String(e);
    }
  }

  function setDone(id: string, done: boolean) {
    const update = (list: Activity[]) => list.map((a) => (a.id === id ? { ...a, done } : a));
    if (week) week = { ...week, activities: update(week.activities) };
    if (todayWeek) todayWeek = { ...todayWeek, activities: update(todayWeek.activities) };
  }

  async function dismissChanges() {
    const ids = changes.map((b) => b.id);
    const mark = (w: Week) => ({
      ...w,
      blocks: w.blocks.map((b) => (ids.includes(b.id) ? { ...b, changeSeen: true } : b)),
    });
    if (week) week = mark(week);
    try {
      await api.acknowledgeChanges(ids);
    } catch (e) {
      error = String(e);
    }
  }

  // "Show anyway": a manual exception that keeps this block, whatever the rules say.
  async function showAnyway(block: ClassBlock) {
    try {
      await api.setBlockIgnored(block.id, false);
      await Promise.all([loadWeek(), loadToday()]);
    } catch (e) {
      error = String(e);
    }
  }

  function loadShowIgnored(): boolean {
    try {
      return localStorage.getItem("lockin.showIgnored") === "true";
    } catch {
      return false;
    }
  }

  function toggleIgnored() {
    showIgnored = !showIgnored;
    try {
      localStorage.setItem("lockin.showIgnored", String(showIgnored));
    } catch {
      // Not saved; it still applies until the app closes.
    }
  }

  // The new week slides in from the side you're going to, fast and with ease-out.
  // The old week disappears at once, so you can keep clicking without waiting.
  // With "reduce motion" on, it only fades. Durations come from tokens.css.
  function weekIn(node: Element) {
    const style = getComputedStyle(document.documentElement);
    const duration = parseFloat(style.getPropertyValue("--duration-panel")) || 200;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches || direction === 0) {
      return fade(node, { duration });
    }
    return fly(node, { x: direction * 32, duration, easing: cubicOut });
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  <header>
    <div class="title">
      <h1>{t("week.title", { number: isoWeek(date) })}</h1>
      {#if layout}
        {@const [first, last] = shownRange(layout)}
        <span class="range">{formatRange(first, last)}</span>
      {/if}
    </div>
    {#if ignoredCount > 0}
      <button class="btn" aria-pressed={showIgnored} onclick={toggleIgnored}>
        {t(showIgnored ? "week.hideIgnored" : "week.showIgnored", { count: ignoredCount })}
      </button>
    {/if}
    <div class="nav">
      <button class="btn btn-icon" aria-label={t("week.previous")} title="{t('week.previous')} (←)" onclick={previousWeek}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M15 6l-6 6 6 6" /></svg>
      </button>
      <button class="btn" title="{t('week.today')} (Home)" onclick={thisWeek}>{t("week.today")}</button>
      <button class="btn btn-icon" aria-label={t("week.next")} title="{t('week.next')} (→)" onclick={nextWeek}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
      </button>
    </div>
  </header>

  {#if error}
    <p class="error" role="alert">{t("week.loadFailed", { error })}</p>
  {/if}

  {#if changes.length > 0 && week}
    <ChangesBanner blocks={changes} activities={week.activities} courses={courseById} onDismiss={dismissChanges} />
  {/if}

  <div class="content">
    <section class="calendar" aria-label={t("week.calendar")}>
      {#if !week || !layout}
        <p class="empty">{t("week.loading")}</p>
      {:else}
        {#key week.monday}
          <div in:weekIn>
            <WeekGrid
              {layout}
              activities={week.activities}
              courses={courseById}
              {rules}
              {now}
              onShowAnyway={showAnyway}
            />
          </div>
        {/key}
        <!-- On top of the grid, so the grid doesn't jump between empty and full weeks. -->
        {#if week.blocks.length === 0}
          <p class="empty">{t("week.empty")}</p>
        {/if}
      {/if}
    </section>

    <aside>
      <TodayPanel day={now} activities={todayActivities} courses={courseById} onToggle={toggle} />
    </aside>
  </div>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
  }

  .title {
    flex: 1 1 auto;
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
  }

  .range {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  .nav {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .error {
    color: var(--status-behind);
  }

  .content {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    align-items: flex-start;
  }

  .calendar {
    flex: 999 1 640px;
    min-width: 0;
    position: relative;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--space-2) var(--space-2) var(--space-3);
    overflow-x: auto;
    /* Keeps the sliding week inside the panel. */
    overflow-y: hidden;
  }

  aside {
    flex: 1 1 300px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    pointer-events: none;
  }
</style>
