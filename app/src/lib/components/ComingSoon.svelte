<script lang="ts">
  // Placeholder for a page from the mockups that a later phase builds.
  import { t } from "$lib/i18n/index.svelte";

  let { title, phase }: { title: string; phase: number } = $props();
</script>

<h1>{title}</h1>
<p>{t("comingSoon", { phase })}</p>

<style>
  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
    margin-bottom: var(--space-3);
  }

  p {
    color: var(--text-muted);
  }
</style>
