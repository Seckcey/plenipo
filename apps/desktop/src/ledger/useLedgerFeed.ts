import { useCallback, useEffect, useRef, useState } from "react";
import type { LedgerEvent, Task } from "@plenipo/types";

import { listRecentEvents, listTasks, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

const MAX_EVENTS = 200;

/**
 * Tasks and recent events from the ledger, kept live by ledger events. The ledger is the
 * source of truth, so this simply reloads (debounced) whenever something is committed.
 */
export function useLedgerFeed() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [events, setEvents] = useState<LedgerEvent[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  /** Increments on every committed ledger event (lets views refresh dependent data). */
  const [revision, setRevision] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      const [t, e] = await Promise.all([listTasks(), listRecentEvents()]);
      setTasks(t);
      setEvents(e);
      setStatus("ready");
      setError(null);
    } catch (reason) {
      setStatus("error");
      setError(toCommandError(reason).message);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    subscribeLedgerEvents((event) => {
      if (disposed) return;
      setEvents((prev) =>
        prev.some((e) => e.seq === event.seq) ? prev : [event, ...prev].slice(0, MAX_EVENTS),
      );
      setRevision((r) => r + 1);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 150);
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
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload]);

  return { tasks, events, status, error, revision, reload };
}
