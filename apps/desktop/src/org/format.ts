import type {
  OrgSnapshot,
  OversightRole,
  PositionKind,
  PositionStatus,
  Staffing,
  TaskState,
} from "@plenipo/types";

/** Shown next to every status color, so status never depends on color alone. */
export const STATUS_LABEL: Record<PositionStatus, string> = {
  vacant: "Vacant",
  idle: "Idle",
  working: "Working",
  waiting: "Waiting on team",
  queued: "Queued",
  unavailable: "Unavailable",
  archived: "Archived",
};

export const KIND_LABEL: Record<PositionKind, string> = {
  superintendent: "Superintendent",
  departmentManager: "Department manager",
  projectCoordinator: "Project coordinator",
  worker: "Worker",
};

export const STAFFING_LABEL: Record<Staffing, string> = {
  persistent: "Persistent",
  onDemand: "On demand",
};

/** What a live worker is doing, from its task's state. */
export const WORKER_STATE_LABEL: Record<TaskState, string> = {
  queued: "Queued",
  running: "Working",
  blocked: "Waiting on replies",
  awaitingApproval: "Awaiting approval",
  succeeded: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

export const OVERSIGHT_LABEL: Record<OversightRole, string> = {
  review: "Reviewer",
  qa: "QA evaluator",
  security: "Security auditor",
};

/** The oversight role mid-sentence ("the team's QA evaluator"). */
export const OVERSIGHT_NOUN: Record<OversightRole, string> = {
  review: "reviewer",
  qa: "QA evaluator",
  security: "security auditor",
};

/** Short chip text for an oversight link. */
export const OVERSIGHT_CHIP: Record<OversightRole, string> = {
  review: "Review",
  qa: "QA",
  security: "Security",
};

export const OVERSIGHT_ROLES: OversightRole[] = ["review", "qa", "security"];

export function runtimeLabel(snapshot: OrgSnapshot, id: string): string {
  return snapshot.runtimes.find((r) => r.id === id)?.label ?? id;
}

export function runtimeReady(snapshot: OrgSnapshot, id: string): boolean {
  return snapshot.runtimes.find((r) => r.id === id)?.ready ?? false;
}

/** "just now", "5 min ago", "3 h ago", "2 d ago". */
export function ago(ms: number, now: number = Date.now()): string {
  const secs = Math.max(0, Math.round((now - ms) / 1000));
  if (secs < 45) return "just now";
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 36) return `${hours} h ago`;
  return `${Math.round(hours / 24)} d ago`;
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}
