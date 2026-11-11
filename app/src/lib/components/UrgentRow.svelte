<script lang="ts">
  // One line of the Urgent list, as in the Urgent mockup: the check button with
  // the status icon, the title with its course, a tag (Activity, Manual, Ufora)
  // and how long it's been open or when it's due.
  // Clicking the title opens the activity's week, the dialog of a deadline you
  // added, or a Ufora deadline's page in Ufora.
  // A deadline added by hand also gets a bin on hover, and the Delete key.
  import type { Course, UrgentItem } from "$lib/api";
  import { formatDay, t } from "$lib/i18n/index.svelte";
  import { urgentIcon, whenText } from "$lib/urgent/when";
  import StatusIcon from "./StatusIcon.svelte";

  let {
    item,
    course,
    done,
    now,
    onToggle,
    onOpen,
    onRemove,
  }: {
    item: UrgentItem;
    course?: Course;
    /** Checked off during this visit. */
    done: boolean;
    now: Date;
    onToggle: () => void;
    onOpen: () => void;
    /** Only for deadlines added by hand; leave out to hide the bin. */
    onRemove?: () => void;
  } = $props();

  const tag = $derived(
    item.kind === "activity"
      ? t("urgent.kindActivity")
      : item.sourceId
        ? t("urgent.kindUfora")
        : t("urgent.kindManual"),
  );

  const when = $derived(done ? t("urgent.doneNow") : whenText(item, now));

  // "moved, was Mon 16 Nov" for a Ufora deadline whose due date changed.
  const moved = $derived(
    item.movedFrom ? t("urgent.movedFrom", { day: formatDay(new Date(item.movedFrom)) }) : null,
  );

  const openTitle = $derived(
    item.kind === "activity"
      ? t("urgent.openWeek")
      : item.sourceId
        ? t("deadline.openInUfora")
        : t("deadline.edit"),
  );

  // The status names, as a screen reader says them.
  const statusLabels = {
    behind: "status.behind",
    approaching: "urgent.approaching",
    planned: "status.planned",
  } as const;

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Delete" && onRemove) {
      event.preventDefault();
      onRemove();
    }
  }
</script>

<div class="row">
  <button
    class="check"
    aria-pressed={done}
    aria-label={t(done ? "urgent.markNotDone" : "urgent.markDone", { title: item.title })}
    onclick={onToggle}
  >
    {#if done}
      <StatusIcon status="done" size={20} />
    {:else}
      <StatusIcon status={urgentIcon(item.status)} size={20} label={t(statusLabels[item.status])} />
    {/if}
  </button>

  <button
    class="text"
    title={openTitle}
    onclick={onOpen}
    onkeydown={onKeydown}
  >
    <span class="title" class:done title={item.title}>{item.title}</span>
    {#if course || moved}
      <span class="course" title={[course?.name, moved].filter(Boolean).join(" · ")}>
        {#if course}
          <span class="dot" style:background={course.color}></span>
          <span class="ellipsis">{course.name}</span>
        {/if}
        {#if moved}<span class="moved">{course ? `· ${moved}` : moved}</span>{/if}
      </span>
    {/if}
  </button>

  <span class="tag">{tag}</span>
  <span class="when {done ? 'done' : item.status}" title={when}>{when}</span>

  <span class="tools">
    {#if onRemove}
      <button class="tool" title="{t('deadline.delete')} (Delete)" aria-label={t("deadline.delete")} onclick={onRemove}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
      </button>
    {/if}
  </span>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px var(--space-3);
    border-radius: var(--radius-md);
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .row:hover {
    background: var(--surface-2);
  }

  /* A 44 px target, as in the mockup. */
  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    cursor: pointer;
    transition:
      background-color var(--duration-micro) var(--ease-out),
      transform var(--duration-micro) var(--ease-out);
  }

  .check:hover {
    background: var(--border);
  }

  /* Feedback first: the icon pops, saving happens after. */
  .check:active {
    transform: scale(0.88);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 0;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .title,
  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }

  .title.done {
    color: var(--text-done);
  }

  .course {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .moved {
    flex: none;
    color: var(--text);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  .tag {
    flex: none;
    font-size: var(--text-sm);
    padding: 2px var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--border);
    color: var(--text-muted);
  }

  .when {
    flex: none;
    width: 150px;
    text-align: right;
    font-size: var(--text-base);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .when.behind {
    color: var(--status-behind-text);
  }

  .when.approaching {
    color: var(--status-partial);
  }

  .when.planned {
    color: var(--text-muted);
  }

  .when.done {
    color: var(--status-done-text);
  }

  /* The bin appears on hover or keyboard focus; its room is always kept,
     so the "when" column stays lined up. */
  .tools {
    flex: none;
    width: 28px;
    margin-left: -6px;
    opacity: 0;
    transition: opacity var(--duration-micro) var(--ease-out);
  }

  .row:hover .tools,
  .row:focus-within .tools {
    opacity: 1;
  }

  .tool {
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

  .tool:hover {
    background: var(--surface-active);
    color: var(--text);
  }
</style>
