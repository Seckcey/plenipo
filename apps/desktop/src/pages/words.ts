/** Plain words for Home and the pages of one thing (Phase 12). */

import type {
  ActivitySeries,
  LedgerEvent,
  OrgSnapshot,
  PositionInfo,
  TaskState,
} from "@plenipo/types";
import type { PipPose, Status } from "@plenipo/ui";

import { describeEvent } from "../ledger/format";

/** A task's state on the status ramp, with its word. */
export const TASK_STATUS: Record<TaskState, { status: Status; label: string }> = {
  queued: { status: "pending", label: "Queued" },
  running: { status: "ok", label: "Working" },
  blocked: { status: "pending", label: "Waiting on replies" },
  awaitingApproval: { status: "pending", label: "Waiting for you" },
  succeeded: { status: "ok", label: "Done" },
  failed: { status: "error", label: "Failed" },
  cancelled: { status: "offline", label: "Cancelled" },
};

/** Whether Core's error says the thing asked for isn't in the Ledger (not a failure to retry). */
export function isNotFound(message: string): boolean {
  return message.startsWith("not found");
}

/** The first line of a text, cut to `max` characters. */
export function firstLine(text: string, max = 140): string {
  const line = text.trim().split("\n")[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/**
 * How an event reads in a page's History: the Activity trail's words, without the AI tool's
 * own conversation number (Diagnostics and the Activity trail keep it).
 */
export function historyLine(e: LedgerEvent): string {
  const p = (e.payload ?? {}) as Record<string, unknown>;
  if (e.eventType === "agent.session_bound") {
    return typeof p.model === "string" && p.model !== ""
      ? `Conversation started · model ${p.model}`
      : "Conversation started";
  }
  return describeEvent(e);
}

/** How an event reads on the status ramp, when it is a problem or a request (else nothing). */
export function eventStatus(e: LedgerEvent): { status: Status; label: string } | undefined {
  const p = (e.payload ?? {}) as Record<string, unknown>;
  switch (e.eventType) {
    case "task.state_changed":
      if (p.to === "failed") return { status: "error", label: "Failed" };
      if (p.to === "succeeded") return { status: "ok", label: "Done" };
      if (p.to === "cancelled") return { status: "offline", label: "Cancelled" };
      return undefined;
    case "guard.denied":
    case "task.transition_rejected":
    case "liaison.handoff_rejected":
      return { status: "warn", label: "Refused" };
    case "liaison.dispatch_failed":
    case "liaison.delivery_failed":
      return { status: "error", label: "Failed" };
    case "liaison.waiting_for_member":
      return { status: "pending", label: "Waiting its turn" };
    case "approval.requested":
      return { status: "pending", label: "Waiting for you" };
    case "approval.resolved":
      return p.state === "approved"
        ? { status: "ok", label: "Approved" }
        : { status: "warn", label: "Not approved" };
    case "approval.expired":
      return { status: "warn", label: "Expired" };
    case "ssh.host_key_changed":
      return { status: "error", label: "Server ID changed" };
    case "lesson.kept":
      return { status: "ok", label: "Kept" };
    case "lesson.discarded":
      return { status: "offline", label: "Discarded" };
  }
  return undefined;
}

/** "Good morning", by the hour on the owner's computer. */
export function greeting(hour: number): string {
  if (hour < 5) return "Working late";
  if (hour < 12) return "Good morning";
  if (hour < 17) return "Good afternoon";
  return "Good evening";
}

/** What Home is showing, for Pip's pose and the line under the greeting. */
export interface HomeMood {
  loading: boolean;
  failed: boolean;
  /** No departments, positions, or objectives yet. */
  empty: boolean;
  waiting: number;
  stuck: number;
  working: number;
  /** Objectives finished in the last day. */
  finishedDay: number;
}

/** Pip's pose on Home: what matters most right now. */
export function homePip(m: HomeMood): PipPose {
  if (m.loading) return "launch";
  if (m.failed || m.stuck > 0) return "support";
  if (m.empty) return "welcome";
  if (m.waiting > 0) return "presenting";
  if (m.working > 0) return "coding";
  if (m.finishedDay > 0) return "celebrating";
  return "recharging";
}

export function count(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** One line under the greeting, saying how things are. */
export function homeLine(m: HomeMood): string {
  if (m.loading) return "Getting everything ready…";
  if (m.failed) return "Plenipo couldn't read how things are. Try again in a moment.";
  if (m.empty) {
    return "Your company is ready for its first department. Open Organization to set it up.";
  }
  const parts: string[] = [];
  if (m.waiting > 0) {
    parts.push(
      m.waiting === 1 ? "1 thing is waiting for you" : `${m.waiting} things are waiting for you`,
    );
  }
  if (m.stuck > 0) parts.push(m.stuck === 1 ? "1 thing is stuck" : `${m.stuck} things are stuck`);
  if (m.working > 0) {
    parts.push(m.working === 1 ? "1 worker is working" : `${m.working} workers are working`);
  }
  if (parts.length === 0) {
    return m.finishedDay > 0
      ? `All quiet. ${count(m.finishedDay, "objective")} finished in the last day.`
      : "All quiet. Nobody is working right now.";
  }
  const sentence =
    parts.length === 1
      ? parts[0]!
      : `${parts.slice(0, -1).join(", ")}, and ${parts[parts.length - 1]!}`;
  return `${sentence.charAt(0).toUpperCase()}${sentence.slice(1)}.`;
}

/** The positions of a department (every position whose nearest department head is its head). */
export function departmentPositions(org: OrgSnapshot, departmentId: string): PositionInfo[] {
  return org.positions.filter((p) => p.active && p.departmentId === departmentId);
}

/** The positions of a project. */
export function projectPositions(org: OrgSnapshot, projectId: string): PositionInfo[] {
  return org.positions.filter((p) => p.active && p.projectId === projectId);
}

/** Working, or waiting on its team: a position at work now (not one queued for a slot). */
export function isBusy(p: PositionInfo): boolean {
  return p.status === "working" || p.status === "waiting";
}

/**
 * A department's health, from its positions and its last day: a position that cannot work or
 * any problem needs attention; work in hand is "Working"; otherwise it is quiet.
 */
export function departmentHealth(
  positions: readonly PositionInfo[],
  series: ActivitySeries | undefined,
  stuck: number,
): { status: Status; label: string } {
  const problems = series?.buckets.reduce((n, b) => n + b.problems, 0) ?? 0;
  if (stuck > 0) return { status: "error", label: `${count(stuck, "thing")} stuck` };
  if (positions.some((p) => p.status === "unavailable")) {
    return { status: "warn", label: "Someone can't work" };
  }
  if (problems > 0) return { status: "warn", label: `${count(problems, "problem")} today` };
  if (positions.some((p) => p.status === "working" || p.status === "waiting")) {
    return { status: "ok", label: "Working" };
  }
  if (positions.some((p) => p.status === "queued")) return { status: "pending", label: "Queued" };
  return { status: "offline", label: "Quiet" };
}

/** "3:05 PM" today, else "Sep 27, 3:05 PM" (the computer's own time zone). */
export function when(ms: number, now: number = Date.now()): string {
  const d = new Date(ms);
  const today = new Date(now);
  const sameDay =
    d.getFullYear() === today.getFullYear() &&
    d.getMonth() === today.getMonth() &&
    d.getDate() === today.getDate();
  return sameDay
    ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })
    : d.toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}
