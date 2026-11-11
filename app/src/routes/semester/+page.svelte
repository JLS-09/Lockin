<script lang="ts">
  // The semester overview, following the semester mockup: a legend, the grid of
  // weeks × courses, and under it the activities of the selected cell.
  // The reward card beside it (a theme for a fully green semester) follows in phase 6.
  import { goto } from "$app/navigation";
  import {
    api,
    type Activity,
    type Course,
    type WeekClasses,
    type WeekProgress,
  } from "$lib/api";
  import SemesterGrid, { type Row } from "$lib/components/SemesterGrid.svelte";
  import StatusIcon from "$lib/components/StatusIcon.svelte";
  import { addDays, formatRange, isoDate, parseIsoDate, t } from "$lib/i18n/index.svelte";
  import { kindLabel } from "$lib/week/labels";
  import {
    cellStatus,
    currentSemester,
    donePercent,
    findSemesters,
    type CellStatus,
  } from "$lib/week/semester";

  let courses = $state<Course[]>([]);
  let classWeeks = $state<WeekClasses[]>([]);
  let progress = $state<WeekProgress[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);
  const now = new Date();

  let semesterKey = $state<string | null>(null);
  let selected = $state<{ row: number; week: number } | null>(null);
  // The selected cell's activities, loaded when it changes, and the kind of
  // class each belongs to ("HOC"), so two "Review the class" can be told apart.
  let cellActivities = $state<Activity[]>([]);
  let kindByBlock = $state(new Map<string, string | null>());

  const semesters = $derived(findSemesters(classWeeks));
  const semester = $derived(
    semesters.find((s) => s.key === semesterKey) ?? currentSemester(semesters, now),
  );
  const weeks = $derived(semester?.weeks ?? []);
  const currentIndex = $derived(weeks.findIndex((w) => w === mondayOf(now)));

  // A row per course with classes or activities this semester, by name.
  const rows = $derived.by((): Row[] => {
    const inSemester = new Set(weeks);
    const classes = classWeeks.filter((c) => inSemester.has(c.week));
    const done = progress.filter((p) => inSemester.has(p.week));
    const active = new Set([...classes, ...done].map((x) => x.courseId));
    return courses
      .filter((course) => active.has(course.id))
      .map((course) => {
        const ownClasses = classes.filter((c) => c.courseId === course.id);
        const ownProgress = done.filter((p) => p.courseId === course.id);
        return {
          course,
          cells: weeks.map((week) =>
            cellStatus(
              ownClasses.find((c) => c.week === week)?.classes ?? 0,
              ownProgress.find((p) => p.week === week),
              week,
              now,
            ),
          ),
          percent: donePercent(ownProgress, weeks, now),
        };
      });
  });

  const selectedRow = $derived(selected ? rows[selected.row] : undefined);
  const selectedWeek = $derived(selected ? weeks[selected.week] : undefined);
  const selectedStatus = $derived(
    selected && selectedRow ? selectedRow.cells[selected.week] : undefined,
  );

  async function load() {
    try {
      [courses, classWeeks, progress] = await Promise.all([
        api.courses(),
        api.classWeeks(),
        api.progress(),
      ]);
      loaded = true;
    } catch (e) {
      error = String(e);
    }
  }
  load();

  // Start on this week of the first course, like the mockup starts on a cell.
  $effect(() => {
    if (selected || rows.length === 0) return;
    selected = { row: 0, week: currentIndex >= 0 ? currentIndex : weeks.length - 1 };
  });

  // Load the activities of the selected cell's course and week.
  $effect(() => {
    const course = selectedRow?.course.id;
    const week = selectedWeek;
    if (!course || !week) return;
    api
      .week(week)
      .then((result) => {
        cellActivities = result.activities.filter((a) => a.courseId === course);
        kindByBlock = new Map(result.blocks.map((b) => [b.id, kindLabel(b.kind)]));
      })
      .catch((e) => (error = String(e)));
  });

  /** "2026-11-09" for any day of that week. */
  function mondayOf(date: Date): string {
    return isoDate(addDays(date, -((date.getDay() + 6) % 7)));
  }

  function changeSemester(key: string) {
    semesterKey = key;
    selected = null; // the effect above picks a cell again
  }

  function openWeek(week: number) {
    goto(`/?date=${weeks[week]}`);
  }

  const statusNames: Record<CellStatus, string> = $derived({
    done: t("status.done"),
    partial: t("status.partial"),
    behind: t("semester.notDone"),
    planned: t("status.planned"),
    none: t("semester.noClasses"),
    empty: t("semester.noActivities"),
  });

  /** "Done" / "Open" / "Planned" for one activity, colored like its cell. */
  function activityState(activity: Activity): { text: string; tone: string } {
    if (activity.done) return { text: t("semester.actDone"), tone: "done" };
    if (selectedStatus === "planned") return { text: t("status.planned"), tone: "planned" };
    return { text: t("semester.actOpen"), tone: selectedStatus === "behind" ? "behind" : "partial" };
  }

  function semesterName(number: number, startYear: number): string {
    return `${t("semester.title", { number })} · ${startYear}–${startYear + 1}`;
  }
