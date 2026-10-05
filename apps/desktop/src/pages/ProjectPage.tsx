import { useState } from "react";
import type { TaskTree, WorkRecord, Workspace } from "@plenipo/types";
import {
  Button,
  CellLink,
  EmptyState,
  LoadingState,
  PageHeader,
  Panel,
  PropertyList,
  RowList,
  Select,
  StatusPill,
  TopologyMap,
  type RowItem,
} from "@plenipo/ui";

import { getProjectRecord, getScopeEvents, getTaskTree } from "../api/commands";
import { EditProjectButton } from "../components/org/EditProject";
import type { Go } from "../components/views";
import { POSITION_STATUS } from "../org/cards";
import { STATUS_LABEL, ago, runtimeLabel } from "../org/format";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { useProjectWork } from "../projects/useProjectWork";
import { useNow } from "../runtime/useNow";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { PageMissing, Screenshots } from "./parts";
import { decisionRows, pullRequestRows, workingRows } from "./rows";
import { treeMap } from "./tree";
import { changesWork, useLive } from "./useLive";
import { TASK_STATUS, count, firstLine, projectPositions, when } from "./words";

/** A working copy's row: its branch, what is on it, and whether it was pushed. */
function copyRow(w: Workspace, now: number): RowItem {
  const changes = [
    count(w.facts.commits.length, "commit"),
    w.facts.uncommitted > 0 ? `${count(w.facts.uncommitted, "file")} not committed` : null,
    w.facts.pushed ? "pushed" : "not pushed",
  ]
    .filter(Boolean)
    .join(" · ");
  return {
    id: w.id,
    title: w.branch,
    detail: w.state === "removed" ? `${changes} · working copy removed; the branch stays` : changes,
    status:
      w.state === "removed"
        ? { status: "offline", label: "Removed" }
        : { status: "ok", label: "Working copy" },
    meta: ago(w.updatedAt, now),
  };
}

/**
 * A project's page (Phase 12): its Supervisor, folder and repository, the task tree of each
 * objective, who's working, branches and pull requests, screenshots, recent decisions, and its
 * history.
 */
