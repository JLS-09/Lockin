<script lang="ts">
  // "Wednesday 11 Nov · 1 of 4 done": today's activities, checked off with a
  // click or with Space on the selected one (a focused button does that by itself).
  import type { Activity, Course } from "$lib/api";
  import { formatDate, t } from "$lib/i18n/index.svelte";
  import ActivityRow from "./ActivityRow.svelte";

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
  <div class="panel-head">
    <h2 class="panel-title" id="today-heading">
      {formatDate(day, { weekday: "long", day: "numeric", month: "short" })}
    </h2>
    {#if activities.length > 0}
      <span class="panel-count">{t("progress.done", { done: doneCount, total: activities.length })}</span>
    {/if}
  </div>

  {#if activities.length === 0}
    <p class="panel-empty">{t("today.empty")}</p>
  {:else}
    <div>
      {#each activities as activity (activity.id)}
        <ActivityRow
          {activity}
          course={activity.courseId ? courses.get(activity.courseId) : undefined}
          onToggle={() => onToggle(activity)}
        />
      {/each}
    </div>
    <p class="panel-hint">{t("today.hint")}</p>
  {/if}
</section>
