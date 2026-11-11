<script lang="ts">
  // Settings → Filter rules: "ignore classes whose title contains …".
  // Rules apply to every class at once; a class can still be kept with
  // "Show anyway" in the week view. Not in the mockups; same style as the panels.
  import { api, type FilterRule } from "$lib/api";
  import { t, tn } from "$lib/i18n/index.svelte";

  let { onChange }: { onChange?: () => void } = $props();

  let rules = $state<FilterRule[]>([]);
  let pattern = $state("");
  let message = $state<{ text: string; error: boolean } | null>(null);

  async function load() {
    try {
      rules = await api.rules();
    } catch (e) {
      message = { text: String(e), error: true };
    }
  }
  load();

  async function add(event: SubmitEvent) {
    event.preventDefault();
    const text = pattern.trim();
    if (!text) return;
    try {
      const changed = await api.addRule(text);
      pattern = "";
      message = { text: tn("rules.added", changed), error: false };
      onChange?.();
    } catch (e) {
      message = { text: String(e), error: true };
    }
    await load();
  }

  async function remove(rule: FilterRule) {
    // Gone from the list at once; it's easy to add again, so no confirmation.
    rules = rules.filter((r) => r.id !== rule.id);
    try {
      const changed = await api.removeRule(rule.id);
      message = { text: tn("rules.removed", changed), error: false };
      onChange?.();
    } catch (e) {
      message = { text: String(e), error: true };
      await load();
    }
  }

  function describe(rule: FilterRule): string {
    const key = rule.field === "location" ? "rules.locationContains" : "rules.titleContains";
    return t(key, { pattern: rule.pattern });
  }
</script>

<section class="panel" id="rules" aria-labelledby="rules-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="rules-heading">{t("rules.title")}</h2>
  </div>
  <p class="intro">{t("rules.hint")}</p>

  {#if rules.length === 0}
    <p class="panel-empty">{t("rules.empty")}</p>
  {:else}
    <ul>
      {#each rules as rule (rule.id)}
        <li>
          <svg class="icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M3 3l18 18M10.6 6.1A9 9 0 0 1 21 12a10 10 0 0 1-2.3 3M6.6 6.6A10 10 0 0 0 3 12c2 4 5.5 6 9 6 1.6 0 3.1-.4 4.4-1.1" /></svg>
          <span class="ellipsis" title={describe(rule)}>{describe(rule)}</span>
          <button class="tool" aria-label={t("rules.remove")} title={t("rules.remove")} onclick={() => remove(rule)}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <form onsubmit={add}>
    <input bind:value={pattern} placeholder={t("rules.placeholder")} aria-label={t("rules.placeholder")} />
    <button class="btn btn-small" type="submit" disabled={!pattern.trim()}>{t("rules.add")}</button>
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

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 40px;
    padding: 4px 10px;
    border-radius: var(--radius);
  }

  li:hover {
    background: var(--surface-hover);
  }

  .icon {
    flex: none;
    color: var(--text-faint);
  }

  .ellipsis {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
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

  li:hover .tool,
  .tool:focus-visible {
    opacity: 1;
  }

  .tool:hover {
    background: var(--surface-active);
    color: var(--text);
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

  .error {
    color: var(--text-cancelled);
  }
</style>
