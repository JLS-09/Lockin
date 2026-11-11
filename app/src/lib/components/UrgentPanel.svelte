<script lang="ts">
  // The Urgent panel of the week view mockup: the three most urgent lines,
  // "Besturingssystemen · open 8 days" or "Databanken · due Fri 23:59 · Ufora",
  // and "See all". A line opens its week: an activity with its class selected,
  // a deadline the week it's due.
  // The Deadlines panel is the same panel with only deadlines (not in the
  // mockups): old activities can fill the Urgent panel and hide them. There the
  // status icon is a button that checks the deadline off (`onToggle`); a checked
  // line stays, as "Done just now", until the panel reloads.
  import type { Course, UrgentItem } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";
  import { urgentIcon, weekHref, whenText } from "$lib/urgent/when";
  import StatusIcon from "./StatusIcon.svelte";

  /** How many lines the panel shows, as in the mockup. */
  const SHOWN = 3;

  let {
    items,
    courses,
    now,
    title,
    seeAllHref = "/urgent",
    emptyText,
    headingId = "urgent-panel-heading",
    onToggle,
    isDone = () => false,
  }: {
    /** The lines in the order to show them; the first three are shown. */
    items: UrgentItem[];
    courses: Map<string, Course>;
    now: Date;
    /** "Urgent" when left out (in the current language). */
    title?: string;
    /** Where "See all" leads. */
    seeAllHref?: string;
    /** Shown when there are no lines; "Nothing urgent." when left out. */
    emptyText?: string;
    /** Unique per panel on the page, for the heading's id. */
    headingId?: string;
    /** Makes the status icon a check-off button. */
    onToggle?: (item: UrgentItem) => void;
    /** Checked off during this visit. */
    isDone?: (item: UrgentItem) => boolean;
  } = $props();

  const statusLabels = {
    behind: "status.behind",
    approaching: "urgent.approaching",
    planned: "status.planned",
  } as const;

  /** "Besturingssystemen · open 8 days · Ufora": "when" starts lowercase inside the line. */
  function detail(item: UrgentItem): string {
    const when = whenText(item, now);
    const parts = [
      item.courseId ? courses.get(item.courseId)?.name : undefined,
      when.charAt(0).toLocaleLowerCase() + when.slice(1),
      item.sourceId ? t("urgent.kindUfora") : undefined,
    ];
    return parts.filter(Boolean).join(" · ");
  }
</script>

<section class="panel urgent" aria-labelledby={headingId}>
  <div class="panel-head">
    <h2 class="panel-title" id={headingId}>{title ?? t("urgent.listTitle")}</h2>
    <a class="see-all" href={seeAllHref}>{t("urgent.seeAll")}</a>
  </div>

  {#if items.length === 0}
    <p class="panel-empty">{emptyText ?? t("urgent.nothing")}</p>
  {:else}
    {#each items.slice(0, SHOWN) as item (`${item.kind}:${item.id}`)}
      {@const done = isDone(item)}
      {@const text = done ? t("urgent.doneNow") : detail(item)}
      {#if onToggle}
        <div class="line">
          <button
            class="check"
            aria-pressed={done}
            aria-label={t(done ? "urgent.markNotDone" : "urgent.markDone", { title: item.title })}
            title={t(done ? "urgent.markNotDone" : "urgent.markDone", { title: item.title })}
            onclick={() => onToggle(item)}
          >
            {#if done}
              <StatusIcon status="done" />
            {:else}
              <StatusIcon status={urgentIcon(item.status)} label={t(statusLabels[item.status])} />
            {/if}
          </button>
          <a class="text" href={weekHref(item)}>
            <span class="title" class:done title={item.title}>{item.title}</span>
            <span class="detail" class:done title={text}>{text}</span>
          </a>
        </div>
      {:else}
        <a class="line" href={weekHref(item)}>
          <span class="icon">
            <StatusIcon status={urgentIcon(item.status)} label={t(statusLabels[item.status])} />
          </span>
          <span class="text">
            <span class="title" title={item.title}>{item.title}</span>
            <span class="detail" title={text}>{text}</span>
          </span>
        </a>
      {/if}
    {/each}
  {/if}
</section>

<style>
  /* The mockup's Urgent panel has a little less padding than the others. */
  .urgent {
    padding: var(--space-4) 6px 6px;
  }

  .see-all {
    font-size: var(--text-sm);
    color: var(--text-muted);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .see-all:hover {
    color: var(--control-hover);
  }

  .line {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px;
    border-radius: var(--radius);
    color: var(--text);
    text-decoration: none;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .line:hover {
    background: var(--surface-hover);
  }

  .icon {
    flex: none;
    display: flex;
    margin-top: 2px;
  }

  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text);
    text-decoration: none;
  }

  /* The check-off button: the icon stays where it is in the Urgent panel, the
     button around it gives a bigger target. */
  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin: -4px -6px -4px -6px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
    transition:
      background-color var(--duration-micro) var(--ease-out),
      transform var(--duration-micro) var(--ease-out);
  }

  .check:hover {
    background: var(--surface-active);
  }

  /* Feedback first: the icon pops, saving happens after. */
  .check:active {
    transform: scale(0.88);
  }

  .title.done {
    color: var(--text-done);
  }

  .detail.done {
    color: var(--status-done-text);
  }

  .title,
  .detail {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-size: var(--text-md);
  }

  .detail {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }
</style>
