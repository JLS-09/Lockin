<script lang="ts">
  // Settings. Not in the mockups yet; for now only the language.
  import { i18n, locales, t, type Locale } from "$lib/i18n/index.svelte";

  // Each language in its own language, so you can always find yours.
  const names: Record<Locale, string> = { en: "English", nl: "Nederlands" };
</script>

<h1>{t("nav.settings")}</h1>

<section class="panel" aria-labelledby="language-heading">
  <h2 id="language-heading">{t("language.label")}</h2>
  <p class="hint">{t("language.hint")}</p>
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

<style>
  h1 {
    font-size: var(--text-lg);
    font-weight: var(--weight-heavy);
    letter-spacing: -0.01em;
    margin-bottom: var(--space-4);
  }

  .panel {
    max-width: 560px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--space-4) 20px 20px;
  }

  h2 {
    font-size: var(--text-title);
    font-weight: var(--weight-bold);
  }

  .hint {
    margin: var(--space-1) 0 var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .choice {
    display: flex;
    gap: var(--space-2);
  }
</style>
