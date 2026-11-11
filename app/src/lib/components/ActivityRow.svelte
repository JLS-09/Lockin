<script lang="ts">
  // One activity, as in the Today panel of the mockup: click (or Space) checks it off.
  // When editable: F2 or the pencil renames it, Delete or the bin removes it.
  import type { Activity, Course } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";
  import StatusIcon from "./StatusIcon.svelte";

  let {
    activity,
    course,
    editable = false,
    onToggle,
    onRename,
    onRemove,
  }: {
    activity: Activity;
    /** Shown under the title; leave out when the panel is already about one course. */
    course?: Course;
    editable?: boolean;
    onToggle: () => void;
    onRename?: (title: string) => void;
    onRemove?: () => void;
  } = $props();

  let editing = $state(false);
  let draft = $state("");

  function startEditing() {
    if (!editable) return;
    draft = activity.title;
    editing = true;
  }

  function save() {
    if (!editing) return;
    editing = false;
    const title = draft.trim();
    if (title && title !== activity.title) onRename?.(title);
  }

  function onRowKeydown(event: KeyboardEvent) {
    if (!editable) return;
    if (event.key === "F2") {
      event.preventDefault();
      startEditing();
    } else if (event.key === "Delete") {
      event.preventDefault();
      onRemove?.();
    }
  }

  function onInputKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      save();
    } else if (event.key === "Escape") {
      // Cancel the rename only; don't let Escape also close the panel.
      event.stopPropagation();
      editing = false;
    }
  }

  // Focus the field and select the text, ready to type over.
  function focusAndSelect(input: HTMLInputElement) {
    input.focus();
    input.select();
  }
</script>

<div class="row">
  {#if editing}
    <div class="act editing">
      <span class="check">{@render icon()}</span>
      <input
        bind:value={draft}
        aria-label={t("activity.rename")}
        onkeydown={onInputKeydown}
        onblur={save}
        use:focusAndSelect
      />
    </div>
  {:else}
    <button class="act" aria-pressed={activity.done} onclick={onToggle} onkeydown={onRowKeydown}>
      <span class="check">{@render icon()}</span>
      <span class="text">
        <span class="title" class:done={activity.done} title={activity.title}>{activity.title}</span>
        {#if course}
          <span class="course" title={course.name}>
            <span class="dot" style:background={course.color}></span>
            <span class="ellipsis">{course.name}</span>
          </span>
        {/if}
      </span>
    </button>
    {#if editable}
      <div class="tools">
        <button class="tool" title="{t('activity.rename')} (F2)" aria-label={t("activity.rename")} onclick={startEditing}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 20h4L19 9l-4-4L4 16zM14 6l4 4" /></svg>
        </button>
        <button class="tool" title="{t('activity.delete')} (Delete)" aria-label={t("activity.delete")} onclick={onRemove}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
        </button>
      </div>
    {/if}
  {/if}
</div>

{#snippet icon()}
  {#if activity.done}
    <StatusIcon status="done" size={20} />
  {:else}
    <svg width="20" height="20" viewBox="0 0 16 16" role="img" aria-label={t("today.notDone")}><circle cx="8" cy="8" r="6.25" stroke="var(--status-planned)" stroke-width="1.5" fill="none" /></svg>
  {/if}
{/snippet}

<style>
  .row {
    position: relative;
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .row:hover {
    background: var(--surface-hover);
  }

  .act {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 44px;
    padding: 6px 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .act.editing {
    cursor: default;
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

  .title,
  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-size: var(--text-md);
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

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  input {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 var(--space-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: var(--text-md);
  }

  input:focus {
    outline: none;
    border-color: var(--control);
  }

  /* Rename and delete appear on hover, or when the row has keyboard focus. */
  .tools {
    display: flex;
    gap: 2px;
    padding-right: 6px;
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
