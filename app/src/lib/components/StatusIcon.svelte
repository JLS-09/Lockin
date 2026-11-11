<script lang="ts">
  // The status icon: every status has its own shape, so it stays readable
  // without color (color-blind users, any theme). Shapes from the UI mockups.
  import { t } from "$lib/i18n/index.svelte";
  import type { Status } from "$lib/week/status";

  let { status, size = 16 }: { status: Status; size?: number } = $props();

  const labels = {
    done: "status.done",
    partial: "status.partial",
    behind: "status.behind",
    planned: "status.planned",
  } as const;
</script>

<svg
  class={status}
  width={size}
  height={size}
  viewBox="0 0 16 16"
  role="img"
  aria-label={t(labels[status])}
>
  {#if status === "done"}
    <circle cx="8" cy="8" r="7" fill="currentColor" />
    <path d="M5 8.2l2 2 4-4.4" stroke="var(--bg)" stroke-width="1.8" fill="none" stroke-linecap="round" stroke-linejoin="round" />
  {:else if status === "behind"}
    <circle cx="8" cy="8" r="7" fill="currentColor" />
    <path d="M8 4.5v4.2M8 11.3v.2" stroke="var(--bg)" stroke-width="2" fill="none" stroke-linecap="round" />
  {:else if status === "partial"}
    <circle cx="8" cy="8" r="6.25" stroke="currentColor" stroke-width="1.5" fill="none" />
    <path d="M8 1.75a6.25 6.25 0 0 1 0 12.5z" fill="currentColor" />
  {:else}
    <circle cx="8" cy="8" r="6.25" stroke="currentColor" stroke-width="1.5" fill="none" />
  {/if}
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }

  .done {
    color: var(--status-done);
  }

  .behind {
    color: var(--status-behind);
  }

  .partial {
    color: var(--status-partial);
  }

  .planned {
    color: var(--status-planned);
  }
</style>
