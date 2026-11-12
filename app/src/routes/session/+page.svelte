<script lang="ts">
  // The Session page, following the Session mockup while a session runs: the
  // timer with its progress bar (breaks in orange), the break credit (B takes a
  // break), the session's log and "Need to stop?" with the emergency stop,
  // which you hold open for two minutes. The daemon runs the session; this page
  // shows it and asks the daemon for a break, a resume or the stop.
  // "Blocked right now" lists the profile's apps and how often the daemon closed
  // one ("Blocking paused" during a break). Points, streak and the Lockling follow
  // in phase 6, websites on the profile in step 5.
  //
  // Without a running session (not in the mockups, built from the same cards):
  // the next session, the coming two weeks and the last seven days.
  // Keys: N plans a session, B takes or ends a break, Escape cancels the emergency stop.
  import { untrack } from "svelte";

  import { api, type AppCloses, type BlockProfile, type SessionEvent, type StudySession } from "$lib/api";
  import SessionDialog from "$lib/components/SessionDialog.svelte";
  import StatusIcon from "$lib/components/StatusIcon.svelte";
  import { formatAgo, formatDay, formatTime, t, tn } from "$lib/i18n/index.svelte";
  import { profileCount, sessionAbout, sessionName, sessionTimes } from "$lib/session/labels";
  import { clockText, hoursText, live } from "$lib/session/live.svelte";
  import { isEditable, saveSession, type SessionFields } from "$lib/session/sessions";
  import type { Course } from "$lib/api";

  const statusLabels = {
    planned: "session.status.planned",
    running: "session.status.running",
    done: "session.status.done",
    stopped: "session.status.stopped",
    missed: "session.status.missed",
    skipped: "session.status.skipped",
  } as const;

  let courses = $state<Course[]>([]);
  const courseById = $derived(new Map(courses.map((c) => [c.id, c])));
  let sessions = $state<StudySession[]>([]);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let sessionDialog = $state<SessionDialog>();
  let profiles = $state<BlockProfile[]>([]);
  let lastProfile = $state<string | null>(null);
  /** How often each app was closed during the running session. */
  let closes = $state<AppCloses[]>([]);

  /** At most this many app chips; the rest is "+3 more". */
  const MAX_CHIPS = 6;
  /** When the emergency stop was started (ms); null when not holding. */
  let stopStartedAt = $state<number | null>(null);

  const running = $derived(live.current);
  const current = $derived(running?.state);
  const onBreak = $derived(current?.phase === "break");
  const course = $derived(running?.session.courseId ? courseById.get(running.session.courseId) : undefined);

  api
    .courses()
    .then((list) => (courses = list))
    .catch((e) => (error = String(e)));

  async function loadProfiles() {
    try {
      [profiles, lastProfile] = await Promise.all([api.blockProfiles(), api.lastProfile()]);
    } catch (e) {
      error = String(e);
    }
  }
  loadProfiles();

  // The running session's closes, every few seconds (the daemon checks every 2).
  $effect(() => {
    const id = live.current?.session.id;
    if (!id) {
      closes = [];
      return;
    }
    const load = () =>
      api
        .sessionCloses(id)
        .then((list) => (closes = list))
        .catch(() => {});
    load();
    const timer = setInterval(load, 5000);
    return () => clearInterval(timer);
  });

  function profileName(session: StudySession): string {
    return profiles.find((p) => p.id === session.profileId)?.name ?? "";
  }

  const profile = $derived(profiles.find((p) => p.id === running?.session.profileId));

  /** Everything the profile blocks, apps first, as chip labels. */
  const blockedNames = $derived(
    profile ? [...profile.apps.map((a) => a.name), ...profile.sites.map((s) => s.domain)] : [],
  );
  /** "Wed 11 Nov · 13:00–15:30 · profile “Strict”", as in the mockup. */
  const headerLine = $derived(
    running
      ? [
          formatDay(new Date(running.session.startAt)),
          sessionTimes(running.session),
          profile ? t("session.profile", { name: profile.name }) : "",
        ]
          .filter(Boolean)
          .join(" · ")
      : "",
  );

  /** "Closed this session: Steam ×2, Discord ×1" */
  const closedText = $derived(
    closes.length === 0
      ? ""
      : t("blocked.closed", { list: closes.map((c) => `${c.name} ×${c.count}`).join(", ") }),
  );

  /** The last 7 days and the coming 14. */
  async function loadSessions() {
    const now = new Date();
    const from = new Date(now.getTime() - 7 * 86400_000);
    const to = new Date(now.getTime() + 14 * 86400_000);
    try {
      sessions = await api.sessions(from.toISOString(), to.toISOString());
    } catch (e) {
      error = String(e);
    }
  }

  // Reload when a session starts or ends.
  $effect(() => {
    void live.current?.session.id;
    untrack(() => loadSessions());
  });

  const coming = $derived(
    sessions.filter((s) => s.status === "planned" && new Date(s.startAt) > live.now),
  );
  const next = $derived(coming[0]);
  const recent = $derived(
    sessions
      .filter((s) => s.status !== "planned" && s.status !== "running" && new Date(s.startAt) <= live.now)
      .reverse(),
  );

  // ---------- The running session ----------

  const totalSeconds = $derived(
    running
      ? (new Date(running.session.endAt).getTime() - new Date(running.session.startAt).getTime()) / 1000
      : 0,
  );
  const elapsedSeconds = $derived(
    running ? (live.now.getTime() - new Date(running.session.startAt).getTime()) / 1000 : 0,
  );

  /** Breaks as [start, end] in ms, for the orange parts of the bar and the log. */
  const breaks = $derived.by(() => {
    if (!running) return [];
    const list: { start: number; end: number; event: SessionEvent }[] = [];
    const events = running.events;
    events.forEach((event, i) => {
      if (event.kind !== "break") return;
      const resume = events.slice(i + 1).find((e) => e.kind === "resume");
      const start = new Date(event.at).getTime();
      let end = resume ? new Date(resume.at).getTime() : live.now.getTime();
      if (!resume && current?.breakEndsAt) end = Math.min(end, new Date(current.breakEndsAt).getTime());
      list.push({ start, end, event });
    });
    return list;
  });

  /** Position of a time on the bar, 0 to 100. */
  function percentOf(time: number): number {
    if (!running) return 0;
    const start = new Date(running.session.startAt).getTime();
    return Math.min(100, Math.max(0, ((time - start) / 1000 / totalSeconds) * 100));
  }

  // Break credit in whole steps for the bar under the number: light = available,
  // middle = used, dark = still to earn this session.
  const creditSegments = $derived.by(() => {
    if (!running || !current) return [];
    const step = running.creditMinutes * 60;
    const total = Math.max(1, Math.floor(totalSeconds / (running.creditEveryMinutes * 60)));
    const earned = Math.round(current.creditEarnedSeconds / step);
    const used = Math.ceil(current.creditUsedSeconds / step);
    return Array.from({ length: total }, (_, i) =>
      i < used ? "used" : i < earned ? "available" : "later",
    );
  });

  /** "Take a 5-min break": less when less credit is left; the full length while there's none yet. */
  const breakMinutes = $derived.by(() => {
    if (!running || !current) return 0;
    const left = Math.floor(current.creditLeftSeconds / 60);
    return left > 0 ? Math.min(running.breakMinutes, left) : running.breakMinutes;
  });
  /** "Credit left after this break: 10 min" while on a break. */
  const creditAfterBreak = $derived(
    current ? Math.max(0, Math.floor((current.creditLeftSeconds - live.breakSecondsLeft) / 60)) : 0,
  );

  /** One log line per event. */
  function logText(event: SessionEvent): string {
    switch (event.kind) {
      case "start":
        if (event.detail === "late") {
          return t("session.log.startLate", { time: formatTime(new Date(running!.session.startAt)) });
        }
        return event.detail === "daemon" ? t("session.log.startDaemon") : t("session.log.start");
      case "break": {
        const found = breaks.find((b) => b.event.id === event.id);
        const ongoing = current?.phase === "break" && found === breaks[breaks.length - 1];
        if (!found || ongoing) return t("session.log.breakNow");
        return t("session.log.break", { minutes: Math.max(1, Math.round((found.end - found.start) / 60000)) });
      }
      case "resume":
        return event.detail === "auto" ? t("session.log.resumeAuto") : t("session.log.resume");
      case "emergency":
        return t("session.log.emergency");
      case "end":
        return t("session.log.end");
    }
  }

  async function toggleBreak() {
    if (!current) return;
    if (onBreak) await live.resume();
    else if (current.canTakeBreak) await live.takeBreak();
  }

  // ---------- Emergency stop ----------

  const stopWait = $derived(running?.emergencyWaitSeconds ?? 120);
  const stopSecondsLeft = $derived(
    stopStartedAt === null ? stopWait : Math.max(0, stopWait - (live.now.getTime() - stopStartedAt) / 1000),
  );

  // After the wait, the session ends. Leaving the page cancels the hold.
  $effect(() => {
    if (stopStartedAt !== null && stopSecondsLeft <= 0) {
      stopStartedAt = null;
      untrack(() => live.emergencyStop());
    }
  });

  // No running session (ended, or stopped elsewhere): nothing to hold any more.
  $effect(() => {
    if (!running) stopStartedAt = null;
  });

  // ---------- Planning ----------

  function plan() {
    sessionDialog?.openNew(new Date());
  }

  function open(session: StudySession) {
    if (isEditable(session, new Date())) sessionDialog?.openEdit(session);
  }

  async function saveFromDialog(id: string | null, fields: SessionFields): Promise<string | null> {
    try {
      const result = await saveSession(id, fields);
      lastProfile = fields.profileId ?? lastProfile;
      if (fields.weekly) {
        notice =
          result.overlapping > 0
            ? `${tn("session.plannedWeekly", result.planned)} ${tn("session.leftOut", result.overlapping)}`
            : tn("session.plannedWeekly", result.planned);
      }
      await loadSessions();
      return null;
    } catch (e) {
      return String(e);
    }
  }

  async function remove(session: StudySession) {
    try {
      await api.removeSession(session.id);
      notice = t("session.removed", { name: sessionName(session, courseById) });
    } catch (e) {
      error = String(e);
    }
    loadSessions();
  }

  async function removeFollowing(session: StudySession) {
    try {
      notice = tn("session.removedFollowing", await api.removeFollowingSessions(session.id));
    } catch (e) {
      error = String(e);
    }
    loadSessions();
  }

  function onKeydown(event: KeyboardEvent) {
    if (sessionDialog?.isOpen()) return;
    if ((event.target as HTMLElement).closest("input, textarea, select, [contenteditable]")) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "Escape" && stopStartedAt !== null) stopStartedAt = null;
    else if ((event.key === "b" || event.key === "B") && running) toggleBreak();
    else if (event.key === "n" || event.key === "N") plan();
    else return;
    event.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  <header>
    <div class="title">
      <h1>{t("nav.session")}</h1>
      {#if running}
        <span class="sub">{headerLine}</span>
      {:else}
        <span class="sub">{t("session.none")}</span>
      {/if}
    </div>
    {#if running}
      <span class="chip muted">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
        {t("session.locked")}
      </span>
    {:else}
      <button class="btn btn-primary" onclick={plan}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
        {t("session.plan")} <span class="kbd">N</span>
      </button>
    {/if}
  </header>

  {#if live.daemonChecked && !live.daemon}
    <p class="warning" role="status">{t(running ? "session.daemonDownRunning" : "session.daemonDown")}</p>
  {/if}
  {#if error || live.error}<p class="error" role="alert">{error ?? live.error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}

  {#if running && current}
    <div class="row">
      <section class="card timer live" aria-label={t("session.timer")}>
        {#if onBreak}
          <div class="big-block">
            <span class="what">{t("session.onBreak")}</span>
            <span class="big">{clockText(live.breakSecondsLeft)}</span>
            {#if current.breakEndsAt}
              <span class="sub">{t("session.breakHint", { time: formatTime(new Date(current.breakEndsAt)) })}</span>
            {/if}
          </div>
          <div class="actions">
            <button class="btn btn-primary" disabled={live.busy} onclick={() => live.resume()}>{t("session.backToWork")}</button>
            <span class="chip">{t("session.creditAfter", { minutes: creditAfterBreak })}</span>
          </div>
        {:else}
          <div class="big-block">
            <span class="what">
              {#if course}<span class="dot" style:--course={course.color} aria-hidden="true"></span>{/if}
              <span class="ellipsis" title={sessionAbout(running.session, courseById) || t("session.untitled")}>
                {sessionAbout(running.session, courseById) || t("session.untitled")}
              </span>
            </span>
            <span class="big">{clockText(live.secondsLeft)}</span>
            <span class="sub">
              {t("session.leftOf", {
                total: hoursText(totalSeconds),
                end: formatTime(new Date(running.session.endAt)),
              })}
            </span>
          </div>
          <div class="progress-block">
            <div
              class="bar"
              role="progressbar"
              aria-label={t("session.progress")}
              aria-valuenow={Math.round(live.progress * 100)}
              aria-valuemin="0"
              aria-valuemax="100"
            >
              <span class="fill" style:transform="scaleX({live.progress})"></span>
              {#each breaks as item (item.event.id)}
                <span
                  class="break-part"
                  style:left="{percentOf(item.start)}%"
                  style:width="{Math.max(0.5, percentOf(item.end) - percentOf(item.start))}%"
                ></span>
              {/each}
            </div>
            <div class="legend">
              <span>{formatTime(new Date(running.session.startAt))}</span>
              {#if breaks.length > 0}
                <span class="legend-break"><span class="swatch" aria-hidden="true"></span>{t("session.breakTaken")}</span>
              {/if}
              <span>{formatTime(new Date(running.session.endAt))}</span>
            </div>
          </div>
          <div class="chips">
            <span class="chip">{t("session.elapsed", { time: hoursText(elapsedSeconds) })}</span>
          </div>
        {/if}
      </section>

      <div class="side">
        <section class="card" aria-labelledby="credit-heading">
          <div class="card-head">
            <h2 id="credit-heading">{t("session.credit")}</h2>
            <span class="small">
              {t("session.creditRule", { credit: running.creditMinutes, every: running.creditEveryMinutes })}
            </span>
          </div>
          <div class="amount">
            <!-- During a break: what's left once it's over, as in the mockup. -->
            <span class="number">{onBreak ? creditAfterBreak : Math.floor(current.creditLeftSeconds / 60)}</span>
            <span class="sub">{t("session.minAvailable")}</span>
          </div>
          <div class="segments" aria-hidden="true">
            {#each creditSegments as segment, i (i)}<span class={segment}></span>{/each}
          </div>
          {#if onBreak}
            <button class="btn" disabled={live.busy} onclick={() => live.resume()}>
              {t("session.endBreak")} <span class="kbd">B</span>
            </button>
          {:else}
            <button
              class="btn"
              disabled={!current.canTakeBreak || live.busy}
              title={current.canTakeBreak ? undefined : t("session.noCredit", { credit: running.creditMinutes, every: running.creditEveryMinutes })}
              onclick={() => live.takeBreak()}
            >
              {t("session.takeBreak", { minutes: breakMinutes })} <span class="kbd">B</span>
            </button>
          {/if}
        </section>

        <section class="card" aria-labelledby="blocked-heading">
          <div class="card-head">
            <h2 id="blocked-heading">
              {t(!profile ? "blocked.nothing" : onBreak ? "blocked.paused" : "blocked.now")}
            </h2>
            {#if profile}<span class="small">{profileCount(profile)}</span>{/if}
          </div>
          {#if !profile}
            <p class="small">{t("blocked.noProfile")}</p>
          {:else if blockedNames.length === 0}
            <p class="small">{t("blocked.emptyProfile", { name: profile.name })} <a href="/settings#profiles">{t("blocked.addApps")}</a></p>
          {:else}
            <div class="chips">
              {#each blockedNames.slice(0, MAX_CHIPS) as name (name)}
                <span class="chip">{name}</span>
              {/each}
              {#if blockedNames.length > MAX_CHIPS}
                <span class="chip muted" title={blockedNames.slice(MAX_CHIPS).join(", ")}>
                  {t("blocked.more", { count: blockedNames.length - MAX_CHIPS })}
                </span>
              {/if}
            </div>
            {#if profile.sites.length > 0 && live.daemon?.websitesProblem}
              <p class="small warn" title={live.daemon.websitesProblem}>{t("blocked.websitesProblem")}</p>
            {/if}
          {/if}
          {#if closedText}<p class="small">{closedText}</p>{/if}
        </section>
      </div>
    </div>

    <div class="row">
      <section class="card log" aria-labelledby="log-heading">
        <h2 id="log-heading">{t("session.logTitle")}</h2>
        <ol>
          {#each running.events as event (event.id)}
            <li><span class="time">{formatTime(new Date(event.at))}</span>{logText(event)}</li>
          {/each}
        </ol>
      </section>

      <section class="card stop" aria-labelledby="stop-heading">
        <h2 id="stop-heading">{t("session.needStop")}</h2>
        {#if stopStartedAt === null}
          <p class="sub">{t("session.stopCost", { minutes: Math.round(stopWait / 60) })}</p>
          <button class="btn btn-danger" onclick={() => (stopStartedAt = Date.now())}>{t("session.emergency")}</button>
        {:else}
          <p>
            {t("session.confirmStop1", { minutes: Math.round(stopWait / 60) })}
            <strong class="cost">{t("session.points", { points: running.emergencyPenaltyPoints })}</strong>{t("session.confirmStop2")}
          </p>
          <div class="stop-bar" aria-hidden="true">
            <span style:transform="scaleX({1 - stopSecondsLeft / stopWait})"></span>
          </div>
          <div class="stop-foot">
            <span class="sub tabular">{t("session.toGo", { time: clockText(stopSecondsLeft) })}</span>
            <button class="btn btn-primary" onclick={() => (stopStartedAt = null)}>{t("session.keepStudying")}</button>
          </div>
        {/if}
      </section>
    </div>
  {:else}
    <div class="row">
      <section class="card timer" aria-labelledby="next-heading">
        <h2 id="next-heading" class="what">{t("session.next")}</h2>
        {#if next}
          {@const nextCourse = next.courseId ? courseById.get(next.courseId) : undefined}
          <div class="big-block">
            <span class="what">
              {#if nextCourse}<span class="dot" style:--course={nextCourse.color} aria-hidden="true"></span>{/if}
              {sessionAbout(next, courseById) || t("session.untitled")}
            </span>
            <span class="big-sm">{formatDay(new Date(next.startAt))} · {sessionTimes(next)}</span>
            <span class="sub">{t("session.starts", { when: formatAgo(new Date(next.startAt), live.now) })}</span>
          </div>
        {:else}
          <p class="sub">{t("session.nonePlanned")}</p>
        {/if}
      </section>
    </div>
  {/if}

  <div class="row">
    <section class="card list" aria-labelledby="coming-heading">
      <h2 id="coming-heading">{t("session.coming")}</h2>
      {#if coming.length === 0}
        <p class="sub">{t("session.nothingComing")}</p>
      {:else}
        <ul>
          {#each coming as item (item.id)}
            {@const itemCourse = item.courseId ? courseById.get(item.courseId) : undefined}
            <li>
              <button class="line" onclick={() => open(item)}>
                <StatusIcon status="planned" />
                <span class="line-main">
                  <span class="ellipsis" title={sessionAbout(item, courseById) || t("session.untitled")}>
                    {sessionAbout(item, courseById) || t("session.untitled")}
                  </span>
                  <span class="line-sub">
                    {#if itemCourse}<span class="dot" style:--course={itemCourse.color} aria-hidden="true"></span>{/if}
                    {[formatDay(new Date(item.startAt)), sessionTimes(item), profileName(item)]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                </span>
                {#if item.seriesId}<span class="tag">{t("session.weekly")}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section class="card list" aria-labelledby="recent-heading">
      <h2 id="recent-heading">{t("session.recent")}</h2>
      {#if recent.length === 0}
        <p class="sub">{t("session.nothingRecent")}</p>
      {:else}
        <ul>
          {#each recent as item (item.id)}
            <li class="line static">
              {#if item.status === "done"}
                <StatusIcon status="done" />
              {:else if item.status === "stopped"}
                <StatusIcon status="behind" label={t("session.status.stopped")} />
              {:else}
                <StatusIcon status="planned" label={t("session.status.missed")} />
              {/if}
              <span class="line-main">
                <span class="ellipsis">{sessionAbout(item, courseById) || t("session.untitled")}</span>
                <span class="line-sub">{formatDay(new Date(item.startAt))} · {sessionTimes(item)}</span>
              </span>
              <span class="state {item.status}">{t(statusLabels[item.status])}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

<SessionDialog
  bind:this={sessionDialog}
  {courses}
  {profiles}
  defaultProfileId={lastProfile}
  onSave={saveFromDialog}
  onDelete={remove}
  onDeleteFollowing={removeFollowing}
/>

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
    flex-direction: column;
    gap: var(--space-1);
  }

  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .sub {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  .small {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .warning,
  .notice {
    padding: var(--space-3) 14px;
    border-radius: var(--radius-md);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
  }

  .error {
    color: var(--status-behind);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 10px;
    border-radius: 99px;
    background: var(--pill);
    font-size: var(--text-base);
  }

  .chip.muted {
    color: var(--text-muted);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    align-items: stretch;
  }

  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }

  .card-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
  }

  /* The timer: the big card, pulsing while a session runs. */
  .timer {
    flex: 999 1 520px;
    padding: 28px var(--space-6);
    gap: 20px;
    background: var(--surface-2);
  }

  .live {
    position: relative;
  }

  .live::after {
    content: "";
    position: absolute;
    inset: -1px;
    border-radius: var(--radius-lg);
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
    .live::after {
      animation: none;
      opacity: 0.7;
    }
  }

  .big-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .what {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    font-size: var(--text-md);
    font-weight: var(--weight-bold);
  }

  .big {
    font-size: 72px;
    font-weight: var(--weight-bold);
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .big-sm {
    font-size: var(--text-lg);
    font-weight: var(--weight-bold);
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    background: var(--course);
  }

  .ellipsis {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .actions,
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  .progress-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .bar {
    position: relative;
    height: 10px;
    border-radius: 5px;
    background: var(--tag);
    overflow: hidden;
  }

  /* Grows by scaling from the left: only transform animates. */
  .fill {
    position: absolute;
    inset: 0;
    background: var(--control);
    border-radius: 5px;
    transform-origin: left;
    transition: transform var(--duration-panel) var(--ease-out);
  }

  .break-part {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--status-partial);
  }

  .legend {
    display: flex;
    justify-content: space-between;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .legend-break {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .swatch {
    width: 10px;
    height: 6px;
    border-radius: 2px;
    background: var(--status-partial);
  }

  .side {
    flex: 1 1 300px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .amount {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }

  .number {
    font-size: 32px;
    font-weight: var(--weight-bold);
    font-variant-numeric: tabular-nums;
  }

  .segments {
    display: grid;
    grid-auto-columns: minmax(0, 1fr);
    grid-auto-flow: column;
    gap: var(--space-1);
  }

  .segments span {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-active);
  }

  .segments .available {
    background: var(--control);
  }

  .segments .used {
    background: var(--border-dashed);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-danger {
    align-self: flex-start;
    border-color: color-mix(in srgb, var(--status-behind) 40%, var(--border-strong));
    background: transparent;
    color: var(--text-cancelled);
  }

  .btn-danger:hover {
    background: color-mix(in srgb, var(--status-behind) 12%, var(--bg));
  }

  .log {
    flex: 1 1 420px;
  }

  ol,
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .log li {
    display: flex;
    gap: 14px;
    font-size: var(--text-md);
  }

  .time {
    flex: none;
    width: 44px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .stop {
    flex: 1 1 320px;
    align-self: flex-start;
  }

  .cost {
    color: var(--text-cancelled);
  }

  .stop-bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-active);
    overflow: hidden;
  }

  .stop-bar span {
    display: block;
    height: 100%;
    background: var(--status-behind);
    border-radius: 3px;
    transform-origin: left;
  }

  .stop-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }

  .tabular {
    font-variant-numeric: tabular-nums;
  }

  .list {
    flex: 1 1 360px;
    gap: var(--space-2);
  }

  .list ul {
    gap: 0;
  }

  .line {
    width: 100%;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 44px;
    padding: 6px 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .line:hover {
    background: var(--surface-hover);
  }

  .line.static {
    cursor: default;
  }

  .line.static:hover {
    background: transparent;
  }

  .line-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .line-sub {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .line-sub .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .tag {
    flex: none;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: var(--tag);
    font-size: var(--text-sm);
  }

  .warn {
    color: var(--status-partial);
  }

  .state {
    flex: none;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .state.done {
    color: var(--status-done-text);
  }

  .state.stopped {
    color: var(--status-behind-text);
  }
</style>
