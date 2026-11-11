<script lang="ts">
  // "This week": activities of the shown week that aren't tied to a class,
  // like "Read chapter 3", optionally for a course. Not in the mockups;
  // built from the same pieces as the Today panel.
  import type { Activity, Course } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";
  import ActivityRow from "./ActivityRow.svelte";
  import AddActivity from "./AddActivity.svelte";

  let {
    activities,
    courses,
    onToggle,
    onRename,
    onRemove,
    onAdd,
  }: {
    /** The week's activities without a block. */
    activities: Activity[];
    courses: Course[];
    onToggle: (activity: Activity) => void;
    onRename: (activity: Activity, title: string) => void;
    onRemove: (activity: Activity) => void;
    onAdd: (title: string, courseId: string | null) => void;
  } = $props();

  let adder: AddActivity;

  /** The A shortcut, when no class is open. */
  export function focusAdd() {
    adder.focus();
  }

  const courseById = $derived(new Map(courses.map((c) => [c.id, c])));
  const doneCount = $derived(activities.filter((a) => a.done).length);
</script>

<section class="panel" aria-labelledby="week-panel-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="week-panel-heading">{t("weekTodos.title")}</h2>
    {#if activities.length > 0}
      <span class="panel-count">{t("progress.done", { done: doneCount, total: activities.length })}</span>
    {/if}
  </div>

  {#if activities.length === 0}
    <p class="panel-empty">{t("weekTodos.empty")}</p>
  {:else}
    <div>
      {#each activities as activity (activity.id)}
        <ActivityRow
          {activity}
          course={activity.courseId ? courseById.get(activity.courseId) : undefined}
          editable
          onToggle={() => onToggle(activity)}
          onRename={(title) => onRename(activity, title)}
          onRemove={() => onRemove(activity)}
        />
      {/each}
    </div>
  {/if}

  <AddActivity bind:this={adder} {courses} {onAdd} />
</section>
