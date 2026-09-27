// Fixtures for Home and the pages of one department, project, worker, or task (Phase 12).
import type {
  HomeView,
  LedgerEvent,
  ObjectiveBrief,
  Task,
  TaskRecord,
  TaskTimeline,
  TaskTree,
  WorkView,
} from "@plenipo/types";

export const NOW = Date.UTC(2026, 8, 27, 17, 0, 0);

let seq = 1000;
export function event(
  eventType: string,
  payload: Record<string, unknown>,
  patch: Partial<LedgerEvent> = {},
): LedgerEvent {
  seq += 1;
  return {
    seq,
    id: `event-${seq}`,
    taskId: null,
    executionId: null,
    source: "plenipo",
    destination: null,
    eventType,
    payload,
    createdAt: NOW - 60_000,
    ...patch,
  };
}

export function brief(patch: Partial<ObjectiveBrief> = {}): ObjectiveBrief {
  return {
    rootTaskId: "task-obj-1",
    objective: "Relaunch the website",
    positionTitle: "Engineering Manager",
    state: "running",
    createdAt: NOW - 30 * 60_000,
    completedAt: null,
    tasks: 4,
    active: 2,
    failed: 0,
    waitingApprovals: 0,
    branch: null,
    projectId: "pr-web",
    answer: null,
    ...patch,
  };
}

export function sampleHome(patch: Partial<HomeView> = {}): HomeView {
  return {
    current: [brief()],
    finished: [
      brief({
        rootTaskId: "task-obj-0",
        objective: "Fix the typo on the pricing page",
        state: "succeeded",
        completedAt: NOW - 2 * 3_600_000,
        answer: "Fixed the typo and checked the page.\nMore detail follows.",
      }),
    ],
    stuck: [
      {
        event: event(
          "task.state_changed",
          { from: "running", to: "failed", reason: "the tests failed" },
          { taskId: "task-fail" },
        ),
        task: {
          id: "task-fail",
          objective: "Ship the newsletter",
          state: "failed",
          positionId: "p-camp",
          positionTitle: "Campaign Supervisor",
          projectId: "pr-camp",
          parentTaskId: null,
          sessionId: null,
          createdAt: NOW - 3_600_000,
          startedAt: NOW - 3_500_000,
          completedAt: NOW - 3_000_000,
        },
      },
    ],
    going: 1,
    finishedDay: 1,
    ...patch,
  };
}

export function task(id: string, patch: Partial<Task> = {}): Task {
  return {
    id,
    parentTaskId: null,
    requestedBy: "owner",
    assignedTo: null,
    projectId: "pr-web",
    objective: `Objective of ${id}`,
    acceptanceCriteria: "",
    priority: 2,
    state: "running",
    metadata: {},
    createdAt: NOW - 20 * 60_000,
    updatedAt: NOW - 60_000,
    startedAt: NOW - 19 * 60_000,
    completedAt: null,
    ...patch,
  };
}

/** An objective given to the Engineering Manager, handed to the Website Supervisor. */
export function sampleTree(): TaskTree {
  return {
    rootId: "task-obj-1",
    focusId: "task-obj-1",
    correlationId: "corr-1",
    nodes: [
      {
        task: task("task-obj-1", {
          objective: "Relaunch the website\n\nBefore the Q4 campaign.",
          acceptanceCriteria: "The new pages are live and pass QA.",
          metadata: { workforce: { positionId: "p-eng" }, sessionId: "session-eng" },
        }),
        depth: 0,
        runtimeId: "claude-code",
        runtimeLabel: "Claude Code",
        sessionId: "session-eng",
        handoff: null,
      },
      {
        task: task("task-web", {
          parentTaskId: "task-obj-1",
          objective: "Build the pricing page",
          state: "succeeded",
          requestedBy: "liaison",
          metadata: { workforce: { positionId: "p-web" } },
          completedAt: NOW - 5 * 60_000,
        }),
        depth: 1,
        runtimeId: "codex",
        runtimeLabel: "Codex",
        sessionId: null,
        handoff: null,
      },
    ],
  };
}

export function sampleTimeline(id = "task-obj-1"): TaskTimeline {
  const node = sampleTree().nodes.find((n) => n.task.id === id)!;
  return {
    task: node.task,
    events: [],
    children: sampleTree()
      .nodes.filter((n) => n.task.parentTaskId === id)
      .map((n) => n.task),
  };
}

export function sampleTaskRecord(patch: Partial<TaskRecord> = {}): TaskRecord {
  return {
    record: {
      pullRequests: [
        {
          url: "https://git.example/web/pull/12",
          number: 12,
          taskId: "task-web",
          worker: "Website Supervisor",
          createdAt: NOW - 10 * 60_000,
        },
      ],
      artifacts: [],
      decisions: [
        event(
          "approval.resolved",
          { state: "approved", summary: "Run npm publish" },
          { taskId: "task-web" },
        ),
      ],
    },
    approvals: [],
    ...patch,
  };
}

export function sampleWorkView(patch: Partial<WorkView> = {}): WorkView {
  const t = (id: string, objective: string, state: Task["state"]) => ({
    id,
    objective,
    state,
    positionId: "p-web",
    positionTitle: "Website Supervisor",
    projectId: "pr-web",
    parentTaskId: null,
    sessionId: null,
    createdAt: NOW - 15 * 60_000,
    startedAt: NOW - 14 * 60_000,
    completedAt: state === "succeeded" ? NOW - 60_000 : null,
  });
  return {
    positionId: "p-web",
    running: [t("task-web", "Build the pricing page", "running")],
    waiting: [],
    queued: [t("task-q", "Write the release notes", "queued")],
    recent: [t("task-old", "Fix the footer links", "succeeded")],
    team: [],
    ...patch,
  };
}
