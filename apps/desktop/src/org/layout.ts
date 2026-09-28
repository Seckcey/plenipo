/**
 * Topology layout for the Organization canvas (pure; no DOM).
 *
 * The owner sits at the far left like the internet uplink on a network map, the organization
 * next to it like the gateway, then everyone who reports to it, column by column. A parent
 * shares its first child's row, so the link to the first child is a straight line; the rest
 * hang off a vertical bus with rounded branches. Live workers are leaves under the on-demand
 * position that spawned them. Collapsed nodes hide their subtree and say how much is hidden.
 *
 * Phase 18 (ADR-053 §3, §6): a tile the owner placed by hand goes where it was put; a tile never
 * moved keeps its automatic offset from its lead, applied to where its lead actually is (so a new
 * hire appears beside its moved Manager). Lines to a tile in its automatic spot look as before;
 * a line to a tile placed elsewhere curves from its lead. Filters hide tiles (ADR-053 §13).
 */
import type {
  OrgSnapshot,
  OversightInfo,
  OversightRole,
  PositionInfo,
  TilePlace,
  WorkerInfo,
} from "@plenipo/types";

import type { Rect } from "./camera";

export const OWNER_ID = "owner";
export const ORG_ID = "organization";

export function workerNodeId(agentId: string): string {
  return `worker:${agentId}`;
}

export type NodeKind = "owner" | "organization" | "position" | "worker";

export const NODE_SIZE: Record<NodeKind, { w: number; h: number }> = {
  owner: { w: 236, h: 84 },
  organization: { w: 288, h: 96 },
  position: { w: 236, h: 84 },
  worker: { w: 212, h: 60 },
};

/** Space between columns: the bus, the branch, and its chip live here. */
export const COLUMN_GAP = 136;
export const ROW_GAP = 20;
/** The bus runs this far to the right of its parent. */
export const BUS_OFFSET = 40;
/** The collapse toggle sits on the trunk, this far from the parent. */
export const TOGGLE_OFFSET = 18;
export const CORNER = 12;
const BOUNDS_PAD = 48;

interface NodeBase {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  depth: number;
  parentId: string | null;
  /** Direct children, shown or not. */
  childCount: number;
  /** Descendants hidden because this node is collapsed. */
  hidden: number;
  collapsed: boolean;
  /** Placed by hand (its spot is saved), rather than by the automatic layout. */
  placed: boolean;
}

export type LayoutNode =
  | (NodeBase & { kind: "owner" })
  | (NodeBase & { kind: "organization" })
  | (NodeBase & { kind: "position"; position: PositionInfo })
  | (NodeBase & { kind: "worker"; worker: WorkerInfo; positionId: string });

export interface LayoutLink {
  id: string;
  /** SVG path in world coordinates. */
  d: string;
  /** Workers hang off dashed links, like wireless clients. */
  style: "tree" | "worker";
  /** Something at the end of it is working. */
  active: boolean;
  /** A line to one report: that report, and the lead's end of the line (for rewiring). */
  childId?: string;
  from?: Point;
}

export interface Point {
  x: number;
  y: number;
}

export interface LayoutChip {
  id: string;
  x: number;
  y: number;
  label: string;
  tone: "department" | "project" | "runtime";
}

export interface LayoutToggle {
  /** The node it collapses or expands. */
  id: string;
  x: number;
  y: number;
  collapsed: boolean;
  /** Direct children. */
  count: number;
  /** Descendants hidden while collapsed. */
  hidden: number;
}

export interface OversightLink {
  id: string;
  role: OversightRole;
  overseerId: string;
  targetId: string;
  d: string;
  x: number;
  y: number;
  /** The overseer's end and the team's end (for rewiring). */
  start: Point;
  end: Point;
}

/** A dashed line from a lent agent to the lead of the team it helps (ADR-054 §8). */
export interface LentLink {
  id: string;
  positionId: string;
  toLeadId: string;
  d: string;
  x: number;
  y: number;
}

export interface OrgLayout {
  /** Shown nodes, parents before children. */
  nodes: LayoutNode[];
  byId: Map<string, LayoutNode>;
  links: LayoutLink[];
  chips: LayoutChip[];
  toggles: LayoutToggle[];
  oversight: OversightLink[];
  lent: LentLink[];
  bounds: Rect;
}

