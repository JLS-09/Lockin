<script lang="ts">
  // One course in Settings → Courses: its templates ("After every WPO: Exercises"),
  // to add or remove. Not in the mockups; built in the style of the panels.
  import type { ActivityTemplate, Course } from "$lib/api";
  import { t, tn } from "$lib/i18n/index.svelte";
  import { kindLabel, kindName } from "$lib/week/labels";

  let {
    course,
    kinds,
    templates,
    highlighted = false,
    onAdd,
    onRemove,
  }: {
    course: Course;
    /** The kinds of class this course has. */
    kinds: string[];
    templates: ActivityTemplate[];
    /** Opened from a class panel: draw attention to this card. */
    highlighted?: boolean;
    /** Returns how many activities the template made. */
    onAdd: (kind: string | null, title: string) => Promise<number>;
    /** Returns how many open activities went with it. */
    onRemove: (template: ActivityTemplate) => Promise<number>;
  } = $props();

  let title = $state("");
  let kind = $state("");
  // The template waiting for "Remove" to be confirmed.
  let confirming = $state<string | null>(null);
  let message = $state("");

  function rule(template: ActivityTemplate): string {
    return template.kind
      ? t("template.everyKind", { kind: kindLabel(template.kind) ?? template.kind })
      : t("template.everyClass");
  }

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const text = title.trim();
    if (!text) return;
    const created = await onAdd(kind || null, text);
    title = "";
    message = tn("template.added", created);
  }

  async function remove(template: ActivityTemplate) {
    confirming = null;
    const removed = await onRemove(template);
    message = tn("template.removed", removed);
  }
</script>

<section class="panel card" class:highlighted id="course-{course.id}" aria-labelledby="course-{course.id}-name">
  <div class="panel-head">
    <h3 class="name" id="course-{course.id}-name" title={course.name}>
      <span class="dot" style:background={course.color} aria-hidden="true"></span>
      <span class="ellipsis">{course.name}</span>
    </h3>
    {#if course.code}<span class="panel-count">{course.code}</span>{/if}
  </div>

  {#if templates.length === 0}
    <p class="panel-empty">{t("template.empty")}</p>
  {:else}
    <ul>
      {#each templates as template (template.id)}
        <li>
          {#if confirming === template.id}
            <span class="confirm">{t("template.confirmRemove")}</span>
            <button class="btn btn-small btn-danger" onclick={() => remove(template)}>{t("template.remove")}</button>
            <button class="btn btn-small" onclick={() => (confirming = null)}>{t("template.cancel")}</button>
          {:else}
            <svg class="icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 2l4 4-4 4M3 11V9a3 3 0 0 1 3-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 0 1-3 3H3" /></svg>
            <span class="text">
              <span class="ellipsis" title={template.title}>{template.title}</span>
              <span class="rule">{rule(template)}</span>
            </span>
            <button class="tool" aria-label={t("template.remove")} title={t("template.remove")} onclick={() => (confirming = template.id)}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  <form onsubmit={add}>
    <select bind:value={kind} aria-label={t("template.when")}>
      <option value="">{t("template.everyClass")}</option>
      {#each kinds as k (k)}
        <option value={k}>{t("template.everyKind", { kind: kindName(k) })}</option>
      {/each}
    </select>
    <input bind:value={title} placeholder={t("template.placeholder")} aria-label={t("template.title")} />
    <button class="btn btn-small" type="submit" disabled={!title.trim()}>{t("template.add")}</button>
  </form>
  {#if message}<p class="panel-hint" role="status">{message}</p>{/if}
</section>

<style>
  .card {
    transition: border-color var(--duration-panel) var(--ease-out);
  }

  .highlighted {
    border-color: var(--control);
  }

  .name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex: none;
  }

  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 44px;
    padding: 4px 10px;
    border-radius: var(--radius);
  }

  li:hover {
    background: var(--surface-hover);
  }

  .icon {
    flex: none;
    color: var(--text-faint);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rule {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .confirm {
    flex: 1;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .tool {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--duration-micro) var(--ease-out);
  }

  li:hover .tool,
  .tool:focus-visible {
    opacity: 1;
  }

  .tool:hover {
    background: var(--surface-active);
    color: var(--text);
  }

  form {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding: var(--space-2) 10px 0;
    border-top: 1px solid var(--border);
  }

  input,
  select {
    height: 32px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
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

  input:focus,
  select:focus {
    outline: none;
    border-color: var(--control);
  }

  /* Drawn by us, not by the system theme (WebKitGTK would make it light grey). */
  select {
    appearance: none;
    flex: none;
    max-width: 50%;
    padding: 0 24px 0 var(--space-2);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23aab1bd' stroke-width='2.4' stroke-linecap='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 8px center;
    font-size: var(--text-sm);
    cursor: pointer;
  }

  option {
    background: var(--surface-2);
    color: var(--text);
  }

  .btn-small {
    flex: none;
    min-height: 32px;
    padding: 0 12px;
    font-size: var(--text-sm);
  }

  .btn-small:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-danger {
    color: var(--text-cancelled);
  }
</style>
