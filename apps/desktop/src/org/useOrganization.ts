import { useCallback, useEffect, useRef, useState } from "react";
import type { OrgSnapshot } from "@plenipo/types";

import { getOrganization, toCommandError } from "../api/commands";
import { subscribeAgentUpdates, subscribeLedgerEvents, subscribeShared } from "../api/events";
import { Newest } from "../api/newest";

/** Ledger events after which the organization may look different (model choices and usage
 * limits change where each position's next worker would go). */
export function affectsOrganization(eventType: string): boolean {
  return (
    eventType.startsWith("org.") ||
    eventType.startsWith("task.") ||
    eventType.startsWith("liaison.") ||
    eventType.startsWith("session.") ||
    eventType.startsWith("router.") ||
    // Learning in layers and kept lessons (experience) show on each agent (Phase 17).
    eventType.startsWith("learning.") ||
    eventType.startsWith("lesson.") ||
    eventType === "agent.result"
  );
}

/**
 * The organization snapshot, kept live: Core owns the organization, so this reloads (debounced)
 * whenever the Ledger commits something that changes it, or runtime readiness changes. Changes
 * made from this window apply the snapshot their command returns, and are followed by one more
 * reload. Only the newest answer shows ({@link Newest}).
 */
export function useOrganization() {
  const [snapshot, setSnapshot] = useState<OrgSnapshot | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  /** Increments on every change, so views can refresh dependent data (a position's work). */
  const [revision, setRevision] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const live = useRef(false);
  const [order] = useState(() => new Newest());

  const reload = useCallback(async () => {
    const newest = order.start();
    try {
      const next = await getOrganization();
      if (!newest.take()) return;
      setSnapshot(next);
      setStatus("ready");
      setError(null);
    } catch (reason) {
      if (!newest.fresh()) return;
      setStatus((s) => (s === "ready" ? s : "error"));
      setError(toCommandError(reason).message);
    }
  }, [order]);

  /** Reload in a moment (several asks in a row make one reload). */
  const soon = useCallback(() => {
    if (!live.current) return;
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => void reload(), 150);
  }, [reload]);

  const apply = useCallback(
    (next: OrgSnapshot) => {
      order.applied();
      setSnapshot(next);
      setStatus("ready");
      setError(null);
      setRevision((r) => r + 1);
      // What a command returns was read before its answer travelled back: a change that came in
      // meanwhile set off a reload that started before this one and is dropped as older. One
      // more reload, newer than the change, brings it in.
      soon();
    },
    [order, soon],
  );

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    live.current = true;
    const schedule = () => {
      if (disposed) return;
      setRevision((r) => r + 1);
      soon();
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
      // Your Workforce is every organization's: a change from another window shows here too.
      Promise.resolve()
        .then(() =>
          subscribeShared((what) => {
            if (what === "workforce") schedule();
          }),
        )
        .then(keep),
    ]).finally(() => {
      if (!disposed) void reload();
    });
    return () => {
      disposed = true;
      live.current = false;
      stops.forEach((stop) => stop());
      if (timer.current) clearTimeout(timer.current);
    };
  }, [reload, soon]);

  return { snapshot, status, error, revision, reload, apply };
}
