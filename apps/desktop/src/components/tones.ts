/**
 * How the older pages' states read on the design system's status ramp (ADR-030 §8): the words
 * stay the pages' own; only the look comes from the library (`StatusPill`, `StatusDot`).
 */

import type { ExecutionState, TaskState } from "@plenipo/types";
import type { Status } from "@plenipo/ui";

/** A task's state. */
export const TASK_TONE: Record<TaskState, Status> = {
  queued: "pending",
  running: "ok",
  blocked: "pending",
  awaitingApproval: "pending",
  succeeded: "ok",
  failed: "error",
  cancelled: "offline",
};

/** A program run's state. */
export const EXECUTION_TONE: Record<ExecutionState, Status> = {
  starting: "pending",
  running: "ok",
  succeeded: "ok",
  failed: "error",
  cancelled: "offline",
  timedOut: "error",
  interrupted: "warn",
};

/** The older pills' tones (`pill--ok`, `pill--warn`, `pill--bad`, and plain or muted). */
export const PILL_TONE = {
  ok: "ok",
  warn: "warn",
  bad: "error",
  muted: "offline",
} as const satisfies Record<string, Status>;

/**
 * A turn's or a handoff's outcome, through the task words the helpers in `agents/format.ts` use.
 * There "blocked" is a refusal, a usage limit, or a sign-in needed: it needs the owner's
 * attention (warn), unlike a task that waits on replies.
 */
export const OUTCOME_TONE: Record<
  "queued" | "running" | "succeeded" | "failed" | "blocked" | "cancelled",
  Status
> = {
  queued: "pending",
  running: "ok",
  succeeded: "ok",
  failed: "error",
  blocked: "warn",
  cancelled: "offline",
};
