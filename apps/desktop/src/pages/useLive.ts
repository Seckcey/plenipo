import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { LedgerEvent } from "@plenipo/types";

import { toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { Newest } from "../api/newest";

/** A page's data: loading, couldn't load, or ready (with the last refresh's error, if any). */
export interface Live<T> {
  status: "loading" | "error" | "ready";
  value: T | null;
  error: string | null;
  reload: () => void;
}

/**
 * What Core says about one thing (a department, project, worker, or task), kept live: loaded
 * when `key` changes and reloaded, at most every `wait` ms, after the Ledger events that may
 * change it. An answer for an earlier key that arrives late is dropped, and so is an older
 * answer for the same key ({@link Newest}). `key` null: nothing.
 */
export function useLive<T>(
  key: string | null,
  load: (key: string) => Promise<T>,
  relevant: (event: LedgerEvent) => boolean,
  wait = 400,
): Live<T> {
  const [state, setState] = useState<{ key: string | null; value: T | null; error: string | null }>(
    { key: null, value: null, error: null },
  );
  // The newest `load` and `relevant` (callers often pass new functions on every render).
  const loadRef = useRef(load);
  const relevantRef = useRef(relevant);
  useLayoutEffect(() => {
    loadRef.current = load;
    relevantRef.current = relevant;
  });
  // The key shown now: a load for an earlier one that finishes late is dropped.
  const latest = useRef(key);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [order] = useState(() => new Newest());

  const fetchNow = useCallback(async () => {
    if (key === null) return;
    const newest = order.start();
    try {
      const value = await loadRef.current(key);
      if (latest.current === key && newest.take()) setState({ key, value, error: null });
    } catch (reason) {
      const error = toCommandError(reason).message;
      if (latest.current === key && newest.fresh()) {
        setState((s) => ({ key, value: s.key === key ? s.value : null, error }));
      }
    }
  }, [key, order]);

  useEffect(() => {
    latest.current = key;
    if (key === null) return;
    let disposed = false;
    let stop: (() => void) | null = null;
    void subscribeLedgerEvents((event) => {
      if (disposed || timer.current || !relevantRef.current(event)) return;
      timer.current = setTimeout(() => {
        timer.current = null;
        if (!disposed) void fetchNow();
      }, wait);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      // Subscribe first, so nothing between the answer and the stream is missed.
      .finally(() => {
        if (!disposed) void fetchNow();
      });
    return () => {
      disposed = true;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
      timer.current = null;
    };
  }, [key, fetchNow, wait]);

  const reload = useCallback(() => void fetchNow(), [fetchNow]);
  if (key === null) return { status: "ready", value: null, error: null, reload };
  if (state.key !== key) return { status: "loading", value: null, error: null, reload };
  if (state.value === null) {
    return { status: state.error ? "error" : "loading", value: null, error: state.error, reload };
  }
  return { status: "ready", value: state.value, error: state.error, reload };
}

/** Ledger events that change what a piece of work looks like (its tasks, approvals, results). */
export function changesWork(event: LedgerEvent): boolean {
  const t = event.eventType;
  return (
    t.startsWith("task.") ||
    t.startsWith("approval.") ||
    t.startsWith("liaison.") ||
    t.startsWith("session.") ||
    t.startsWith("workspace.") ||
    t.startsWith("lesson.") ||
    t.startsWith("guard.") ||
    t.startsWith("ssh.") ||
    t.startsWith("control.") ||
    t === "capability.used" ||
    t === "agent.result" ||
    t === "artifact.recorded" ||
    // A lead needs a worker, and a hire answers it (Phase 25, item 2.7).
    t === "org.hire_needed" ||
    t === "org.position_created"
  );
}

/** Every Ledger event (history lists show them all). */
export const anyEvent = () => true;
