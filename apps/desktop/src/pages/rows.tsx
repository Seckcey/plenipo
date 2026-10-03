/** Rows the pages share: who's working, tasks, decisions, approvals, and pull requests. */

import type {
  ApprovalStatus,
  ApprovalView,
  LedgerEvent,
  OrgSnapshot,
  PositionInfo,
  PullRequestRef,
  TaskBrief,
} from "@plenipo/types";
import { StatusPill, type RowItem, type Status } from "@plenipo/ui";

import { StopButton } from "../components/stop/StopWork";
import { isStoppable, workToStop } from "../components/stop/stopWork";
import type { Go } from "../components/views";
import { describeEvent } from "../ledger/format";
import { POSITION_STATUS } from "../org/cards";
import { STATUS_LABEL, ago, positionToolLabel } from "../org/format";
import { TASK_STATUS, eventStatus, firstLine } from "./words";

/**
 * Someone working now: a full-time position working or waiting on its team, or an on-call
 * worker working or waiting on replies (not those still queued for a slot).
 */
export function workingRows(
  org: OrgSnapshot,
  go: Go,
  positions: readonly PositionInfo[],
): RowItem[] {
  const rows: RowItem[] = [];
  for (const p of positions) {
    if (p.staffing === "persistent") {
      if (p.status !== "working" && p.status !== "waiting") continue;
      rows.push({
        id: p.id,
        title: p.title,
        detail: p.currentTask ? firstLine(p.currentTask.objective) : STATUS_LABEL[p.status],
        status: { status: POSITION_STATUS[p.status], label: STATUS_LABEL[p.status] },
        meta: positionToolLabel(org, p),
        onOpen: () => go({ view: "worker", id: p.id }),
        // Stop, beside the row (Phase 25, item 3.3).
        actions: <StopButton who={p.title} work={workToStop(p)} fullTime />,
      });
    } else {
      for (const w of p.workers) {
        if (w.state === "queued") continue;
        rows.push({
          id: `${p.id}:${w.agentId}`,
          title: p.title,
          detail: firstLine(w.objective),
          status: TASK_STATUS[w.state],
          meta: org.runtimes.find((r) => r.id === w.runtimeId)?.label ?? w.runtimeId,
          onOpen: () => go({ view: "worker", id: p.id }),
          actions: (
            <StopButton
              who={p.title}
              work={
                w.sessionId && isStoppable(w.state)
                  ? [{ sessionId: w.sessionId, objective: firstLine(w.objective) }]
                  : []
              }
            />
          ),
        });
      }
    }
  }
  return rows;
}

/** A position's row: its title, what it is doing, and its AI tool. */
export function positionRow(org: OrgSnapshot, go: Go, p: PositionInfo): RowItem {
  const doing =
    p.currentTask?.objective ??
    (p.workers.length > 0
      ? `${p.workers.length} ${p.workers.length === 1 ? "worker" : "workers"} on call now`
      : null);
  return {
    id: p.id,
    title: p.title,
    detail: doing ? `${p.roleName} · ${firstLine(doing)}` : p.roleName,
    status: { status: POSITION_STATUS[p.status], label: STATUS_LABEL[p.status] },
    meta: positionToolLabel(org, p),
    onOpen: () => go({ view: "worker", id: p.id }),
  };
}

/** Tasks, each opening its page (the same task listed twice shows once). */
export function taskRows(tasks: readonly TaskBrief[], go: Go, now: number): RowItem[] {
  const seen = new Set<string>();
  return tasks
    .filter((t) => {
      if (seen.has(t.id)) return false;
      seen.add(t.id);
      return true;
    })
    .map((t) => ({
      id: t.id,
      title: firstLine(t.objective) || "(no objective)",
      detail: t.positionTitle ?? undefined,
      status: TASK_STATUS[t.state],
      meta: ago(t.completedAt ?? t.startedAt ?? t.createdAt, now),
      onOpen: () => go({ view: "task", id: t.id }),
    }));
}

/** Decisions (approvals answered, refusals, lessons, stops), each opening its task. */
export function decisionRows(
  events: readonly LedgerEvent[],
  go: Go,
  now: number,
  currentTaskId: string | null = null,
): RowItem[] {
  return events.map((e) => {
    const opens = e.taskId !== null && e.taskId !== currentTaskId;
    return {
      id: String(e.seq),
      title: describeEvent(e),
      status: eventStatus(e),
      meta: ago(e.createdAt, now),
      onOpen: opens ? () => go({ view: "task", id: e.taskId! }) : undefined,
      openLabel: opens ? `${describeEvent(e)}. Open its task` : undefined,
    };
  });
}

const APPROVAL: Record<ApprovalStatus, { status: Status; label: string }> = {
  pending: { status: "pending", label: "Waiting for you" },
  approved: { status: "ok", label: "Approved" },
  rejected: { status: "warn", label: "Not approved" },
  expired: { status: "offline", label: "Expired" },
};

/** Approval requests; one still waiting opens the Approvals page. */
export function approvalRows(approvals: readonly ApprovalView[], go: Go, now: number): RowItem[] {
  return approvals.map((a) => ({
    id: a.id,
    title: a.summary,
    detail: [a.worker, a.capabilityLabel, a.server].filter(Boolean).join(" · "),
    tag:
      a.environment === "production" ? <StatusPill status="error" label="PRODUCTION" /> : undefined,
    status: APPROVAL[a.status],
    meta: ago(a.resolvedAt ?? a.requestedAt, now),
    onOpen: a.status === "pending" ? () => go({ view: "approvals", id: null }) : undefined,
    openLabel: a.status === "pending" ? `${a.summary}. Review it` : undefined,
  }));
}

/** Pull requests workers opened: the number, who opened it, and its address. */
export function pullRequestRows(pulls: readonly PullRequestRef[], go: Go, now: number): RowItem[] {
  return pulls.map((pr) => ({
    id: `${pr.taskId}:${pr.url}`,
    title: pr.number !== null ? `Pull request #${pr.number}` : "Pull request",
    detail: [pr.worker, pr.url].filter(Boolean).join(" · "),
    meta: ago(pr.createdAt, now),
    onOpen: () => go({ view: "task", id: pr.taskId }),
    openLabel: `${pr.number !== null ? `Pull request #${pr.number}` : "Pull request"}. Open the task that opened it`,
  }));
}
