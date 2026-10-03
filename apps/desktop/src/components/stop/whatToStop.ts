/**
 * What Stop stops (Phase 25, item 3.3): the work running or waiting now. A full-time agent's
 * current task only: its conversation stays, so it can take a new objective after. Work still
 * queued has no conversation yet; it ends with the task that asked for it.
 */
import type { PositionInfo, TaskState } from "@plenipo/types";

import { firstLine } from "../../pages/words";

/** A conversation to stop, and what it is doing. */
export interface StopTarget {
  sessionId: string;
  objective: string;
}

const LIVE: ReadonlySet<TaskState> = new Set(["running", "blocked", "awaitingApproval"]);

/** Whether a task in this state can be stopped (it has started and isn't finished). */
export const isStoppable = (state: TaskState) => LIVE.has(state);

/** A position's work that Stop would end: its current task, or each of its workers' tasks. */
export function workToStop(p: PositionInfo): StopTarget[] {
  if (p.staffing === "persistent") {
    const task = p.currentTask;
    const sessionId = task?.sessionId ?? p.agent?.sessionId ?? null;
    return task && sessionId && LIVE.has(task.state)
      ? [{ sessionId, objective: firstLine(task.objective) }]
      : [];
  }
  return p.workers.flatMap((w) =>
    w.sessionId && LIVE.has(w.state)
      ? [{ sessionId: w.sessionId, objective: firstLine(w.objective) }]
      : [],
  );
}

/** "Stop Alex's task?", or "Stop Senior Developer's 3 tasks?". */
export function stopQuestion(who: string, count: number): string {
  return count === 1 ? `Stop ${who}'s task?` : `Stop ${who}'s ${count} tasks?`;
}
