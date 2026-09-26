import { useCallback, useEffect, useMemo, useReducer, type ReactNode } from "react";

import {
  cancelAgentTurn,
  closeAgentSession,
  getAgentOverview,
  getAgentSession,
  refreshAgentRuntimes,
  resumeAgentSession,
  startAgentSession,
  toCommandError,
} from "../api/commands";
import { subscribeAgentUpdates } from "../api/events";
import { AgentsContext } from "./context";
import { agentReducer, initialAgentState } from "./store";

/**
 * Owns agent runtime and session state for the whole shell so it survives navigation, and
 * rebuilds it from the backend on mount (e.g. after the webview reloads).
 */
export function AgentsProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(agentReducer, initialAgentState);

  const reload = useCallback(async () => {
    try {
      dispatch({ type: "overviewLoaded", overview: await getAgentOverview() });
    } catch (reason) {
      dispatch({ type: "loadFailed", message: toCommandError(reason).message });
    }
  }, []);

  const loadSession = useCallback(async (sessionId: string) => {
    dispatch({ type: "sessionLoaded", detail: await getAgentSession(sessionId) });
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    // Subscribe before fetching so nothing is missed between snapshot and stream.
    subscribeAgentUpdates((update) => {
      if (!disposed) dispatch({ type: "update", update });
    })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!disposed) void reload();
      });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [reload]);

  const refresh = useCallback(async () => {
    dispatch({ type: "runtimesLoaded", runtimes: await refreshAgentRuntimes() });
  }, []);

  const start = useCallback(async (runtimeId: string, objective: string, model?: string) => {
    const detail = await startAgentSession(runtimeId, objective, model);
    dispatch({ type: "sessionLoaded", detail });
    return detail.session.id;
  }, []);

  const resume = useCallback(async (sessionId: string, objective: string) => {
    dispatch({ type: "sessionLoaded", detail: await resumeAgentSession(sessionId, objective) });
  }, []);

  const cancel = useCallback(async (sessionId: string) => {
    dispatch({ type: "sessionLoaded", detail: await cancelAgentTurn(sessionId) });
  }, []);

  const close = useCallback(async (sessionId: string) => {
    const session = await closeAgentSession(sessionId);
    dispatch({ type: "update", update: { kind: "session", ...session } });
  }, []);

  const value = useMemo(
    () => ({ state, reload, refresh, loadSession, start, resume, cancel, close }),
    [state, reload, refresh, loadSession, start, resume, cancel, close],
  );
  return <AgentsContext.Provider value={value}>{children}</AgentsContext.Provider>;
}
