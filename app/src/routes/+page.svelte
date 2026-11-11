<script lang="ts">
  // The week page. For now a plain list per day, to check that data, tokens and
  // translations all arrive; step 3 replaces the list with the CSS Grid week view.
  import { api, isIgnored, type ClassBlock, type Course, type Week } from "$lib/api";
  import {
    addDays,
    formatDate,
    formatRange,
    formatTime,
    isoDate,
    isoWeek,
    parseIsoDate,
    t,
    tn,
  } from "$lib/i18n/index.svelte";

  // Any day in the shown week; starts at today.
  let date = $state(new Date());
  let week = $state<Week | null>(null);
  let courses = $state<Course[]>([]);
  let error = $state<string | null>(null);

  const monday = $derived(week ? parseIsoDate(week.monday) : null);
  const courseById = $derived(new Map(courses.map((c) => [c.id, c])));

  // The blocks per day, Monday to Sunday; weekend days only when they have classes.
  const days = $derived.by(() => {
    if (!week || !monday) return [];
    return Array.from({ length: 7 }, (_, i) => {
      const day = addDays(monday, i);
      const blocks = week!.blocks.filter((b) => isoDate(new Date(b.startAt)) === isoDate(day));
      return { day, blocks };
    }).filter(({ day, blocks }) => day.getDay() % 6 !== 0 || blocks.length > 0);
  });

  api
    .courses()
    .then((list) => (courses = list))
    .catch((e) => (error = String(e)));

  // Reload whenever the date changes. A slow answer for an older week is dropped.
  let latest = 0;
  $effect(() => {
    const request = ++latest;
    api
      .week(isoDate(date))
      .then((result) => {
        if (request === latest) {
          week = result;
          error = null;
        }
      })
      .catch((e) => {
        if (request === latest) error = String(e);
      });
  });

  function activityCount(block: ClassBlock) {
    const list = week?.activities.filter((a) => a.blockId === block.id) ?? [];
    return { done: list.filter((a) => a.done).length, total: list.length };
  }

  function courseName(block: ClassBlock): string {
    return (block.courseId && courseById.get(block.courseId)?.name) || block.title;
  }

  function courseColor(block: ClassBlock): string {
    return (block.courseId && courseById.get(block.courseId)?.color) || "var(--text-faint)";
  }
</script>

<header>
  <div class="title">
    <h1>{t("week.title", { number: isoWeek(date) })}</h1>
    {#if monday}
      <span class="range">{formatRange(monday, addDays(monday, 6))}</span>
    {/if}
  </div>
  <div class="nav">
    <button class="btn btn-icon" aria-label={t("week.previous")} onclick={() => (date = addDays(date, -7))}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M15 6l-6 6 6 6" /></svg>
    </button>
    <button class="btn" onclick={() => (date = new Date())}>{t("week.today")}</button>
    <button class="btn btn-icon" aria-label={t("week.next")} onclick={() => (date = addDays(date, 7))}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
    </button>
  </div>
</header>

{#if error}
  <p class="message error" role="alert">{t("week.loadFailed", { error })}</p>
{:else if !week}
  <p class="message">{t("week.loading")}</p>
{:else if week.blocks.length === 0}
  <p class="message">{t("week.empty")}</p>
{:else}
  <div class="days">
    {#each days as { day, blocks } (isoDate(day))}
      <section>
        <h2>{formatDate(day, { weekday: "long", day: "numeric", month: "short" })}</h2>
        <ul>
          {#each blocks as block (block.id)}
            {@const count = activityCount(block)}
            <li
              class:ignored={isIgnored(block)}
              class:cancelled={block.status === "cancelled"}
              style:--course={courseColor(block)}
            >
              <span class="time">
                {formatTime(new Date(block.startAt))}–{formatTime(new Date(block.endAt))}
              </span>
              <span class="dot" aria-hidden="true"></span>
              <span class="name">{courseName(block)}</span>
              {#if block.kind}<span class="muted">{block.kind}</span>{/if}
              {#if block.location}<span class="muted">{block.location}</span>{/if}
              {#if block.status === "moved"}<span class="tag">{t("block.moved")}</span>{/if}
              {#if block.status === "cancelled"}<span class="tag">{t("block.cancelled")}</span>{/if}
              {#if isIgnored(block)}<span class="tag">{t("block.ignored")}</span>{/if}
              {#if count.total > 0}
                <span class="muted count">{tn("block.activities", count.total, count)}</span>
              {/if}
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>
{/if}

<style>
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    margin-bottom: var(--space-5);
  }

  .title {
    flex: 1 1 auto;
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  h1 {
    font-size: var(--text-lg);
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .range,
  .muted,
  .message {
    color: var(--text-muted);
  }

  .nav {
    display: flex;
    gap: var(--space-2);
  }

  .error {
    color: var(--status-behind);
  }

  .days {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  h2 {
    font-size: var(--text-md);
    font-weight: var(--weight-bold);
    margin-bottom: var(--space-2);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius);
    /* The course color tints the background, as in the mockups. */
    background: color-mix(in srgb, var(--course) var(--course-tint), var(--surface-1));
  }

  li.ignored {
    opacity: 0.5;
  }

  li.cancelled .name {
    text-decoration: line-through;
  }

  .time {
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    min-width: 96px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--course);
  }

  .name {
    font-weight: var(--weight-bold);
  }

  .tag {
    font-size: var(--text-sm);
    padding: 1px var(--space-2);
    border-radius: 99px;
    background: var(--surface-active);
  }

  .count {
    margin-left: auto;
    font-size: var(--text-sm);
  }
</style>
