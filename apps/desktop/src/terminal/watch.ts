/**
 * Watch tabs (Phase 12, ADR-031 §5): one per worker using a server, built from the same Ledger
 * events as the Activity trail. A tab shows each command Guard let through and its output as it
 * arrives (secrets already hidden), each refused command, and whether the worker is still
 * connected. It is read-only: the owner can only Stop the command running now, or Disconnect.
 */

import type { Environment, LedgerEvent } from "@plenipo/types";
import type { LogLine } from "@plenipo/ui";

/** Most output lines kept per command, and per tab (the oldest go first). */
export const MAX_COMMAND_LINES = 2000;
export const MAX_TAB_LINES = 6000;
/** Plenipo records the first 2,000 output lines of a command, and counts the rest. */
export const RECORDED_COMMAND_LINES = 2000;

export interface WatchLine {
  stream: "out" | "err";
  text: string;
}

export interface WatchCommand {
  kind: "command";
  commandId: string;
  /** The command as Guard let it through, secrets hidden. */
  command: string;
  cwd: string | null;
  lines: WatchLine[];
  /** Lines left out to keep the tab small. */
  dropped: number;
  state: "running" | "stopping" | "finished";
  /** How it ended ("exited", "stopped", …), its exit code, and why. */
  ending: string | null;
  exitCode: number | null;
  why: string | null;
  seconds: number | null;
  /** How many lines it printed in all (when it finished). */
  total: number | null;
  at: number;
}

export interface WatchRefusal {
  kind: "refused";
  id: string;
  summary: string;
  reason: string;
  at: number;
}

export type WatchItem = WatchCommand | WatchRefusal;

export interface WatchTab {
  /** `watch:<grant>:<server>`. */
  id: string;
  grantId: string;
  taskId: string | null;
  worker: string;
  serverId: string;
  server: string;
  environment: Environment | null;
  address: string | null;
  connected: boolean;
  /** Why the worker's connection ended. */
  disconnected: string | null;
  items: WatchItem[];
  openedAt: number;
}

type Payload = Record<string, unknown>;

const str = (v: unknown): string | null => (typeof v === "string" && v !== "" ? v : null);
const num = (v: unknown): number | null => (typeof v === "number" ? v : null);

export const watchTabId = (grantId: string, serverId: string) => `watch:${grantId}:${serverId}`;

function payload(e: LedgerEvent): Payload {
  return typeof e.payload === "object" && e.payload !== null ? (e.payload as Payload) : {};
}

function environmentOf(v: unknown): Environment | null {
  return v === "development" || v === "staging" || v === "production" ? v : null;
}

/** Change one command, wherever it is. */
function mapCommand(
  tabs: WatchTab[],
  commandId: string,
  change: (c: WatchCommand, tab: WatchTab) => WatchCommand,
): WatchTab[] {
  let changed = false;
  const next = tabs.map((tab) => {
    const i = tab.items.findIndex((it) => it.kind === "command" && it.commandId === commandId);
    if (i < 0) return tab;
    changed = true;
    const items = tab.items.slice();
    items[i] = change(items[i] as WatchCommand, tab);
    return { ...tab, items };
  });
  return changed ? next : tabs;
}

/** Keep a tab within `MAX_TAB_LINES`, dropping the oldest output first. */
function trim(tab: WatchTab): WatchTab {
  let total = tab.items.reduce((n, it) => n + (it.kind === "command" ? it.lines.length : 0), 0);
  if (total <= MAX_TAB_LINES) return tab;
  const items = tab.items.map((it) => {
    if (it.kind !== "command" || total <= MAX_TAB_LINES || it.lines.length === 0) return it;
    const cut = Math.min(it.lines.length, total - MAX_TAB_LINES);
    total -= cut;
    return { ...it, lines: it.lines.slice(cut), dropped: it.dropped + cut };
  });
  return { ...tab, items };
}

/**
 * The watch tabs after one Ledger event. Events that are not about a worker's server work leave
 * them as they are.
 */
