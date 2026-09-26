import { memo } from "react";
import type { PositionInfo, PositionStatus } from "@plenipo/types";

import {
  OVERSIGHT_CHIP,
  STAFFING_LABEL,
  STATUS_LABEL,
  WORKER_STATE_LABEL,
  ago,
  plural,
  positionToolLabel,
} from "../../org/format";
import type { LayoutNode } from "../../org/layout";
import { nodeLabel, workerStatus, type DropState, type NodeContext } from "../../org/nodes";
import { rankName, titlesOf } from "../../org/titles";
import { Glyph } from "./Glyph";

interface Props {
  node: LayoutNode;
  ctx: NodeContext;
  selected: boolean;
  /** Dimmed by a search that does not match it. */
  dimmed: boolean;
  /** Being dragged. */
  lifted: boolean;
  drop: DropState;
  now: number;
  onSelect: (id: string) => void;
  onFocusNode: (id: string) => void;
}

export const OrgNode = memo(function OrgNode({
  node,
  ctx,
  selected,
  dimmed,
  lifted,
  drop,
  now,
  onSelect,
  onFocusNode,
}: Props) {
  const status = nodeStatus(node);
  const classes = [
    "topo-node",
    `topo-node--${node.kind}`,
    selected && "is-selected",
    dimmed && "is-dimmed",
    lifted && "is-lifted",
    drop === "valid" && "is-drop-target",
    drop === "invalid" && "is-drop-refused",
    node.kind === "position" && node.position.staffing === "onDemand" && "topo-node--on-demand",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type="button"
      className={classes}
      data-node-id={node.id}
      data-status={status ?? undefined}
      aria-label={nodeLabel(node, ctx)}
      aria-current={selected ? "true" : undefined}
      title={node.kind === "position" ? (node.position.statusDetail ?? undefined) : undefined}
      style={{ left: node.x, top: node.y, width: node.w, height: node.h }}
      onClick={() => onSelect(node.id)}
      onFocus={() => onFocusNode(node.id)}
    >
      <NodeBody node={node} ctx={ctx} now={now} />
    </button>
  );
});

function nodeStatus(node: LayoutNode): PositionStatus | null {
  if (node.kind === "position") return node.position.status;
  if (node.kind === "worker") return workerStatus(node.worker.state);
  return null;
}

function NodeBody({ node, ctx, now }: { node: LayoutNode; ctx: NodeContext; now: number }) {
  switch (node.kind) {
    case "owner":
      return (
        <>
          <span className="topo-node__glyph topo-node__glyph--owner">
            <Glyph name="owner" size={20} />
          </span>
          <span className="topo-node__body">
            <span className="topo-node__title">You</span>
            <span className="topo-node__meta">{rankName(titlesOf(ctx.snapshot), "owner")}</span>
          </span>
        </>
      );
    case "organization": {
      const s = ctx.snapshot.stats;
      return (
        <>
          <span className="topo-node__glyph topo-node__glyph--org">
            <Glyph name="organization" size={22} />
          </span>
          <span className="topo-node__body">
            <span className="topo-node__title">{ctx.snapshot.name}</span>
            <span className="topo-node__meta">
              {plural(s.positions, "position")} · {plural(s.activeWorkers, "live worker")}
            </span>
            <span className="topo-node__stats">
              <span className="topo-stat topo-stat--working">
                <span aria-hidden="true">▲</span> {s.working} working
              </span>
              <span className="topo-stat topo-stat--waiting">
                <span aria-hidden="true">◆</span> {s.waiting} waiting
              </span>
              <span className="topo-stat">
                <span aria-hidden="true">▼</span> {s.queued} queued
              </span>
            </span>
          </span>
        </>
      );
    }
    case "position":
      return <PositionBody p={node.position} ctx={ctx} />;
    case "worker": {
      const w = node.worker;
      return (
        <>
          <span className="topo-node__glyph topo-node__glyph--worker">
            <Glyph name="worker" size={16} />
          </span>
          <span className="topo-node__body">
            <span className="topo-node__title topo-node__title--task">{w.objective}</span>
            <span className="topo-node__meta">
              <StatusPill status={workerStatus(w.state)} label={WORKER_STATE_LABEL[w.state]} /> ·{" "}
              {ctx.runtime(w.runtimeId)} · {ago(w.startedAt ?? w.spawnedAt, now)}
            </span>
          </span>
        </>
      );
    }
  }
}

function PositionBody({ p, ctx }: { p: PositionInfo; ctx: NodeContext }) {
  const oversees = ctx.oversees(p.id);
  const overseenBy = ctx.overseenBy(p.id);
  const live = p.workers.length;
  const rank = rankName(titlesOf(ctx.snapshot), p.kind);
  // A worker's role, when its title does not already say it ("Backend Developer" is a Senior
  // Developer).
  const role = p.kind === "worker" && p.title !== p.roleName ? ` · ${p.roleName}` : "";
  return (
    <>
      <span className={`topo-node__glyph topo-node__glyph--${p.kind}`}>
        <Glyph name={ctx.glyph(p.roleId)} size={20} />
      </span>
      <span className="topo-node__body">
        <span className="topo-node__title">{p.title}</span>
        <span className="topo-node__meta">
          {p.title === rank ? STAFFING_LABEL[p.staffing] : rank}
          {role}
          {p.model ? ` · ${p.model}` : ""}
        </span>
        <span className="topo-node__foot">
          <StatusPill status={p.status} label={STATUS_LABEL[p.status]} />
          <span className="topo-node__runtime">{positionToolLabel(ctx.snapshot, p)}</span>
          {p.staffing === "onDemand" && live > 0 && (
            <span className="topo-node__count">{plural(live, "live worker")}</span>
          )}
          {p.staffing === "persistent" && p.counts.queued > 0 && (
            <span className="topo-node__count">{p.counts.queued} queued</span>
          )}
          {oversees.map((o) => (
            <span key={o.id} className={`topo-badge topo-badge--${o.role}`}>
              {OVERSIGHT_CHIP[o.role]} → {ctx.title(o.targetId)}
            </span>
          ))}
          {overseenBy.map((o) => (
            <span
              key={o.id}
              className={`topo-badge topo-badge--${o.role}`}
              title={`${ctx.title(o.overseerId)} oversees this team`}
            >
              {OVERSIGHT_CHIP[o.role]}
            </span>
          ))}
        </span>
      </span>
    </>
  );
}

export function StatusPill({ status, label }: { status: PositionStatus; label: string }) {
  return (
    <span className="topo-status" data-status={status}>
      <span className="topo-status__dot" aria-hidden="true" />
      {label}
    </span>
  );
}