export interface LayoutOptions {
  /** Tiles placed by hand (default: the snapshot's). */
  places?: readonly TilePlace[];
  /** The positions shown (filters); `null` or absent: all. */
  shown?: ReadonlySet<string> | null;
  /** Space between rows (more when each working tile shows where its work is). */
  rowGap?: number;
}

/** The row gap that leaves room for a "where" line under each tile (ADR-053 §18). */
export const WHERE_ROW_GAP = 48;

type Item =
  | { kind: "owner"; id: string }
  | { kind: "organization"; id: string }
  | { kind: "position"; id: string; position: PositionInfo }
  | { kind: "worker"; id: string; worker: WorkerInfo; positionId: string };

/** The tree as parent → children, before anything is placed. */
export function organizationTree(
  snapshot: OrgSnapshot,
  shownOnly: ReadonlySet<string> | null = null,
): Map<string, Item[]> {
  const children = new Map<string, Item[]>();
  const add = (parent: string, item: Item) => {
    const list = children.get(parent);
    if (list) list.push(item);
    else children.set(parent, [item]);
  };
  add(OWNER_ID, { kind: "organization", id: ORG_ID });
  const shows = (p: PositionInfo) => p.active && (shownOnly === null || shownOnly.has(p.id));
  const active = new Set(snapshot.positions.filter(shows).map((p) => p.id));
  // Positions come in tree order, so each team keeps its order.
  for (const p of snapshot.positions) {
    if (!shows(p)) continue;
    const parent = p.reportsTo && active.has(p.reportsTo) ? p.reportsTo : ORG_ID;
    add(parent, { kind: "position", id: p.id, position: p });
  }
  for (const p of snapshot.positions) {
    if (!shows(p)) continue;
    for (const w of p.workers) {
      add(p.id, { kind: "worker", id: workerNodeId(w.agentId), worker: w, positionId: p.id });
    }
  }
  return children;
}

function isActive(item: Item): boolean {
  if (item.kind === "position") return item.position.status === "working";
  if (item.kind === "worker") return item.worker.state === "running";
  return false;
}

export interface LayoutLabels {
  department: (id: string) => string | null;
  project: (id: string) => string | null;
  runtime: (id: string) => string;
}

export function labelsFor(snapshot: OrgSnapshot): LayoutLabels {
  const departments = new Map(snapshot.departments.map((d) => [d.id, d.name]));
  const projects = new Map(snapshot.projects.map((p) => [p.id, p.name]));
  const runtimes = new Map(snapshot.runtimes.map((r) => [r.id, r.label]));
  return {
    department: (id) => departments.get(id) ?? null,
    project: (id) => projects.get(id) ?? null,
    runtime: (id) => runtimes.get(id) ?? id,
  };
}

