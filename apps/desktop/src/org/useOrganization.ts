import { useCallback, useEffect, useRef, useState } from "react";
import type { OrgSnapshot } from "@plenipo/types";

import { getOrganization, toCommandError } from "../api/commands";
import { subscribeAgentUpdates, subscribeLedgerEvents } from "../api/events";

/** Ledger events after which the organization may look different. */
export function affectsOrganization(eventType: string): boolean {
  return (
    eventType.startsWith("org.") ||
    eventType.startsWith("task.") ||
    eventType.startsWith("liaison.") ||
    eventType.startsWith("session.")
  );
}

/**
 * The organization snapshot, kept live: Core owns the organization, so this reloads (debounced)
 * whenever the Ledger commits something that changes it, or runtime readiness changes. Changes
 * made from this window apply the snapshot their command returns.
 */
export function useOrganization() {
  const [snapshot, setSnapshot] = useState<OrgSnapshot | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  /** Increments on every change, so views can refresh dependent data (a position's work). */
  const [revision, setRevision] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const reload = useCallback(async () => {
    try {
      const next = await getOrganization();
      setSnapshot(next);
      setStatus("ready");
      setError(null);
    } catch (reason) {
      setStatus((s) => (s === "ready" ? s : "error"));
      setError(toCommandError(reason).message);
    }
  }, []);

  const apply = useCallback((next: OrgSnapshot) => {
    setSnapshot(next);
    setStatus("ready");
    setError(null);
    setRevision((r) => r + 1);
  }, []);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const schedule = () => {
      if (disposed) return;
      setRevision((r) => r + 1);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => void reload(), 150);
    };
    const keep = (stop: () => void) => {
      if (disposed) stop();
      else stops.push(stop);
    };
    // Subscribe before fetching so nothing is missed between the snapshot and the stream.
    void Promise.allSettled([
      subscribeLedgerEvents((event) => {
        if (affectsOrganization(event.eventType)) schedule();
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

  return { snapshot, status, error, revision, reload, apply };
}
