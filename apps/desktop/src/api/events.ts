// Typed subscription to events streamed from Plenipo Core.
// This is the ONLY module that calls `listen` (enforced by ESLint).

import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type {
  AgentUpdate,
  DroppedFiles,
  ControlStatus,
  LedgerEvent,
  PopOutNotice,
  RuntimeEvent,
  WatchUpdate,
} from "@plenipo/types";

import { watchChanges } from "./commands";

/**
 * Listen for `event` sent to this window, or to every window (Phase 21, ADR-094 §12). Never to
 * another window: Tauri gives an event sent to one window to any page listening for every
 * window's, and another window may show another organization.
 */
function listenHere<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  let label: string | null = null;
  try {
    label = getCurrentWebviewWindow().label;
  } catch {
    // Not in a Plenipo window (tests): every window's.
  }
  return label === null
    ? listen<T>(event, handler)
    : listen<T>(event, handler, { target: { kind: "WebviewWindow", label } });
}

export const RUNTIME_EVENT = "plenipo://runtime";
export const LEDGER_EVENT = "plenipo://ledger";
export const AGENT_EVENT = "plenipo://agents";
export const CONTROL_EVENT = "plenipo://control";
export const WINDOWS_EVENT = "plenipo://windows";
export const DROP_EVENT = "plenipo://drop";
export const ORGANIZATIONS_EVENT = "plenipo://organizations";
/** Your tile or your Workforce changed: they are shared by every organization (Phase 21). */
export const SHARED_EVENT = "plenipo://shared";
/** The PC's license changed: Free or Pro, a key entered or removed, or a check (Phase 11A). */
export const LICENSE_EVENT = "plenipo://license";
/** Phone access changed (Phase 14): a phone added, signed in, or removed; the switch; pairing. */
export const REMOTE_EVENT = "plenipo://remote";

/** Subscribe to runtime events. Resolves with an unsubscribe function. */
export async function subscribeRuntimeEvents(
  handler: (event: RuntimeEvent) => void,
): Promise<() => void> {
  return listenHere<RuntimeEvent>(RUNTIME_EVENT, (event) => handler(event.payload));
}

/** Subscribe to committed ledger events. Resolves with an unsubscribe function. */
export async function subscribeLedgerEvents(
  handler: (event: LedgerEvent) => void,
): Promise<() => void> {
  return listenHere<LedgerEvent>(LEDGER_EVENT, (event) => handler(event.payload));
}

/** Subscribe to agent runtime updates (activity, turns, sessions, runtimes). */
export async function subscribeAgentUpdates(
  handler: (update: AgentUpdate) => void,
): Promise<() => void> {
  return listenHere<AgentUpdate>(AGENT_EVENT, (event) => handler(event.payload));
}

/** Subscribe to changes of browser and desktop control (Phase 10). */
export async function subscribeControl(
  handler: (status: ControlStatus) => void,
): Promise<() => void> {
  return listenHere<ControlStatus>(CONTROL_EVENT, (event) => handler(event.payload));
}

/**
 * Subscribe to Watch (Phase 18): each file change a worker is writing, saved, or refused. It
 * comes through the main window's own channel, not an event (ADR-055).
 */
export async function subscribeWatch(handler: (update: WatchUpdate) => void): Promise<() => void> {
  return watchChanges(handler);
}

/**
 * Subscribe to what happens to this window's popped-out panels (Phase 21, ADR-092): a pop-out
 * window closed, so its panel goes back to a dock.
 */
export async function subscribePopOuts(
  handler: (notice: PopOutNotice) => void,
): Promise<() => void> {
  return listenHere<PopOutNotice>(WINDOWS_EVENT, (event) => handler(event.payload));
}

/**
 * Subscribe to files dropped on this window from File Explorer (Phase 21, ADR-093 §20): their
 * names, where they landed, and Plenipo's ticket for them (never where they are on the PC).
 */
export async function subscribeDrops(
  handler: (dropped: DroppedFiles) => void,
): Promise<() => void> {
  return listenHere<DroppedFiles>(DROP_EVENT, (event) => handler(event.payload));
}

/**
 * Subscribe to changes in your list of organizations (one made, renamed, opened in a window,
 * archived, or deleted; Phase 21, ADR-094). The list itself is read with `getOrganizations`.
 */
export async function subscribeOrganizations(handler: () => void): Promise<() => void> {
  return listenHere<null>(ORGANIZATIONS_EVENT, () => handler());
}

/**
 * Subscribe to changes in what every organization shares (Phase 21, ADR-094 §5): your tile, or
 * your Workforce (an agent saved, hired, deleted, or moved), from any organization's window.
 */
export async function subscribeShared(
  handler: (what: "tile" | "workforce") => void,
): Promise<() => void> {
  return listenHere<"tile" | "workforce">(SHARED_EVENT, (event) => handler(event.payload));
}

/**
 * Subscribe to changes in the PC's license (Phase 11A): a key entered or removed, a check with 8
 * West, or Pro starting or ending. The license itself is read with `getLicense`.
 */
export async function subscribeLicense(handler: () => void): Promise<() => void> {
  return listenHere<null>(LICENSE_EVENT, () => handler());
}

/** Phone access changed (Settings → Devices reads it again). */
export async function subscribeRemote(handler: () => void): Promise<() => void> {
  return listenHere<string>(REMOTE_EVENT, () => handler());
}
