import type { AuthState, HoldFor, InstallState, LedgerEvent, TaskState } from "@plenipo/types";

/** Plain words for the phone's page (docs/design/vocabulary.md). */

export const STATE_WORDS: Record<TaskState, string> = {
  queued: "Waiting its turn",
  running: "Working",
  blocked: "Stuck",
  awaitingApproval: "Waiting for you",
  succeeded: "Done",
  failed: "Didn't finish",
  cancelled: "Stopped",
};

/** "just now", "5 minutes ago", "3 hours ago", or the date. */
export function ago(ms: number, now: number = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return m === 1 ? "1 minute ago" : `${m} minutes ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return h === 1 ? "1 hour ago" : `${h} hours ago`;
  return new Date(ms).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** "4 minutes left", "less than a minute left", or "ended". */
export function timeLeft(endsAt: number | null, now: number = Date.now()): string | null {
  if (endsAt === null) return null;
  const ms = endsAt - now;
  if (ms <= 0) return "Ended";
  if (ms < 60_000) return "Less than a minute left";
  const m = Math.floor(ms / 60_000);
  return m === 1 ? "1 minute left" : `${m} minutes left`;
}

function text(p: unknown, key: string): string | null {
  if (p && typeof p === "object" && key in p) {
    const v = (p as Record<string, unknown>)[key];
    return typeof v === "string" && v.trim() ? v.trim() : null;
  }
  return null;
}

/** One line for an Activity entry. */
export function describe(event: LedgerEvent): string {
  const p = event.payload;
  const who = text(p, "name");
  switch (event.eventType) {
    case "approval.requested":
      return `Asked you: ${text(p, "summary") ?? "an approval"}`;
    case "approval.resolved":
      return text(p, "note") ?? "An approval was answered";
    case "approval.expired":
      return "An approval ended with no answer";
    case "task.created":
      return `New task: ${text(p, "objective") ?? "a task"}`;
    case "task.state_changed":
      return `A task is now ${STATE_WORDS[(text(p, "to") ?? "running") as TaskState]?.toLowerCase() ?? text(p, "to")}`;
    case "control.stopped":
      return "Everything was stopped";
    case "control.allowed":
      return "Work was allowed again";
    case "remote.request":
      return `${who ?? "A phone"} asked to ${text(p, "kind") ?? "do something"}`;
    case "remote.refused":
      return `${who ?? "A phone"} was refused: ${text(p, "kind") ?? "a request"}`;
    case "remote.signed_in":
      return `${who ?? "A phone"} signed in`;
    case "remote.signed_out":
      return `${who ?? "A phone"} signed out`;
    case "remote.device_added":
      return `${who ?? "A phone"} was added`;
    case "remote.device_removed":
      return `${who ?? "A phone"} was removed`;
    case "lesson.added":
      return `${text(p, "worker") ?? "A worker"} learned something`;
    case "lesson.kept":
      return "A lesson was kept";
    case "lesson.discarded":
      return "A lesson was discarded";
    default:
      return event.eventType.replace(/[._]/g, " ");
  }
}

/** What this phone probably is, for its name and its browser line. */
export function guessPhone(ua: string = navigator.userAgent): { name: string; browser: string } {
  const iphone = /iPhone/.test(ua);
  const ipad = /iPad/.test(ua) || (/Macintosh/.test(ua) && "ontouchend" in globalThis);
  const android = /Android/.test(ua);
  const device = iphone ? "iPhone" : ipad ? "iPad" : android ? "Android phone" : "Computer";
  const browser = /EdgA?\//.test(ua)
    ? "Edge"
    : /Firefox|FxiOS/.test(ua)
      ? "Firefox"
      : /Chrome|CriOS/.test(ua)
        ? "Chrome"
        : /Safari/.test(ua)
          ? "Safari"
          : "a web browser";
  return { name: device, browser: `${browser} on ${device}` };
}

/** An iPhone or iPad, in Safari rather than from the Home Screen (where pairing must happen). */
export function iPhoneOutsideHomeScreen(): boolean {
  const ua = navigator.userAgent;
  const apple = /iPhone|iPad/.test(ua) || (/Macintosh/.test(ua) && "ontouchend" in globalThis);
  const standalone =
    window.matchMedia?.("(display-mode: standalone)").matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true;
  return apple && !standalone;
}

/** One AI tool as your PC sends it: its own name, and whether it can work now. */
export interface PhoneAiTool {
  id: string;
  label: string;
  ready: boolean;
  install: InstallState;
  auth: AuthState;
  held: HoldFor | null;
}

/** What an AI tool on your PC can do now, in plain words. */
export function aiToolStatus(tool: PhoneAiTool, outOfService: string | null): string {
  if (outOfService) return outOfService;
  if (tool.install === "checking" || tool.auth === "checking") return "Checking…";
  if (tool.install !== "installed") return "Not working on your PC. AI tools on your PC says why.";
  if (!tool.ready) return "Can't work yet: sign in, or add its key, in AI tools on your PC.";
  if (tool.held === "update") return "Being updated: new work waits a moment.";
  if (tool.held === "signIn") return "Signing in on your PC: new work waits a moment.";
  return "Ready";
}
