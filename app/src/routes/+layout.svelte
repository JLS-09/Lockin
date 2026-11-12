<script lang="ts">
  // The frame around every page: global styles, fonts and the sidebar.
  // It also opens the page the tray asks for ("Emergency stop…" → Session) and
  // keeps the language in step with the one saved for the daemon.
  import "@fontsource-variable/inter";
  import "$lib/styles/tokens.css";
  import "$lib/styles/base.css";

  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { i18n } from "$lib/i18n/index.svelte";
  import { live } from "$lib/session/live.svelte";

  let { children } = $props();

  // Follow the running session and the daemon on every page (sidebar, week, session).
  $effect(() => live.follow());

  onMount(() => {
    // A language chosen in Settings is saved for the daemon too; that one wins.
    // A choice from before it was saved there (only in this webview) is copied over.
    api
      .settings()
      .then((saved) => {
        const language = saved["language"];
        if (language === "en" || language === "nl") {
          if (language !== i18n.locale) i18n.setLocale(language);
        } else if (i18n.chosen) {
          api.setSetting("language", i18n.locale);
        }
      })
      .catch(() => {});

    // The tray started the app on a page, or asks the open app for one.
    api
      .startupPage()
      .then((page) => {
        if (page) goto(`/${page}`);
      })
      .catch(() => {});
    const stopListening = listen<string>("open-page", (event) => goto(`/${event.payload}`));
    return () => {
      stopListening.then((stop) => stop());
    };
  });

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
    padding: var(--space-5) 28px;
  }
</style>
