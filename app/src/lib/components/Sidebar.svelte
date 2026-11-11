<script lang="ts">
  // Left column from the mockup: logo and navigation, each item with its key.
  // Pressing the key (outside a text field) opens that page.
  // The study session card and the points card at the bottom follow in phases 4 and 6.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  // The full logo, with "Lockin" already drawn as outlines (no font needed).
  // It uses currentColor, so its colour comes from the CSS `color` below.
  import logo from "$lib/assets/lockin-logo.svg?raw";
  import { t } from "$lib/i18n/index.svelte";

  // Icons are 24×24 stroke paths from the mockup, drawn by the <svg> below.
  const links = [
    { href: "/", label: "nav.week", key: "W", icon: "M4 6h16v14H4zM4 10h16M9 3v4M15 3v4" },
    { href: "/semester", label: "nav.semester", key: "M", icon: "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z" },
    { href: "/urgent", label: "nav.urgent", key: "U", icon: "M12 3l9 16H3zM12 10v4M12 17v.5" },
    { href: "/session", label: "nav.session", key: "S", icon: "M12 8v5l3 2M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM9 2h6" },
    { href: "/stats", label: "nav.stats", key: "T", icon: "M5 20V10M12 20V4M19 20v-7" },
    { href: "/shop", label: "nav.shop", key: "P", icon: "M5 8h14l-1 12H6zM9 8V6a3 3 0 0 1 6 0v2" },
    {
      href: "/settings",
      label: "nav.settings",
      key: ",",
      icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19 12h2M3 12h2M12 3v2M12 19v2M17 7l1.5-1.5M5.5 18.5L7 17M17 17l1.5 1.5M5.5 5.5L7 7",
    },
  ] as const;

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable]")) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const link = links.find((l) => l.key === event.key.toUpperCase());
    if (!link) return;
    event.preventDefault();
    goto(link.href);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<nav aria-label={t("nav.main")}>
  <div class="logo" role="img" aria-label={t("app.name")}>
    <span aria-hidden="true">{@html logo}</span>
  </div>

  <div class="links">
    {#each links as link (link.href)}
      {@const active = page.url.pathname === link.href}
      <a href={link.href} class="nav-link" class:active aria-current={active ? "page" : undefined}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d={link.icon} />
        </svg>
        <span class="label">{t(link.label)}</span>
        <span class="key">{link.key}</span>
      </a>
    {/each}
  </div>
</nav>

<style>
  nav {
    height: 100%;
    padding: 20px 14px;
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
    height: 27px;
    width: auto;
  }

  .links {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
    padding: 0 var(--space-3);
    border-radius: var(--radius);
    color: var(--text-muted);
    text-decoration: none;
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    transition:
      background-color var(--duration-micro) var(--ease-out),
      color var(--duration-micro) var(--ease-out);
  }

  .nav-link:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .nav-link.active {
    background: var(--surface-active);
    color: var(--text);
  }

  .nav-link.active svg {
    color: var(--control);
  }

  .label {
    flex: 1;
  }

  .key {
    font-size: var(--text-sm);
    color: var(--text-faint);
  }
</style>
