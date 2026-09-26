// Typed client for the Tauri command boundary.
// This is the ONLY module that calls `invoke`. Components import these functions,
// never `@tauri-apps/api/core` directly (enforced by ESLint).

import { invoke } from "@tauri-apps/api/core";
import type {
  AgentOverview,
  AgentRuntimeInfo,
  AgentSession,
  AgentSessionDetail,
  AppInfo,
  BackupInfo,
  CommandError,
  CommandErrorKind,
  DepartmentInput,
  ExecutionOutput,
  ExecutionRecord,
  ExportInfo,
  HireInput,
  IntegrityReport,
  LedgerEvent,
  LedgerStatus,
  LiaisonOverview,
  OrgSnapshot,
  OversightRole,
  PositionPatchInput,
  ProjectInput,
  RoleInput,
  RuntimeOverview,
  SyntheticTaskAction,
  Task,
  TaskHandoffs,
  TaskTimeline,
  TaskTree,
  WorkView,
} from "@plenipo/types";

/** Error thrown by every command wrapper. Mirrors the Rust `CommandError` DTO. */
export class PlenipoCommandError extends Error {
  readonly kind: CommandErrorKind;

  constructor(kind: CommandErrorKind, message: string) {
    super(message);
    this.name = "PlenipoCommandError";
    this.kind = kind;
  }
}

function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (v.kind === "invalidInput" || v.kind === "internal") && typeof v.message === "string";
}

/** Normalize anything a failed `invoke` rejects with into a PlenipoCommandError. */
export function toCommandError(reason: unknown): PlenipoCommandError {
  if (reason instanceof PlenipoCommandError) return reason;
  if (isCommandError(reason)) return new PlenipoCommandError(reason.kind, reason.message);
  const message =
    reason instanceof Error
      ? reason.message
      : typeof reason === "string"
        ? reason
        : "Unknown error";
  return new PlenipoCommandError("internal", message);
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw toCommandError(reason);
  }
}

export function getAppInfo(): Promise<AppInfo> {
  return call<AppInfo>("get_app_info");
}

/** Signals that the shell rendered. Used by the launch smoke test; a no-op otherwise. */
export function frontendReady(): Promise<void> {
  return call<void>("frontend_ready");
}

/** Launch profiles, execution history (newest first), active count, and notices. */
export function getRuntimeOverview(): Promise<RuntimeOverview> {
  return call<RuntimeOverview>("get_runtime_overview");
}

/** Start an approved launch profile. The UI can never supply a command or path. */
export function startExecution(profileId: string): Promise<ExecutionRecord> {
  return call<ExecutionRecord>("start_execution", { profileId });
}

/** Terminate an execution's process tree; resolves with its final record. */
export function cancelExecution(executionId: string): Promise<ExecutionRecord> {
  return call<ExecutionRecord>("cancel_execution", { executionId });
}

/** Buffered output, used to rebuild the view after navigation or a reload. */
export function getExecutionOutput(executionId: string): Promise<ExecutionOutput> {
  return call<ExecutionOutput>("get_execution_output", { executionId });
}

// ---- Ledger -------------------------------------------------------------------------------

export function getLedgerStatus(): Promise<LedgerStatus> {
  return call<LedgerStatus>("get_ledger_status");
}

/** Tasks, newest first. */
export function listTasks(): Promise<Task[]> {
  return call<Task[]>("list_tasks");
}

/** A task's complete ordered activity trail and its direct children. */
export function getTaskTimeline(taskId: string): Promise<TaskTimeline> {
  return call<TaskTimeline>("get_task_timeline", { taskId });
}

/** Most recent ledger events, newest first. */
export function listRecentEvents(): Promise<LedgerEvent[]> {
  return call<LedgerEvent[]>("list_recent_events");
}

/** Diagnostics: create a synthetic task. */
export function createSyntheticTask(): Promise<Task> {
  return call<Task>("create_synthetic_task");
}

/** Diagnostics: act on a synthetic task. The ledger's state machine decides what is allowed. */
export function advanceSyntheticTask(taskId: string, action: SyntheticTaskAction): Promise<Task> {
  return call<Task>("advance_synthetic_task", { taskId, action });
}

export function runIntegrityCheck(): Promise<IntegrityReport> {
  return call<IntegrityReport>("run_integrity_check");
}

/** Verified backup; Core chooses the location. */
export function createLedgerBackup(): Promise<BackupInfo> {
  return call<BackupInfo>("create_ledger_backup");
}

/** JSON export; Core chooses the location. */
export function exportLedger(): Promise<ExportInfo> {
  return call<ExportInfo>("export_ledger");
}

// ---- Agent runtimes (Phase 3) -------------------------------------------------------------

/** Runtimes (installation, sign-in, capabilities), sessions (newest first), and notices. */
export function getAgentOverview(): Promise<AgentOverview> {
  return call<AgentOverview>("get_agent_overview");
}

