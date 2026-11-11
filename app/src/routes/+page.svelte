<script lang="ts">
  // The week page, following the week view mockup: header with week switching,
  // the schedule changes banner, the week grid, and beside it the panel of the
  // class you clicked, today's activities and this week's other activities.
  // The Urgent panel and the study session button follow in phases 3 and 4.
  //
  // Keys: ← / → switch weeks, Home goes to this week, A adds an activity,
  // Escape closes the class panel, Ctrl+Z brings back a deleted activity.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import { isIgnored, type Activity, type ClassBlock } from "$lib/api";
  import BlockPanel from "$lib/components/BlockPanel.svelte";
  import ChangesBanner from "$lib/components/ChangesBanner.svelte";
  import TodayPanel from "$lib/components/TodayPanel.svelte";
  import WeekGrid from "$lib/components/WeekGrid.svelte";
  import WeekPanel from "$lib/components/WeekPanel.svelte";
  import {
    addDays,
    formatRange,
    isoDate,
    isoWeek,
    parseIsoDate,
    t,
    tn,
  } from "$lib/i18n/index.svelte";
  import { measureFrames } from "$lib/perf";
  import { WeekData } from "$lib/week/data.svelte";
  import { layoutWeek, shownRange } from "$lib/week/layout";

  const data = new WeekData();
  // "/?date=2026-10-26" opens that week (the semester overview links here like that).
  const startDate = page.url.searchParams.get("date");
  if (startDate) data.date = parseIsoDate(startDate);
  let now = $state(new Date());
  // -1 = the new week slides in from the left, 1 = from the right.
  let direction = $state(0);
  let showIgnored = $state(loadShowIgnored());
  let selectedId = $state<string | null>(null);
  // The last deleted activity, for Undo and Ctrl+Z.
  let deleted = $state<Activity | null>(null);
  // A short message at the bottom with one action: "Deleted … · Undo",
  // "Added to 12 classes · Templates". It goes away by itself.
  let toast = $state<{ text: string; action: string; run: () => void } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  let blockPanel = $state<BlockPanel>();
  let weekPanel = $state<WeekPanel>();

  const week = $derived(data.week);
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

  // The open class: only while it's in the shown week.
  const selected = $derived(week?.blocks.find((b) => b.id === selectedId) ?? null);
  const selectedActivities = $derived(
    selected ? (week?.activities.filter((a) => a.blockId === selected.id) ?? []) : [],
  );

  // Activities of the shown week that aren't tied to a class.
  const weekActivities = $derived(week?.activities.filter((a) => a.blockId === null) ?? []);

  // Today's activities: the ones on today's classes, in the order of the classes.
  const todayActivities = $derived.by(() => {
    const todayWeek = data.todayWeek;
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

  data.loadCoursesAndRules();
  data.loadToday();

  // Reload whenever the date changes. The old week stays on screen until the
  // new one is there, so nothing flashes.
  $effect(() => {
    isoDate(data.date); // reload when the date changes
    data.loadWeek().then(() => {
      if (import.meta.env.DEV && direction !== 0) measureFrames("week switch");
    });
  });

  // Keep "now" fresh for the today column, the now line and the statuses.
  $effect(() => {
    const timer = setInterval(() => {
      const previous = isoDate(now);
      now = new Date();
      if (isoDate(now) !== previous) data.loadToday(); // past midnight: a new today
    }, 60_000);
    return () => clearInterval(timer);
  });

  function goTo(date: Date, newDirection: number) {
    direction = newDirection;
    data.date = date;
  }

  const previousWeek = () => goTo(addDays(data.date, -7), -1);
  const nextWeek = () => goTo(addDays(data.date, 7), 1);
  const thisWeek = () => {
    const today = new Date();
    goTo(today, Math.sign(today.getTime() - data.date.getTime()));
  };

  function select(block: ClassBlock) {
    selectedId = selectedId === block.id ? null : block.id;
  }

  function showToast(text: string, action: string, run: () => void) {
    toast = { text, action, run };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toast = null;
      deleted = null;
    }, 6000);
  }

  async function remove(activity: Activity) {
    if (!(await data.remove(activity))) return;
    deleted = activity;
    showToast(t("activity.deleted", { title: activity.title }), t("activity.undo"), undo);
  }

  function undo() {
    if (!deleted) return;
    data.restore(deleted);
    deleted = null;
    toast = null;
    clearTimeout(toastTimer);
  }

  // From the class panel: one activity, or with "Every WPO" on, a template
  // that gives every WPO of the course this activity.
  async function addFromPanel(block: ClassBlock, title: string, repeat: boolean) {
    if (!repeat || !block.courseId) {
      await data.addToBlock(block, title);
      return;
    }
    const courseId = block.courseId;
    const created = await data.addTemplate(courseId, block.kind, title);
    deleted = null;
    showToast(tn("template.added", created), t("template.manageShort"), () =>
      goto(`/settings?course=${courseId}`),
    );
  }

  function onKeydown(event: KeyboardEvent) {
    const typing = (event.target as HTMLElement).closest(
      "input, textarea, select, [contenteditable]",
    );
    if (event.key === "Escape" && selectedId && !typing) {
      selectedId = null;
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key === "z" && deleted && !typing) {
      event.preventDefault();
      undo();
      return;
    }
    if (typing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "ArrowLeft") previousWeek();
    else if (event.key === "ArrowRight") nextWeek();
    else if (event.key === "Home") thisWeek();
    else if (event.key === "a" || event.key === "A") {
      // Add to the open class, or else to this week's list.
      if (selected) blockPanel?.focusAdd();
      else weekPanel?.focusAdd();
    } else return;
    event.preventDefault();
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
    const duration = cssDuration("--duration-panel", 200);
    if (matchMedia("(prefers-reduced-motion: reduce)").matches || direction === 0) {
      return fade(node, { duration });
    }
    return fly(node, { x: direction * 32, duration, easing: cubicOut });
  }

  // Panels and the toast fade and rise in a little; nothing waits for them.
  function panelIn(node: Element) {
    const duration = cssDuration("--duration-panel", 200);
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return fade(node, { duration });
    return fly(node, { y: 8, duration, easing: cubicOut });
  }

  function cssDuration(name: string, fallback: number): number {
    return parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || fallback;
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  <header>
    <div class="title">
      <h1>{t("week.title", { number: isoWeek(data.date) })}</h1>
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

  {#if data.error}
    <p class="error" role="alert">{t("week.loadFailed", { error: data.error })}</p>
  {/if}

  {#if changes.length > 0 && week}
    <ChangesBanner
      blocks={changes}
      activities={week.activities}
      courses={data.courseById}
      onDismiss={() => data.acknowledgeChanges(changes)}
    />
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
              courses={data.courseById}
              rules={data.rules}
              {now}
              {selectedId}
              onSelect={select}
              onShowAnyway={(block) => data.showAnyway(block)}
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
      {#if selected}
        {#key selected.id}
          <div in:panelIn>
            <BlockPanel
              bind:this={blockPanel}
              block={selected}
              course={selected.courseId ? data.courseById.get(selected.courseId) : undefined}
              activities={selectedActivities}
              onToggle={(a) => data.toggle(a)}
              onRename={(a, title) => data.rename(a, title)}
              onRemove={remove}
              onAdd={(title, repeat) => addFromPanel(selected, title, repeat)}
              onClose={() => (selectedId = null)}
            />
          </div>
        {/key}
      {/if}
      <TodayPanel day={now} activities={todayActivities} courses={data.courseById} onToggle={(a) => data.toggle(a)} />
      {#if week}
        <WeekPanel
          bind:this={weekPanel}
          activities={weekActivities}
          courses={data.courses}
          onToggle={(a) => data.toggle(a)}
          onRename={(a, title) => data.rename(a, title)}
          onRemove={remove}
          onAdd={(title, courseId) => data.addToWeek(courseId, title)}
        />
      {/if}
    </aside>
  </div>
</div>

{#if toast}
  <div class="toast" role="status" in:panelIn>
    <span class="toast-text" title={toast.text}>{toast.text}</span>
    <button class="btn btn-small" title={deleted ? `${toast.action} (Ctrl+Z)` : toast.action} onclick={toast.run}>
      {toast.action}
    </button>
  </div>
{/if}

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

  .toast {
    position: fixed;
    left: 50%;
    bottom: var(--space-5);
    translate: -50% 0;
    max-width: min(480px, calc(100vw - 64px));
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    z-index: 10;
  }

  .toast-text {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .btn-small {
    flex: none;
    min-height: 28px;
    padding: 0 10px;
    font-size: var(--text-sm);
  }
</style>
