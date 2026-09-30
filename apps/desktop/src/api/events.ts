// Typed subscription to events streamed from Plenipo Core.
// This is the ONLY module that calls `listen` (enforced by ESLint).

import { listen } from "@tauri-apps/api/event";
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

export const RUNTIME_EVENT = "plenipo://runtime";
export const LEDGER_EVENT = "plenipo://ledger";
export const AGENT_EVENT = "plenipo://agents";
export const CONTROL_EVENT = "plenipo://control";
export const WINDOWS_EVENT = "plenipo://windows";
export const DROP_EVENT = "plenipo://drop";

/** Subscribe to runtime events. Resolves with an unsubscribe function. */
export async function subscribeRuntimeEvents(
  handler: (event: RuntimeEvent) => void,
): Promise<() => void> {
  return listen<RuntimeEvent>(RUNTIME_EVENT, (event) => handler(event.payload));
}

/** Subscribe to committed ledger events. Resolves with an unsubscribe function. */
export async function subscribeLedgerEvents(
  handler: (event: LedgerEvent) => void,
): Promise<() => void> {
  return listen<LedgerEvent>(LEDGER_EVENT, (event) => handler(event.payload));
}

/** Subscribe to agent runtime updates (activity, turns, sessions, runtimes). */
export async function subscribeAgentUpdates(
  handler: (update: AgentUpdate) => void,
): Promise<() => void> {
  return listen<AgentUpdate>(AGENT_EVENT, (event) => handler(event.payload));
}

/** Subscribe to changes of browser and desktop control (Phase 10). */
export async function subscribeControl(
  handler: (status: ControlStatus) => void,
): Promise<() => void> {
  return listen<ControlStatus>(CONTROL_EVENT, (event) => handler(event.payload));
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
  return listen<PopOutNotice>(WINDOWS_EVENT, (event) => handler(event.payload));
}

/**
 * Subscribe to files dropped on this window from File Explorer (Phase 21, ADR-093 §20): their
 * names, where they landed, and Plenipo's ticket for them (never where they are on the PC).
 */
export async function subscribeDrops(
  handler: (dropped: DroppedFiles) => void,
): Promise<() => void> {
  return listen<DroppedFiles>(DROP_EVENT, (event) => handler(event.payload));
}
