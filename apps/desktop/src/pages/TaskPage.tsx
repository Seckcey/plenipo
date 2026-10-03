import type { TaskRecord, TaskTimeline, TaskTree } from "@plenipo/types";
import {
  Button,
  CellLink,
  EmptyState,
  LoadingState,
  PageHeader,
  Panel,
  PropertyList,
  RowList,
  StatusPill,
  TopologyMap,
} from "@plenipo/ui";

import { LiveConversation } from "../live/LiveConversation";
import { useOpenWatch } from "../terminal/useTerminal";
import { StopButton } from "../components/stop/StopWork";
import { isStoppable } from "../components/stop/stopWork";
import { getTaskEvents, getTaskRecord, getTaskTimeline, getTaskTree } from "../api/commands";
import { ObjectiveResult } from "../components/ObjectiveResult";
import type { Go } from "../components/views";
import { sourceLabel, toolName } from "../ledger/format";
import { useOrganization } from "../org/useOrganization";
import { useObjectiveReport } from "../projects/useProjectWork";
import { useNow } from "../runtime/useNow";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { PageMissing, Screenshots } from "./parts";
import { approvalRows, decisionRows, pullRequestRows } from "./rows";
import { taskPosition, treeMap } from "./tree";
import { changesWork, useLive } from "./useLive";
import { TASK_STATUS, count, firstLine, isNotFound, when } from "./words";

/**
 * A task's page (Phase 12): its objective and what counts as done, the delegation tree, its
 * activity as it happens, its approvals, screenshots and pull requests, the decisions made, and
 * the final result.
 */
