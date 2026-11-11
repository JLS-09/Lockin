<script lang="ts">
  // Left column: logo, navigation and the language switch.
  import { page } from "$app/state";
  // The full logo, with "Lockin" already drawn as outlines (no font needed).
  // It uses currentColor, so its colour comes from the CSS `color` below.
  import logo from "$lib/assets/lockin-logo.svg?raw";
  import { i18n, locales, t } from "$lib/i18n/index.svelte";

  // Icons are 24×24 stroke paths, drawn by the <svg> below.
  const links = [
    { href: "/", label: "nav.week", icon: "M4 6h16v14H4zM4 10h16M9 3v4M15 3v4" },
    {
      href: "/semester",
      label: "nav.semester",
      icon: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z",
    },
  ] as const;
</script>

<nav aria-label={t("nav.main")}>
  <div class="logo" role="img" aria-label={t("app.name")}>
    <span aria-hidden="true">{@html logo}</span>
  </div>

  <ul>
    {#each links as link (link.href)}
      {@const active = page.url.pathname === link.href}
      <li>
        <a href={link.href} class:active aria-current={active ? "page" : undefined}>
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d={link.icon} />
          </svg>
          {t(link.label)}
        </a>
      </li>
    {/each}
  </ul>

  <div class="spacer"></div>

  <div class="language" role="group" aria-label={t("language.label")}>
    {#each locales as locale (locale)}
      <button
        type="button"
        class:active={i18n.locale === locale}
        aria-pressed={i18n.locale === locale}
        onclick={() => i18n.setLocale(locale)}
      >
        {locale.toUpperCase()}
      </button>
    {/each}
  </div>
</nav>

<style>
  nav {
    height: 100%;
    padding: var(--space-5) var(--space-3);
    background: var(--surface-1);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }

  .logo {
    padding: 0 var(--space-2);
    color: var(--text);
  }

  .logo :global(svg) {
    display: block;
    height: 28px;
    width: auto;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  a {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
    padding: 0 var(--space-3);
    border-radius: var(--radius);
    color: var(--text-muted);
    text-decoration: none;
    font-weight: var(--weight-medium);
    transition:
      background-color var(--duration-micro) var(--ease-out),
      color var(--duration-micro) var(--ease-out);
  }

  a:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  a.active {
    background: var(--surface-active);
    color: var(--text);
  }

  .spacer {
    flex: 1;
  }

  .language {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius);
    background: var(--surface-2);
  }

  .language button {
    flex: 1;
    min-height: 30px;
    border: 0;
    border-radius: calc(var(--radius) - 2px);
    background: transparent;
    color: var(--text-muted);
    font-size: var(--text-sm);
    font-weight: var(--weight-bold);
    cursor: pointer;
    transition: background-color var(--duration-micro) var(--ease-out);
  }

  .language button:hover {
    color: var(--text);
  }

  .language button.active {
    background: var(--surface-active);
    color: var(--text);
  }
</style>
