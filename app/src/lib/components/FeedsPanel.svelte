<script lang="ts">
  // "Feeds" from the Urgent mockup: each saved feed and when it was last fetched.
  // The links themselves are never shown (they hold a personal key).
  import type { FeedInfo } from "$lib/api";
  import { formatDay, formatTime, isoDate, t } from "$lib/i18n/index.svelte";

  let { feeds, now }: { feeds: FeedInfo[]; now: Date } = $props();

  /** "fetched 14:00" today, "fetched Mon 9 Nov 14:00" before. */
  function fetched(feed: FeedInfo): string {
    if (!feed.lastFetchedAt) return t("feeds.never");
    const time = new Date(feed.lastFetchedAt);
    const when =
      isoDate(time) === isoDate(now)
        ? formatTime(time)
        : `${formatDay(time)} ${formatTime(time)}`;
    return t("feeds.fetched", { when });
  }

  /** "TimeEdit", or "TimeEdit · cloud.timeedit.net" when there are two of a kind. */
  function name(feed: FeedInfo): string {
    const kind = feed.kind === "ufora" ? t("feeds.ufora") : t("feeds.timeedit");
    const twins = feeds.filter((f) => f.kind === feed.kind).length > 1;
    return twins ? `${kind} · ${feed.origin}` : kind;
  }
</script>

<section class="card" aria-labelledby="feeds-heading">
  <h2 id="feeds-heading">{t("feeds.title")}</h2>

  {#if feeds.length === 0}
    <p class="note">{t("feeds.none")}</p>
  {/if}
  {#each feeds as feed (feed.id)}
    <div class="feed">
      <span class="name" title={name(feed)}>{name(feed)}</span>
      <span class="fetched">{fetched(feed)}</span>
    </div>
  {/each}

  <p class="note">{t("feeds.uforaSoon")}</p>
  <a href="/settings#schedule">{t("feeds.manage")}</a>
</section>

<style>
  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .feed {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
    font-size: var(--text-md);
  }

  .name {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .fetched {
    flex: none;
    color: var(--text-muted);
  }

  .note {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  a {
    align-self: flex-start;
    font-size: var(--text-base);
    color: var(--text-muted);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  a:hover {
    color: var(--control-hover);
  }
</style>