export function applyWatchEvent(tabs: WatchTab[], e: LedgerEvent): WatchTab[] {
  const p = payload(e);
  const grantId = str(p.grantId);
  switch (e.eventType) {
    case "ssh.connected": {
      const serverId = str(p.serverId);
      if (!grantId || !serverId) return tabs;
      const id = watchTabId(grantId, serverId);
      const existing = tabs.find((t) => t.id === id);
      if (existing) {
        return tabs.map((t) => (t.id === id ? { ...t, connected: true, disconnected: null } : t));
      }
      return [
        ...tabs,
        {
          id,
          grantId,
          taskId: e.taskId,
          worker: str(p.worker) ?? "A worker",
          serverId,
          server: str(p.server) ?? "a server",
          environment: environmentOf(p.environment),
          address: str(p.address),
          connected: true,
          disconnected: null,
          items: [],
          openedAt: e.createdAt,
        },
      ];
    }
    case "ssh.command_started": {
      const serverId = str(p.serverId);
      const commandId = str(p.commandId);
      if (!grantId || !serverId || !commandId) return tabs;
      const id = watchTabId(grantId, serverId);
      return tabs.map((t) =>
        t.id === id && !t.items.some((it) => it.kind === "command" && it.commandId === commandId)
          ? {
              ...t,
              items: [
                ...t.items,
                {
                  kind: "command",
                  commandId,
                  command: str(p.command) ?? "a command",
                  cwd: str(p.cwd),
                  lines: [],
                  dropped: 0,
                  state: "running",
                  ending: null,
                  exitCode: null,
                  why: null,
                  seconds: null,
                  total: null,
                  at: e.createdAt,
                },
              ],
            }
          : t,
      );
    }
    case "ssh.output": {
      const commandId = str(p.commandId);
      const lines = Array.isArray(p.lines) ? p.lines.filter((l) => typeof l === "string") : [];
      if (!commandId || lines.length === 0) return tabs;
      const stream = p.stream === "err" ? "err" : "out";
      const next = mapCommand(tabs, commandId, (c) => {
        const added: WatchLine[] = lines.map((text) => ({ stream, text }));
        const all = [...c.lines, ...added];
        const cut = Math.max(0, all.length - MAX_COMMAND_LINES);
        return { ...c, lines: cut ? all.slice(cut) : all, dropped: c.dropped + cut };
      });
      return next === tabs ? tabs : next.map(trim);
    }
    case "ssh.command_stop_requested": {
      const commandId = str(p.commandId);
      if (!commandId) return tabs;
      return mapCommand(tabs, commandId, (c) =>
        c.state === "running" ? { ...c, state: "stopping" } : c,
      );
    }
    case "ssh.command_finished": {
      const commandId = str(p.commandId);
      if (!commandId) return tabs;
      return mapCommand(tabs, commandId, (c) => ({
        ...c,
        state: "finished",
        ending: str(p.ending),
        exitCode: num(p.exitCode),
        why: str(p.why),
        seconds: num(p.seconds),
        total: num(p.lines),
      }));
    }
    case "ssh.disconnected": {
      const serverId = str(p.serverId);
      if (!grantId) return tabs;
      return tabs.map((t) =>
        t.grantId === grantId && (!serverId || t.serverId === serverId)
          ? { ...t, connected: false, disconnected: str(p.why) ?? "the connection ended" }
          : t,
      );
    }
    case "guard.denied": {
      if (!grantId || !str(p.tool)?.startsWith("ssh_run")) return tabs;
      const mine = tabs.filter((t) => t.grantId === grantId);
      if (mine.length === 0) return tabs;
      const server = str(p.server);
      // The tab of the server named, else the worker's latest tab.
      const tab = mine.find((t) => server !== null && t.server === server) ?? mine[mine.length - 1];
      if (!tab || tab.items.some((it) => it.kind === "refused" && it.id === e.id)) return tabs;
      return tabs.map((t) =>
        t.id === tab.id
          ? {
              ...t,
              items: [
                ...t.items,
                {
                  kind: "refused",
                  id: e.id,
                  summary: str(p.summary) ?? "a command",
                  reason: str(p.reason) ?? "Guard refused it",
                  at: e.createdAt,
                },
              ],
            }
          : t,
      );
    }
  }
  return tabs;
}

