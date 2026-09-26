import type { OrgSnapshot, OversightInfo, PositionStatus } from "@plenipo/types";

import { STATUS_LABEL, WORKER_STATE_LABEL } from "./format";
import type { LayoutNode } from "./layout";
import { rankName, titlesOf } from "./titles";

/** Lookups the nodes need, built once per snapshot. */
export interface NodeContext {
  snapshot: OrgSnapshot;
  glyph: (roleId: string) => string;
  runtime: (id: string) => string;
  title: (positionId: string) => string;
  /** Oversight assignments held by a position. */
  oversees: (positionId: string) => OversightInfo[];
  /** Oversight assignments over a lead's team. */
  overseenBy: (positionId: string) => OversightInfo[];
}

export function nodeContext(snapshot: OrgSnapshot): NodeContext {
  const glyphs = new Map(snapshot.roles.map((r) => [r.id, r.glyph]));
  const runtimes = new Map(snapshot.runtimes.map((r) => [r.id, r.label]));
  const titles = new Map(snapshot.positions.map((p) => [p.id, p.title]));
  return {
    snapshot,
    glyph: (roleId) => glyphs.get(roleId) ?? "worker",
    runtime: (id) => runtimes.get(id) ?? id,
    title: (id) => titles.get(id) ?? "a former position",
    oversees: (id) => snapshot.oversight.filter((o) => o.overseerId === id),
    overseenBy: (id) => snapshot.oversight.filter((o) => o.targetId === id),
  };
}

/** How a drag over this node would land. */
export type DropState = "valid" | "invalid" | null;

/** A worker's task state as a node status. */
export function workerStatus(state: string): PositionStatus {
  if (state === "running") return "working";
  if (state === "blocked") return "waiting";
  return "queued";
}

export function nodeLabel(node: LayoutNode, ctx: NodeContext): string {
  switch (node.kind) {
    case "owner":
      return `You, ${rankName(titlesOf(ctx.snapshot), "owner")}`;
    case "organization":
      return `${ctx.snapshot.name}, organization`;
    case "position": {
      const p = node.position;
      return `${p.title}, ${STATUS_LABEL[p.status]}`;
    }
    case "worker":
      return `Worker for ${ctx.title(node.positionId)}: ${node.worker.objective}, ${
        WORKER_STATE_LABEL[node.worker.state]
      }`;
  }
}