export function layoutOrganization(
  snapshot: OrgSnapshot,
  collapsed: ReadonlySet<string> = new Set(),
  options: LayoutOptions = {},
): OrgLayout {
  const tree = organizationTree(snapshot, options.shown ?? null);
  const saved = new Map((options.places ?? snapshot.places).map((p) => [p.tileId, p]));
  const rowGap = options.rowGap ?? ROW_GAP;
  const labels = labelsFor(snapshot);
  const kids = (id: string) => tree.get(id) ?? [];
  const isCollapsed = (id: string) => id !== OWNER_ID && collapsed.has(id) && kids(id).length > 0;
  const shown = (id: string) => (isCollapsed(id) ? [] : kids(id));
  const descendants = (id: string): number =>
    kids(id).reduce((n, c) => n + 1 + descendants(c.id), 0);

  // Pass 1: depth and column widths of what is shown.
  const root: Item = { kind: "owner", id: OWNER_ID };
  const depthOf = new Map<string, number>();
  const widths: number[] = [];
  const visit = (item: Item, depth: number) => {
    depthOf.set(item.id, depth);
    widths[depth] = Math.max(widths[depth] ?? 0, NODE_SIZE[item.kind].w);
    for (const c of shown(item.id)) visit(c, depth + 1);
  };
  visit(root, 0);
  const columns: number[] = [];
  for (let d = 0; d < widths.length; d++) {
    columns[d] = d === 0 ? 0 : (columns[d - 1] ?? 0) + (widths[d - 1] ?? 0) + COLUMN_GAP;
  }

  // Pass 2: vertical extents around each node's center line; the first child shares it.
  const up = new Map<string, number>();
  const down = new Map<string, number>();
  const offset = new Map<string, number>();
  const measure = (item: Item) => {
    const half = NODE_SIZE[item.kind].h / 2;
    let top = half;
    let bottom = half;
    let cursor = 0;
    shown(item.id).forEach((c, i) => {
      measure(c);
      const at = i === 0 ? 0 : cursor + rowGap + (up.get(c.id) ?? 0);
      offset.set(c.id, at);
      top = Math.max(top, (up.get(c.id) ?? 0) - at);
      cursor = at + (down.get(c.id) ?? 0);
      bottom = Math.max(bottom, cursor);
    });
    up.set(item.id, top);
    down.set(item.id, bottom);
  };
  measure(root);

  // Pass 3: place, parents before children. A tile placed by hand goes where it was put; the
  // others keep their automatic offset from their lead, moved with it (`shift`).
  const nodes: LayoutNode[] = [];
  const byId = new Map<string, LayoutNode>();
  /** How far each tile is from its automatic spot. */
  const shift = new Map<string, Point>();
  const place = (item: Item, parentId: string | null, cy: number) => {
    const depth = depthOf.get(item.id) ?? 0;
    const size = NODE_SIZE[item.kind];
    const count = kids(item.id).length;
    const folded = isCollapsed(item.id);
    const autoX = columns[depth] ?? 0;
    const autoY = cy - size.h / 2;
    const inherited = (parentId ? shift.get(parentId) : undefined) ?? { x: 0, y: 0 };
    const spot = item.kind === "worker" ? undefined : saved.get(item.id);
    const x = spot ? spot.x : autoX + inherited.x;
    const y = spot ? spot.y : autoY + inherited.y;
    shift.set(item.id, { x: x - autoX, y: y - autoY });
    const base: NodeBase = {
      id: item.id,
      x,
      y,
      w: size.w,
      h: size.h,
      depth,
      parentId,
      childCount: count,
      hidden: folded ? descendants(item.id) : 0,
      collapsed: folded,
      placed: spot !== undefined,
    };
    const node: LayoutNode =
      item.kind === "position"
        ? { ...base, kind: "position", position: item.position }
        : item.kind === "worker"
          ? { ...base, kind: "worker", worker: item.worker, positionId: item.positionId }
          : { ...base, kind: item.kind };
    nodes.push(node);
    byId.set(node.id, node);
    for (const c of shown(item.id)) place(c, item.id, cy + (offset.get(c.id) ?? 0));
  };
  place(root, null, up.get(OWNER_ID) ?? 0);

  // Links, chips, and toggles.
  const links: LayoutLink[] = [];
  const chips: LayoutChip[] = [];
  const toggles: LayoutToggle[] = [];
  for (const parent of nodes) {
    const list = shown(parent.id);
    const px = parent.x + parent.w;
    const py = parent.y + parent.h / 2;
    if (parent.childCount > 0 && parent.id !== OWNER_ID) {
      toggles.push({
        id: parent.id,
        x: px + TOGGLE_OFFSET,
        y: py,
        collapsed: parent.collapsed,
        count: parent.childCount,
        hidden: parent.hidden,
      });
    }
    if (parent.collapsed) {
      links.push({
        id: `stub:${parent.id}`,
        d: `M ${px} ${py} H ${px + TOGGLE_OFFSET}`,
        style: "tree",
        active: false,
      });
      continue;
    }
    if (list.length === 0) continue;
    // Reports in their automatic spot beside this lead share its bus; the others get a curve.
    const same = (id: string) => {
      const a = shift.get(id);
      const b = shift.get(parent.id);
      return !!a && !!b && Math.abs(a.x - b.x) < 0.5 && Math.abs(a.y - b.y) < 0.5;
    };
    const attached = list.filter((c) => same(c.id));
    const detached = list.filter((c) => !same(c.id));
    const busX = px + BUS_OFFSET;
    if (attached.length > 0) {
      const children = attached.map((c) => byId.get(c.id)).filter((n): n is LayoutNode => !!n);
      const last = children[children.length - 1];
      const lastCy = last ? last.y + last.h / 2 : py;
      const workersOnly = attached.every((c) => c.kind === "worker");
      links.push({
        id: `bus:${parent.id}`,
        // Down to the last attached report whenever it is below the lead's row (even when it is
        // the only one left: its first report may have been moved away).
        d: `M ${px} ${py} H ${busX}` + (lastCy - CORNER > py ? ` V ${lastCy - CORNER}` : ""),
        style: workersOnly ? "worker" : "tree",
        active: attached.some(isActive),
      });
    }
    attached.forEach((item) => {
      const child = byId.get(item.id);
      if (!child) return;
      const cy = child.y + child.h / 2;
      // Straight on only for a report on the lead's own row; a lower one curves off the bus.
      const straight = cy - CORNER <= py;
      links.push({
        id: `link:${child.id}`,
        d: straight
          ? `M ${busX} ${cy} H ${child.x}`
          : `M ${busX} ${cy - CORNER} Q ${busX} ${cy} ${busX + CORNER} ${cy} H ${child.x}`,
        style: item.kind === "worker" ? "worker" : "tree",
        active: isActive(item),
        childId: child.id,
        from: { x: busX, y: cy },
      });
      const label = chipLabel(item, labels);
      if (label) {
        chips.push({
          id: `chip:${child.id}`,
          x: (busX + CORNER + child.x) / 2,
          y: cy,
          label: label.text,
          tone: label.tone,
        });
      }
    });
    for (const item of detached) {
      const child = byId.get(item.id);
      if (!child) continue;
      const line = pathBetween(parent, child);
      links.push({
        id: `link:${child.id}`,
        d: line.d,
        style: item.kind === "worker" ? "worker" : "tree",
        active: isActive(item),
        childId: child.id,
        from: line.start,
      });
      const label = chipLabel(item, labels);
      if (label) {
        chips.push({
          id: `chip:${child.id}`,
          x: line.mid.x,
          y: line.mid.y,
          label: label.text,
          tone: label.tone,
        });
      }
    }
  }

  const oversight = snapshot.oversight
    .map((o) => oversightLink(o, byId))
    .filter((l): l is OversightLink => l !== null);

  const lent: LentLink[] = [];
  for (const p of snapshot.positions) {
    const from = p.loan ? byId.get(p.id) : undefined;
    const to = p.loan ? byId.get(p.loan.toLeadId) : undefined;
    if (!from || !to || !p.loan) continue;
    const line = pathBetween(from, to);
    lent.push({
      id: `lent:${p.id}`,
      positionId: p.id,
      toLeadId: p.loan.toLeadId,
      d: line.d,
      x: line.mid.x,
      y: line.mid.y,
    });
  }

  return { nodes, byId, links, chips, toggles, oversight, lent, bounds: boundsOf(nodes) };
}

