import { useEffect, useState } from "react";
import type { TaskCost, TaskTreeCost } from "@plenipo/types";

import { getTaskCost, getTaskTreeCost } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";

/** Ledger events after which a task's cost may be different: a run's result, a paid request. */
function changesCost(eventType: string): boolean {
  return (
    eventType === "agent.result" ||
    eventType.startsWith("spending.") ||
    eventType === "task.state_changed"
  );
}

/** A counter that goes up (debounced) when the Ledger records something a cost depends on. */
function useCostRevision(on: boolean): number {
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!on) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    subscribeLedgerEvents((event) => {
      if (disposed || !changesCost(event.eventType)) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => setRevision((r) => r + 1), 300);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      stop?.();
      if (timer) clearTimeout(timer);
    };
  }, [on]);
  return revision;
}

/** Nothing about it can change any more: nothing runs, and no money is still set aside. */
function settled(cost: TaskCost): boolean {
  return !cost.running && cost.setAsideMicros === 0;
}

function useCost<T extends { taskId: string }>(
  taskId: string | null,
  read: (taskId: string) => Promise<T>,
  done: (cost: T) => boolean,
): T | null {
  const [cost, setCost] = useState<T | null>(null);
  const current = cost && cost.taskId === taskId ? cost : null;
  // Once settled, it stops listening: a long chat keeps no listener per finished answer.
  const revision = useCostRevision(taskId !== null && !(current && done(current)));
  useEffect(() => {
    if (!taskId) return;
    let cancelled = false;
    read(taskId)
      .then((c) => {
        if (!cancelled) setCost(c);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [taskId, revision, read]);
  return current;
}

/** What one task cost (I2), kept current until settled; `null` while loading or without one. */
export function useTaskCost(taskId: string | null): TaskCost | null {
  return useCost(taskId, getTaskCost, settled);
}

/** What a task and every task handed out beneath it cost (I2), kept current until settled. */
export function useTaskTreeCost(taskId: string | null): TaskTreeCost | null {
  return useCost(taskId, getTaskTreeCost, (tree: TaskTreeCost) => settled(tree.total));
}
