import { useCallback, useEffect, useRef, useState } from "react";
import type { RoutingSnapshot } from "@plenipo/types";

import { getRouting, toCommandError } from "../api/commands";
import { subscribeAgentUpdates, subscribeLedgerEvents } from "../api/events";

/** Ledger events after which the model settings may look different: the settings themselves,
 * new roles, and turn results (usage limits, models seen in use). */
export function affectsRouting(eventType: string): boolean {
  return (
    eventType.startsWith("router.") ||
    eventType === "org.role_created" ||
    eventType === "org.role_renamed" ||
    eventType === "agent.result"
  );
}

/**
 * The model settings (Router snapshot), kept live like the organization: reloaded (debounced)
 * after relevant Ledger events and AI tool changes; a change made here applies the snapshot its
 * command returns.
 */
export function useRouting() {
  const [snapshot, setSnapshot] = useState<RoutingSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      setSnapshot(await getRouting());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
  }, []);

  const apply = useCallback((next: RoutingSnapshot) => {
    setSnapshot(next);
    setError(null);
  }, []);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const schedule = () => {
      if (disposed) return;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 150);
    };
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    void Promise.allSettled([
      subscribeLedgerEvents((event) => {
        if (affectsRouting(event.eventType)) schedule();
      }).then(keep),
      subscribeAgentUpdates((update) => {
        if (update.kind === "runtimes") schedule();
      }).then(keep),
    ]).finally(() => {
      if (!disposed) void reload();
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload]);

  return { snapshot, error, reload, apply };
}