type Box = Pick<NodeBase, "x" | "y" | "w" | "h">;

/**
 * A curve between two tiles, from the facing edges: side by side, from one's right edge to the
 * other's left; one above the other, from the bottom edge to the top. Its middle carries a chip
 * or a marker.
 */
export function pathBetween(a: Box, b: Box): { d: string; start: Point; end: Point; mid: Point } {
  const acx = a.x + a.w / 2;
  const acy = a.y + a.h / 2;
  const bcx = b.x + b.w / 2;
  const bcy = b.y + b.h / 2;
  let start: Point;
  let end: Point;
  let d: string;
  if (b.x >= a.x + a.w || b.x + b.w <= a.x) {
    const right = b.x >= a.x + a.w;
    start = { x: right ? a.x + a.w : a.x, y: acy };
    end = { x: right ? b.x : b.x + b.w, y: bcy };
    const k = Math.max(40, Math.abs(end.x - start.x) / 2) * (right ? 1 : -1);
    d = `M ${start.x} ${start.y} C ${start.x + k} ${start.y} ${end.x - k} ${end.y} ${end.x} ${end.y}`;
  } else {
    const down = bcy >= acy;
    start = { x: acx, y: down ? a.y + a.h : a.y };
    end = { x: bcx, y: down ? b.y : b.y + b.h };
    const k = Math.max(30, Math.abs(end.y - start.y) / 2) * (down ? 1 : -1);
    d = `M ${start.x} ${start.y} C ${start.x} ${start.y + k} ${end.x} ${end.y - k} ${end.x} ${end.y}`;
  }
  return { d, start, end, mid: { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 } };
}

