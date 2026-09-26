import { useContext } from "react";

import { AgentsContext, type AgentsContextValue } from "./context";

export function useAgents(): AgentsContextValue {
  const value = useContext(AgentsContext);
  if (!value) throw new Error("useAgents must be used inside <AgentsProvider>");
  return value;
}
