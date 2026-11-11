<script lang="ts">
  // The Urgent panel of the week view mockup: the three most urgent lines,
  // "Besturingssystemen · open 8 days" or "Databanken · due Fri 23:59 · Ufora",
  // and "See all". A line opens its week: an activity with its class selected,
  // a deadline the week it's due.
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
  }: {
    /** The Urgent list, most urgent first. */
    items: UrgentItem[];
    courses: Map<string, Course>;
    now: Date;
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

<section class="panel urgent" aria-labelledby="urgent-panel-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="urgent-panel-heading">{t("urgent.listTitle")}</h2>
    <a class="see-all" href="/urgent">{t("urgent.seeAll")}</a>
  </div>

  {#if items.length === 0}
    <p class="panel-empty">{t("urgent.nothing")}</p>
  {:else}
    {#each items.slice(0, SHOWN) as item (`${item.kind}:${item.id}`)}
      {@const text = detail(item)}
      <a class="line" href={weekHref(item)}>
        <span class="icon">
          <StatusIcon status={urgentIcon(item.status)} label={t(statusLabels[item.status])} />
        </span>
        <span class="text">
          <span class="title" title={item.title}>{item.title}</span>
          <span class="detail" title={text}>{text}</span>
        </span>
      </a>
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
