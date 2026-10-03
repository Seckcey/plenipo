import { useMemo } from "react";
import type {
  ActivityScope,
  ActivitySeries,
  HomeView,
  OrgSnapshot,
  StuckItem,
} from "@plenipo/types";
import {
  Button,
  EmptyState,
  EntityCard,
  EntityCardSkeleton,
  ErrorState,
  Hero,
  Panel,
  RowList,
  StatGrid,
  type RowItem,
  type StatItem,
} from "@plenipo/ui";

import { SetupTourButton } from "../tour/SetupTourButton";
import { getHome, hirePosition } from "../api/commands";
import type { Go } from "../components/views";
import type { useApprovals } from "../guard/usePermissions";
import type { Learning } from "../learning/useLearning";
import { describeEvent } from "../ledger/format";
import { useActivity } from "../ledger/useActivity";
import { ago } from "../org/format";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { useNow } from "../runtime/useNow";
import { useOpenWatch } from "../terminal/useTerminal";
import { approvalRows, workingRows } from "./rows";
import { changesWork, useLive } from "./useLive";
import {
  TASK_STATUS,
  count,
  departmentHealth,
  departmentPositions,
  eventStatus,
  firstLine,
  greeting,
  homeLine,
  homePip,
  isBusy,
  type HomeMood,
} from "./words";

type Approvals = ReturnType<typeof useApprovals>;

/**
 * A lead needs a worker for a job nobody in its department does (Phase 25, item 2.7): choosing
 * the row hires one on call for its team.
 */
function hireRow(item: StuckItem, now: number): RowItem {
  const p = (item.event.payload ?? {}) as Record<string, unknown>;
  const lead = typeof p.lead === "string" ? p.lead : "A lead";
  const role = typeof p.role === "string" ? p.role : "worker";
  return {
    id: `stuck:${item.event.seq}`,
    title: `${lead} needs a ${role}`,
    detail: `Nobody in its department does this job. Hire one? Choose this to hire a ${role} on call for its team.`,
    status: eventStatus(item.event),
    meta: ago(item.event.createdAt, now),
    onOpen: () =>
      void hirePosition({
        roleId: typeof p.roleId === "string" ? p.roleId : "",
        title: role,
        reportsTo: typeof p.leadId === "string" ? p.leadId : null,
      }).catch(() => undefined),
    openLabel: `Hire a ${role} for ${lead}'s team`,
  };
}

/** A stuck item's row: its task (or the server), what went wrong, and where to fix it. */
function stuckRow(item: StuckItem, go: Go, now: number): RowItem {
  if (item.event.eventType === "org.hire_needed") return hireRow(item, now);
  const what = describeEvent(item.event);
  const task = item.task;
  return {
    id: `stuck:${item.event.seq}`,
    title: task ? firstLine(task.objective) : what,
    detail: task ? `${task.positionTitle ?? "Nobody"} · ${what}` : "Fix it in Settings → Servers",
    status: eventStatus(item.event),
    meta: ago(item.event.createdAt, now),
    onOpen: task
      ? () => go({ view: "task", id: task.id })
      : () => go({ view: "settings", id: "servers" }),
    openLabel: task
      ? `${firstLine(task.objective)}: ${what}. Open the task`
      : `${what}. Open Settings`,
  };
}

/** Bring a part of Home into view and move the focus to its heading. */
function showPanel(id: string) {
  const heading = document.getElementById(id);
  if (!heading) return;
  if (!heading.hasAttribute("tabindex")) heading.setAttribute("tabindex", "-1");
  heading.scrollIntoView?.({ block: "start" });
  heading.focus({ preventScroll: true });
}

/**
 * Home (Phase 12): Pip's greeting and how things are, then what's waiting for the owner,
 * what's stuck, each department's health, the objectives going, who's working, and what just
 * finished. Everything opens its own page.
 */
