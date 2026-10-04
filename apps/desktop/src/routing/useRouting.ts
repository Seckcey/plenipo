import { useCallback, useEffect, useRef, useState } from "react";
import type { RoutingSnapshot } from "@plenipo/types";

import { getRouting, toCommandError } from "../api/commands";
import { subscribeAgentUpdates, subscribeLedgerEvents } from "../api/events";
import { Newest } from "../api/newest";

/** Ledger events after which the model settings may look different: the settings themselves,
 * new roles, and turn results (usage limits, models seen in use). */
export function affectsRouting(eventType: string): boolean {
  return (
    eventType.startsWith("router.") ||
    eventType === "org.role_created" ||
    eventType === "org.role_renamed" ||
    eventType === "org.role_updated" ||
    eventType === "agent.result"
  );
}

/**
 * The model settings (Router snapshot), kept live like the organization: reloaded (debounced)
 * after relevant Ledger events and AI tool changes; a change made here applies the snapshot its
 * command returns, and is followed by one more reload. Only the newest answer shows
 * ({@link Newest}).
 */
export function useRouting() {
  const [snapshot, setSnapshot] = useState<RoutingSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const live = useRef(false);
  const [order] = useState(() => new Newest());

  const reload = useCallback(async () => {
    const newest = order.start();
    try {
      const next = await getRouting();
      if (!newest.take()) return;
      setSnapshot(next);
      setError(null);
    } catch (reason) {
      if (newest.fresh()) setError(toCommandError(reason).message);
    }
  }, [order]);

  /** Reload in a moment (several asks in a row make one reload). */
  const schedule = useCallback(() => {
    if (!live.current) return;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => void reload(), 150);
  }, [reload]);

  const apply = useCallback(
    (next: RoutingSnapshot) => {
      order.applied();
      setSnapshot(next);
      setError(null);
      // What a command returns was read before its answer travelled back: one more reload,
      // newer than the change, brings in anything that came meanwhile.
      schedule();
    },
    [order, schedule],
  );

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    live.current = true;
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    void Promise.allSettled([
      subscribeLedgerEvents((event) => {
        if (!disposed && affectsRouting(event.eventType)) schedule();
      }).then(keep),
      subscribeAgentUpdates((update) => {
        if (!disposed && update.kind === "runtimes") schedule();
      }).then(keep),
    ]).finally(() => {
      if (!disposed) void reload();
    });
    return () => {
      disposed = true;
      live.current = false;
      stops.forEach((stop) => stop());
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload, schedule]);

  return { snapshot, error, reload, apply };
}

/**
 * The model settings loaded once, for a form that offers model choices outside Settings (a hire,
 * a position's AI model, a new conversation). `null` until loaded, or if they cannot be read: the
 * form then offers only the AI tool's default and a typed name.
 */
export function useRoutingOnce(): RoutingSnapshot | null {
  const [snapshot, setSnapshot] = useState<RoutingSnapshot | null>(null);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(getRouting)
      .then(
        (s) => {
          if (live) setSnapshot(s);
        },
        () => undefined,
      );
    return () => {
      live = false;
    };
  }, []);
  return snapshot;
}
