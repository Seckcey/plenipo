import { useEffect, useState } from "react";
import type { HandoffView, TaskHandoffs } from "@plenipo/types";

import { getTaskHandoffs } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/** Ledger events after which a task's handoffs may look different. */
function isLiaisonChange(eventType: string): boolean {
  return eventType.startsWith("liaison.") || eventType === "task.state_changed";
}

/**
 * A counter that increases (debounced) whenever the Ledger commits a Liaison event or a task
 * state change, so views can refresh the handoffs they show.
 */
export function useLiaisonRevision(): number {
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    subscribeLedgerEvents((event) => {
      if (disposed || !isLiaisonChange(event.eventType)) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setRevision((r) => r + 1), 150);
    })
      .then((stop) => {
        if (disposed) stop();
        else unsubscribe = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unsubscribe?.();
      if (timer) clearTimeout(timer);
    };
  }, []);
  return revision;
}

function isOpen(view: HandoffView): boolean {
  return (
    view.state === "accepted" ||
    view.state === "dispatched" ||
    (view.reply !== null && view.reply.state === "pending")
  );
}

/** Nothing about these handoffs can change any more. */
export function isSettled(handoffs: TaskHandoffs | null): boolean {
  if (!handoffs) return false;
  const received = handoffs.received;
  return !handoffs.sent.some(isOpen) && !(received && isOpen(received));
}

/** Handoffs requested open (accepted, running, or with a reply not yet delivered). */
export function openHandoffs(handoffs: TaskHandoffs | null): number {
  return handoffs ? handoffs.sent.filter(isOpen).length : 0;
}

/**
 * A task's handoffs, fetched from Core when `turnKey` changes and, until the turn is `finished`
 * and nothing about its handoffs can change any more, on every Liaison `revision`. `null` while
 * loading, and when `taskId` is `null` (the session does not use Liaison).
 */
export function useTaskHandoffs(
  taskId: string | null,
  turnKey: string,
  revision: number,
  finished: boolean,
): TaskHandoffs | null {
  const [data, setData] = useState<TaskHandoffs | null>(null);
  const current = data && data.taskId === taskId ? data : null;
  const refreshKey = finished && isSettled(current) ? "settled" : revision;
  useEffect(() => {
    if (!taskId) return;
    let cancelled = false;
    getTaskHandoffs(taskId)
      .then((h) => {
        if (!cancelled) setData(h);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [taskId, turnKey, refreshKey]);
  return current;
}
