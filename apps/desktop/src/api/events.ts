// Typed subscription to events streamed from Plenipo Core.
// This is the ONLY module that calls `listen` (enforced by ESLint).

import { listen } from "@tauri-apps/api/event";
import type { RuntimeEvent } from "@plenipo/types";

export const RUNTIME_EVENT = "plenipo://runtime";

/** Subscribe to runtime events. Resolves with an unsubscribe function. */
export async function subscribeRuntimeEvents(
  handler: (event: RuntimeEvent) => void,
): Promise<() => void> {
  return listen<RuntimeEvent>(RUNTIME_EVENT, (event) => handler(event.payload));
}
