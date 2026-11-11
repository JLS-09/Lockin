<script lang="ts">
  // "Add an activity…": type and press Enter.
  // - With `courses`, a course can be picked too (activities not tied to a class).
  // - With `repeatLabel`, a toggle like "Every WPO" turns the activity into a
  //   template, so every class of that kind gets it.
  // Both appear once you start typing, so the field keeps its room.
  import type { Course } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";

  let {
    courses,
    repeatLabel,
    onAdd,
  }: {
    courses?: Course[];
    /** E.g. "Every WPO"; leave out to hide the toggle. */
    repeatLabel?: string;
    onAdd: (title: string, courseId: string | null, repeat: boolean) => void;
  } = $props();

  let title = $state("");
  let courseId = $state("");
  let repeat = $state(false);
  let input: HTMLInputElement;

  const typing = $derived(title.trim() !== "");

  /** Put the cursor in the field: the A shortcut. */
  export function focus() {
    input.focus();
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const text = title.trim();
    if (!text) return;
    onAdd(text, courseId || null, repeat);
    title = "";
    repeat = false;
  }

  function onKeydown(event: KeyboardEvent) {
    // Escape leaves the field instead of closing the panel.
    if (event.key === "Escape") {
      event.stopPropagation();
      input.blur();
    }
  }
</script>

<form onsubmit={submit}>
  <svg class="plus" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
  <input
    bind:this={input}
    bind:value={title}
    placeholder={t("activity.add")}
    aria-label={t("activity.add")}
    title="{t('activity.add')} (A)"
    onkeydown={onKeydown}
  />
  {#if courses && typing}
    <select bind:value={courseId} aria-label={t("activity.course")}>
      <option value="">{t("activity.noCourse")}</option>
      {#each courses as course (course.id)}
        <option value={course.id}>{course.name}</option>
      {/each}
    </select>
  {/if}
  {#if repeatLabel && typing}
    <button
      type="button"
      class="repeat"
      class:on={repeat}
      aria-pressed={repeat}
      title={t("template.repeatHint")}
      onclick={() => (repeat = !repeat)}
    >
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 2l4 4-4 4M3 11V9a3 3 0 0 1 3-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 0 1-3 3H3" /></svg>
      {repeatLabel}
    </button>
  {/if}
</form>

<style>
  form {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) 10px;
  }

  .plus {
    flex: none;
    margin-right: 4px;
    color: var(--text-faint);
  }

  input,
  select {
    height: 32px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    font-size: var(--text-md);
  }

  input {
    flex: 1;
    min-width: 0;
    padding: 0 var(--space-2);
  }

  input::placeholder {
    color: var(--text-faint);
  }

  input:hover,
  select:hover {
    background: var(--surface-hover);
  }

  input:focus,
  select:focus {
    outline: none;
    border-color: var(--border-strong);
    background: var(--bg);
  }

  /* Drawn by us, not by the system theme (WebKitGTK would make it light grey). */
  select {
    appearance: none;
    flex: none;
    max-width: 45%;
    padding: 0 22px 0 var(--space-2);
    border-color: var(--border-strong);
    background-color: var(--surface-2);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23aab1bd' stroke-width='2.4' stroke-linecap='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 6px center;
    font-size: var(--text-sm);
    color: var(--text-muted);
    cursor: pointer;
  }

  select:focus {
    background-color: var(--surface-2);
  }

  option {
    background: var(--surface-2);
    color: var(--text);
  }

  /* "↻ Every WPO": a small toggle, filled when on. */
  .repeat {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 var(--space-2);
    border: 1px solid var(--border-strong);
    border-radius: 99px;
    background: transparent;
    color: var(--text-muted);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--duration-micro) var(--ease-out),
      color var(--duration-micro) var(--ease-out);
  }

  .repeat:hover {
    color: var(--text);
  }

  .repeat.on {
    background: var(--control);
    border-color: var(--control);
    color: var(--control-text);
  }
</style>
