import { useContext } from "react";

import { TerminalContext, type TerminalApi } from "./context";

/** The terminal panel and its tabs (from `TerminalProvider`). */
export function useTerminal(): TerminalApi {
  const api = useContext(TerminalContext);
  if (!api) throw new Error("useTerminal needs a TerminalProvider");
  return api;
}

/**
 * Watch an agent write code: opens (or goes to) its Watch tab in the terminal panel. `null`
 * where there is no terminal panel (a page shown on its own, as in some tests): hide the Watch
 * button then.
 */
export function useOpenWatch(): ((positionId: string, title: string) => void) | null {
  return useContext(TerminalContext)?.openWatch ?? null;
}
