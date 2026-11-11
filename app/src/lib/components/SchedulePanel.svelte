<script lang="ts">
  // Settings → Schedule: the feeds, TimeEdit (classes) and Ufora (deadlines).
  // Paste either link to add one (Lockin tells them apart and syncs right away),
  // sync all again, remove one, or import the earlier weeks from a downloaded
  // TimeEdit file. Not in the mockups; built in the style of the panels.
  // The full link is never shown: it holds a personal key.
  import { api, type FeedInfo } from "$lib/api";
  import { formatAgo, t, tn } from "$lib/i18n/index.svelte";

  let { onChange }: { onChange?: () => void } = $props();

  let feeds = $state<FeedInfo[]>([]);
  let url = $state("");
  let busy = $state(false);
  let confirming = $state<string | null>(null);
  // The last result, good or bad, under the panel.
  let message = $state<{ text: string; error: boolean } | null>(null);
  let fileInput: HTMLInputElement;

  async function load() {
    try {
      feeds = await api.feeds();
    } catch (e) {
      message = { text: String(e), error: true };
    }
  }
  load();

  /** Run a feed action with the buttons disabled, then reload the list. */
  async function run(action: () => Promise<string>) {
    busy = true;
    message = { text: t("schedule.syncing"), error: false };
    try {
      message = { text: await action(), error: false };
      onChange?.();
    } catch (e) {
      message = { text: String(e), error: true };
    } finally {
      busy = false;
      await load();
    }
  }

  function add(event: SubmitEvent) {
    event.preventDefault();
    const pasted = url.trim();
    if (!pasted) return;
    run(async () => {
      const result = await api.addFeed(pasted);
      url = "";
      return tn(result.kind === "ufora" ? "schedule.addedDeadlines" : "schedule.added", result.added);
    });
  }

  function syncNow() {
    run(async () => {
      const results = await api.syncFeeds();
      const failed = results.find((r) => r.error);
      if (failed) {
        const origin = feeds.find((f) => f.id === failed.sourceId)?.origin ?? "";
        throw t("schedule.syncFailed", { origin, error: failed.error ?? "" });
      }
      const sum = (key: "added" | "moved" | "cancelled") =>
        results.reduce((total, r) => total + r[key], 0);
      return t("schedule.synced", {
        added: sum("added"),
        moved: sum("moved"),
        cancelled: sum("cancelled"),
      });
    });
  }

  function remove(feed: FeedInfo) {
    confirming = null;
    const key = feed.kind === "ufora" ? "schedule.removedDeadlines" : "schedule.removed";
    run(async () => tn(key, await api.removeFeed(feed.id)));
  }

  // The file is read here and its text sent to Rust, so the app needs no file access.
  async function importFile() {
    const file = fileInput.files?.[0];
    fileInput.value = "";
    if (!file) return;
    const text = await file.text();
    run(async () => {
      const result = await api.importHistory(text);
      return t("schedule.historyResult", { added: result.added, total: result.total });
    });
  }

  function lastSync(feed: FeedInfo): string {
    return feed.lastFetchedAt
      ? t("schedule.lastSync", { ago: formatAgo(new Date(feed.lastFetchedAt)) })
      : t("schedule.neverSynced");
  }
</script>

<section class="panel" id="schedule" aria-labelledby="schedule-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="schedule-heading">{t("schedule.title")}</h2>
    {#if feeds.length > 0}
      <button class="btn btn-small" disabled={busy} onclick={syncNow}>{t("schedule.syncNow")}</button>
    {/if}
  </div>
  <p class="intro">{t("schedule.hint")}</p>

  {#if feeds.length > 0}
    <ul>
      {#each feeds as feed (feed.id)}
        <li>
          {#if confirming === feed.id}
            <span class="confirm">{t(feed.kind === "ufora" ? "schedule.confirmRemoveUfora" : "schedule.confirmRemove")}</span>
            <button class="btn btn-small btn-danger" disabled={busy} onclick={() => remove(feed)}>{t("schedule.remove")}</button>
            <button class="btn btn-small" onclick={() => (confirming = null)}>{t("template.cancel")}</button>
          {:else}
            <svg class="icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 6h16v14H4zM4 10h16M9 3v4M15 3v4" /></svg>
            <span class="text">
              {#if feed.kind === "ufora"}
                <span class="ellipsis" title={feed.origin}>{t("schedule.uforaTitle", { origin: feed.origin })}</span>
                <span class="sub">{tn("schedule.deadlines", feed.deadlines)} · {lastSync(feed)}</span>
              {:else}
                <span class="ellipsis" title={feed.origin}>{t("schedule.feedTitle", { origin: feed.origin })}</span>
                <span class="sub">{tn("schedule.classes", feed.classes)} · {lastSync(feed)}</span>
              {/if}
            </span>
            <button class="tool" aria-label={t("schedule.remove")} title={t("schedule.remove")} onclick={() => (confirming = feed.id)}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <form onsubmit={add}>
    <input
      bind:value={url}
      placeholder={t("schedule.placeholder")}
      aria-label={t("schedule.placeholder")}
      autocomplete="off"
      spellcheck="false"
    />
    <button class="btn btn-small" type="submit" disabled={busy || !url.trim()}>{t("schedule.add")}</button>
  </form>

  <div class="history">
    <div class="text">
      <span>{t("schedule.historyTitle")}</span>
      <span class="sub">{t("schedule.historyHint")}</span>
    </div>
    <input bind:this={fileInput} type="file" accept=".ics,text/calendar" hidden onchange={importFile} />
    <button class="btn btn-small" disabled={busy} onclick={() => fileInput.click()}>{t("schedule.historyButton")}</button>
  </div>

  {#if message}
    <p class="panel-hint" class:error={message.error} role="status">{message.text}</p>
  {/if}
</section>

<style>
  .intro {
    padding: 0 10px var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 48px;
    padding: 4px 10px;
    border-radius: var(--radius);
  }

  li:hover {
    background: var(--surface-hover);
  }

  .icon {
    flex: none;
    color: var(--text-faint);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sub {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .confirm {
    flex: 1;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .tool {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--duration-micro) var(--ease-out);
  }

  li:hover .tool,
  .tool:focus-visible {
    opacity: 1;
  }

  .tool:hover {
    background: var(--surface-active);
    color: var(--text);
  }

  form,
  .history {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding: var(--space-2) 10px 0;
    border-top: 1px solid var(--border);
  }

  .history {
    gap: var(--space-3);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 var(--space-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: var(--text-md);
  }

  input::placeholder {
    color: var(--text-faint);
  }

  input:focus {
    outline: none;
    border-color: var(--control);
  }

  .btn-small {
    flex: none;
    min-height: 32px;
    padding: 0 12px;
    font-size: var(--text-sm);
  }

  .btn-small:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-danger {
    color: var(--text-cancelled);
  }

  .error {
    color: var(--text-cancelled);
  }
</style>
