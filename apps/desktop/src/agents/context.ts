import { createContext } from "react";

import type { AgentState } from "./store";

export interface AgentsContextValue {
  state: AgentState;
  reload: () => Promise<void>;
  /** Re-detect installation and sign-in of every runtime. */
  refresh: () => Promise<void>;
  loadSession: (sessionId: string) => Promise<void>;
  /** Start a session; resolves with its ID. */
  start: (runtimeId: string, objective: string, model?: string) => Promise<string>;
  resume: (sessionId: string, objective: string) => Promise<void>;
  cancel: (sessionId: string) => Promise<void>;
  close: (sessionId: string) => Promise<void>;
}

export const AgentsContext = createContext<AgentsContextValue | null>(null);
