import { useContext } from "react";

import { TerminalContext, type TerminalApi } from "./context";

/** The terminal panel and its tabs (from `TerminalProvider`). */
export function useTerminal(): TerminalApi {
  const api = useContext(TerminalContext);
  if (!api) throw new Error("useTerminal needs a TerminalProvider");
  return api;
}
