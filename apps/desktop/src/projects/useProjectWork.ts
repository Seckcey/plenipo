import { useCallback, useEffect, useRef, useState } from "react";
import type { ObjectiveReport, ProjectWork } from "@plenipo/types";

import { getObjectiveReport, getProjectWork, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { Newest } from "../api/newest";

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
 * changes and reloaded (debounced) after relevant Ledger events; a change made here applies what
 * its command returns, and is followed by one more reload. Only the newest answer shows
 * ({@link Newest}). `null` ID: nothing.
 */
function useLiveFor<T>(id: string | null, load: (id: string) => Promise<T>) {
  const [state, setState] = useState<{ id: string | null; value: T | null; error: string | null }>({
    id: null,
    value: null,
    error: null,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const live = useRef(false);
  // The ID shown now: a load for an earlier one that finishes late is dropped.
  const latest = useRef(id);
  useEffect(() => {
    latest.current = id;
  }, [id]);
  const [order] = useState(() => new Newest());

  const reload = useCallback(async () => {
    if (id === null) return;
    const newest = order.start();
    try {
      const value = await load(id);
      if (latest.current === id && newest.take()) setState({ id, value, error: null });
    } catch (reason) {
      const error = toCommandError(reason).message;
      if (latest.current === id && newest.fresh()) {
        setState((s) => ({ id, value: s.id === id ? s.value : null, error }));
      }
    }
  }, [id, load, order]);

  /** Reload in a moment (several asks in a row make one reload). */
  const schedule = useCallback(() => {
    if (!live.current) return;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => void reload(), 150);
  }, [reload]);

  const apply = useCallback(
    (value: T) => {
      if (id === null) return;
      order.applied();
      setState({ id, value, error: null });
      // What a command returns was read before its answer travelled back: one more reload,
      // newer than the change, brings in anything that came meanwhile.
      schedule();
    },
    [id, order, schedule],
  );

  useEffect(() => {
    if (id === null) return;
    let disposed = false;
    let stop: (() => void) | null = null;
    live.current = true;
    void subscribeLedgerEvents((event) => {
      if (!disposed && affectsProjectWork(event.eventType)) schedule();
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
      live.current = false;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
    };
  }, [id, reload, schedule]);

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