export function HomePage({
  go,
  approvals,
  learning,
}: {
  go: Go;
  approvals: Approvals;
  learning: Learning;
}) {
  const now = useNow(30_000);
  const home = useLive<HomeView>("home", getHome, changesWork, 800);
  const organization = useOrganization();
  const openWatch = useOpenWatch();
  const org = organization.snapshot;
  const departments = useMemo(() => org?.departments.filter((d) => d.active) ?? [], [org]);
  const scopes = useMemo(
    () => departments.map((d): ActivityScope => ({ kind: "department", id: d.id })),
    [departments],
  );
  const activity = useActivity(scopes);

  const pending = approvals.queue?.pending ?? [];
  const lessons = learning.snapshot?.waiting ?? [];
  const unavailable = org?.positions.filter((p) => p.active && p.status === "unavailable") ?? [];
  const stuck = home.value?.stuck ?? [];
  const working = org
    ? workingRows(
        org,
        go,
        org.positions.filter((p) => p.active),
        openWatch,
      )
    : [];
  const finishedDay = home.value?.finishedDay ?? 0;
  // What couldn't be read is never shown as "nothing" (nothing waiting, nothing stuck).
  const waitingError = approvals.error ?? learning.error;
  const mood: HomeMood = {
    loading: organization.status === "loading" || home.status === "loading",
    failed: organization.status === "error" || home.status === "error" || waitingError !== null,
    empty:
      org !== null &&
      org.departments.length === 0 &&
      org.positions.filter((p) => p.active).length <= 1 &&
      (home.value?.current.length ?? 0) === 0,
    waiting: pending.length + lessons.length,
    stuck: stuck.length + unavailable.length,
    working: working.length,
    finishedDay,
  };
  const t = org ? titlesOf(org) : null;

  const waitingRows: RowItem[] = [
    ...approvalRows(pending, go, now),
    ...lessons.map((l): RowItem => ({
      id: `lesson:${l.id}`,
      title: `${l.worker} learned something`,
      detail: firstLine(l.text),
      status: { status: "pending", label: "A lesson to keep" },
      meta: ago(l.createdAt, now),
      onOpen: () => go({ view: "approvals", id: null }),
      openLabel: `${l.worker} learned something. Review it`,
    })),
  ];

  const stuckRows: RowItem[] = [
    ...stuck.map((s) => stuckRow(s, go, now)),
    ...unavailable.map((p): RowItem => ({
      id: `unavailable:${p.id}`,
      title: `${p.title} can't work`,
      detail: p.statusDetail ?? "No AI tool can take its work now",
      status: { status: "warn", label: "Can't work" },
      onOpen: () => go({ view: "worker", id: p.id }),
    })),
  ];

  // One stuck thing opens it; more show the list (Phase 25, item 1.7).
  const openStuck = () => {
    const only = stuckRows.length === 1 ? stuckRows[0]?.onOpen : undefined;
    if (only) only();
    else showPanel("home-stuck");
  };
  const current = home.value?.current ?? [];
  const openObjectives = () => {
    const only = current.length === 1 ? current[0] : undefined;
    if (only) go({ view: "task", id: only.rootTaskId });
    else showPanel("home-objectives");
  };

  const stats: StatItem[] = [
    waitingError
      ? {
          label: "Waiting for you",
          value: "—",
          hint: "Couldn't check",
          onOpen: () => go({ view: "approvals", id: null }),
        }
      : {
          label: "Waiting for you",
          value: mood.waiting,
          status: mood.waiting > 0 ? "pending" : "ok",
          hint: `${count(pending.length, "request")}, ${count(lessons.length, "lesson")}`,
          onOpen: () => go({ view: "approvals", id: null }),
        },
    home.status === "error"
      ? { label: "Stuck", value: "—", hint: "Couldn't check", onOpen: openStuck }
      : {
          label: "Stuck",
          value: mood.stuck,
          status: mood.stuck > 0 ? "error" : "ok",
          hint:
            mood.stuck === 1 ? "Open it" : mood.stuck > 1 ? "See What's stuck" : "Nothing stuck",
          onOpen: openStuck,
        },
    {
      label: "Working now",
      value: working.length,
      status: working.length > 0 ? "ok" : "offline",
      hint: org ? `${count(org.stats.queued, "task")} queued` : undefined,
      onOpen: () => go({ view: "workers", id: null }),
    },
    {
      label: "Objectives going",
      value: home.value?.going ?? "—",
      hint: "Given to your company",
      onOpen: openObjectives,
    },
    {
      label: "Finished",
      value: home.value ? finishedDay : "—",
      status: finishedDay > 0 ? "ok" : undefined,
      hint: org
        ? `In the last day · ${count(org.stats.failed24h, "task")} failed`
        : "In the last day",
      onOpen: () => go({ view: "activity", id: null }),
    },
  ];

  return (
    <div className="page page--home">
      <Hero
        id="home-title"
        pip={homePip(mood)}
        title={greeting(new Date(now).getHours())}
        actions={
          <>
            {mood.waiting > 0 ? (
              <Button variant="primary" onClick={() => go({ view: "approvals", id: null })}>
                Review what's waiting
              </Button>
            ) : mood.empty ? (
              <Button variant="primary" onClick={() => go({ view: "organization", id: null })}>
                Set up your company
              </Button>
            ) : null}
            <SetupTourButton />
          </>
        }
      >
        {homeLine(mood)}
      </Hero>

      <StatGrid label="How things are" stats={stats} />

      <div className="page__grid">
        <Panel
          id="home-waiting"
          title="Waiting for you"
          count={waitingRows.length}
          countLabel="waiting for you"
          actions={
            waitingRows.length > 0 ? (
              <Button size="sm" onClick={() => go({ view: "approvals", id: null })}>
                Review
              </Button>
            ) : undefined
          }
        >
          <RowList
            label="Waiting for you"
            items={waitingRows}
            state={waitingError ? "error" : approvals.queue === null ? "loading" : "ready"}
            error={waitingError}
            onRetry={() => {
              void approvals.reload();
              void learning.reload();
            }}
            empty={<EmptyState compact title="Nothing is waiting for you" />}
          />
        </Panel>

        <Panel id="home-stuck" title="What's stuck" count={stuckRows.length} countLabel="stuck">
          <RowList
            label="What's stuck"
            items={stuckRows}
            state={home.status}
            error={home.error}
            onRetry={home.reload}
            empty={<EmptyState compact title="Nothing is stuck" />}
          />
        </Panel>

        <Panel
          id="home-departments"
          title="Departments"
          count={departments.length}
          countLabel="departments"
          wide
        >
          <Departments
            org={org}
            status={organization.status}
            error={organization.error}
            onRetry={() => void organization.reload()}
            series={activity.series}
            activityStatus={activity.status}
            stuck={stuck}
            now={now}
            go={go}
            manager={t ? rankName(t, "departmentManager") : "Manager"}
          />
        </Panel>

        <Panel
          id="home-objectives"
          title="Current objectives"
          count={home.value?.current.length}
          countLabel="objectives going"
        >
          <RowList
            label="Current objectives"
            state={home.status}
            error={home.error}
            onRetry={home.reload}
            items={(home.value?.current ?? []).map((b): RowItem => ({
              id: b.rootTaskId,
              title: firstLine(b.objective) || "(no objective)",
              detail: [
                b.positionTitle ?? "Nobody yet",
                b.projectId ? org?.projects.find((p) => p.id === b.projectId)?.name : null,
                count(b.tasks, "task"),
                b.waitingApprovals > 0 ? `${b.waitingApprovals} waiting for you` : null,
              ]
                .filter(Boolean)
                .join(" · "),
              status: TASK_STATUS[b.state],
              meta: ago(b.createdAt, now),
              onOpen: () => go({ view: "task", id: b.rootTaskId }),
            }))}
            empty={
              <EmptyState compact title="No objectives are going">
                Give one from Organization or Projects.
              </EmptyState>
            }
          />
        </Panel>

        <Panel id="home-working" title="Who's working" count={working.length} countLabel="working">
          <RowList
            label="Who's working"
            items={working}
            state={organization.status}
            error={organization.error}
            onRetry={() => void organization.reload()}
            empty={<EmptyState compact title="Nobody is working right now" />}
          />
        </Panel>

        <Panel id="home-finished" title="Just finished" wide>
          <RowList
            label="Just finished"
            state={home.status}
            error={home.error}
            onRetry={home.reload}
            items={(home.value?.finished ?? []).map((b): RowItem => ({
              id: b.rootTaskId,
              title: firstLine(b.objective) || "(no objective)",
              detail: b.answer
                ? `${b.positionTitle ?? "Your company"}: ${firstLine(b.answer, 200)}`
                : (b.positionTitle ?? undefined),
              status: TASK_STATUS[b.state],
              meta: ago(b.completedAt ?? b.createdAt, now),
              onOpen: () => go({ view: "task", id: b.rootTaskId }),
            }))}
            empty={<EmptyState compact title="Nothing finished this week yet" />}
          />
        </Panel>
      </div>
    </div>
  );
}

