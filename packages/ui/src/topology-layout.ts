/** Layout for the relationship map: a tidy top-down tree. */

import type { IconName } from "./icon-data";
import type { Status } from "./status-types";

export interface MapNode {
  id: string;
  label: string;
  status: Status;
  /** The status word, shown on the tile. */
  statusLabel: string;
  /** Under the tile, e.g. "3 tasks · 12 min". */
  caption?: string;
  icon?: IconName;
}

export interface MapLink {
  from: string;
  to: string;
  label?: string;
}

export const TILE_W = 132;
export const TILE_H = 46;
const GAP_X = 20;
const GAP_Y = 64;
const PAD = 16;

interface Placed {
  node: MapNode;
  x: number;
  y: number;
}

/**
 * A tidy top-down layout: roots at the top, each parent centered over its children, leaves
 * side by side. A node reached twice (a handoff chain that joins) is placed once.
 */
export function layoutMap(nodes: readonly MapNode[], links: readonly MapLink[]) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const children = new Map<string, string[]>();
  const hasParent = new Set<string>();
  for (const l of links) {
    if (!byId.has(l.from) || !byId.has(l.to) || l.from === l.to) continue;
    children.set(l.from, [...(children.get(l.from) ?? []), l.to]);
    hasParent.add(l.to);
  }
  const roots = nodes.filter((n) => !hasParent.has(n.id));
  const placed = new Map<string, Placed>();
  let nextLeaf = 0;
  let depthMax = 0;
  const place = (id: string, depth: number): number => {
    const node = byId.get(id);
    if (!node || placed.has(id)) return placed.get(id)?.x ?? 0;
    // Reserve the node first, so a cycle stops here.
    placed.set(id, { node, x: 0, y: PAD + depth * (TILE_H + GAP_Y) });
    depthMax = Math.max(depthMax, depth);
    const kids = (children.get(id) ?? []).filter((k) => !placed.has(k));
    let x: number;
    if (kids.length === 0) {
      x = PAD + nextLeaf * (TILE_W + GAP_X);
      nextLeaf += 1;
    } else {
      const xs = kids.map((k) => place(k, depth + 1));
      x = ((xs[0] ?? 0) + (xs[xs.length - 1] ?? 0)) / 2;
    }
    const p = placed.get(id);
    if (p) p.x = x;
    return x;
  };
  for (const r of roots.length > 0 ? roots : nodes.slice(0, 1)) place(r.id, 0);
  // Anything left (only reachable through a cycle) goes on its own.
  for (const n of nodes) if (!placed.has(n.id)) place(n.id, 0);
  const width = PAD * 2 + Math.max(1, nextLeaf) * (TILE_W + GAP_X) - GAP_X;
  const height = PAD * 2 + (depthMax + 1) * (TILE_H + GAP_Y) - GAP_Y + 18;
  return { placed, width, height };
}
