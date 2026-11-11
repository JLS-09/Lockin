<script lang="ts">
  // The dialog to add or edit a deadline: title, course (optional), date and time.
  // Not in the mockups; built from the same pieces as the panels.
  // Opened with `openNew()` (the Deadline button, D) or `openEdit(deadline)`.
  // Enter saves, Escape closes; editing also offers Delete.
  import type { Course, Deadline } from "$lib/api";
  import { isoDate, t } from "$lib/i18n/index.svelte";
  import type { DeadlineFields } from "$lib/urgent/data.svelte";

  let {
    courses,
    onSave,
    onDelete,
  }: {
    courses: Course[];
    /** `id` is null for a new deadline. Resolves false when saving failed. */
    onSave: (id: string | null, fields: DeadlineFields) => Promise<boolean>;
    onDelete: (deadline: Deadline) => void;
  } = $props();

  let dialog: HTMLDialogElement;
  let titleInput: HTMLInputElement;

  let editing = $state<Deadline | null>(null);
  let title = $state("");
  let courseId = $state("");
  let date = $state("");
  let time = $state("23:59");
  let problem = $state<string | null>(null);
  let saving = $state(false);

  export function openNew() {
    editing = null;
    title = "";
    courseId = "";
    date = "";
    time = "23:59";
    show();
  }

  export function openEdit(deadline: Deadline) {
    const due = new Date(deadline.dueAt);
    editing = deadline;
    title = deadline.title;
    courseId = deadline.courseId ?? "";
    date = isoDate(due);
    time = `${String(due.getHours()).padStart(2, "0")}:${String(due.getMinutes()).padStart(2, "0")}`;
    show();
  }

  export function isOpen(): boolean {
    return dialog?.open ?? false;
  }

  function show() {
    problem = null;
    saving = false;
    dialog.showModal();
    titleInput.focus();
    titleInput.select();
  }

  function close() {
    dialog.close();
  }

  /** The due time from the date and time fields, in local time; null if they don't make one. */
  function dueTime(): Date | null {
    const dateMatch = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
    if (!dateMatch) return null;
    const timeMatch = /^(\d{1,2}):(\d{2})$/.exec(time || "23:59");
    if (!timeMatch) return null;
    const [, year, month, day] = dateMatch.map(Number);
    const [, hours, minutes] = timeMatch.map(Number);
    if (hours > 23 || minutes > 59) return null;
    const due = new Date(year, month - 1, day, hours, minutes);
    // 31 February rolls over to March: refuse it instead.
    return due.getDate() === day ? due : null;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const text = title.trim();
    if (!text) {
      problem = t("deadline.needTitle");
      titleInput.focus();
      return;
    }
    const due = dueTime();
    if (!due) {
      problem = t("deadline.needDate");
      return;
    }
    saving = true;
    const saved = await onSave(editing?.id ?? null, {
      title: text,
      courseId: courseId || null,
      due: due.toISOString(),
    });
    saving = false;
    if (saved) close();
  }

  function remove() {
    if (!editing) return;
    const deadline = editing;
    close();
    onDelete(deadline);
  }

  // A click on the dim backdrop (outside the card) closes the dialog.
  function onBackdrop(event: MouseEvent) {
    if (event.target === dialog) close();
  }
</script>

<dialog bind:this={dialog} aria-labelledby="deadline-heading" onclick={onBackdrop}>
  <!-- novalidate: our own message under the fields, not the browser's bubble. -->
  <form class="card" novalidate onsubmit={submit}>
    <h2 id="deadline-heading">{t(editing ? "deadline.edit" : "deadline.new")}</h2>

    <label class="field">
      <span class="label">{t("deadline.title")}</span>
      <input bind:this={titleInput} bind:value={title} placeholder={t("deadline.titlePlaceholder")} />
    </label>

    <label class="field">
      <span class="label">{t("deadline.course")}</span>
      <select bind:value={courseId}>
        <option value="">{t("activity.noCourse")}</option>
        {#each courses as course (course.id)}
          <option value={course.id}>{course.name}</option>
        {/each}
      </select>
    </label>

    <div class="when">
      <label class="field">
        <span class="label">{t("deadline.date")}</span>
        <input type="date" bind:value={date} />
      </label>
      <label class="field time">
        <span class="label">{t("deadline.time")}</span>
        <input type="time" bind:value={time} />
      </label>
    </div>

    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

    <div class="buttons">
      {#if editing}
        <button type="button" class="btn btn-danger" onclick={remove}>{t("deadline.delete")}</button>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="btn" onclick={close}>{t("deadline.cancel")}</button>
      <button type="submit" class="btn btn-primary" disabled={saving}>
        {t(editing ? "deadline.save" : "deadline.add")}
      </button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    max-width: calc(100vw - 64px);
  }

  dialog::backdrop {
    background: var(--overlay);
  }

  /* Opening rises in a little, fast and with ease-out; never in the way. */
  dialog[open] .card {
    animation: rise var(--duration-panel) var(--ease-out);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  /* "Reduce motion": only a short fade. */
  @media (prefers-reduced-motion: reduce) {
    dialog[open] .card {
      animation-name: fade;
    }

    @keyframes fade {
      from {
        opacity: 0;
      }
    }
  }

  .card {
    width: 420px;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .label {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .when {
    display: flex;
    gap: var(--space-3);
  }

  .when .field {
    flex: 1;
  }

  .when .time {
    flex: 0 0 120px;
  }

  input,
  select {
    height: 36px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    background: var(--bg);
    font-size: var(--text-md);
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
    padding-right: 28px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23aab1bd' stroke-width='2.4' stroke-linecap='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
    cursor: pointer;
  }

  option {
    background: var(--surface-2);
    color: var(--text);
  }

  .problem {
    font-size: var(--text-sm);
    color: var(--status-behind-text);
  }

  .buttons {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }

  .spacer {
    flex: 1;
  }

  .btn-danger {
    color: var(--text-cancelled);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
