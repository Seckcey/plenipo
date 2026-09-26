// Typed subscription to events streamed from Plenipo Core.
// This is the ONLY module that calls `listen` (enforced by ESLint).

import { listen } from "@tauri-apps/api/event";
import type { AgentUpdate, LedgerEvent, RuntimeEvent } from "@plenipo/types";

export const RUNTIME_EVENT = "plenipo://runtime";
export const LEDGER_EVENT = "plenipo://ledger";
export const AGENT_EVENT = "plenipo://agents";

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
