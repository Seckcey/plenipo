/**
 * Plain words and small sums for the AI tools page (Phase 19, ADR-058 to ADR-060): versions,
 * updates, "left of your plan", and usage by day and week.
 */

import type {
  AgentSession,
  AiToolUpdateState,
  AiToolUsage,
  PlanWindow,
  UsageModel,
} from "@plenipo/types";

import { count } from "../../pages/words";
import { tokens } from "../../routing/format";

/** The switch's words, the same on the AI tools page and in Settings → Switches. */
export const AUTO_UPDATE_LABEL = "Update AI tools by themselves";
export const AUTO_UPDATE_HINT =
  "Off (the default): Plenipo tells you when a new version is ready and updates only when you press Update. On: it updates each AI tool by itself, only when no task is using it.";

/** Update states while something is happening (the page reads itself again every 2 seconds). */
export const MOVING: ReadonlySet<AiToolUpdateState> = new Set(["waiting", "updating", "checking"]);

// ---- Versions (ADR-059 §1) -------------------------------------------------------------------

/** "1.0.41" → [1, 0, 41]; a pre-release ("1.0.43-beta.1") counts as its release. */
function parts(version: string): number[] | null {
  const found = /(\d+(?:\.\d+)*)/.exec(version);
  return found?.[1] ? found[1].split(".").map(Number) : null;
}

/** -1 when `a` is older than `b`, 1 when newer, 0 the same; `null` when one is not a version. */
export function compareVersions(a: string, b: string): -1 | 0 | 1 | null {
  const x = parts(a);
  const y = parts(b);
  if (!x || !y) return null;
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return d < 0 ? -1 : 1;
  }
  return 0;
}

/** The notice when the installed version is not the one Plenipo was checked with. */
export function versionNotice(installed: string | null, checked: string): string | null {
  if (!installed) return null;
  const c = compareVersions(installed, checked);
  if (c === 1) {
    return `This version is newer than the one Plenipo was checked with (${checked}). It should work; if something looks wrong, check for a new version of Plenipo.`;
  }
  if (c === -1) {
    return `This version is older than the one Plenipo was checked with (${checked}). Update it to get every model.`;
  }
  return null;
}

// ---- Tasks using a tool (ADR-058 §5, ADR-059 §4) ---------------------------------------------

/** How many tasks are running on an AI tool now. */
export function tasksUsing(sessions: Readonly<Record<string, AgentSession>>, id: string): number {
  return Object.values(sessions).filter((s) => s.runtimeId === id && s.activeTaskId).length;
}

/** "1 task is using Codex", "2 tasks are using Codex". */
export function usingWords(n: number, label: string): string {
  return `${count(n, "task")} ${n === 1 ? "is" : "are"} using ${label}`;
}

// ---- Left of your plan (ADR-060 §3) ----------------------------------------------------------

/** "5-hour limit", "Weekly limit": a window's name, from its length. */
export function planWindowName(minutes: number | null): string {
  switch (minutes) {
    case 300:
      return "5-hour limit";
    case 1440:
      return "Daily limit";
    case 10080:
      return "Weekly limit";
    default:
      return "Your plan";
  }
}

/** "91% of your plan left", or only what the tool said. */
export function planLeft(
  window: PlanWindow,
  report: { limited: boolean; warning: boolean },
): string {
  if (report.limited) return "Limit reached";
  const used = window.usedPercent;
  const left = used === null ? null : `${Math.max(0, Math.round(100 - used))}% of your plan left`;
  if (report.warning) return left ? `Close to the limit: ${left}` : "Close to the limit";
  return left ?? "Within your plan";
}

// ---- Usage by day and week (ADR-060 §1) ------------------------------------------------------