export function ProjectPage({
  id,
  go,
  onBack,
}: {
  id: string;
  go: Go;
  onBack?: (() => void) | undefined;
}) {
  const now = useNow(30_000);
  const organization = useOrganization();
  const org = organization.snapshot;
  const project = org?.projects.find((p) => p.id === id) ?? null;
  const { work, error: workError, reload: reloadWork } = useProjectWork(project ? id : null);
  const record = useLive<WorkRecord>(project ? id : null, getProjectRecord, changesWork, 800);
  const [picked, setPicked] = useState<string | null>(null);
  const objectives = work?.objectives ?? [];
  const shown =
    (picked && objectives.some((o) => o.rootTaskId === picked) ? picked : null) ??
    objectives[0]?.rootTaskId ??
    null;
  const tree = useLive<TaskTree>(shown, getTaskTree, changesWork, 800);

  if (!org) {
    return organization.status === "error" ? (
      <PageMissing
        kind="project"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
        error={organization.error}
        onRetry={() => void organization.reload()}
      />
    ) : (
      <div className="page">
        <LoadingState label="Loading the project" lines={6} />
      </div>
    );
  }
  if (!project) {
    return (
      <PageMissing kind="project" onBack={onBack} onHome={() => go({ view: "home", id: null })} />
    );
  }

  const t = titlesOf(org);
  const supervisor = rankName(t, "projectCoordinator");
  const lead = project.coordinatorPositionId
    ? org.positions.find((p) => p.id === project.coordinatorPositionId)
    : undefined;
  const department = org.departments.find((d) => d.id === project.departmentId);
  const working = workingRows(org, go, projectPositions(org, id));
  const map = tree.value ? treeMap(tree.value, org) : null;
  const workState = work ? "ready" : workError ? "error" : "loading";

  const objectiveRows: RowItem[] = objectives.map((o) => ({
    id: o.rootTaskId,
    title: firstLine(o.objective) || "(no objective)",
    detail: [
      o.positionTitle,
      count(o.tasks, "task"),
      o.active > 0 ? `${o.active} going` : null,
      o.failed > 0 ? `${o.failed} failed` : null,
      o.waitingApprovals > 0 ? `${o.waitingApprovals} waiting for you` : null,
      o.branch,
    ]
      .filter(Boolean)
      .join(" · "),
    status: TASK_STATUS[o.state],
    meta: ago(o.createdAt, now),
    onOpen: () => go({ view: "task", id: o.rootTaskId }),
  }));

  return (
    <div className="page">
      <PageHeader
        id="project-title"
        kicker={department ? `Project · ${department.name}` : "Project"}
        title={project.name}
        lead={project.description || undefined}
        status={
          !project.active ? (
            <StatusPill
              status="offline"
              label={project.deleted ? "Deleted for good" : "Archived"}
            />
          ) : lead ? (
            <StatusPill status={POSITION_STATUS[lead.status]} label={STATUS_LABEL[lead.status]} />
          ) : (
            <StatusPill status="offline" label={`No ${supervisor} yet`} />
          )
        }
        onBack={onBack}
        actions={
          project.active ? (
            <>
              <EditProjectButton snapshot={org} project={project} onSaved={organization.apply} />
              <Button size="sm" variant="primary" onClick={() => go({ view: "projects", id })}>
                Give an objective
              </Button>
            </>
          ) : undefined
        }
      />

      <div className="page__grid">
        <Panel id="project-about" title="About">
          <PropertyList
            items={[
              {
                label: supervisor,
                value: lead ? (
                  <CellLink onClick={() => go({ view: "worker", id: lead.id })}>
                    {lead.title}
                  </CellLink>
                ) : (
                  `No ${supervisor} yet`
                ),
              },
              {
                label: "Department",
                value: department ? (
                  <CellLink onClick={() => go({ view: "department", id: department.id })}>
                    {department.name}
                  </CellLink>
                ) : (
                  "None"
                ),
              },
              {
                label: "Project folder",
                value: project.localPath ? (
                  <code className="page__path">{project.localPath}</code>
                ) : (
                  "None (workers get no file tools)"
                ),
              },
              {
                label: "Repository",
                value: project.repositoryUrl ? (
                  <code className="page__path">{project.repositoryUrl}</code>
                ) : (
                  "None"
                ),
              },
              {
                label: "Branches",
                value: project.branchPerObjective
                  ? "A new branch and working copy for each objective"
                  : "Workers change the project folder itself",
              },
              {
                label: "AI tools allowed",
                value:
                  project.allowedRuntimes.length === 0
                    ? "None"
                    : project.allowedRuntimes.map((r) => runtimeLabel(org, r)).join(", "),
              },
              { label: "Started", value: when(project.createdAt, now) },
            ]}
          />
        </Panel>

        <Panel id="project-working" title="Working now" count={working.length} countLabel="working">
          <RowList
            label="Working now"
            items={working}
            empty={<EmptyState compact title="Nobody is working on this project right now" />}
          />
        </Panel>

        <Panel
          id="project-objectives"
          title="Objectives"
          count={objectives.length}
          countLabel="objectives"
          wide
        >
          <RowList
            label="Objectives"
            items={objectiveRows}
            state={workState}
            error={workError}
            onRetry={() => void reloadWork()}
            empty={
              <EmptyState compact title="No objectives yet">
                Give the project an objective to get its team working.
              </EmptyState>
            }
          />
        </Panel>

        <Panel
          id="project-tree"
          title="Task tree"
          wide
          actions={
            objectives.length > 1 && shown ? (
              <Select
                label="Objective"
                hideLabel
                value={shown}
                options={objectives.map((o) => ({
                  value: o.rootTaskId,
                  label: firstLine(o.objective, 60) || "(no objective)",
                }))}
                onChange={setPicked}
              />
            ) : undefined
          }
        >
          <TopologyMap
            label="Task tree"
            nodes={map?.nodes ?? []}
            links={map?.links ?? []}
            // Until the objectives are read, the tree waits on them (and says so if they fail).
            state={workState !== "ready" ? workState : shown === null ? "ready" : tree.status}
            error={workState === "error" ? workError : tree.error}
            onRetry={workState === "error" ? () => void reloadWork() : tree.reload}
            onSelect={(taskId) => go({ view: "task", id: taskId })}
            empty={<EmptyState compact title="No objectives yet, so no tasks to map" />}
          />
        </Panel>

        <Panel id="project-branches" title="Branches and pull requests">
          <RowList
            label="Branches and pull requests"
            items={[
              ...pullRequestRows(record.value?.pullRequests ?? [], go, now),
              ...(work?.workingCopies ?? []).map((w) => copyRow(w, now)),
            ]}
            state={record.status}
            error={record.error}
            onRetry={record.reload}
            empty={
              <EmptyState compact title="No branches or pull requests yet">
                {project.branchPerObjective
                  ? "Each objective gets its own branch when the folder is a git repository."
                  : "Workers change the project folder itself, so there are no branches."}
              </EmptyState>
            }
          />
        </Panel>

        <Panel id="project-decisions" title="Recent decisions">
          <RowList
            label="Recent decisions"
            items={decisionRows(record.value?.decisions ?? [], go, now)}
            state={record.status}
            error={record.error}
            onRetry={record.reload}
            empty={<EmptyState compact title="No decisions yet" />}
          />
        </Panel>

        <Panel id="project-screenshots" title="Screenshots">
          {record.value ? (
            <Screenshots artifacts={record.value.artifacts} now={now} />
          ) : (
            <RowList
              label="Screenshots"
              items={[]}
              state={record.status === "error" ? "error" : "loading"}
              error={record.error}
              onRetry={record.reload}
            />
          )}
        </Panel>

        <Panel id="project-history" title="History" wide>
          <EventHistory
            label={`${project.name} history`}
            historyKey={`project:${id}`}
            load={(before) => getScopeEvents({ kind: "project", id }, HISTORY_PAGE, before)}
            now={now}
            onOpenTask={(taskId) => go({ view: "task", id: taskId })}
          />
        </Panel>
      </div>
    </div>
  );
}
