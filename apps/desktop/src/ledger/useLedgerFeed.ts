import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { LedgerEvent, Task } from "@plenipo/types";

import { getScopeEvents, listRecentEvents, listTasks, toCommandError } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

const MAX_EVENTS = 200;
/** Events read at a time with "Show older events". */
const OLDER_PAGE = 200;

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

  // Older events, read a page at a time when asked. They are kept with the live ones shown when
  // they were read, so new events pushing old ones out of the live list leave no gap.
  const [older, setOlder] = useState<LedgerEvent[]>([]);
  const [olderLeft, setOlderLeft] = useState(true);
  const shown = useMemo(() => {
    if (older.length === 0) return events;
    const seen = new Set<number>();
    return [...events, ...older]
      .filter((e) => (seen.has(e.seq) ? false : (seen.add(e.seq), true)))
      .sort((a, b) => b.seq - a.seq);
  }, [events, older]);
  const loadOlder = useCallback(async () => {
    const before = shown.at(-1)?.seq;
    if (before === undefined) return;
    try {
      const page = await getScopeEvents({ kind: "all" }, OLDER_PAGE, before);
      setOlder((prev) => [...prev, ...events, ...page]);
      if (page.length < OLDER_PAGE) setOlderLeft(false);
    } catch (reason) {
      setStatus("error");
      setError(toCommandError(reason).message);
    }
  }, [shown, events]);
  /** There may be older events than those shown (the live list is full, and more are left). */
  const hasOlder = olderLeft && shown.length >= MAX_EVENTS;

  return { tasks, events: shown, status, error, revision, reload, loadOlder, hasOlder };
}