/** The days the Usage tab asks for, and where today and the weeks start (weeks run Mon–Sun). */
export interface UsageWindow {
  /** Each day's start (midnight on this computer), then the end of the last day (tomorrow). */
  starts: number[];
  today: number;
  tomorrow: number;
  thisWeek: number;
  lastWeek: number;
  /** The first of the last 14 days (today is the 14th). */
  last14: number;
}

/** Midnight `days` days after the day `ms` falls on (this computer's time, summer time too). */
function midnight(ms: number, days = 0): number {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + days).getTime();
}

/** From last week's Monday or 14 days ago, whichever is earlier, to tomorrow's midnight. */
export function usageWindow(now: number): UsageWindow {
  const today = midnight(now);
  const sinceMonday = (new Date(today).getDay() + 6) % 7;
  const thisWeek = midnight(today, -sinceMonday);
  const lastWeek = midnight(thisWeek, -7);
  const last14 = midnight(today, -13);
  const tomorrow = midnight(today, 1);
  const first = Math.min(lastWeek, last14);
  const starts: number[] = [];
  for (let day = first; day <= tomorrow; day = midnight(day, 1)) starts.push(day);
  return { starts, today, tomorrow, thisWeek, lastWeek, last14 };
}

/** Tokens and tasks added up. */
export interface UsageSum {
  tasks: number;
  steps: number;
  countedSteps: number;
  read: number;
  reused: number;
  written: number;
}

const ZERO: UsageSum = { tasks: 0, steps: 0, countedSteps: 0, read: 0, reused: 0, written: 0 };

function add(a: UsageSum, b: UsageSum | UsageModel): UsageSum {
  return {
    tasks: a.tasks + b.tasks,
    steps: a.steps + b.steps,
    countedSteps: a.countedSteps + b.countedSteps,
    read: a.read + b.read,
    reused: a.reused + b.reused,
    written: a.written + b.written,
  };
}

/** The key of the AI tool's default model (a model with no name). */
export const DEFAULT_MODEL = "";

/** Each model's usage on the days from `from` up to (not including) `to`, and all models'. */
export function usageBetween(
  usage: AiToolUsage,
  from: number,
  to: number,
): { byModel: Map<string, UsageSum>; total: UsageSum } {
  const byModel = new Map<string, UsageSum>();
  let total = ZERO;
  for (const day of usage.days) {
    if (day.start < from || day.start >= to) continue;
    for (const m of day.models) {
      const key = m.model ?? DEFAULT_MODEL;
      byModel.set(key, add(byModel.get(key) ?? ZERO, m));
      total = add(total, m);
    }
  }
  return { byModel, total };
}

/** Every model that has usage, most used (read and written) first; the default last. */
export function usageModels(usage: AiToolUsage): string[] {
  const { byModel } = usageBetween(usage, -Infinity, Infinity);
  return [...byModel.entries()]
    .sort(
      ([a, x], [b, y]) =>
        Number(a === DEFAULT_MODEL) - Number(b === DEFAULT_MODEL) ||
        y.read + y.written - (x.read + x.written) ||
        y.tasks - x.tasks ||
        a.localeCompare(b),
    )
    .map(([name]) => name);
}

/** A tool ran steps but reported token counts for none (Kimi): only tasks are counted. */
export function countsNothing(usage: AiToolUsage): boolean {
  const { total } = usageBetween(usage, -Infinity, Infinity);
  return total.steps > 0 && total.countedSteps === 0;
}

/** "3,000 read (400 reused) · 300 written · 3 tasks"; tasks only when no counts come. */
export function usageLine(sum: UsageSum, counts: boolean): string {
  const tasks = count(sum.tasks, "task");
  if (!counts) return tasks;
  const reused = sum.reused > 0 ? ` (${tokens(sum.reused)} reused)` : "";
  return `${tokens(sum.read)} read${reused} · ${tokens(sum.written)} written · ${tasks}`;
}

/** "Today", else "Wed, Sep 30". */
export function dayName(start: number, today: number): string {
  if (start === today) return "Today";
  return new Date(start).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}
