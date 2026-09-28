/**
 * Arranging the canvas by hand (Phase 18, ADR-053 §2–§5): which tiles get a saved spot when one
 * is moved. A tile moves with its team: its reports that sit in their automatic spot follow it
 * by themselves, and those placed by hand move by the same amount. Moved alone (Alt), its reports
 * stay where they are, so each of them keeps its spot. Live workers always follow their agent and
 * are never placed. Pure; no DOM.
 */
import type { TilePlace } from "@plenipo/types";

import type { OrgLayout } from "./layout";

/** How far Alt + an arrow key moves the selected tile. */
export const NUDGE = 20;
/** The Ledger keeps spots within this distance of the middle of the canvas. */
export const MAX_COORDINATE = 100_000;
/** The most spots saved in one step. */
export const MAX_PLACES = 500;

const clamp = (v: number) => Math.max(-MAX_COORDINATE, Math.min(MAX_COORDINATE, Math.round(v)));

/** The shown tiles under `id` (its team, all the way down), without live workers. */
function teamOf(layout: OrgLayout, id: string): string[] {
  const below = new Map<string, string[]>();
  for (const n of layout.nodes) {
    if (n.kind === "worker" || n.parentId === null) continue;
    const list = below.get(n.parentId);
    if (list) list.push(n.id);
    else below.set(n.parentId, [n.id]);
  }
  const out: string[] = [];
  const walk = (at: string) => {
    for (const c of below.get(at) ?? []) {
      out.push(c);
      walk(c);
    }
  };
  walk(id);
  return out;
}

/** The spots to save when tile `id` moves by (`dx`, `dy`); empty when it cannot be moved. */
export function movedPlaces(
  layout: OrgLayout,
  id: string,
  dx: number,
  dy: number,
  alone: boolean,
): TilePlace[] {
  const node = layout.byId.get(id);
  if (!node || node.kind === "worker") return [];
  const out: TilePlace[] = [{ tileId: id, x: clamp(node.x + dx), y: clamp(node.y + dy) }];
  if (alone) {
    // Its direct reports stay put: those in their automatic spot are pinned where they are.
    for (const n of layout.nodes) {
      if (n.parentId !== id || n.kind === "worker" || n.placed) continue;
      out.push({ tileId: n.id, x: clamp(n.x), y: clamp(n.y) });
    }
  } else {
    // Its team comes along: those placed by hand move by the same amount.
    for (const t of teamOf(layout, id)) {
      const n = layout.byId.get(t);
      if (n?.placed) out.push({ tileId: t, x: clamp(n.x + dx), y: clamp(n.y + dy) });
    }
  }
  return out.slice(0, MAX_PLACES);
}

/** `current` with `changes` applied (a tile's newer spot replaces its older one). */
export function mergePlaces(current: readonly TilePlace[], changes: readonly TilePlace[]) {
  const byTile = new Map(current.map((p) => [p.tileId, p]));
  for (const p of changes) byTile.set(p.tileId, p);
  return [...byTile.values()];
}
