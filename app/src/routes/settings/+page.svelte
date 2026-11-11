<script lang="ts">
  // Settings. Not in the mockups; built in the style of the panels.
  // - Language
  // - Courses: the activity templates per course ("After every WPO: Exercises").
  //   The class panel links here with ?course=<id> to open that course.
  import { tick } from "svelte";
  import { page } from "$app/state";
  import { api, type ActivityTemplate, type Course, type CourseKind } from "$lib/api";
  import CourseTemplates from "$lib/components/CourseTemplates.svelte";
  import { i18n, locales, t, type Locale } from "$lib/i18n/index.svelte";

  // Each language in its own language, so you can always find yours.
  const names: Record<Locale, string> = { en: "English", nl: "Nederlands" };

  let courses = $state<Course[]>([]);
  let kinds = $state<CourseKind[]>([]);
  let templates = $state<ActivityTemplate[]>([]);
  let error = $state<string | null>(null);
  let loaded = $state(false);

  const wanted = $derived(page.url.searchParams.get("course"));

  async function load() {
    try {
      [courses, kinds, templates] = await Promise.all([
        api.courses(),
        api.courseKinds(),
        api.templates(),
      ]);
      loaded = true;
    } catch (e) {
      error = String(e);
    }
  }

  // Load, then scroll to the course the class panel asked for.
  load().then(async () => {
    if (!wanted) return;
    await tick();
    document.getElementById(`course-${wanted}`)?.scrollIntoView({ block: "start" });
  });

  async function addTemplate(courseId: string, kind: string | null, title: string) {
    try {
      const created = await api.addTemplate(courseId, kind, title);
      templates = await api.templates();
      return created;
    } catch (e) {
      error = String(e);
      return 0;
    }
  }

  async function removeTemplate(template: ActivityTemplate) {
    // Gone from the list at once; back if removing fails.
    templates = templates.filter((x) => x.id !== template.id);
    try {
      return await api.removeTemplate(template.id);
    } catch (e) {
      templates = await api.templates();
      error = String(e);
      return 0;
    }
  }
</script>

<div class="page">
  <h1>{t("nav.settings")}</h1>

  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <section class="panel" aria-labelledby="language-heading">
    <div class="panel-head">
      <h2 class="panel-title" id="language-heading">{t("language.label")}</h2>
    </div>
    <p class="intro">{t("language.hint")}</p>
    <div class="choice" role="group" aria-labelledby="language-heading">
      {#each locales as locale (locale)}
        <button
          class="btn"
          class:btn-primary={i18n.locale === locale}
          aria-pressed={i18n.locale === locale}
          onclick={() => i18n.setLocale(locale)}
        >
          {names[locale]}
        </button>
      {/each}
    </div>
  </section>

  <section aria-labelledby="courses-heading" class="courses">
    <h2 class="section-title" id="courses-heading">{t("settings.courses")}</h2>
    <p class="intro">{t("template.intro")}</p>
    {#if loaded && courses.length === 0}
      <p class="intro">{t("settings.noCourses")}</p>
    {/if}
    {#each courses as course (course.id)}
      <CourseTemplates
        {course}
        kinds={kinds.filter((k) => k.courseId === course.id).map((k) => k.kind)}
        templates={templates.filter((x) => x.courseId === course.id)}
        highlighted={course.id === wanted}
        onAdd={(kind, title) => addTemplate(course.id, kind, title)}
        onRemove={removeTemplate}
      />
    {/each}
  </section>
</div>

<style>
  .page {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
  }

  .section-title {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
    margin-top: var(--space-3);
  }

  .intro {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .panel .intro {
    padding: 0 10px var(--space-3);
  }

  .choice {
    display: flex;
    gap: var(--space-2);
    padding: 0 10px var(--space-2);
  }

  .courses {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .error {
    color: var(--status-behind);
  }
</style>
