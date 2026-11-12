<script lang="ts">
  // Settings → Tray and notifications. Not in the mockups; built from the panels'
  // pieces. The daemon shows the tray and sends the notifications; this panel
  // only switches kinds of notification on or off (the `setting` table, read by
  // the daemon every few seconds) and asks the daemon for a test notification.
  import { api } from "$lib/api";
  import { t } from "$lib/i18n/index.svelte";
  import { live } from "$lib/session/live.svelte";

  // One switch per kind, as the daemon groups them (lockin_core::notices::Kind).
  const kinds = [
    { key: "notify.sessions", label: "notifications.sessions", hint: "notifications.sessionsHint" },
    { key: "notify.breaks", label: "notifications.breaks", hint: "notifications.breaksHint" },
    { key: "notify.deadlines", label: "notifications.deadlines", hint: "notifications.deadlinesHint" },
    { key: "notify.closedApps", label: "notifications.closedApps", hint: "notifications.closedAppsHint" },
  ] as const;

  /** The saved settings; a missing notify.* key means on. */
  let saved = $state<Record<string, string>>({});
  let message = $state<{ text: string; error: boolean } | null>(null);
  let sending = $state(false);

  api
    .settings()
    .then((found) => (saved = found))
    .catch((e) => (message = { text: String(e), error: true }));

  const isOn = (key: string) => saved[key] !== "off";

  // Feedback first: the switch flips at once and flips back if saving fails.
  async function toggle(key: string, on: boolean) {
    const before = saved[key];
    saved = { ...saved, [key]: on ? "on" : "off" };
    try {
      await api.setSetting(key, on ? "on" : "off");
    } catch (e) {
      saved = { ...saved, [key]: before };
      message = { text: String(e), error: true };
    }
  }

  async function sendTest() {
    sending = true;
    message = null;
    try {
      await api.testNotification();
      message = { text: t("notifications.testSent"), error: false };
    } catch (e) {
      message = { text: String(e), error: true };
    } finally {
      sending = false;
    }
  }
</script>

<section class="panel" id="notifications" aria-labelledby="notifications-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="notifications-heading">{t("notifications.label")}</h2>
  </div>
  <p class="intro">{t("notifications.hint")}</p>

  <ul>
    {#each kinds as kind (kind.key)}
      <li>
        <label class="check">
          <input
            type="checkbox"
            checked={isOn(kind.key)}
            onchange={(event) => toggle(kind.key, event.currentTarget.checked)}
          />
          <span class="text">
            <span class="label">{t(kind.label)}</span>
            <span class="hint">{t(kind.hint)}</span>
          </span>
        </label>
      </li>
    {/each}
  </ul>

  <div class="actions">
    <button class="btn" disabled={sending || !live.daemon} onclick={sendTest}>{t("notifications.test")}</button>
    {#if !live.daemon}<span class="hint">{t("daemon.notRunning")}</span>{/if}
  </div>

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
    padding: 0 10px;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .check {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    cursor: pointer;
  }

  .check input {
    width: 16px;
    height: 16px;
    margin: 2px 0 0;
    flex: none;
    accent-color: var(--control);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .label {
    font-size: var(--text-md);
  }

  .hint {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) 10px var(--space-2);
  }

  .error {
    color: var(--status-behind);
  }
</style>
