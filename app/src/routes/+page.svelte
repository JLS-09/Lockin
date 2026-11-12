<script lang="ts">
  // The week page, following the week view mockup: header with week switching,
  // the schedule changes banner, the week grid, and beside it the panel of the
  // class you clicked, today's activities, the Urgent and Deadlines panels and
  // this week's other activities. Deadlines show as chips under the day headers; clicking
  // one opens the same dialog as on the Urgent page. Study sessions are blocks in
  // the grid: the Study session button (N) plans one, clicking a planned one
  // changes it, the running one opens the Session page.
  //
  // Keys: ← / → switch weeks, Home goes to this week, A adds an activity,
  // N plans a study session, Escape closes the class panel, Ctrl+Z brings back
  // a deleted activity, deadline or study session.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { untrack } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import {
    api,
    isIgnored,
    type Activity,
    type BlockProfile,
    type ClassBlock,
    type Deadline,
    type StudySession,
  } from "$lib/api";
  import BlockPanel from "$lib/components/BlockPanel.svelte";
  import ChangesBanner from "$lib/components/ChangesBanner.svelte";
  import DeadlineDialog from "$lib/components/DeadlineDialog.svelte";
  import SessionDialog from "$lib/components/SessionDialog.svelte";
  import TodayPanel from "$lib/components/TodayPanel.svelte";
  import UrgentPanel from "$lib/components/UrgentPanel.svelte";
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
  import { sessionName } from "$lib/session/labels";
  import { live } from "$lib/session/live.svelte";
  import { isEditable, saveSession, type SessionFields } from "$lib/session/sessions";
  import { WeekData } from "$lib/week/data.svelte";
  import { layoutWeek, shownRange } from "$lib/week/layout";

  const data = new WeekData();
  let now = $state(new Date());
  // -1 = the new week slides in from the left, 1 = from the right.
  let direction = $state(0);
  let showIgnored = $state(loadShowIgnored());
  let selectedId = $state<string | null>(null);
  // Brings back the last deleted activity or deadline: for Undo and Ctrl+Z.
  let undoDelete = $state<(() => void) | null>(null);
  // A short message at the bottom with one action: "Deleted … · Undo",
  // "Added to 12 classes · Templates". It goes away by itself.
  let toast = $state<{ text: string; action: string; run: () => void } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  let blockPanel = $state<BlockPanel>();
  let weekPanel = $state<WeekPanel>();
  let deadlineDialog = $state<DeadlineDialog>();
  let sessionDialog = $state<SessionDialog>();
  // Block profiles, for the dialog and the "· Strict" on sessions.
  let profiles = $state<BlockProfile[]>([]);
  let lastProfile = $state<string | null>(null);
  async function loadProfiles() {
    try {
      [profiles, lastProfile] = await Promise.all([api.blockProfiles(), api.lastProfile()]);
    } catch (e) {
      data.error = String(e);
    }
  }
  loadProfiles();

  // "/?date=2026-10-26" opens that week (the semester overview links here like
  // that); "&block=<id>" also opens that class's panel (the Urgent list does).
  // An effect, not a one-off: a link in the Urgent panel stays on this page.
  $effect(() => {
    const date = page.url.searchParams.get("date");
    const block = page.url.searchParams.get("block");
    untrack(() => {
      if (date) {
        const target = parseIsoDate(date);
        direction = Math.sign(target.getTime() - data.date.getTime());
        data.date = target;
      }
      selectedId = block;
    });
  });

  const week = $derived(data.week);
  const ignoredCount = $derived(week?.blocks.filter(isIgnored).length ?? 0);
  const layout = $derived.by(() => {
    if (!week) return null;
    const blocks = showIgnored ? week.blocks : week.blocks.filter((b) => !isIgnored(b));
    return layoutWeek(parseIsoDate(week.monday), blocks, data.sessions);
  });

  // A session that starts or ends while the page is open: reload the sessions,
  // so its block shows "Live", and afterwards "Done".
  $effect(() => {
    void live.current?.session.id; // runs again when the running session changes
    untrack(() => data.loadSessions());
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

  // The Urgent panel, without deadlines just checked off in the Deadlines panel.
  const urgentItems = $derived(
    data.urgent.filter((i) => !(i.kind === "deadline" && data.checkedDeadlines.has(i.id))),
  );

  // The Deadlines panel: open deadlines (late ones first), soonest first.
  const deadlineItems = $derived(
    data.urgent
      .filter((item) => item.kind === "deadline")
      .sort((a, b) => (a.dueAt ?? "").localeCompare(b.dueAt ?? "")),
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
  data.loadUrgent();
  data.syncIfStale();

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
      undoDelete = null;
    }, 6000);
  }

  /** "Deleted … · Undo" for 6 seconds; Undo or Ctrl+Z runs `restore`. */
  function offerUndo(title: string, restore: () => void) {
    undoDelete = restore;
    showToast(t("activity.deleted", { title }), t("activity.undo"), undo);
  }

  async function remove(activity: Activity) {
    if (!(await data.remove(activity))) return;
    offerUndo(activity.title, () => data.restore(activity));
  }

  async function removeDeadline(deadline: Deadline) {
    if (!(await data.removeDeadline(deadline))) return;
    offerUndo(deadline.title, () => data.restoreDeadline(deadline));
  }

  function undo() {
    if (!undoDelete) return;
    undoDelete();
    undoDelete = null;
    toast = null;
    clearTimeout(toastTimer);
  }

  /** A deadline chip: one added by hand opens the dialog, a Ufora one its page in Ufora. */
  async function openDeadline(deadline: Deadline) {
    if (!deadline.sourceId) {
      deadlineDialog?.openEdit(deadline);
      return;
    }
    if (!deadline.url) return;
    try {
      await api.openInBrowser(deadline.url);
    } catch (e) {
      data.error = String(e);
    }
  }

  /** The Study session button (N): a new session on today, or on the shown week's Monday. */
  function planSession() {
    const today = new Date();
    const shownMonday = week ? parseIsoDate(week.monday) : today;
    const inShownWeek = today >= shownMonday && today < addDays(shownMonday, 7);
    sessionDialog?.openNew(inShownWeek ? today : shownMonday);
  }

  /** A click on a session: change a planned one, follow the running one. */
  function openSession(session: StudySession) {
    if (live.current?.session.id === session.id) goto("/session");
    else if (isEditable(session, new Date())) sessionDialog?.openEdit(session);
  }

  async function saveFromDialog(id: string | null, fields: SessionFields): Promise<string | null> {
    try {
      const result = await saveSession(id, fields);
      await data.loadSessions();
      lastProfile = fields.profileId ?? lastProfile;
      if (fields.weekly) {
        const text =
          result.overlapping > 0
            ? `${tn("session.plannedWeekly", result.planned)} ${tn("session.leftOut", result.overlapping)}`
            : tn("session.plannedWeekly", result.planned);
        undoDelete = null;
        showToast(text, t("session.seeAll"), () => goto("/session"));
      }
      return null;
    } catch (e) {
      return String(e);
    }
  }

  async function removeSession(session: StudySession) {
    try {
      await api.removeSession(session.id);
    } catch (e) {
      data.error = String(e);
      return;
    }
    await data.loadSessions();
    offerUndo(sessionName(session, data.courseById), async () => {
      try {
        await api.restoreSession(session.id);
      } catch (e) {
        data.error = String(e);
      }
      data.loadSessions();
    });
  }

  async function removeFollowing(session: StudySession) {
    try {
      const removed = await api.removeFollowingSessions(session.id);
      undoDelete = null;
      showToast(tn("session.removedFollowing", removed), t("session.seeAll"), () => goto("/session"));
    } catch (e) {
      data.error = String(e);
    }
    data.loadSessions();
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
    undoDelete = null;
    showToast(tn("template.added", created), t("template.manageShort"), () =>
      goto(`/settings?course=${courseId}`),
    );
  }

  function onKeydown(event: KeyboardEvent) {
    if (deadlineDialog?.isOpen() || sessionDialog?.isOpen()) return; // dialogs have their own keys
    const typing = (event.target as HTMLElement).closest(
      "input, textarea, select, [contenteditable]",
    );
    if (event.key === "Escape" && selectedId && !typing) {
      selectedId = null;
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key === "z" && undoDelete && !typing) {
      event.preventDefault();
      undo();
      return;
    }
    if (typing || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "ArrowLeft") previousWeek();
    else if (event.key === "ArrowRight") nextWeek();
    else if (event.key === "Home") thisWeek();
    else if (event.key === "n" || event.key === "N") planSession();
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
      {#if data.syncing}<span class="syncing" role="status">{t("week.syncing")}</span>{/if}
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
    <button class="btn btn-primary" onclick={planSession}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
      {t("week.newSession")} <span class="kbd">N</span>
    </button>
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
              deadlines={data.deadlines}
              onSelect={select}
              onShowAnyway={(block) => data.showAnyway(block)}
              onOpenDeadline={openDeadline}
              onOpenSession={openSession}
              {profiles}
            />
          </div>
        {/key}
        <!-- On top of the grid, so the grid doesn't jump between empty and full weeks. -->
        {#if !data.hasSchedule}
          <div class="empty no-schedule">
            <div class="no-schedule-card">
              <h2>{t("week.noSchedule")}</h2>
              <p>{t("week.noScheduleHint")}</p>
              <a class="btn btn-primary" href="/settings#schedule">{t("week.addSchedule")}</a>
            </div>
          </div>
        {:else if week.blocks.length === 0 && data.sessions.length === 0}
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
      <UrgentPanel items={urgentItems} courses={data.courseById} {now} />
      <UrgentPanel
        items={deadlineItems}
        courses={data.courseById}
        {now}
        title={t("urgent.tabDeadlines")}
        seeAllHref="/urgent?tab=deadlines"
        emptyText={t("week.noDeadlines")}
        headingId="deadlines-panel-heading"
        onToggle={(item) => data.toggleDeadline(item)}
        isDone={(item) => data.checkedDeadlines.has(item.id)}
      />
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

<DeadlineDialog
  bind:this={deadlineDialog}
  courses={data.courses}
  onSave={(id, fields) => (id ? data.updateDeadline(id, fields) : Promise.resolve(false))}
  onDelete={removeDeadline}
/>

<SessionDialog
  bind:this={sessionDialog}
  courses={data.courses}
  {profiles}
  defaultProfileId={lastProfile}
  onSave={saveFromDialog}
  onDelete={removeSession}
  onDeleteFollowing={removeFollowing}
/>

{#if toast}
  <div class="toast" role="status" in:panelIn>
    <span class="toast-text" title={toast.text}>{toast.text}</span>
    <button class="btn btn-small" title={undoDelete ? `${toast.action} (Ctrl+Z)` : toast.action} onclick={toast.run}>
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

  .syncing {
    font-size: var(--text-sm);
    color: var(--text-faint);
  }

  /* No feed yet: a card on top of the empty grid, pointing to Settings. */
  .no-schedule {
    pointer-events: auto;
    background: var(--overlay);
    /* Above the grid's blocks and now line (z-index 1 and 2). */
    z-index: 3;
  }

  .no-schedule-card {
    max-width: 360px;
    padding: var(--space-5);
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-strong);
    background: var(--surface-2);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    color: var(--text);
  }

  .no-schedule-card h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .no-schedule-card p {
    font-size: var(--text-md);
    color: var(--text-muted);
    margin-bottom: var(--space-2);
  }

  .no-schedule-card a {
    text-decoration: none;
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
