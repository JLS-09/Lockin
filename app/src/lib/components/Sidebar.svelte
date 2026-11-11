<script lang="ts">
  // Left column from the mockup: logo and navigation, each item with its key.
  // Pressing the key (outside a text field) opens that page.
  // At the bottom: the running study session (pulsing, as in the mockup) and
  // whether the daemon runs. Points and the streak join that card in phase 6.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { api, type Course } from "$lib/api";
  import { clockText, hoursText, live } from "$lib/session/live.svelte";
  import { sessionName } from "$lib/session/labels";
  // The full logo, with "Lockin" already drawn as outlines (no font needed).
  // It uses currentColor, so its colour comes from the CSS `color` below.
  import logo from "$lib/assets/lockin-logo.svg?raw";
  import { formatAgo, t } from "$lib/i18n/index.svelte";

  // Course names and colors for the live card.
  let courses = $state(new Map<string, Course>());
  api
    .courses()
    .then((list) => (courses = new Map(list.map((c) => [c.id, c]))))
    .catch(() => {});

  const running = $derived(live.current);
  // "Daemon running · synced 2 min ago", as in the mockup.
  const daemonText = $derived.by(() => {
    const last = live.daemon?.lastSyncAt;
    const synced = last ? t("schedule.lastSync", { ago: formatAgo(new Date(last), live.now) }) : null;
    return [t("daemon.running"), synced].filter(Boolean).join(" · ");
  });

  const runningCourse = $derived(
    running?.session.courseId ? courses.get(running.session.courseId) : undefined,
  );

  // Icons are 24×24 stroke paths from the mockup, drawn by the <svg> below.
  const links = [
    { href: "/", label: "nav.week", key: "W", icon: "M4 6h16v14H4zM4 10h16M9 3v4M15 3v4" },
    { href: "/semester", label: "nav.semester", key: "M", icon: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z" },
    { href: "/urgent", label: "nav.urgent", key: "U", icon: "M12 3l9 16H3zM12 10v4M12 17v.5" },
    { href: "/session", label: "nav.session", key: "S", icon: "M12 8v5l3 2M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM9 2h6" },
    { href: "/stats", label: "nav.stats", key: "T", icon: "M5 20V10M12 20V4M19 20v-7" },
    { href: "/shop", label: "nav.shop", key: "P", icon: "M5 8h14l-1 12H6zM9 8V6a3 3 0 0 1 6 0v2" },
    {
      href: "/settings",
      label: "nav.settings",
      key: ",",
      icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19 12h2M3 12h2M12 3v2M12 19v2M17 7l1.5-1.5M5.5 18.5L7 17M17 17l1.5 1.5M5.5 5.5L7 7",
    },
  ] as const;

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable]")) return;
    // Not while a dialog is open (the deadline dialog): it has its own keys.
    if (document.querySelector("dialog[open]")) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const link = links.find((l) => l.key === event.key.toUpperCase());
    if (!link) return;
    event.preventDefault();
    goto(link.href);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<nav aria-label={t("nav.main")}>
  <div class="logo" role="img" aria-label={t("app.name")}>
    <span aria-hidden="true">{@html logo}</span>
  </div>

  <div class="links">
    {#each links as link (link.href)}
      {@const active = page.url.pathname === link.href}
      <a href={link.href} class="nav-link" class:active aria-current={active ? "page" : undefined}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d={link.icon} />
        </svg>
        <span class="label">{t(link.label)}</span>
        <span class="key">{link.key}</span>
      </a>
    {/each}
  </div>

  <div class="spacer"></div>

  {#if running}
    <a href="/session" class="live-card">
      <span class="live-head"><span class="live-dot" aria-hidden="true"></span>{t("live.active")}</span>
      <span class="live-main">
        <span class="live-name">
          {#if runningCourse}<span class="course-dot" style:--course={runningCourse.color} aria-hidden="true"></span>{/if}
          <span class="ellipsis" title={sessionName(running.session, courses)}>{sessionName(running.session, courses)}</span>
        </span>
        <span class="live-left">{t("live.left", { time: hoursText(live.secondsLeft + 59) })}</span>
      </span>
      <span class="live-bar" aria-hidden="true"><span style:transform="scaleX({live.progress})"></span></span>
      <span class="live-sub">
        {#if running.state.phase === "break"}
          {t("live.onBreak", { time: clockText(live.breakSecondsLeft) })}
        {:else}
          {t("live.credit", { minutes: Math.floor(running.state.creditLeftSeconds / 60) })}
        {/if}
      </span>
    </a>
  {/if}

  {#if live.daemonChecked}
    <div class="status-card">
      {#if live.daemon}
        <span class="daemon"><span class="state-dot up" aria-hidden="true"></span>{daemonText}</span>
      {:else}
        <span class="daemon" title={t("daemon.notRunningHint")}>
          <span class="state-dot down" aria-hidden="true"></span>{t("daemon.notRunning")}
        </span>
      {/if}
    </div>
  {/if}
</nav>

<style>
  nav {
    height: 100%;
    padding: 20px 14px;
    background: var(--surface-1);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .logo {
    padding: 0 var(--space-2);
    color: var(--text);
  }

  .logo :global(svg) {
    display: block;
    height: 27px;
    width: auto;
  }

  .links {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
    padding: 0 var(--space-3);
    border-radius: var(--radius);
    color: var(--text-muted);
    text-decoration: none;
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    transition:
      background-color var(--duration-micro) var(--ease-out),
      color var(--duration-micro) var(--ease-out);
  }

  .nav-link:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .nav-link.active {
    background: var(--surface-active);
    color: var(--text);
  }

  .nav-link.active svg {
    color: var(--control);
  }

  .label {
    flex: 1;
  }

  .key {
    font-size: var(--text-sm);
    color: var(--text-faint);
  }

  .spacer {
    flex: 1;
  }

  /* The running session, with the pulsing border of "study session active". */
  .live-card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--surface-hover);
    color: var(--text);
    text-decoration: none;
  }

  .live-card::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: var(--radius-md);
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
    .live-card::after {
      animation: none;
      opacity: 0.7;
    }
  }

  .live-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    font-weight: var(--weight-bold);
  }

  .live-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--status-active);
  }

  .live-main {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-md);
    font-weight: var(--weight-bold);
  }

  .live-name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .ellipsis {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .course-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--course);
  }

  .live-left {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .live-bar {
    height: 4px;
    border-radius: 2px;
    background: var(--tag);
    overflow: hidden;
  }

  /* Only transform animates: the bar grows by scaling from the left. */
  .live-bar span {
    display: block;
    height: 100%;
    background: var(--control);
    border-radius: 2px;
    transform-origin: left;
    transition: transform var(--duration-panel) var(--ease-out);
  }

  .live-sub {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .status-card {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--surface-2);
  }

  .daemon {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .state-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .state-dot.up {
    background: var(--status-done);
  }

  /* Not running: a hollow ring, so it differs by shape too, not only color. */
  .state-dot.down {
    border: 1.5px solid var(--status-behind);
  }
</style>
