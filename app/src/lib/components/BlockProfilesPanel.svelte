<script lang="ts">
  // Settings → Block profiles: your own lists of apps a study session closes and
  // websites it blocks, e.g. "Strict" and "Light". Lockin ships none: everyone's
  // distractions differ. Websites are typed ("youtube.com"); they're blocked
  // through /etc/hosts, which needs a one-time setup (tools/install-helper.sh).
  // Not in the mockups; built from the panels' pieces (the course template cards).
  //
  // Each profile shows its apps as chips. "Add app" opens a searchable list of
  // the apps in your menu; a program that isn't there (a game, a script) can be
  // added by its name. While the running session uses a profile, apps can be
  // added but not removed, and it can't be deleted. The same goes for websites.
  import { tick } from "svelte";
  import { api, type BlockProfile, type InstalledApp } from "$lib/api";
  import { t, tn } from "$lib/i18n/index.svelte";
  import { profileCount } from "$lib/session/labels";
  import { live } from "$lib/session/live.svelte";

  let profiles = $state<BlockProfile[]>([]);
  let installed = $state<InstalledApp[] | null>(null);
  let newName = $state("");
  let message = $state<{ text: string; error: boolean } | null>(null);

  /** The profile whose app picker is open, and what's typed in it. */
  let picking = $state<string | null>(null);
  let search = $state("");
  let searchInput = $state<HTMLInputElement>();
  /** The profile being renamed, and the new name. */
  let renaming = $state<string | null>(null);
  let renameText = $state("");
  /** The profile whose website field is open, and what's typed in it. */
  let addingSite = $state<string | null>(null);
  let siteText = $state("");
  let siteInput = $state<HTMLInputElement>();
  /** The profile asking "Delete?", with how many planned sessions use it. */
  let confirming = $state<{ id: string; sessions: number } | null>(null);

  async function load() {
    try {
      profiles = await api.blockProfiles();
    } catch (e) {
      message = { text: String(e), error: true };
    }
  }
  load();

  function fail(e: unknown) {
    message = { text: String(e), error: true };
  }

  async function addProfile(event: SubmitEvent) {
    event.preventDefault();
    const name = newName.trim();
    if (!name) return;
    try {
      const id = await api.addBlockProfile(name);
      newName = "";
      message = null;
      await load();
      openPicker(id);
    } catch (e) {
      fail(e);
    }
  }

  async function openPicker(profileId: string) {
    picking = profileId;
    search = "";
    if (installed === null) {
      try {
        installed = await api.installedApps();
      } catch (e) {
        installed = [];
        fail(e);
      }
    }
    await tick();
    searchInput?.focus();
  }

  function closePicker() {
    picking = null;
    search = "";
  }

  /** The menu apps matching the search, by name or program. */
  const matching = $derived.by(() => {
    const text = search.trim().toLowerCase();
    const list = installed ?? [];
    if (!text) return list;
    return list.filter(
      (app) =>
        app.name.toLowerCase().includes(text) ||
        app.id.toLowerCase().includes(text) ||
        app.execNames.some((n) => n.toLowerCase().includes(text)),
    );
  });

  /** A typed name that can be added as a program: one word, no folder. */
  const programName = $derived(/^[^\s/]+$/.test(search.trim()) ? search.trim() : "");

  function onList(profile: BlockProfile, app: InstalledApp): boolean {
    return profile.apps.some((a) => a.appId === app.id);
  }

  async function addApp(profile: BlockProfile, app: InstalledApp) {
    if (onList(profile, app)) return;
    try {
      await api.addBlockedApp(profile.id, app);
      message = null;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function addProgram(profile: BlockProfile) {
    if (!programName) return;
    try {
      await api.addBlockedProgram(profile.id, programName);
      search = "";
      message = null;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function removeApp(profile: BlockProfile, entryId: string) {
    // Gone at once; back if it can't be removed.
    profiles = profiles.map((p) =>
      p.id === profile.id ? { ...p, apps: p.apps.filter((a) => a.id !== entryId) } : p,
    );
    try {
      await api.removeBlockedApp(entryId);
    } catch (e) {
      fail(e);
    }
    await load();
  }

  async function openSiteInput(profileId: string) {
    addingSite = profileId;
    siteText = "";
    await tick();
    siteInput?.focus();
  }

  async function addSite(event: SubmitEvent, profile: BlockProfile) {
    event.preventDefault();
    if (!siteText.trim()) return;
    try {
      await api.addBlockedSite(profile.id, siteText);
      // Stay open for the next one.
      siteText = "";
      message = null;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function removeSite(profile: BlockProfile, entryId: string) {
    profiles = profiles.map((p) =>
      p.id === profile.id ? { ...p, sites: p.sites.filter((s) => s.id !== entryId) } : p,
    );
    try {
      await api.removeBlockedSite(entryId);
    } catch (e) {
      fail(e);
    }
    await load();
  }

  async function startRename(profile: BlockProfile) {
    renaming = profile.id;
    renameText = profile.name;
  }

  async function saveRename(profile: BlockProfile) {
    const name = renameText.trim();
    renaming = null;
    if (!name || name === profile.name) return;
    try {
      await api.renameBlockProfile(profile.id, name);
      message = null;
    } catch (e) {
      fail(e);
    }
    await load();
  }

  async function askRemove(profile: BlockProfile) {
    try {
      confirming = { id: profile.id, sessions: await api.sessionsUsingProfile(profile.id) };
    } catch (e) {
      fail(e);
    }
  }

  async function remove(profile: BlockProfile) {
    confirming = null;
    try {
      const freed = await api.removeBlockProfile(profile.id);
      message = {
        text:
          freed > 0
            ? `${t("profiles.removed", { name: profile.name })} ${tn("profiles.freed", freed)}`
            : t("profiles.removed", { name: profile.name }),
        error: false,
      };
    } catch (e) {
      fail(e);
    }
    await load();
  }

  function onPickerKey(event: KeyboardEvent, profile: BlockProfile) {
    if (event.key === "Escape") {
      event.preventDefault();
      closePicker();
    } else if (event.key === "Enter") {
      event.preventDefault();
      // Enter adds the only match, or else the typed program.
      if (matching.length === 1) addApp(profile, matching[0]);
      else if (matching.length === 0) addProgram(profile);
    }
  }
</script>

<section class="panel" id="profiles" aria-labelledby="profiles-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="profiles-heading">{t("profiles.title")}</h2>
  </div>
  <p class="intro">{t("profiles.hint")}</p>
  {#if live.daemon?.websitesProblem && profiles.some((p) => p.sites.length > 0)}
    <p class="setup" role="status" title={live.daemon.websitesProblem}>{t("profiles.websitesSetup")}</p>
  {/if}

  {#if profiles.length === 0}
    <p class="panel-empty">{t("profiles.empty")}</p>
  {/if}

  {#each profiles as profile (profile.id)}
    <div class="profile">
      <div class="profile-head">
        {#if renaming === profile.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameText}
            aria-label={t("profiles.rename")}
            autofocus
            onblur={() => saveRename(profile)}
            onkeydown={(e) => {
              if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
              if (e.key === "Escape") renaming = null;
            }}
          />
        {:else}
          <h3>{profile.name}</h3>
          <span class="count">{profileCount(profile)}</span>
          {#if profile.inUse}
            <span class="in-use" title={t("profiles.inUseHint")}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
              {t("profiles.inUse")}
            </span>
          {/if}
          <span class="spacer"></span>
          <button class="tool" aria-label={t("profiles.rename")} title={t("profiles.rename")} onclick={() => startRename(profile)}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 20h4L19 9l-4-4L4 16zM14 6l4 4" /></svg>
          </button>
          {#if !profile.inUse}
            <button class="tool" aria-label={t("profiles.delete")} title={t("profiles.delete")} onclick={() => askRemove(profile)}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
            </button>
          {/if}
        {/if}
      </div>

      {#if confirming?.id === profile.id}
        <div class="confirm" role="alertdialog" aria-label={t("profiles.delete")}>
          <span>
            {confirming.sessions > 0
              ? tn("profiles.deleteUsed", confirming.sessions)
              : t("profiles.deleteAsk")}
          </span>
          <button class="btn btn-small btn-danger" onclick={() => remove(profile)}>{t("profiles.delete")}</button>
          <button class="btn btn-small" onclick={() => (confirming = null)}>{t("sessionDialog.cancel")}</button>
        </div>
      {/if}

      <div class="chips">
        {#each profile.apps as app (app.id)}
          <span class="chip" title={app.execNames.split("\n").join(", ")}>
            {app.name}
            {#if !profile.inUse}
              <button class="chip-x" aria-label={t("profiles.removeApp", { name: app.name })} onclick={() => removeApp(profile, app.id)}>
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
              </button>
            {/if}
          </span>
        {/each}
        {#if picking !== profile.id}
          <button class="chip add" onclick={() => openPicker(profile.id)}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
            {t("profiles.addApp")}
          </button>
        {/if}
      </div>

      <div class="chips">
        {#each profile.sites as site (site.id)}
          <span class="chip">
            <svg class="globe" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></svg>
            {site.domain}
            {#if !profile.inUse}
              <button class="chip-x" aria-label={t("profiles.removeApp", { name: site.domain })} onclick={() => removeSite(profile, site.id)}>
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
              </button>
            {/if}
          </span>
        {/each}
        {#if addingSite === profile.id}
          <form class="site-form" onsubmit={(e) => addSite(e, profile)}>
            <input
              bind:this={siteInput}
              bind:value={siteText}
              placeholder={t("profiles.sitePlaceholder")}
              aria-label={t("profiles.sitePlaceholder")}
              onkeydown={(e) => {
                if (e.key === "Escape") addingSite = null;
              }}
              onblur={() => {
                if (!siteText.trim()) addingSite = null;
              }}
            />
            <button class="btn btn-small" type="submit" disabled={!siteText.trim()}>{t("profiles.addSite")}</button>
          </form>
        {:else}
          <button class="chip add" onclick={() => openSiteInput(profile.id)}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
            {t("profiles.addSite")}
          </button>
        {/if}
      </div>

      {#if picking === profile.id}
        <div class="picker">
          <div class="picker-head">
            <input
              bind:this={searchInput}
              bind:value={search}
              placeholder={t("profiles.search")}
              aria-label={t("profiles.search")}
              onkeydown={(e) => onPickerKey(e, profile)}
            />
            <button class="btn btn-small" onclick={closePicker}>{t("profiles.done")}</button>
          </div>
          {#if installed === null}
            <p class="panel-hint">{t("profiles.loading")}</p>
          {:else}
            <ul class="apps" aria-label={t("profiles.installed")}>
              {#each matching as app (app.id)}
                {@const added = onList(profile, app)}
                <li>
                  <button class="app" disabled={added} onclick={() => addApp(profile, app)}>
                    <span class="app-name">{app.name}</span>
                    <span class="app-id" title={app.flatpakId ?? app.execNames.join(", ")}>
                      {app.flatpakId ? `Flatpak · ${app.flatpakId}` : app.execNames.join(", ")}
                    </span>
                    {#if added}<span class="added">{t("profiles.added")}</span>{/if}
                  </button>
                </li>
              {/each}
              {#if programName}
                <li>
                  <button class="app program" onclick={() => addProgram(profile)}>
                    <span class="app-name">{t("profiles.addProgram", { name: programName })}</span>
                    <span class="app-id">{t("profiles.programHint")}</span>
                  </button>
                </li>
              {:else if matching.length === 0}
                <li class="panel-hint">{t("profiles.noMatch")}</li>
              {/if}
            </ul>
          {/if}
        </div>
      {/if}
    </div>
  {/each}

  <form onsubmit={addProfile}>
    <input bind:value={newName} placeholder={t("profiles.newPlaceholder")} aria-label={t("profiles.newPlaceholder")} />
    <button class="btn btn-small" type="submit" disabled={!newName.trim()}>{t("profiles.add")}</button>
  </form>

  {#if message}
    <p class="panel-hint" class:error={message.error} role="status">{message.text}</p>
  {/if}
</section>

<style>
  .intro {
    padding: 0 10px var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .profile {
    margin: 0 0 var(--space-2);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .profile-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }

  h3 {
    font-size: var(--text-md);
    font-weight: var(--weight-bold);
  }

  .count {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .in-use {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 8px;
    border-radius: 99px;
    background: var(--pill);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .spacer {
    flex: 1;
  }

  .tool {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--duration-micro) var(--ease-out);
  }

  .profile:hover .tool,
  .tool:focus-visible {
    opacity: 1;
  }

  .tool:hover {
    background: var(--surface-active);
    color: var(--text);
  }

  .confirm {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }

  .confirm span {
    flex: 1 1 240px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 10px;
    border-radius: 99px;
    background: var(--pill);
    font-size: var(--text-base);
  }

  .chip-x {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    margin-right: -4px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .chip-x:hover {
    background: var(--surface-active);
    color: var(--text);
  }

  .chip.add {
    border: 1px dashed var(--border-dashed);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: color var(--duration-micro) var(--ease-out);
  }

  .chip.add:hover {
    color: var(--text);
  }

  .globe {
    color: var(--text-muted);
  }

  .site-form {
    display: flex;
    gap: var(--space-2);
    flex: 1 1 280px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .site-form input {
    height: 28px;
  }

  .site-form .btn-small {
    min-height: 28px;
  }

  .setup {
    margin: 0 10px var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    font-size: var(--text-sm);
  }

  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .picker-head {
    display: flex;
    gap: var(--space-2);
  }

  .apps {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
  }

  .app {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    min-height: 36px;
    padding: 6px 10px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .app:hover:not(:disabled) {
    background: var(--surface-hover);
  }

  .app:disabled {
    cursor: default;
    color: var(--text-muted);
  }

  .app-name {
    flex: none;
    font-size: var(--text-md);
  }

  .app-id {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: var(--text-sm);
    color: var(--text-faint);
  }

  .added {
    flex: none;
    font-size: var(--text-sm);
    color: var(--status-done-text);
  }

  .program .app-name {
    font-weight: var(--weight-medium);
  }

  form {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding: var(--space-2) 10px 0;
    border-top: 1px solid var(--border);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 var(--space-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: var(--text-md);
  }

  .rename {
    flex: 0 1 240px;
  }

  input::placeholder {
    color: var(--text-faint);
  }

  input:focus {
    outline: none;
    border-color: var(--control);
  }

  .btn-small {
    flex: none;
    min-height: 32px;
    padding: 0 12px;
    font-size: var(--text-sm);
  }

  .btn-small:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-danger {
    color: var(--text-cancelled);
  }

  .error {
    color: var(--text-cancelled);
  }
</style>
