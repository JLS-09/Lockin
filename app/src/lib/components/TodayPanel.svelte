<script lang="ts">
  // "Wednesday 11 Nov · 1 of 4 done": today's activities, checked off with a
  // click or with Space on the selected one (a focused button does that by itself).
  import type { Activity, Course } from "$lib/api";
  import { formatDate, t } from "$lib/i18n/index.svelte";
  import StatusIcon from "./StatusIcon.svelte";

  let {
    day,
    activities,
    courses,
    onToggle,
  }: {
    day: Date;
    /** Today's activities, in the order to show them. */
    activities: Activity[];
    courses: Map<string, Course>;
    onToggle: (activity: Activity) => void;
  } = $props();

  const doneCount = $derived(activities.filter((a) => a.done).length);
</script>

<section class="panel" aria-labelledby="today-heading">
  <div class="head">
    <h2 id="today-heading">{formatDate(day, { weekday: "long", day: "numeric", month: "short" })}</h2>
    {#if activities.length > 0}
      <span class="count">{t("today.done", { done: doneCount, total: activities.length })}</span>
    {/if}
  </div>

  {#if activities.length === 0}
    <p class="empty">{t("today.empty")}</p>
  {:else}
    <div class="list">
      {#each activities as activity (activity.id)}
        {@const course = activity.courseId ? courses.get(activity.courseId) : undefined}
        <button class="act" aria-pressed={activity.done} onclick={() => onToggle(activity)}>
          <span class="check">
            {#if activity.done}
              <StatusIcon status="done" size={20} />
            {:else}
              <svg width="20" height="20" viewBox="0 0 16 16" role="img" aria-label={t("today.notDone")}><circle cx="8" cy="8" r="6.25" stroke="var(--status-planned)" stroke-width="1.5" fill="none" /></svg>
            {/if}
          </span>
          <span class="text">
            <span class="title" class:done={activity.done}>{activity.title}</span>
            {#if course}
              <span class="course"><span class="dot" style:background={course.color}></span>{course.name}</span>
            {/if}
          </span>
        </button>
      {/each}
    </div>
    <p class="hint">{t("today.hint")}</p>
  {/if}
</section>

<style>
  .panel {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--space-4) 10px 10px;
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 0 10px var(--space-2);
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  /* "woensdag" → "Woensdag": Intl writes Dutch weekdays in lower case. */
  h2::first-letter {
    text-transform: uppercase;
  }

  .count,
  .course {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .list {
    display: flex;
    flex-direction: column;
  }

  .act {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: 44px;
    padding: 6px 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .act:hover {
    background: var(--surface-hover);
  }

  .check {
    flex: none;
    display: flex;
    /* Checking off pops the icon briefly: feedback first, saving happens after. */
    transition: transform var(--duration-micro) var(--ease-out);
  }

  .act:active .check {
    transform: scale(0.85);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .title {
    font-size: var(--text-md);
    color: var(--text);
  }

  .title.done {
    color: var(--text-done);
  }

  .course {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  .empty {
    padding: 0 10px var(--space-2);
    color: var(--text-muted);
  }

  .hint {
    margin: var(--space-2) 10px var(--space-1);
    font-size: var(--text-sm);
    color: var(--text-faint);
  }
</style>
