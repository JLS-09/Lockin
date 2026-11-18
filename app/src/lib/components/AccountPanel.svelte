<script lang="ts">
  // Settings → Account. Not in the mockups; built from the panels' and the
  // deadline dialog's pieces. Logged out: "Log in" or "Create account" (with an
  // invite code); logged in: the email and "Log out". The login itself stays in
  // Rust (~/.config/lockin/account.json); this panel only learns the email.
  // An invite link (lockin://invite/K7QM-2XWD) opens Settings with ?invite=,
  // which arrives here as `invite` and opens "Create account" with the code.
  import { api, type Account } from "$lib/api";
  import { t, type Key } from "$lib/i18n/index.svelte";

  let { invite = null }: { invite?: string | null } = $props();

  type Mode = "logIn" | "signUp";

  let account = $state<Account | null>(null);
  let loaded = $state(false);
  let mode = $state<Mode>("logIn");
  let email = $state("");
  let password = $state("");
  let inviteCode = $state("");
  let busy = $state(false);
  let problem = $state<string | null>(null);
  let notice = $state<string | null>(null);

  api
    .account()
    .then((found) => {
      account = found;
      loaded = true;
    })
    .catch((e) => (problem = String(e)));

  // A code from an invite link: straight to "Create account", filled in.
  $effect(() => {
    if (invite) {
      mode = "signUp";
      inviteCode = formatCode(invite);
      notice = t("account.inviteFromLink");
      // Also when Settings was already open.
      document.getElementById("account")?.scrollIntoView({ block: "start" });
    }
  });

  /** "K7QM2XWD" → "K7QM-2XWD", as the invite shows it. */
  function formatCode(code: string): string {
    const clean = code.toUpperCase().replace(/[^A-Z0-9]/g, "");
    return clean.length === 8 ? `${clean.slice(0, 4)}-${clean.slice(4)}` : clean;
  }

  /** Rust sends known errors as their translation key. */
  function errorText(error: unknown): string {
    const text = String(error);
    return text.startsWith("account.error.") ? t(text as Key) : text;
  }

  function choose(next: Mode) {
    mode = next;
    problem = null;
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    problem = null;
    if (!email.trim() || !password) {
      problem = t("account.missing");
      return;
    }
    if (mode === "signUp" && !inviteCode.trim()) {
      problem = t("account.missingInvite");
      return;
    }
    busy = true;
    try {
      account =
        mode === "signUp"
          ? await api.signUp(email, password, inviteCode)
          : await api.logIn(email, password);
      notice = mode === "signUp" ? t("account.signedUp") : null;
      password = "";
      inviteCode = "";
    } catch (e) {
      problem = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function logOut() {
    busy = true;
    try {
      await api.logOut();
      email = account?.email ?? "";
      account = null;
      mode = "logIn";
      notice = null;
    } catch (e) {
      problem = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="panel" id="account" aria-labelledby="account-heading">
  <div class="panel-head">
    <h2 class="panel-title" id="account-heading">{t("account.label")}</h2>
  </div>

  {#if !loaded}
    {#if problem}<p class="panel-hint error" role="alert">{problem}</p>{/if}
  {:else if account}
    <div class="row">
      <svg class="icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="8" r="4" /><path d="M4 21c0-4 3.6-7 8-7s8 3 8 7" /></svg>
      <span class="text">
        <span class="ellipsis" title={account.email}>{t("account.loggedInAs", { email: account.email })}</span>
        <span class="sub">{t("account.loggedInHint")}</span>
      </span>
      <button class="btn btn-small" disabled={busy} onclick={logOut}>{t("account.logOut")}</button>
    </div>
    {#if notice}<p class="panel-hint" role="status">{notice}</p>{/if}
    {#if problem}<p class="panel-hint error" role="alert">{problem}</p>{/if}
  {:else}
    <p class="intro">{t("account.intro")}</p>

    <div class="choice" role="group" aria-label={t("account.label")}>
      <button class="btn btn-small" class:btn-primary={mode === "logIn"} aria-pressed={mode === "logIn"} onclick={() => choose("logIn")}>
        {t("account.logIn")}
      </button>
      <button class="btn btn-small" class:btn-primary={mode === "signUp"} aria-pressed={mode === "signUp"} onclick={() => choose("signUp")}>
        {t("account.signUp")}
      </button>
    </div>

    <!-- novalidate: our own message under the fields, not the browser's bubble. -->
    <form novalidate onsubmit={submit}>
      <label class="field">
        <span class="label">{t("account.email")}</span>
        <input type="email" bind:value={email} autocomplete="username" spellcheck="false" />
      </label>
      <label class="field">
        <span class="label">{t("account.password")}</span>
        <input
          type="password"
          bind:value={password}
          autocomplete={mode === "signUp" ? "new-password" : "current-password"}
        />
        {#if mode === "signUp"}<span class="sub">{t("account.passwordHint")}</span>{/if}
      </label>
      {#if mode === "signUp"}
        <label class="field">
          <span class="label">{t("account.invite")}</span>
          <input
            class="code"
            bind:value={inviteCode}
            placeholder="K7QM-2XWD"
            autocomplete="off"
            spellcheck="false"
            onblur={() => (inviteCode = formatCode(inviteCode))}
          />
        </label>
      {/if}

      {#if notice && mode === "signUp"}<p class="hint" role="status">{notice}</p>{/if}
      {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

      <div class="buttons">
        <button type="submit" class="btn btn-primary" disabled={busy}>
          {t(mode === "signUp" ? "account.signUp" : "account.logIn")}
        </button>
      </div>
      {#if mode === "logIn"}<p class="sub">{t("account.forgot")}</p>{/if}
    </form>
  {/if}
</section>

<style>
  .intro {
    padding: 0 10px var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .choice {
    display: flex;
    gap: var(--space-2);
    padding: 0 10px var(--space-3);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 380px;
    padding: 0 10px var(--space-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .label {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  input {
    height: 36px;
    padding: 0 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
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

  .code {
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .btn-small {
    flex: none;
    min-height: 32px;
    padding: 0 12px;
    font-size: var(--text-sm);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .buttons {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .btn {
    white-space: nowrap;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 48px;
    padding: 4px 10px;
  }

  .icon {
    flex: none;
    color: var(--text-faint);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ellipsis {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sub,
  .hint {
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .problem,
  .error {
    font-size: var(--text-sm);
    color: var(--status-behind);
  }
</style>