function chipLabel(
  item: Item,
  labels: LayoutLabels,
): { text: string; tone: LayoutChip["tone"] } | null {
  if (item.kind === "worker")
    return { text: labels.runtime(item.worker.runtimeId), tone: "runtime" };
  if (item.kind !== "position") return null;
  const p = item.position;
  const department = p.headsDepartmentId ? labels.department(p.headsDepartmentId) : null;
  if (department) return { text: department, tone: "department" };
  const project = p.coordinatesProjectId ? labels.project(p.coordinatesProjectId) : null;
  if (project) return { text: project, tone: "project" };
  return null;
}

/**
 * A dotted line from an overseer to the lead of the team it oversees. Nodes in one column are
 * joined by a bracket in the gutter to their left (clear of the trunk and toggles on their right);
 * otherwise by an S-curve between facing edges. Both ends attach above the node's center line,
 * where no tree link runs.
 */
function oversightLink(o: OversightInfo, byId: Map<string, LayoutNode>): OversightLink | null {
  const from = byId.get(o.overseerId);
  const to = byId.get(o.targetId);
  if (!from || !to) return null;
  const fy = from.y + 18;
  const ty = to.y + 18;
  let d: string;
  let mid: [number, number];
  let x1: number;
  let x2: number;
  if (Math.abs(from.x - to.x) < 1) {
    x1 = x2 = from.x;
    const bulge = 44;
    d = `M ${x1} ${fy} C ${x1 - bulge} ${fy} ${x1 - bulge} ${ty} ${x1} ${ty}`;
    mid = [x1 - bulge * 0.75, (fy + ty) / 2];
  } else {
    const leftToRight = from.x < to.x;
    x1 = leftToRight ? from.x + from.w : from.x;
    x2 = leftToRight ? to.x : to.x + to.w;
    const k = Math.max(48, Math.abs(x2 - x1) / 2) * (leftToRight ? 1 : -1);
    d = `M ${x1} ${fy} C ${x1 + k} ${fy} ${x2 - k} ${ty} ${x2} ${ty}`;
    mid = [(x1 + x2) / 2, (fy + ty) / 2];
  }
  return {
    id: o.id,
    role: o.role,
    overseerId: o.overseerId,
    targetId: o.targetId,
    d,
    x: mid[0],
    y: mid[1],
    start: { x: x1, y: fy },
    end: { x: x2, y: ty },
  };
}

function boundsOf(nodes: LayoutNode[]): Rect {
  if (nodes.length === 0) return { x: 0, y: 0, w: 1, h: 1 };
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const n of nodes) {
    left = Math.min(left, n.x);
    top = Math.min(top, n.y);
    right = Math.max(right, n.x + n.w);
    bottom = Math.max(bottom, n.y + n.h);
  }
  return {
    x: left - BOUNDS_PAD,
    y: top - BOUNDS_PAD,
    w: right - left + BOUNDS_PAD * 2,
    h: bottom - top + BOUNDS_PAD * 2,
  };
}

/** The shown node under a world point (workers included), if any. */
export function nodeAt(layout: OrgLayout, wx: number, wy: number): LayoutNode | null {
  for (let i = layout.nodes.length - 1; i >= 0; i--) {
    const n = layout.nodes[i];
    if (n && wx >= n.x && wx <= n.x + n.w && wy >= n.y && wy <= n.y + n.h) return n;
  }
  return null;
}

/**
 * The canvas nodes above `id`, the owner first, so a hidden node can be revealed by expanding
 * them. Worker node IDs are `worker:<agentId>`.
 */
export function ancestorsOf(snapshot: OrgSnapshot, id: string): string[] {
  if (id === OWNER_ID) return [];
  if (id === ORG_ID) return [OWNER_ID];
  const byId = new Map(snapshot.positions.map((p) => [p.id, p]));
  const chain: string[] = [];
  let current: string | null = id;
  if (id.startsWith("worker:")) {
    const agent = id.slice("worker:".length);
    current =
      snapshot.positions.find((p) => p.workers.some((w) => w.agentId === agent))?.id ?? null;
    if (current) chain.push(current);
  }
  const seen = new Set<string>();
  while (current && !seen.has(current)) {
    seen.add(current);
    current = byId.get(current)?.reportsTo ?? null;
    if (current) chain.push(current);
  }
  chain.push(ORG_ID, OWNER_ID);
  return chain.reverse();
}
