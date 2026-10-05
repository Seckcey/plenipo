import type {
  AgentSessionDetail,
  AgentTurn,
  GrantView,
  PermissionsSnapshot,
  WorkView,
} from "@plenipo/types";
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
  type Status,
} from "@plenipo/ui";

import { AskQuestionButton } from "../components/sideChat/AskQuestion";
import { useChatIfAny } from "../chat/context";
import { positionChatTarget } from "../chat/positionTarget";
import { LiveConversation } from "../live/LiveConversation";
import { liveWork } from "../live/words";
import { StopButton } from "../components/stop/StopWork";
import { workToStop } from "../components/stop/whatToStop";
import { getAgentSession, getPermissions, getScopeEvents, getWork } from "../api/commands";
import type { Go } from "../components/views";
import { OUTCOME_LABEL, outcomeTone } from "../agents/format";
import { POSITION_STATUS } from "../org/cards";
import { STAFFING_LABEL, STATUS_LABEL, ago, positionToolLabel } from "../org/format";
import { rankName, titlesOf } from "../org/titles";
import { useOrganization } from "../org/useOrganization";
import { useNow } from "../runtime/useNow";
import { useOpenWatch } from "../terminal/useTerminal";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { PageMissing } from "./parts";
import { taskRows } from "./rows";
import { changesWork, useLive } from "./useLive";
import { count, firstLine, when } from "./words";

const TONE: Record<ReturnType<typeof outcomeTone>, Status> = {
  succeeded: "ok",
  failed: "error",
  blocked: "warn",
  cancelled: "offline",
};

/** A turn of the conversation: what it was asked, and how it ended (or that it is going). */
function turnRow(turn: AgentTurn, go: Go, now: number): RowItem {
  const status: { status: Status; label: string } = turn.running
    ? { status: "ok", label: "Working" }
    : turn.waiting
      ? { status: "pending", label: "Waiting on replies" }
      : turn.result
        ? {
            status: TONE[outcomeTone(turn.result.outcome)],
            label: OUTCOME_LABEL[turn.result.outcome],
          }
        : { status: "offline", label: "Ended" };
  return {
    id: turn.taskId,
    title: `${turn.number}. ${firstLine(turn.objective) || "(no objective)"}`,
    detail: turn.result
      ? firstLine(turn.result.text ?? turn.result.summary, 200)
      : turn.running
        ? "Working on it now"
        : undefined,
    status,
    meta: ago(turn.endedAt ?? turn.startedAt, now),
    onOpen: () => go({ view: "task", id: turn.taskId }),
  };
}

/** A worker's permissions in use now: what it may do for one step, and how they were used. */
function grantRow(g: GrantView, go: Go, now: number): RowItem {
  return {
    id: g.grantId,
    title: g.permissions.map((p) => p.label).join(", ") || "No permissions",
    detail: [
      `Step ${g.step}`,
      g.project,
      `${g.used} used`,
      g.blocked > 0 ? `${g.blocked} blocked` : null,
      g.asked > 0 ? `${g.asked} asked you` : null,
    ]
      .filter(Boolean)
      .join(" · "),
    status: g.revoked ? { status: "offline", label: "Revoked" } : { status: "ok", label: "In use" },
    meta: ago(g.openedAt, now),
    onOpen: () => go({ view: "task", id: g.taskId }),
    openLabel: `Permissions for step ${g.step}. Open its task`,
  };
}

/**
 * A worker's page (Phase 12), for one position: its role, AI tool and model (and why), what it is
 * working on, its queue, the permissions it is using now, its conversation, and its history.
 */
