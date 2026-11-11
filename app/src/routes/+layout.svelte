<script lang="ts">
  // The frame around every page: global styles, fonts and the sidebar.
  import "@fontsource-variable/inter";
  import "$lib/styles/tokens.css";
  import "$lib/styles/base.css";

  import Sidebar from "$lib/components/Sidebar.svelte";
  import { i18n } from "$lib/i18n/index.svelte";

  let { children } = $props();

  // Keep <html lang> in step with the chosen language (screen readers, hyphenation).
  $effect(() => {
    document.documentElement.lang = i18n.locale;
  });
</script>

<div class="app">
  <Sidebar />
  <main>
    {@render children()}
  </main>
</div>

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
  }

  main {
    min-width: 0;
    overflow-y: auto;
    padding: var(--space-5) var(--space-6);
  }
</style>
