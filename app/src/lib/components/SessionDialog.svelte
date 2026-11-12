<script lang="ts">
  // The dialog to plan or change a study session: what (optional), course
  // (optional), the day and the time, and for a new one "every week until the
  // end of the semester". Not in the mockups; built like the deadline dialog.
  // The block profile picks the apps the session closes (Settings → Block profiles);
  // a new session starts with the profile of the last one planned.
  // Opened with `openNew(day)` (the Study session button, N) or `openEdit(session)`.
  // Enter saves, Escape closes. A session of a weekly repeat can be deleted
  // alone or together with its later weeks.
  import type { BlockProfile, Course, StudySession } from "$lib/api";
  import { formatDay, isoDate, t } from "$lib/i18n/index.svelte";
  import type { SessionFields } from "$lib/session/sessions";
  import { semesterEnd } from "$lib/week/semester";

  let {
    courses,
    profiles,
    defaultProfileId = null,
    onSave,
    onDelete,
    onDeleteFollowing,
  }: {
    courses: Course[];
    profiles: BlockProfile[];
    /** The profile a new session starts with (the last one used). */
    defaultProfileId?: string | null;
    /** `id` is null for a new session. Resolves to null when saved, else to the reason it wasn't. */
    onSave: (id: string | null, fields: SessionFields) => Promise<string | null>;
    onDelete: (session: StudySession) => void;
    onDeleteFollowing: (session: StudySession) => void;
  } = $props();

  /** A session lasts at most this long (`MAX_SESSION_HOURS` in session.rs). */
  const MAX_HOURS = 12;

  let dialog: HTMLDialogElement;
  let titleInput: HTMLInputElement;

  let editing = $state<StudySession | null>(null);
  let title = $state("");
  let courseId = $state("");
  let profileId = $state("");
  let date = $state("");
  let start = $state("13:00");
  let end = $state("15:00");
  let weekly = $state(false);
  let problem = $state<string | null>(null);
  let saving = $state(false);

  // "Every week until the end of the semester (Sun 31 Jan)": the day the repeat stops.
  const repeatUntil = $derived.by(() => {
    const day = parseDate(date);
    return day ? formatDay(semesterEnd(day)) : "";
  });

  /** "13:00" for a Date. */
  function clock(time: Date): string {
    return `${String(time.getHours()).padStart(2, "0")}:${String(time.getMinutes()).padStart(2, "0")}`;
  }

  /**
   * A new session on `day`. Today it starts at the next whole hour, another day
   * at 13:00; it lasts two hours, as most sessions in the mockups do.
   */
  export function openNew(day: Date) {
    const now = new Date();
    const today = isoDate(day) === isoDate(now);
    const from = today ? now.getHours() + 1 : 13;
    editing = null;
    title = "";
    courseId = "";
    profileId = profiles.some((p) => p.id === defaultProfileId) ? (defaultProfileId ?? "") : "";
    date = isoDate(day);
    start = `${String(Math.min(from, 23)).padStart(2, "0")}:00`;
    end = `${String(Math.min(from + 2, 23)).padStart(2, "0")}:${from + 2 > 23 ? "59" : "00"}`;
    weekly = false;
    show();
  }

  export function openEdit(session: StudySession) {
    const from = new Date(session.startAt);
    editing = session;
    title = session.title ?? "";
    courseId = session.courseId ?? "";
    profileId = session.profileId ?? "";
    date = isoDate(from);
    start = clock(from);
    end = clock(new Date(session.endAt));
    weekly = false;
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

  function parseDate(text: string): Date | null {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
    if (!match) return null;
    const [, year, month, day] = match.map(Number);
    const result = new Date(year, month - 1, day);
    // 31 February rolls over to March: refuse it instead.
    return result.getDate() === day ? result : null;
  }

  /** The local time `text` ("13:00") on `day`; null if it isn't a time. */
  function at(day: Date, text: string): Date | null {
    const match = /^(\d{1,2}):(\d{2})$/.exec(text);
    if (!match) return null;
    const [, hours, minutes] = match.map(Number);
    if (hours > 23 || minutes > 59) return null;
    return new Date(day.getFullYear(), day.getMonth(), day.getDate(), hours, minutes);
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const day = parseDate(date);
    const from = day && at(day, start);
    const to = day && at(day, end);
    if (!from || !to) {
      problem = t("sessionDialog.needTimes");
      return;
    }
    if (to <= from) {
      problem = t("sessionDialog.endAfterStart");
      return;
    }
    if (to.getTime() - from.getTime() > MAX_HOURS * 3600_000) {
      problem = t("sessionDialog.tooLong", { hours: MAX_HOURS });
      return;
    }
    if (from <= new Date()) {
      problem = t("sessionDialog.inPast");
      return;
    }
    saving = true;
    problem = await onSave(editing?.id ?? null, {
      courseId: courseId || null,
      title: title.trim() || null,
      profileId: profileId || null,
      start: from.toISOString(),
      end: to.toISOString(),
      weekly: !editing && weekly,
    });
    saving = false;
    if (problem === null) close();
  }

  function remove() {
    if (!editing) return;
    const session = editing;
    close();
    onDelete(session);
  }

  function removeFollowing() {
    if (!editing) return;
    const session = editing;
    close();
    onDeleteFollowing(session);
  }

  // A click on the dim backdrop (outside the card) closes the dialog.
  function onBackdrop(event: MouseEvent) {
    if (event.target === dialog) close();
  }
</script>

<dialog bind:this={dialog} aria-labelledby="session-heading" onclick={onBackdrop}>
  <!-- novalidate: our own message under the fields, not the browser's bubble. -->
  <form class="card" novalidate onsubmit={submit}>
    <h2 id="session-heading">{t(editing ? "sessionDialog.edit" : "sessionDialog.new")}</h2>

    <label class="field">
      <span class="label">{t("sessionDialog.what")}</span>
      <input bind:this={titleInput} bind:value={title} placeholder={t("sessionDialog.whatPlaceholder")} />
    </label>

    <label class="field">
      <span class="label">{t("sessionDialog.course")}</span>
      <select bind:value={courseId}>
        <option value="">{t("activity.noCourse")}</option>
        {#each courses as course (course.id)}
          <option value={course.id}>{course.name}</option>
        {/each}
      </select>
    </label>

    <label class="field">
      <span class="label">{t("sessionDialog.profile")}</span>
      <select bind:value={profileId}>
        <option value="">{t("sessionDialog.noProfile")}</option>
        {#each profiles as profile (profile.id)}
          <option value={profile.id}>
            {profile.name} · {[...profile.apps.map((a) => a.name), ...profile.sites.map((s) => s.domain)].join(", ") ||
              t("sessionDialog.emptyProfile")}
          </option>
        {/each}
      </select>
      {#if profiles.length === 0}
        <span class="hint">{t("sessionDialog.noProfiles")} <a href="/settings#profiles">{t("sessionDialog.makeProfile")}</a></span>
      {/if}
    </label>

    <div class="when">
      <label class="field">
        <span class="label">{t("sessionDialog.date")}</span>
        <input type="date" bind:value={date} />
      </label>
      <label class="field time">
        <span class="label">{t("sessionDialog.from")}</span>
        <input type="time" bind:value={start} />
      </label>
      <label class="field time">
        <span class="label">{t("sessionDialog.to")}</span>
        <input type="time" bind:value={end} />
      </label>
    </div>

    {#if editing}
      {#if editing.seriesId}
        <p class="hint">{t("sessionDialog.partOfRepeat")}</p>
      {/if}
    {:else}
      <label class="check">
        <input type="checkbox" bind:checked={weekly} />
        <span>{t("sessionDialog.weekly", { date: repeatUntil })}</span>
      </label>
    {/if}

    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

    <div class="buttons">
      {#if editing}
        <button type="button" class="btn btn-danger" onclick={remove}>{t("sessionDialog.delete")}</button>
        {#if editing.seriesId}
          <button type="button" class="btn btn-danger" onclick={removeFollowing}>
            {t("sessionDialog.deleteFollowing")}
          </button>
        {/if}
      {/if}
      <span class="spacer"></span>
      <button type="button" class="btn" onclick={close}>{t("sessionDialog.cancel")}</button>
      <button type="submit" class="btn btn-primary" disabled={saving}>
        {t(editing ? "sessionDialog.save" : "sessionDialog.plan")}
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
    width: 460px;
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
    flex: 0 0 100px;
  }

  input:not([type="checkbox"]),
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

  .check {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-md);
    cursor: pointer;
  }

  .check input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--control);
  }

  .hint {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .problem {
    font-size: var(--text-sm);
    color: var(--status-behind-text);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
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
