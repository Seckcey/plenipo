import { createContext } from "react";

import type { AgentSessionDetail } from "@plenipo/types";

import type { AgentState } from "./store";

export interface AgentsContextValue {
  state: AgentState;
  reload: () => Promise<void>;
  /** Re-detect installation and sign-in of every runtime. */
  refresh: () => Promise<void>;
  /** Fetch a session again; resolves with it, as Plenipo has it now. */
  loadSession: (sessionId: string) => Promise<AgentSessionDetail>;
  /** Start a session; resolves with its ID. With `handoffs`, the worker may ask others for help. */
  start: (
    runtimeId: string,
    objective: string,
    model?: string,
    handoffs?: boolean,
  ) => Promise<string>;
  resume: (sessionId: string, objective: string) => Promise<void>;
  /** Stop the session's turn; resolves with the session once the stop is recorded. */
  cancel: (sessionId: string) => Promise<AgentSessionDetail>;
  close: (sessionId: string) => Promise<void>;
}

export const AgentsContext = createContext<AgentsContextValue | null>(null);