</script>

<div class="page">
  <header>
    <div class="title">
      <h1>{semester ? t("semester.title", { number: semester.number }) : t("nav.semester")}</h1>
      {#if semester}
        <span class="sub">
          {currentIndex >= 0
            ? t("semester.subtitle", {
                year: `${semester.startYear}–${semester.startYear + 1}`,
                current: currentIndex + 1,
                total: weeks.length,
              })
            : t("semester.subtitleWeeks", {
                year: `${semester.startYear}–${semester.startYear + 1}`,
                total: weeks.length,
              })}
        </span>
      {/if}
    </div>
    {#if semesters.length > 0 && semester}
      <select
        class="btn picker"
        aria-label={t("semester.pick")}
        value={semester.key}
        onchange={(e) => changeSemester(e.currentTarget.value)}
      >
        {#each semesters as s (s.key)}
          <option value={s.key}>{semesterName(s.number, s.startYear)}</option>
        {/each}
      </select>
    {/if}
  </header>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if loaded && semesters.length === 0}
    <p class="muted">{t("semester.empty")}</p>
  {:else if semester}
    <div class="legend">
      <span><StatusIcon status="done" />{t("status.done")}</span>
      <span><StatusIcon status="partial" />{t("status.partial")}</span>
      <span><StatusIcon status="behind" />{t("semester.notDone")}</span>
      <span><StatusIcon status="planned" />{t("status.planned")}</span>
      <span><span class="none"></span>{t("semester.noClasses")}</span>
    </div>

    <section class="card table" aria-label={t("semester.gridLabel")}>
      <SemesterGrid
        {weeks}
        {currentIndex}
        {rows}
        {selected}
        onSelect={(row, week) => (selected = { row, week })}
        onOpen={openWeek}
      />
    </section>

    {#if selected && selectedRow && selectedWeek && selectedStatus}
      {@const monday = parseIsoDate(selectedWeek)}
      <section class="card detail" aria-live="polite">
        <span class="when">
          {t("semester.when", { number: selected.week + 1, range: formatRange(monday, addDays(monday, 4)) })}
        </span>
        <div class="detail-head">
          <h2>
            <span class="dot" style:background={selectedRow.course.color}></span>
            {selectedRow.course.name} · {statusNames[selectedStatus]}
          </h2>
          <button class="btn btn-small" title="Enter" onclick={() => openWeek(selected!.week)}>
            {t("semester.openWeek")}
          </button>
        </div>
        {#if cellActivities.length === 0}
          <p class="muted">
            {selectedStatus === "none" ? t("semester.noClassesText") : t("semester.noActivitiesText")}
          </p>
        {:else}
          <ul>
            {#each cellActivities as activity (activity.id)}
              {@const state = activityState(activity)}
              {@const kind = activity.blockId ? kindByBlock.get(activity.blockId) : null}
              <li>
                <span class="ellipsis" title={activity.title}>
                  {activity.title}{#if kind}<span class="kind">{kind}</span>{/if}
                </span>
                <span class="state {state.tone}">{state.text}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
  }

  .title {
    flex: 1 1 auto;
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
  }

  .sub,
  .muted {
    font-size: var(--text-md);
    color: var(--text-muted);
  }

  /* The mockup's "Semester 1 ⌄" button, as a real dropdown. */
  .picker {
    appearance: none;
    padding-right: 34px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%23e4e6eb' stroke-width='2' stroke-linecap='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
  }

  .picker option {
    background: var(--surface-2);
    color: var(--text);
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4) 20px;
    font-size: var(--text-base);
    color: var(--text-muted);
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .none {
    width: 14px;
    height: 2px;
    background: var(--text-none);
  }

  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }

  .table {
    padding: var(--space-3);
    overflow-x: auto;
  }

  .detail {
    max-width: 720px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .when {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--text-md);
  }

  li {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .ellipsis {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .kind {
    margin-left: var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-faint);
  }

  .state {
    flex: none;
  }

  .state.done {
    color: var(--status-done-text);
  }

  .state.partial {
    color: var(--status-partial);
  }

  .state.behind {
    color: var(--text-cancelled);
  }

  .state.planned {
    color: var(--text-muted);
  }

  .btn-small {
    flex: none;
    min-height: 30px;
    padding: 0 12px;
    font-size: var(--text-sm);
  }

  .error {
    color: var(--status-behind);
  }
</style>