/** Re-detect every runtime's installation and sign-in. */
export function refreshAgentRuntimes(): Promise<AgentRuntimeInfo[]> {
  return call<AgentRuntimeInfo[]>("refresh_agent_runtimes");
}

/** A session with its turns and recent live activity. */
export function getAgentSession(sessionId: string): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("get_agent_session", { sessionId });
}

/**
 * Start a session on a runtime with a first objective. The objective is sent to the runtime on
 * stdin by Core; the UI never supplies a command, path, or flag. With `handoffs`, the worker may
 * ask other workers for help through Plenipo Liaison.
 */
export function startAgentSession(
  runtimeId: string,
  objective: string,
  model?: string,
  handoffs = false,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("start_agent_session", {
    runtimeId,
    objective,
    model: model?.trim() ? model.trim() : null,
    handoffs,
  });
}

/**
 * Give an existing session its next objective (resumes the provider session). A handoff worker's
 * session takes work only through Liaison.
 */
export function resumeAgentSession(
  sessionId: string,
  objective: string,
): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("resume_agent_session", { sessionId, objective });
}

/**
 * Cancel the session's running turn, or end a turn waiting for handoff replies; resolves once the
 * turn is recorded. Liaison then stops the handoffs it was waiting for.
 */
export function cancelAgentTurn(sessionId: string): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("cancel_agent_turn", { sessionId });
}

export function closeAgentSession(sessionId: string): Promise<AgentSession> {
  return call<AgentSession>("close_agent_session", { sessionId });
}

// ---- Liaison (Phase 4) --------------------------------------------------------------------

/** The handoff that created a task (if any) and the handoffs it made, with their replies. */
export function getTaskHandoffs(taskId: string): Promise<TaskHandoffs> {
  return call<TaskHandoffs>("get_task_handoffs", { taskId });
}

/** A task's whole delegation tree, from its root task, depth-first. */
export function getTaskTree(taskId: string): Promise<TaskTree> {
  return call<TaskTree>("get_task_tree", { taskId });
}

/** Liaison's protocol, limits, destinations, open handoffs, and notices. */
export function getLiaisonOverview(): Promise<LiaisonOverview> {
  return call<LiaisonOverview>("get_liaison_overview");
}

// ---- Workforce (Phase 5) ------------------------------------------------------------------
// Every change returns the organization as it is afterwards. The Ledger enforces the
// structure; a refused change rejects with the reason.

/** Departments, projects, positions with live status, oversight, and stats. */
export function getOrganization(): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("get_organization");
}

/** The work a position owns and its team's unfinished work; omit the ID for the organization. */
export function getWork(positionId?: string): Promise<WorkView> {
  return call<WorkView>("get_work", { positionId: positionId ?? null });
}

export function renameOrganization(name: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("rename_organization", { name });
}

export function createRole(input: RoleInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_role", { input });
}

/** Create a department with its head position. */
export function createDepartment(input: DepartmentInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_department", { input });
}

export function updateDepartment(
  departmentId: string,
  input: DepartmentInput,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_department", { departmentId, input });
}

/** Delete a department that has no projects (its head position is archived). */
export function removeDepartment(departmentId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("remove_department", { departmentId });
}

/** Create a project with its coordinator position. */
export function createProject(input: ProjectInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("create_project", { input });
}

export function updateProject(projectId: string, input: ProjectInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_project", { projectId, input });
}

/** Archive a project and its whole team. */
export function archiveProject(projectId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("archive_project", { projectId });
}

/** Hire into a team: a new position (and, for a persistent one, its agent). */
export function hirePosition(input: HireInput): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("hire_position", { input });
}

/** Hire an agent into a vacant persistent position. */
export function fillPosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("fill_position", { positionId });
}

/** Let a persistent position's agent go; the position stays, vacant. */
export function vacatePosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("vacate_position", { positionId });
}

export function updatePosition(
  positionId: string,
  input: PositionPatchInput,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("update_position", { positionId, input });
}

/** Make a position report to another (`null`: the owner). */
export function movePosition(positionId: string, reportsTo: string | null): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("move_position", { positionId, reportsTo });
}

export function archivePosition(positionId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("archive_position", { positionId });
}

/** Assign an on-demand position to review, QA, or security-audit a team. */
export function assignOversight(
  overseerId: string,
  targetId: string,
  role: OversightRole,
): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("assign_oversight", { overseerId, targetId, role });
}

export function endOversight(oversightId: string): Promise<OrgSnapshot> {
  return call<OrgSnapshot>("end_oversight", { oversightId });
}

/**
 * Give a staffed persistent position's agent an objective. Core builds its instructions and
 * chooses its session; the UI names only the position.
 */
export function giveObjective(positionId: string, objective: string): Promise<AgentSessionDetail> {
  return call<AgentSessionDetail>("give_objective", { positionId, objective });
}
