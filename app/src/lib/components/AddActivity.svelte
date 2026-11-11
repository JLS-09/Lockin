<script lang="ts">
  // "Add an activity…": type and press Enter. With `courses`, a course can be
  // picked too (for activities that aren't tied to a class).
  import type { Course } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";

  let {
    courses,
    onAdd,
  }: {
    courses?: Course[];
    onAdd: (title: string, courseId: string | null) => void;
  } = $props();

  let title = $state("");
  let courseId = $state("");
  let input: HTMLInputElement;

  /** Put the cursor in the field: the A shortcut. */
  export function focus() {
    input.focus();
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const text = title.trim();
    if (!text) return;
    onAdd(text, courseId || null);
    title = "";
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
  <!-- The course picker appears once you start typing, so the field keeps its room. -->
  {#if courses && title.trim()}
    <select bind:value={courseId} aria-label={t("activity.course")}>
      <option value="">{t("activity.noCourse")}</option>
      {#each courses as course (course.id)}
        <option value={course.id}>{course.name}</option>
      {/each}
    </select>
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
</style>
