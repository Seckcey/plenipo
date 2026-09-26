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
  ExecutionOutput,
  ExecutionRecord,
  ExportInfo,
  IntegrityReport,
  LedgerEvent,
  LedgerStatus,
  LiaisonOverview,
  RuntimeOverview,
  SyntheticTaskAction,
  Task,
  TaskHandoffs,
  TaskTimeline,
  TaskTree,
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
