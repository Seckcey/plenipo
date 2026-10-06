import { useContext } from "react";

import { AgentsContext, type AgentsContextValue } from "./context";

export function useAgents(): AgentsContextValue {
  const value = useContext(AgentsContext);
  if (!value) throw new Error("useAgents must be used inside <AgentsProvider>");
  return value;
}

/** The agents' store, where there is one (a page drawn without it, as in a test, has none). */
export function useAgentsIfAny(): AgentsContextValue | null {
  return useContext(AgentsContext);
}