/** A card for each department: its health, its Manager, its projects and workers, its day. */
function Departments({
  org,
  status,
  error,
  onRetry,
  series,
  activityStatus,
  stuck,
  now,
  go,
  manager,
}: {
  org: OrgSnapshot | null;
  status: "loading" | "ready" | "error";
  error: string | null;
  onRetry: () => void;
  series: readonly ActivitySeries[];
  activityStatus: "loading" | "ready" | "error";
  stuck: readonly StuckItem[];
  now: number;
  go: Go;
  manager: string;
}) {
  if (status === "error" && !org) {
    return (
      <ErrorState
        compact
        title="Couldn't load your departments"
        message={error}
        onRetry={onRetry}
      />
    );
  }
  if (!org) {
    return (
      <div className="page__cards">
        <EntityCardSkeleton />
        <EntityCardSkeleton />
      </div>
    );
  }
  const departments = org.departments.filter((d) => d.active);
  if (departments.length === 0) {
    return (
      <EmptyState
        pip="teamwork"
        title="No departments yet"
        action={
          <Button size="sm" onClick={() => go({ view: "organization", id: null })}>
            Open Organization
          </Button>
        }
      >
        Departments group your workers and projects, each led by a {manager}.
      </EmptyState>
    );
  }
  return (
    <ul className="page__cards" aria-label="Departments">
      {departments.map((d, i) => {
        const positions = departmentPositions(org, d.id);
        const ids = new Set(positions.map((p) => p.id));
        const inTrouble = stuck.filter((s) => s.task?.positionId && ids.has(s.task.positionId));
        const s = series[i];
        const health = departmentHealth(positions, s, inTrouble.length);
        const head = d.headPositionId
          ? org.positions.find((p) => p.id === d.headPositionId)
          : undefined;
        const busy = positions.filter(isBusy).length;
        const projects = d.projectIds.filter(
          (id) => org.projects.find((p) => p.id === id)?.active,
        ).length;
        return (
          <li key={d.id}>
            <EntityCard
              title={d.name}
              status={health.status}
              statusLabel={health.label}
              subtype={`${count(positions.length, "position")} · ${busy} working`}
              activity={
                activityStatus === "loading"
                  ? "loading"
                  : activityStatus === "error"
                    ? "error"
                    : (s ?? null)
              }
              now={now}
              owner={{
                icon: "user",
                label: head
                  ? `${manager}: ${head.title}${head.status === "vacant" ? " · Vacant" : ""}`
                  : `No ${manager} yet`,
              }}
              footerNote={projects === 0 ? "No projects yet" : count(projects, "project")}
              onOpen={() => go({ view: "department", id: d.id })}
            />
          </li>
        );
      })}
    </ul>
  );
}
