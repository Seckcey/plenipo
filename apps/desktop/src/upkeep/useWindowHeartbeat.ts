import { useEffect } from "react";

import { windowAlive } from "../api/commands";

/** How often the page tells Plenipo it is alive. */
export const HEARTBEAT_MS = 5_000;

/**
 * The window's page tells Plenipo every few seconds that it is alive, and whether it can be
 * seen (Phase 13, ADR-036 item 3). When it goes quiet while showing, Plenipo reloads it, or
 * opens the window again.
 */
export function useWindowHeartbeat() {
  useEffect(() => {
    const beat = () => {
      windowAlive(document.visibilityState === "visible").catch(() => undefined);
    };
    beat();
    const timer = setInterval(beat, HEARTBEAT_MS);
    document.addEventListener("visibilitychange", beat);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", beat);
    };
  }, []);
}
