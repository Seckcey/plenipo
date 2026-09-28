import type { WorkView } from "@plenipo/types";
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
  type RowItem,
} from "@plenipo/ui";

import { getScopeEvents, getWork } from "../api/commands";
import type { Go } from "../components/views";
import { STATUS_LABEL, positionToolLabel } from "../org/format";
import { POSITION_STATUS } from "../org/cards";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { useNow } from "../runtime/useNow";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { PageMissing, Strip } from "./parts";
import { positionRow, taskRows, workingRows } from "./rows";
import { changesWork, useLive } from "./useLive";
import { useStrips } from "./useStrips";
import { count, departmentPositions, isBusy, when } from "./words";

/** Everything a department has in hand: its head's own tasks and its team's unfinished ones. */
function queueOf(work: WorkView) {
  return [...work.running, ...work.waiting, ...work.queued, ...work.team];
}

/**
 * A department's page (Phase 12): its Manager, projects, workers now, queue, and activity (the
 * last day, the last week, and every event).
 */
export function DepartmentPage({
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
  const department = org?.departments.find((d) => d.id === id) ?? null;
  const headId = department?.headPositionId ?? null;
  const work = useLive<WorkView>(headId, (h) => getWork(h), changesWork, 800);
  const strips = useStrips({ kind: "department", id });

  if (!org) {
    return organization.status === "error" ? (
      <PageMissing
        kind="department"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
        error={organization.error}
        onRetry={() => void organization.reload()}
      />
    ) : (
      <div className="page">
        <LoadingState label="Loading the department" lines={6} />
      </div>
    );
  }
  if (!department) {
    return (
      <PageMissing
        kind="department"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
      />
    );
  }

  const t = titlesOf(org);
  const manager = rankName(t, "departmentManager");
  const head = headId ? org.positions.find((p) => p.id === headId) : undefined;
  const positions = departmentPositions(org, id);
  // A project deleted for good leaves every list (its short record still names old work).
  const projects = org.projects.filter((p) => p.departmentId === id && !p.deleted);
  const busy = positions.filter(isBusy);
  const working = workingRows(org, go, positions);
  const queue = work.value ? taskRows(queueOf(work.value), go, now) : [];

  const projectRows: RowItem[] = projects.map((p) => {
    const lead = p.coordinatorPositionId
      ? org.positions.find((x) => x.id === p.coordinatorPositionId)
      : undefined;
    return {
      id: p.id,
      title: p.name,
      detail: lead
        ? `${rankName(t, "projectCoordinator")}: ${lead.title}`
        : `No ${rankName(t, "projectCoordinator")} yet`,
      status: !p.active
        ? { status: "offline", label: "Archived" }
        : lead
          ? { status: POSITION_STATUS[lead.status], label: STATUS_LABEL[lead.status] }
          : { status: "offline", label: "No lead" },
      onOpen: () => go({ view: "project", id: p.id }),
    };
  });

  return (
    <div className="page">
      <PageHeader
        id="department-title"
        kicker="Department"
        title={department.name}
        lead={department.description || undefined}
        status={
          department.active ? (
            head ? (
              <StatusPill status={POSITION_STATUS[head.status]} label={STATUS_LABEL[head.status]} />
            ) : (
              <StatusPill status="offline" label={`No ${manager} yet`} />
            )
          ) : (
            <StatusPill
              status="offline"
              label={
                department.deleted
                  ? "Deleted for good"
                  : department.archivedAt !== null
                    ? "Archived"
                    : "Inactive"
              }
            />
          )
        }
        onBack={onBack}
        actions={
          <Button
            size="sm"
            icon="organization"
            onClick={() => go({ view: "organization", id: head?.id ?? null })}
          >
            Show on the map
          </Button>
        }
      />

      <div className="page__grid">
        <Panel id="department-about" title="About">
          <PropertyList
            items={[
              {
                label: manager,
                value: head ? (
                  <CellLink onClick={() => go({ view: "worker", id: head.id })}>
                    {head.title}
                  </CellLink>
                ) : (
                  `No ${manager} yet`
                ),
              },
              ...(head ? [{ label: "AI tool", value: positionToolLabel(org, head) }] : []),
              {
                label: "Projects",
                value: count(projects.filter((p) => p.active).length, "project"),
              },
              { label: "Positions", value: count(positions.length, "position") },
              { label: "Working now", value: count(busy.length, "position") },
              { label: "Started", value: when(department.createdAt, now) },
            ]}
          />
        </Panel>

        <Panel id="department-activity" title="Activity">
          <Strip
            heading="Last 24 hours"
            label={`${department.name} activity, last 24 hours`}
            live={strips.day}
            now={now}
          />
          <Strip
            heading="Last 7 days"
            label={`${department.name} activity, last 7 days`}
            live={strips.week}
            now={now}
          />
        </Panel>

        <Panel
          id="department-projects"
          title="Projects"
          count={projects.length}
          countLabel="projects"
        >
          <RowList
            label="Projects"
            items={projectRows}
            empty={<EmptyState compact title="No projects yet" />}
          />
        </Panel>

        <Panel
          id="department-working"
          title="Working now"
          count={working.length}
          countLabel="working"
        >
          <RowList
            label="Working now"
            items={working}
            empty={<EmptyState compact title="Nobody here is working right now" />}
          />
        </Panel>

        <Panel id="department-queue" title="Queue" count={queue.length} countLabel="tasks in hand">
          <RowList
            label="Queue"
            items={queue}
            state={headId ? work.status : "ready"}
            error={work.error}
            onRetry={work.reload}
            empty={
              <EmptyState compact title="Nothing in hand">
                Tasks the department is working on or waiting to start show here.
              </EmptyState>
            }
          />
        </Panel>

        <Panel
          id="department-positions"
          title="Everyone"
          count={positions.length}
          countLabel="positions"
        >
          <RowList
            label="Everyone in the department"
            items={positions.map((p) => positionRow(org, go, p))}
            empty={<EmptyState compact title={`Nobody works here yet`} />}
          />
        </Panel>

        <Panel id="department-history" title="History" wide>
          <EventHistory
            label={`${department.name} history`}
            historyKey={`department:${id}`}
            load={(before) => getScopeEvents({ kind: "department", id }, HISTORY_PAGE, before)}
            now={now}
            onOpenTask={(taskId) => go({ view: "task", id: taskId })}
          />
        </Panel>
      </div>
    </div>
  );
}