export function WorkerPage({
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
  const organization = useOrganization();
  const org = organization.snapshot;
  const p = org?.positions.find((x) => x.id === id) ?? null;
  const openWatch = useOpenWatch();
  const chat = useChatIfAny();
  const work = useLive<WorkView>(p ? id : null, (k) => getWork(k), changesWork, 800);
  const permissions = useLive<PermissionsSnapshot>(
    p ? "permissions" : null,
    () => getPermissions(),
    (e) =>
      e.eventType.startsWith("guard.") ||
      e.eventType.startsWith("approval.") ||
      e.eventType === "capability.used",
    800,
  );
  const sessionId =
    p?.agent?.sessionId ?? p?.currentTask?.sessionId ?? p?.workers[0]?.sessionId ?? null;
  const session = useLive<AgentSessionDetail>(
    sessionId,
    getAgentSession,
    (e) =>
      e.eventType.startsWith("session.") ||
      e.eventType === "agent.result" ||
      e.eventType === "task.state_changed",
    800,
  );

  if (!org) {
    return organization.status === "error" ? (
      <PageMissing
        kind="worker"
        onBack={onBack}
        onHome={() => go({ view: "home", id: null })}
        error={organization.error}
        onRetry={() => void organization.reload()}
      />
    ) : (
      <div className="page">
        <LoadingState label="Loading the worker" lines={6} />
      </div>
    );
  }
  if (!p) {
    return (
      <PageMissing kind="worker" onBack={onBack} onHome={() => go({ view: "home", id: null })} />
    );
  }

  const t = titlesOf(org);
  const boss = p.reportsTo ? org.positions.find((x) => x.id === p.reportsTo) : undefined;
  const department = org.departments.find((d) => d.id === p.departmentId);
  const project = org.projects.find((x) => x.id === p.projectId);
  const grants = (permissions.value?.grants ?? []).filter((g) => g.positionId === id && !g.revoked);
  const inHand = work.value
    ? [...work.value.running, ...work.value.waiting, ...work.value.queued]
    : [];
  const team = work.value?.team ?? [];
  const turns = [...(session.value?.turns ?? [])].sort((a, b) => b.number - a.number).slice(0, 6);
  const route = p.route;
  const live = liveWork(p);
  // Its chat in a window of its own (ADR-203).
  const chatTarget = chat?.canPopOut ? positionChatTarget(p) : null;

  return (
    <div className="page">
      <PageHeader
        id="worker-title"
        kicker={rankName(t, p.kind)}
        title={p.title}
        lead={`${p.roleName} · ${STAFFING_LABEL[p.staffing]}`}
        status={
          <StatusPill
            status={POSITION_STATUS[p.status]}
            label={
              p.inWorkforce
                ? "In your Workforce"
                : p.deleted
                  ? "Deleted for good"
                  : STATUS_LABEL[p.status]
            }
          />
        }
        onBack={onBack}
        actions={
          <>
            {sessionId && (
              <Button size="sm" icon="workers" onClick={() => onOpenSession(sessionId)}>
                Open the conversation
              </Button>
            )}
            {/* Watch from its own page too (Phase 25, item 1.8). */}
            {openWatch && p.active && (p.agent || p.staffing !== "persistent") && (
              <Button size="sm" onClick={() => openWatch(p.id, p.title)}>
                Watch
              </Button>
            )}
            <Button
              size="sm"
              icon="organization"
              onClick={() => go({ view: "organization", id: p.id })}
            >
              Show on the map
            </Button>
            {chat && chatTarget && (
              <Button size="sm" icon="external" onClick={() => chat.openWindow(chatTarget)}>
                Pop out chat
              </Button>
            )}
            {/* A side chat while it works (Phase 25, item 3.5). */}
            <AskQuestionButton p={p} onAsked={onOpenSession} />
            {/* Phase 25, item 3.3. */}
            <StopButton who={p.title} work={workToStop(p)} fullTime={p.staffing === "persistent"} />
          </>
        }
      />

      <div className="page__grid">
        {/* What it says and does now, live (Phase 25, item 3.1). */}
        {live.length > 0 && (
          <Panel id="worker-live" title="Live conversation" wide>
            {live.map((w) => (
              <div key={w.taskId} className="worker-live">
                {live.length > 1 && <h3 className="worker-live__task">{w.objective}</h3>}
                <LiveConversation
                  taskId={w.taskId}
                  sessionId={w.sessionId}
                  startedAt={w.startedAt}
                  running
                  who={p.title}
                />
              </div>
            ))}
          </Panel>
        )}
        <Panel id="worker-about" title="About">
          <PropertyList
            items={[
              { label: "Role", value: p.roleName },
              {
                label: "Reports to",
                value: boss ? (
                  <CellLink onClick={() => go({ view: "worker", id: boss.id })}>
                    {boss.title}
                  </CellLink>
                ) : (
                  "You"
                ),
              },
              ...(department
                ? [
                    {
                      label: "Department",
                      value: (
                        <CellLink onClick={() => go({ view: "department", id: department.id })}>
                          {department.name}
                        </CellLink>
                      ),
                    },
                  ]
                : []),
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
              { label: "Staffing", value: STAFFING_LABEL[p.staffing] },
              { label: "AI tool", value: positionToolLabel(org, p) },
              { label: "Model", value: p.model ?? "The AI tool's own choice" },
              ...(route ? [{ label: "Why this AI tool", value: route.reason }] : []),
              ...(p.statusDetail ? [{ label: "Note", value: p.statusDetail }] : []),
              ...(p.agent ? [{ label: "Hired", value: when(p.agent.hiredAt, now) }] : []),
              {
                label: "Workers who left",
                value:
                  p.history.retired + p.history.failed === 0
                    ? "None yet"
                    : `${count(p.history.retired, "finished", "finished")}, ${count(p.history.failed, "failed", "failed")}`,
              },
            ]}
          />
        </Panel>

        <Panel id="worker-now" title="Working on" count={inHand.length} countLabel="tasks in hand">
          <RowList
            label="Working on"
            items={taskRows(inHand, go, now)}
            state={work.status}
            error={work.error}
            onRetry={work.reload}
            empty={<EmptyState compact title="Nothing in hand right now" />}
          />
        </Panel>

        {team.length > 0 && (
          <Panel id="worker-team" title="Their team's work" count={team.length} countLabel="tasks">
            <RowList label="Their team's work" items={taskRows(team, go, now)} />
          </Panel>
        )}

        <Panel
          id="worker-permissions"
          title="Permissions in use"
          count={grants.length}
          countLabel="in use"
        >
          <RowList
            label="Permissions in use"
            items={grants.map((g) => grantRow(g, go, now))}
            state={permissions.status}
            error={permissions.error}
            onRetry={permissions.reload}
            empty={
              <EmptyState compact title="Not using any permissions right now">
                A worker gets permissions for one step of a task, from its role's permission set.
              </EmptyState>
            }
          />
        </Panel>

        <Panel
          id="worker-conversation"
          title="Conversation"
          actions={
            sessionId ? (
              <Button size="sm" onClick={() => onOpenSession(sessionId)}>
                Open
              </Button>
            ) : undefined
          }
        >
          <RowList
            label="Conversation"
            items={turns.map((turn) => turnRow(turn, go, now))}
            state={sessionId ? session.status : "ready"}
            error={session.error}
            onRetry={session.reload}
            empty={
              <EmptyState compact title="No conversation yet">
                {p.staffing === "persistent"
                  ? "It starts with the first objective or task this position gets."
                  : "Each worker on call has its own conversation while it works."}
              </EmptyState>
            }
          />
        </Panel>

        <Panel id="worker-recent" title="Recently finished">
          <RowList
            label="Recently finished"
            items={taskRows(work.value?.recent.slice(0, 10) ?? [], go, now)}
            state={work.status}
            error={work.error}
            onRetry={work.reload}
            empty={<EmptyState compact title="Nothing finished yet" />}
          />
        </Panel>

        <Panel id="worker-history" title="History" wide>
          <EventHistory
            label={`${p.title} history`}
            historyKey={`position:${id}`}
            load={(before) => getScopeEvents({ kind: "position", id }, HISTORY_PAGE, before)}
            now={now}
            onOpenTask={(taskId) => go({ view: "task", id: taskId })}
          />
        </Panel>
      </div>
    </div>
  );
}
