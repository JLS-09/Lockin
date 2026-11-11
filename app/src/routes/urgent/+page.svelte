<script lang="ts">
  // The Urgent page, following the Urgent mockup: the header with today's date
  // and the Deadline button, the Urgent list with its All / Activities /
  // Deadlines tabs, and beside it "Coming 2 weeks" and "Feeds".
  // The deadline dialog isn't in the mockups; it's built from the same pieces.
  //
  // Keys: D adds a deadline, Ctrl+Z brings back a deleted one.
  // In the list: Space checks off, Enter opens, Delete removes a deadline.
  import { goto } from "$app/navigation";
  import { cubicOut } from "svelte/easing";
  import { fade, fly } from "svelte/transition";

  import type { Deadline, UrgentItem } from "$lib/api";
  import ComingPanel from "$lib/components/ComingPanel.svelte";
  import DeadlineDialog from "$lib/components/DeadlineDialog.svelte";
  import FeedsPanel from "$lib/components/FeedsPanel.svelte";
  import UrgentRow from "$lib/components/UrgentRow.svelte";
  import { formatDay, t } from "$lib/i18n/index.svelte";
  import { itemKey, UrgentData, type DeadlineFields } from "$lib/urgent/data.svelte";

  type Tab = "all" | "activities" | "deadlines";
  const tabs: { id: Tab; label: "urgent.tabAll" | "urgent.tabActivities" | "urgent.tabDeadlines" }[] = [
    { id: "all", label: "urgent.tabAll" },
    { id: "activities", label: "urgent.tabActivities" },
    { id: "deadlines", label: "urgent.tabDeadlines" },
  ];

  const data = new UrgentData();
  data.load();

  let tab = $state<Tab>("all");
  let now = $state(new Date());
  let dialog = $state<DeadlineDialog>();
  // The last deleted deadline, for Undo and Ctrl+Z.
  let deleted = $state<Deadline | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const shown = $derived(
    data.items.filter((item) =>
      tab === "all" ? true : tab === "activities" ? item.kind === "activity" : item.kind === "deadline",
    ),
  );

  const emptyText = $derived(
    tab === "activities"
      ? t("urgent.emptyActivities")
      : tab === "deadlines"
        ? t("urgent.emptyDeadlines")
        : t("urgent.empty"),
  );

  // Keep "now" fresh, so "Open 3 days" and "Due today" stay right.
  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 60_000);
    return () => clearInterval(timer);
  });

  /** Activities open their week; deadlines added by hand open the dialog. */
  function open(item: UrgentItem) {
    if (item.kind === "activity") {
      if (item.week) goto(`/?date=${item.week}`);
      return;
    }
    const deadline = data.deadline(item.id);
    if (deadline && !deadline.sourceId) dialog?.openEdit(deadline);
  }

  /** The bin is only for deadlines added by hand: feed deadlines come back with the next sync. */
  function removable(item: UrgentItem): (() => void) | undefined {
    if (item.kind !== "deadline" || item.sourceId) return undefined;
    return () => {
      const deadline = data.deadline(item.id);
      if (deadline) remove(deadline);
    };
  }

  async function save(id: string | null, fields: DeadlineFields): Promise<boolean> {
    return id ? data.updateDeadline(id, fields) : data.addDeadline(fields);
  }

  async function remove(deadline: Deadline) {
    if (!(await data.removeDeadline(deadline))) return;
    deleted = deadline;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (deleted = null), 6000);
  }

  function undo() {
    if (!deleted) return;
    data.restoreDeadline(deleted);
    deleted = null;
    clearTimeout(toastTimer);
  }

  function onKeydown(event: KeyboardEvent) {
    if (dialog?.isOpen()) return;
    const typing = (event.target as HTMLElement).closest(
      "input, textarea, select, [contenteditable]",
    );
    if (typing) return;
    if ((event.ctrlKey || event.metaKey) && event.key === "z" && deleted) {
      event.preventDefault();
      undo();
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "d" || event.key === "D") {
      event.preventDefault();
      dialog?.openNew();
    }
  }

  // The toast rises in a little; with "reduce motion" it only fades.
  function toastIn(node: Element) {
    const duration =
      parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--duration-panel")) || 200;
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return fade(node, { duration });
    return fly(node, { y: 8, duration, easing: cubicOut });
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  <header>
    <div class="title">
      <h1>{t("nav.urgent")}</h1>
      <span class="date">{formatDay(now)}</span>
    </div>
    <button class="btn btn-primary" title="{t('deadline.new')} (D)" onclick={() => dialog?.openNew()}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
      {t("urgent.addDeadline")}
      <span class="kbd">D</span>
    </button>
  </header>

  {#if data.error}
    <p class="error" role="alert">{t("urgent.loadFailed", { error: data.error })}</p>
  {/if}

  <div class="content">
    <section class="card list" aria-labelledby="urgent-heading">
      <div class="list-head">
        <div class="list-title">
          <h2 id="urgent-heading">{t("urgent.listTitle")}</h2>
          <span class="hint">{t("urgent.listHint")}</span>
        </div>
        <div class="tabs" role="tablist" aria-label={t("urgent.tabs")}>
          {#each tabs as option (option.id)}
            <button
              role="tab"
              class="tab"
              class:on={tab === option.id}
              aria-selected={tab === option.id}
              onclick={() => (tab = option.id)}
            >
              {t(option.label)}
            </button>
          {/each}
        </div>
      </div>

      {#if !data.loaded}
        <p class="empty">{t("urgent.loading")}</p>
      {:else if shown.length === 0}
        <p class="empty">{emptyText}</p>
      {:else}
        <div role="tabpanel" aria-labelledby="urgent-heading">
          {#each shown as item (itemKey(item))}
            <UrgentRow
              {item}
              course={item.courseId ? data.courseById.get(item.courseId) : undefined}
              done={data.isDone(item)}
              {now}
              onToggle={() => data.toggle(item)}
              onOpen={() => open(item)}
              onRemove={removable(item)}
            />
          {/each}
        </div>
      {/if}
    </section>

    <aside>
      <ComingPanel deadlines={data.deadlines} blocks={data.blocks} courses={data.courseById} {now} />
      <FeedsPanel feeds={data.feeds} {now} />
    </aside>
  </div>
</div>

<DeadlineDialog bind:this={dialog} courses={data.courses} onSave={save} onDelete={remove} />

{#if deleted}
  <div class="toast" role="status" in:toastIn>
    <span class="toast-text" title={deleted.title}>{t("activity.deleted", { title: deleted.title })}</span>
    <button class="btn btn-small" title="{t('activity.undo')} (Ctrl+Z)" onclick={undo}>{t("activity.undo")}</button>
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

  .date {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  .kbd {
    font-size: var(--text-sm);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: var(--kbd);
    font-weight: var(--weight-bold);
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

  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  .list {
    flex: 999 1 560px;
    min-width: 0;
    padding: 18px var(--space-3) var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .list-head {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2) 6px;
  }

  .list-title {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .hint {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .tabs {
    display: flex;
    gap: var(--space-1);
  }

  .tab {
    min-height: 32px;
    padding: 0 var(--space-3);
    border: 0;
    border-radius: 99px;
    background: transparent;
    color: var(--text-muted);
    font-size: var(--text-base);
    font-weight: var(--weight-medium);
    cursor: pointer;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .tab:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .tab.on {
    background: var(--surface-active);
    color: var(--text);
  }

  .empty {
    padding: var(--space-3) var(--space-2) var(--space-2);
    color: var(--text-muted);
  }

  aside {
    flex: 1 1 300px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
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