/** The watch tabs after several events, oldest first. */
export function applyWatchEvents(tabs: WatchTab[], events: readonly LedgerEvent[]): WatchTab[] {
  return [...events].sort((a, b) => a.seq - b.seq).reduce(applyWatchEvent, tabs);
}

/** "Operations Engineer · Shop". */
export function watchTitle(tab: WatchTab): string {
  return `${tab.worker} · ${tab.server}`;
}

/** The command running now in a tab, if any (Stop acts on it). */
export function runningCommand(tab: WatchTab): WatchCommand | null {
  for (let i = tab.items.length - 1; i >= 0; i--) {
    const it = tab.items[i];
    if (it?.kind === "command" && it.state !== "finished") return it;
  }
  return null;
}

/** How a finished command ended, in plain words. */
export function endingWords(c: WatchCommand): string {
  const secs = c.seconds !== null ? ` after ${c.seconds.toFixed(1)} s` : "";
  switch (c.ending) {
    case "exited":
      return c.exitCode === 0
        ? `Finished${secs}`
        : `Ended with exit code ${c.exitCode ?? "?"}${secs}`;
    case "stopped":
      return `Stopped (${c.why ?? "by Plenipo"})`;
    case "timedOut":
      return "Stopped: it ran past its time limit";
    case "connectionLost":
      return "The connection was lost while it ran";
    case "signal":
      return "Ended by a signal on the server";
    case "refused":
      return `The server refused it${c.why ? ` (${c.why})` : ""}`;
    default:
      return "Ended";
  }
}

/** The lines a watch tab shows: each command, its output, and how it ended; each refusal. */
export function watchLines(tab: WatchTab): LogLine[] {
  const lines: LogLine[] = [];
  for (const item of tab.items) {
    if (item.kind === "refused") {
      lines.push({
        id: item.id,
        text: `Refused: ${item.summary} — ${item.reason}`,
        tone: "warn",
      });
      continue;
    }
    const c = item;
    lines.push({
      id: `${c.commandId}:cmd`,
      text: `$ ${c.command}${c.cwd ? `   (in ${c.cwd})` : ""}`,
      tone: "command",
    });
    if (c.dropped > 0) {
      lines.push({
        id: `${c.commandId}:dropped`,
        text: `… ${c.dropped} earlier lines are in the Activity trail`,
        tone: "muted",
      });
    }
    c.lines.forEach((l, i) =>
      lines.push({
        id: `${c.commandId}:${c.dropped + i}`,
        text: l.text,
        tone: l.stream === "err" ? "error" : "normal",
      }),
    );
    // Past its first 2,000 lines, a command's output is counted, not kept: say so.
    const shown = c.dropped + c.lines.length;
    const notKept = c.total !== null ? Math.max(0, c.total - shown) : 0;
    if (notKept > 0) {
      lines.push({
        id: `${c.commandId}:not-kept`,
        text: `… ${notKept} more lines were not kept (Plenipo keeps the first ${RECORDED_COMMAND_LINES} lines of a command)`,
        tone: "muted",
      });
    } else if (c.state !== "finished" && shown >= RECORDED_COMMAND_LINES) {
      lines.push({
        id: `${c.commandId}:not-kept`,
        text: `… later lines are not shown (Plenipo keeps the first ${RECORDED_COMMAND_LINES} lines of a command)`,
        tone: "muted",
      });
    }
    if (c.state === "stopping") {
      lines.push({ id: `${c.commandId}:stopping`, text: "Stopping…", tone: "muted" });
    } else if (c.state === "finished") {
      const ok = c.ending === "exited" && c.exitCode === 0;
      lines.push({
        id: `${c.commandId}:end`,
        text: endingWords(c),
        tone: ok ? "ok" : c.ending === "stopped" ? "muted" : "error",
      });
    }
  }
  if (!tab.connected && tab.disconnected) {
    lines.push({ id: "disconnected", text: `Disconnected: ${tab.disconnected}`, tone: "muted" });
  }
  return lines;
}
