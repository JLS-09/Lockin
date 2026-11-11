<script lang="ts">
  // The class you clicked in the grid: when and where, and its activities to
  // check off, rename, delete or add. Not in the mockups; built from the same
  // pieces as the Today panel. Escape or the × closes it.
  import type { Activity, ClassBlock, Course } from "$lib/api";
  import { isIgnored } from "$lib/api";
  import { formatDate, formatTime, t } from "$lib/i18n/index.svelte";
  import { kindLabel } from "$lib/week/labels";
  import ActivityRow from "./ActivityRow.svelte";
  import AddActivity from "./AddActivity.svelte";

  let {
    block,
    course,
    activities,
    onToggle,
    onRename,
    onRemove,
    onAdd,
    onClose,
  }: {
    block: ClassBlock;
    course?: Course;
    /** This block's activities. */
    activities: Activity[];
    onToggle: (activity: Activity) => void;
    onRename: (activity: Activity, title: string) => void;
    onRemove: (activity: Activity) => void;
    /** `repeat`: make it a template for every class of this kind of the course. */
    onAdd: (title: string, repeat: boolean) => void;
    onClose: () => void;
  } = $props();

  let adder: AddActivity;

  /** The A shortcut. */
  export function focusAdd() {
    adder.focus();
  }

  const start = $derived(new Date(block.startAt));
  const end = $derived(new Date(block.endAt));
  const doneCount = $derived(activities.filter((a) => a.done).length);
  // "Every WPO", or "Every class" when TimeEdit gives no kind. Only for blocks of a course.
  const repeatLabel = $derived(
    !block.courseId
      ? undefined
      : block.kind
        ? t("template.repeatKind", { kind: kindLabel(block.kind) ?? block.kind })
        : t("template.repeatAll"),
  );
  const name = $derived(course?.name ?? block.title);
  const when = $derived(
    [
      formatDate(start, { weekday: "long", day: "numeric", month: "short" }),
      kindLabel(block.kind),
      `${formatTime(start)}–${formatTime(end)}`,
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

<section class="panel" aria-labelledby="block-heading" style:--course={course?.color ?? "var(--text-faint)"}>
  <div class="panel-head">
    <h2 class="panel-title name" id="block-heading" title={name}>
      <span class="dot" aria-hidden="true"></span><span class="ellipsis">{name}</span>
    </h2>
    <button class="close" aria-label={t("block.close")} title="{t('block.close')} (Esc)" onclick={onClose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  </div>

  <div class="details">
    <span class="when" title={when}>{when}</span>
    {#if block.location}<span title={block.location}>{block.location}</span>{/if}
    {#if block.status === "cancelled"}
      <span class="cancelled">{t("block.cancelled")}</span>
    {:else if isIgnored(block)}
      <span>{t("block.ignored")}</span>
    {/if}
  </div>

  <div class="panel-head sub">
    <h3>{t("block.activities")}</h3>
    {#if activities.length > 0}
      <span class="panel-count">{t("progress.done", { done: doneCount, total: activities.length })}</span>
    {/if}
  </div>

  {#if activities.length === 0}
    <p class="panel-empty">{t("block.empty")}</p>
  {:else}
    <div>
      {#each activities as activity (activity.id)}
        <ActivityRow
          {activity}
          editable
          onToggle={() => onToggle(activity)}
          onRename={(title) => onRename(activity, title)}
          onRemove={() => onRemove(activity)}
        />
      {/each}
    </div>
  {/if}

  <AddActivity bind:this={adder} {repeatLabel} onAdd={(title, _course, repeat) => onAdd(title, repeat)} />
  <p class="panel-hint">{t("block.hint")}</p>
  {#if block.courseId}
    <a class="templates-link" href="/settings?course={block.courseId}">{t("template.manage")}</a>
  {/if}
</section>

<style>
  .name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .ellipsis,
  .details span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex: none;
    background: var(--course);
  }

  .close {
    flex: none;
    align-self: center;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .close:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 10px var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  /* Intl writes Dutch weekdays in lower case. */
  .when::first-letter {
    text-transform: uppercase;
  }

  .cancelled {
    color: var(--text-cancelled);
    font-weight: var(--weight-bold);
  }

  .sub {
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
    margin: 0 10px;
    padding-left: 0;
    padding-right: 0;
  }

  .templates-link {
    display: inline-block;
    margin: 0 10px var(--space-1);
    font-size: var(--text-sm);
    color: var(--text-muted);
    text-underline-offset: 2px;
  }

  .templates-link:hover {
    color: var(--text);
  }

  h3 {
    font-size: var(--text-base);
    font-weight: var(--weight-bold);
  }
</style>
