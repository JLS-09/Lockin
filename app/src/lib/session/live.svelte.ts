// The running study session and the daemon's status, shared by the sidebar,
// the week grid and the Session page.
//
// The daemon runs the session; the app only shows it and asks for changes.
// While a session runs, its state is read from the database every second, so
// the timer and the break credit are always what Rust works out (no copy of the
// rules here). Without one, every few seconds, to notice when one starts.
// The daemon is asked whether it's running every 15 seconds.

import { api, type CurrentSession, type DaemonStatus } from "$lib/api";

const FAST_MS = 1000;
const SLOW_MS = 5000;
const DAEMON_MS = 15_000;

class LiveSession {
  /** The running session, or null. */
  current = $state<CurrentSession | null>(null);
  /** The daemon's status; null when it isn't running. */
  daemon = $state<DaemonStatus | null>(null);
  /** False until the first answer about the daemon came in. */
  daemonChecked = $state(false);
  /** The clock, ticking every second for the countdowns. */
  now = $state(new Date());
  /** The last action that failed (a break without credit, the daemon gone). */
  error = $state<string | null>(null);
  /** True while an action waits for the daemon's answer. */
  busy = $state(false);

  #timer: ReturnType<typeof setTimeout> | undefined;
  #daemonTimer: ReturnType<typeof setInterval> | undefined;
  #clock: ReturnType<typeof setInterval> | undefined;
  #users = 0;

  /** Start following. Returns the function that stops again (for an $effect). */
  follow(): () => void {
    this.#users += 1;
    if (this.#users === 1) {
      this.#poll();
      this.refreshDaemon();
      this.#daemonTimer = setInterval(() => this.refreshDaemon(), DAEMON_MS);
      this.#clock = setInterval(() => (this.now = new Date()), 1000);
    }
    return () => {
      this.#users -= 1;
      if (this.#users > 0) return;
      clearTimeout(this.#timer);
      clearInterval(this.#daemonTimer);
      clearInterval(this.#clock);
    };
  }

  async #poll() {
    await this.refresh();
    this.#timer = setTimeout(() => this.#poll(), this.current ? FAST_MS : SLOW_MS);
  }

  async refresh() {
    try {
      this.current = await api.currentSession();
    } catch (e) {
      this.error = String(e);
    }
  }

  async refreshDaemon() {
    try {
      this.daemon = await api.daemonStatus();
    } catch {
      this.daemon = null;
    }
    this.daemonChecked = true;
  }

  /** Seconds left until the running session ends. */
  get secondsLeft(): number {
    if (!this.current) return 0;
    return Math.max(0, (new Date(this.current.session.endAt).getTime() - this.now.getTime()) / 1000);
  }

  /** Seconds left of the current break. */
  get breakSecondsLeft(): number {
    const ends = this.current?.state.breakEndsAt;
    if (!ends) return 0;
    return Math.max(0, (new Date(ends).getTime() - this.now.getTime()) / 1000);
  }

  /** How far along the session is, 0 to 1. */
  get progress(): number {
    if (!this.current) return 0;
    const start = new Date(this.current.session.startAt).getTime();
    const end = new Date(this.current.session.endAt).getTime();
    return Math.min(1, Math.max(0, (this.now.getTime() - start) / (end - start)));
  }

  takeBreak = () => this.#ask(() => api.takeBreak());
  resume = () => this.#ask(() => api.resumeSession());
  emergencyStop = () => this.#ask(() => api.emergencyStop());

  /** Ask the daemon for something, then show the new state at once. */
  async #ask(action: () => Promise<void>): Promise<boolean> {
    this.busy = true;
    this.error = null;
    try {
      await action();
      return true;
    } catch (e) {
      this.error = String(e);
      this.refreshDaemon();
      return false;
    } finally {
      await this.refresh();
      this.busy = false;
    }
  }
}

export const live = new LiveSession();

/** "1:10:24" or "4:58" */
export function clockText(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor(total / 60) % 60;
  const rest = String(total % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
}

/** "1:10" (hours and minutes) or "0:45", for "1:10 left" and "Elapsed 1:19". */
export function hoursText(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds / 60));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
