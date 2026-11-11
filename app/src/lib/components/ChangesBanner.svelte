<script lang="ts">
  // "TimeEdit changed 2 blocks: Lineaire algebra WPO moved Fri 13:00 → Wed 16:00
  // (its 2 activities moved with it), and Lineaire algebra HOC on Fri is cancelled."
  // Shows the moved and cancelled blocks of the week you haven't dismissed yet.
  import type { Activity, ClassBlock, Course } from "$lib/api";
  import { formatDate, formatTime, i18n, t, tn } from "$lib/i18n/index.svelte";
  import { kindLabel } from "$lib/week/labels";

  let {
    blocks,
    activities,
    courses,
    onDismiss,
  }: {
    /** The changed blocks to tell about. */
    blocks: ClassBlock[];
    activities: Activity[];
    courses: Map<string, Course>;
    onDismiss: () => void;
  } = $props();

  /** "Fri 13:00" */
  function dayAndTime(text: string): string {
    const date = new Date(text);
    return `${formatDate(date, { weekday: "short" })} ${formatTime(date)}`;
  }

  /** One sentence part per block: the name in bold, then what happened. */
  const items = $derived(
    blocks.map((block) => {
      const course = block.courseId ? courses.get(block.courseId)?.name : undefined;
      const kind = kindLabel(block.kind);
      const name = [course ?? block.title, kind].filter(Boolean).join(" ");

      let rest: string;
      if (block.status === "cancelled") {
        rest = t("banner.cancelled", { day: formatDate(new Date(block.startAt), { weekday: "short" }) });
      } else {
        const to = dayAndTime(block.startAt);
        rest = block.movedFrom
          ? t("banner.moved", { from: dayAndTime(block.movedFrom), to })
          : t("banner.movedTo", { to });
        const count = activities.filter((a) => a.blockId === block.id).length;
        if (count > 0) rest += tn("banner.activitiesMoved", count);
      }
      return { name, rest };
    }),
  );

  // "A, B and C" / "A, B en C": Intl puts the commas and the last "and" right.
  const parts = $derived(
    new Intl.ListFormat(i18n.intlTag, { type: "conjunction" }).formatToParts(
      items.map((_, i) => String(i)),
    ),
  );
</script>

<div class="banner" role="status">
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--control)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h13l-3-3M20 17H7l3 3" /></svg>
  <p>
    {tn("banner.intro", blocks.length)}
    {#each parts as part, i (i)}
      {#if part.type === "element"}
        {@const item = items[Number(part.value)]}
        <strong>{item.name}</strong>{item.rest}
      {:else}{part.value}{/if}
    {/each}.
  </p>
  <button class="btn" onclick={onDismiss}>{t("banner.gotIt")}</button>
</div>

<style>
  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) 14px;
    border-radius: var(--radius-md);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
  }

  p {
    flex: 1 1 320px;
    font-size: var(--text-md);
  }

  strong {
    font-weight: var(--weight-bold);
  }
</style>
