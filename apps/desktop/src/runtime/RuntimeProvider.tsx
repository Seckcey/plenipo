import { useCallback, useEffect, useMemo, useReducer, type ReactNode } from "react";

import {
  cancelExecution,
  getExecutionOutput,
  getRuntimeOverview,
  startExecution,
  toCommandError,
} from "../api/commands";
import { subscribeRuntimeEvents } from "../api/events";
import { RuntimeContext } from "./context";
import { initialRuntimeState, isActive, runtimeReducer } from "./store";

/**
 * Owns runtime state for the whole shell so it survives navigation between views, and
 * rebuilds it from the backend on mount (e.g. after the webview reloads).
 */
export function RuntimeProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(runtimeReducer, initialRuntimeState);

  const loadOutput = useCallback(async (executionId: string) => {
    try {
      const output = await getExecutionOutput(executionId);
      dispatch({ type: "outputLoaded", output });
    } catch {
      // Output is best-effort; the record itself is still shown.
    }
  }, []);

  const reload = useCallback(async () => {
    try {
      const overview = await getRuntimeOverview();
      dispatch({ type: "overviewLoaded", overview });
      // Restore live output for anything still running.
      await Promise.all(overview.executions.filter(isActive).map((r) => loadOutput(r.id)));
    } catch (reason) {
      dispatch({ type: "loadFailed", message: toCommandError(reason).message });
    }
  }, [loadOutput]);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    // Subscribe before fetching so nothing is missed between snapshot and stream.
    subscribeRuntimeEvents((event) => {
      if (!disposed) dispatch({ type: "event", event, at: Date.now() });
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

  const start = useCallback(async (profileId: string) => {
    const record = await startExecution(profileId);
    dispatch({ type: "recordUpdated", record });
    return record.id;
  }, []);

  const cancel = useCallback(async (executionId: string) => {
    const record = await cancelExecution(executionId);
    dispatch({ type: "recordUpdated", record });
  }, []);

  const value = useMemo(
    () => ({ state, start, cancel, loadOutput, reload }),
    [state, start, cancel, loadOutput, reload],
  );
  return <RuntimeContext.Provider value={value}>{children}</RuntimeContext.Provider>;
}
