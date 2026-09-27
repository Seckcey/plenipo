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

import { getTaskEvents, getTaskRecord, getTaskTimeline, getTaskTree } from "../api/commands";
import { ObjectiveResult } from "../components/ObjectiveResult";
import type { Go } from "../components/views";
import { sourceLabel } from "../ledger/format";
import { useOrganization } from "../org/useOrganization";
import { useObjectiveReport } from "../projects/useProjectWork";
import { useNow } from "../runtime/useNow";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { PageMissing, Screenshots } from "./parts";
import { approvalRows, decisionRows, pullRequestRows } from "./rows";
import { taskPosition, treeMap } from "./tree";
import { changesWork, useLive } from "./useLive";
import { TASK_STATUS, count, firstLine, when } from "./words";

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
  const org = useOrganization().snapshot;
  const timeline = useLive<TaskTimeline>(
    id,
    getTaskTimeline,
    (e) => e.taskId === id || e.eventType.startsWith("task."),
    500,
  );
  const tree = useLive<TaskTree>(id, getTaskTree, changesWork, 800);
  const record = useLive<TaskRecord>(id, getTaskRecord, changesWork, 800);
  const { report, error: reportError } = useObjectiveReport(tree.value?.rootId ?? null);

  if (timeline.status === "loading") {
    return (
      <div className="page">
        <LoadingState label="Loading the task" lines={6} />
      </div>
    );
  }
  const task = timeline.value?.task;
  if (!task) {
    return (
      <PageMissing
        kind="task"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
        error={timeline.error}
        onRetry={timeline.reload}
      />
    );
  }

  const isRoot = task.parentTaskId === null;
  const positionId = taskPosition(task.metadata);
  const position = positionId ? org?.positions.find((p) => p.id === positionId) : undefined;
  const project = task.projectId ? org?.projects.find((p) => p.id === task.projectId) : undefined;
  const sessionId = typeof task.metadata.sessionId === "string" ? task.metadata.sessionId : null;
  const map = tree.value ? treeMap(tree.value, org) : null;
  const rootId = tree.value?.rootId ?? null;
  const rootTask = tree.value?.nodes.find((n) => n.task.id === rootId)?.task;
  const own = report?.tasks.find((t) => t.taskId === id);
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
                  "No acceptance criteria were given"
                ),
              },
              {
                label: "Given to",
                value: position ? (
                  <CellLink onClick={() => go({ view: "worker", id: position.id })}>
                    {position.title}
                  </CellLink>
                ) : (
                  (task.assignedTo ?? "Nobody yet")
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
                value: count(timeline.value?.children.length ?? 0, "task"),
              },
              { label: "Created", value: when(task.createdAt, now) },
              ...(task.startedAt ? [{ label: "Started", value: when(task.startedAt, now) }] : []),
              ...(task.completedAt
                ? [{ label: "Finished", value: when(task.completedAt, now) }]
                : []),
            ]}
          />
        </Panel>

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
            onSelect={(taskId) => {
              if (taskId !== id) go({ view: "task", id: taskId });
            }}
          />
        </Panel>

        <Panel id="task-result" title="Result" wide>
          {reportError && !report ? (
            <p className="page__note page__note--error" role="alert">
              Couldn't build the result: {reportError}
            </p>
          ) : !report ? (
            <LoadingState label="Loading the result" lines={3} />
          ) : isRoot ? (
            // The result keeps the look it has on the Projects page.
            <div className="view page__result">
              <ObjectiveResult
                report={report}
                openLabel={null}
                onOpenTask={(taskId) => go({ view: "task", id: taskId })}
                onOpenApprovals={() => go({ view: "approvals", id: null })}
              />
            </div>
          ) : (
            <EmptyState
              compact
              title={
                own?.summary
                  ? firstLine(own.summary, 300)
                  : task.state === "succeeded"
                    ? "Finished, with nothing to report"
                    : "No result yet"
              }
            >
              This task is one part of an objective; its whole result is on the objective's page.
            </EmptyState>
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
