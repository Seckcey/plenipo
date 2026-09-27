import { useCallback, useEffect, useRef, useState } from "react";
import type { ObjectiveReport, ProjectWork } from "@plenipo/types";

import { getObjectiveReport, getProjectWork, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/** Ledger events after which a project's work, or an objective's result, may look different. */
export function affectsProjectWork(eventType: string): boolean {
  return (
    eventType.startsWith("task.") ||
    eventType.startsWith("liaison.") ||
    eventType.startsWith("workspace.") ||
    eventType.startsWith("approval.") ||
    eventType.startsWith("session.") ||
    eventType === "capability.used" ||
    eventType === "agent.result"
  );
}

/**
 * A value Core builds for one ID (a project, an objective), kept live: loaded when the ID
 * changes and reloaded (debounced) after relevant Ledger events. `null` ID: nothing.
 */
function useLiveFor<T>(id: string | null, load: (id: string) => Promise<T>) {
  const [state, setState] = useState<{ id: string | null; value: T | null; error: string | null }>({
    id: null,
    value: null,
    error: null,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // The ID shown now: a load for an earlier one that finishes late is dropped.
  const latest = useRef(id);
  useEffect(() => {
    latest.current = id;
  }, [id]);

  const reload = useCallback(async () => {
    if (id === null) return;
    try {
      const value = await load(id);
      if (latest.current === id) setState({ id, value, error: null });
    } catch (reason) {
      const error = toCommandError(reason).message;
      if (latest.current === id) {
        setState((s) => ({ id, value: s.id === id ? s.value : null, error }));
      }
    }
  }, [id, load]);

  const apply = useCallback(
    (value: T) => {
      if (id !== null) setState({ id, value, error: null });
    },
    [id],
  );

  useEffect(() => {
    if (id === null) return;
    let disposed = false;
    let stop: (() => void) | null = null;
    const schedule = () => {
      if (disposed) return;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 150);
    };
    void subscribeLedgerEvents((event) => {
      if (affectsProjectWork(event.eventType)) schedule();
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!disposed) void reload();
      });
    return () => {
      disposed = true;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
    };
  }, [id, reload]);

  const current = state.id === id;
  return {
    value: current ? state.value : null,
    error: current ? state.error : null,
    reload,
    apply,
  };
}

/** A project's objectives and working copies, kept live. */
export function useProjectWork(projectId: string | null) {
  const { value, error, reload, apply } = useLiveFor<ProjectWork>(projectId, getProjectWork);
  return { work: value, error, reload, apply };
}

/** Plenipo's result for an objective, kept live while its team works. */
export function useObjectiveReport(taskId: string | null) {
  const { value, error, reload } = useLiveFor<ObjectiveReport>(taskId, getObjectiveReport);
  return { report: value, error, reload };
}
