<script lang="ts">
  // Placeholder: the semester overview is built in step 6.
  import { t } from "$lib/i18n/index.svelte";
</script>

<h1>{t("nav.semester")}</h1>
<p>{t("semester.comingSoon")}</p>

<style>
  h1 {
    font-size: var(--text-lg);
    font-weight: 700;
    margin-bottom: var(--space-3);
  }

  p {
    color: var(--text-muted);
  }
</style>