export function TaskPage({
  id,
  go,
  onBack,
  onOpenSession,
}: {
  id: string;
  go: Go;
  onBack?: (() => void) | undefined;
  onOpenSession: (sessionId: string) => void;
}) {
  const now = useNow(30_000);
  const openWatch = useOpenWatch();
  const org = useOrganization().snapshot;
  // The task and those around it come with its delegation tree (tasks only, no events).
  const tree = useLive<TaskTree>(id, getTaskTree, changesWork, 800);
  const node = tree.value?.nodes.find((n) => n.task.id === id);
  // A very large tree lists its first 500 tasks: a task past them is read on its own.
  const alone = useLive<TaskTimeline>(
    tree.value && !node ? id : null,
    getTaskTimeline,
    (e) => e.taskId === id,
    1_000,
  );
  const record = useLive<TaskRecord>(id, getTaskRecord, changesWork, 800);
  const task = node?.task ?? alone.value?.task ?? null;
  const isRoot = task !== null && task.parentTaskId === null;
  // The objective's result (built from all of its work) only on the objective's own page.
  const { report, error: reportError } = useObjectiveReport(isRoot ? id : null);

  if (tree.status === "loading" || (tree.value && !node && alone.status === "loading")) {
    return (
      <div className="page">
        <LoadingState label="Loading the task" lines={6} />
      </div>
    );
  }
  if (!task) {
    const error = tree.error ?? alone.error;
    return (
      <PageMissing
        kind="task"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
        // A task that isn't in the Ledger (any more) is not an error to try again.
        error={error && !isNotFound(error) ? error : null}
        onRetry={() => {
          tree.reload();
          alone.reload();
        }}
      />
    );
  }

  const positionId = taskPosition(task.metadata);
  const position = positionId ? org?.positions.find((p) => p.id === positionId) : undefined;
  const project = task.projectId ? org?.projects.find((p) => p.id === task.projectId) : undefined;
  const sessionId = typeof task.metadata.sessionId === "string" ? task.metadata.sessionId : null;
  const map = tree.value ? treeMap(tree.value, org) : null;
  const rootId = tree.value?.rootId ?? null;
  const rootTask = tree.value?.nodes.find((n) => n.task.id === rootId)?.task;
  const approvals = record.value?.approvals ?? [];
  const status = TASK_STATUS[task.state];

  return (
    <div className="page">
      <PageHeader
        id="task-title"
        kicker={isRoot ? "Objective" : "Task"}
        title={firstLine(task.objective, 120) || "(no objective)"}
        status={<StatusPill status={status.status} label={status.label} />}
        lead={[
          position ? `Given to ${position.title}` : null,
          project ? project.name : null,
          `asked by ${sourceLabel(task.requestedBy)}`,
        ]
          .filter(Boolean)
          .join(" · ")}
        onBack={onBack}
        actions={
          <>
            {sessionId && (
              <Button size="sm" icon="workers" onClick={() => onOpenSession(sessionId)}>
                Open the conversation
              </Button>
            )}
            <Button size="sm" icon="activity" onClick={() => go({ view: "activity", id })}>
              Show in Activity
            </Button>
            {/* Watch its worker's file changes (Phase 25, items 1.8 and 3.1). */}
            {openWatch && position && position.active && (
              <Button size="sm" onClick={() => openWatch(position.id, position.title)}>
                Watch
              </Button>
            )}
            {/* Stop this task (Phase 25, item 3.3): its worker's conversation stays. */}
            <StopButton
              who={position?.title ?? "this worker"}
              work={
                sessionId && isStoppable(task.state)
                  ? [{ sessionId, objective: firstLine(task.objective) }]
                  : []
              }
              fullTime={position?.staffing === "persistent"}
            />
          </>
        }
      />

      <div className="page__grid">
        <Panel id="task-about" title="About">
          <PropertyList
            items={[
              { label: "Objective", value: <div className="page__text">{task.objective}</div> },
              {
                label: "Done when",
                value: task.acceptanceCriteria.trim() ? (
                  <div className="page__text">{task.acceptanceCriteria}</div>
                ) : (
                  "Not said when it was given"
                ),
              },
              {
                label: "Given to",
                value: position ? (
                  <CellLink onClick={() => go({ view: "worker", id: position.id })}>
                    {position.title}
                  </CellLink>
                ) : (
                  // A task handed to an AI tool directly names the tool.
                  (node?.runtimeLabel ?? toolName(task.assignedTo) ?? "Nobody yet")
                ),
              },
              { label: "Asked by", value: sourceLabel(task.requestedBy, { capitalize: true }) },
              ...(project
                ? [
                    {
                      label: "Project",
                      value: (
                        <CellLink onClick={() => go({ view: "project", id: project.id })}>
                          {project.name}
                        </CellLink>
                      ),
                    },
                  ]
                : []),
              ...(!isRoot && rootTask
                ? [
                    {
                      label: "Part of",
                      value: (
                        <CellLink onClick={() => go({ view: "task", id: rootTask.id })}>
                          {firstLine(rootTask.objective, 80)}
                        </CellLink>
                      ),
                    },
                  ]
                : []),
              {
                label: "Tasks under it",
                value: count(
                  node
                    ? (tree.value?.nodes.filter((n) => n.task.parentTaskId === id).length ?? 0)
                    : (alone.value?.children.length ?? 0),
                  "task",
                ),
              },
              { label: "Created", value: when(task.createdAt, now) },
              ...(task.startedAt ? [{ label: "Started", value: when(task.startedAt, now) }] : []),
              ...(task.completedAt
                ? [{ label: "Finished", value: when(task.completedAt, now) }]
                : []),
            ]}
          />
        </Panel>

        {/* Its worker's words and steps, live (Phase 25, item 3.1). */}
        {sessionId && isStoppable(task.state) && (
          <Panel id="task-live" title="Live conversation" wide>
            <LiveConversation
              taskId={id}
              sessionId={sessionId}
              startedAt={task.startedAt ?? null}
              running
              who={position?.title ?? "The worker"}
            />
          </Panel>
        )}

        <Panel
          id="task-approvals"
          title="Approvals"
          count={approvals.length}
          countLabel="approvals"
        >
          <RowList
            label="Approvals"
            items={approvalRows(approvals, go, now)}
            state={record.status}
            error={record.error}
            onRetry={record.reload}
            empty={<EmptyState compact title="No approvals were needed" />}
          />
        </Panel>

        <Panel id="task-tree" title="Delegation tree" wide>
          <TopologyMap
            label="Delegation tree"
            nodes={map?.nodes ?? []}
            links={map?.links ?? []}
            selectedId={id}
            state={tree.status}
            error={tree.error}
            onRetry={tree.reload}
            onSelect={(taskId) => {
              if (taskId !== id) go({ view: "task", id: taskId });
            }}
          />
        </Panel>

        <Panel id="task-result" title="Result" wide>
          {!isRoot ? (
            <EmptyState
              compact
              title={
                task.state === "succeeded"
                  ? "Finished"
                  : task.state === "failed" || task.state === "cancelled"
                    ? TASK_STATUS[task.state].label
                    : "No result yet"
              }
              action={
                rootTask ? (
                  <Button size="sm" onClick={() => go({ view: "task", id: rootTask.id })}>
                    Open the objective's result
                  </Button>
                ) : undefined
              }
            >
              This task is one part of an objective. What it did is in its Activity below; the whole
              result is on the objective's page.
            </EmptyState>
          ) : reportError && !report ? (
            <p className="page__note page__note--error" role="alert">
              Couldn't build the result: {reportError}
            </p>
          ) : !report ? (
            <LoadingState label="Loading the result" lines={3} />
          ) : (
            // The result keeps the look it has on the Projects page.
            <div className="view page__result">
              <ObjectiveResult
                report={report}
                openLabel={null}
                onOpenTask={(taskId) => go({ view: "task", id: taskId })}
                onOpenApprovals={() => go({ view: "approvals", id: null })}
              />
            </div>
          )}
        </Panel>

        <Panel id="task-record" title="Screenshots and pull requests">
          <RowList
            label="Pull requests"
            items={pullRequestRows(record.value?.record.pullRequests ?? [], go, now)}
            state={record.status}
            error={record.error}
            onRetry={record.reload}
            empty={<EmptyState compact title="No pull requests" />}
          />
          {record.value && <Screenshots artifacts={record.value.record.artifacts} now={now} />}
        </Panel>

        <Panel id="task-decisions" title="Decisions">
          <RowList
            label="Decisions"
            items={decisionRows(record.value?.record.decisions ?? [], go, now, id)}
            state={record.status}
            error={record.error}
            onRetry={record.reload}
            empty={<EmptyState compact title="No decisions yet" />}
          />
        </Panel>

        <Panel id="task-activity" title="Activity" wide>
          <EventHistory
            label="Activity of this task and the tasks under it"
            historyKey={`task:${id}`}
            load={(before) => getTaskEvents(id, HISTORY_PAGE, before)}
            now={now}
            currentTaskId={id}
            onOpenTask={(taskId) => go({ view: "task", id: taskId })}
          />
        </Panel>
      </div>
    </div>
  );
}
