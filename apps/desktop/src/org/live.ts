/**
 * The canvas's live view, in words (Phase 18, ADR-053 §17–§20): for each worker in a step now,
 * where its model thinks (its AI company's cloud), where its work runs besides (this PC, or a
 * server by name), and what it touched last (a folder, a server, a website, or the screen); and
 * the hand-offs of the last minute, as marks between two tiles. From `get_live_view`, which reads
 * what Plenipo already records. Pure; no DOM.
 */
import type { LiveView, LiveWorker, OrgSnapshot } from "@plenipo/types";

import { pathBetween, workerNodeId, type OrgLayout, type Point } from "./layout";
import type { SymbolKey } from "./symbols";

/** How long a hand-off shows on the canvas. */
export const HANDOFF_SHOWN_MS = 60_000;

export interface WherePart {
  symbol: SymbolKey;
  words: string;
}

/** One working tile's "where" line. */
export interface WhereLine {
  tileId: string;
  thinksIn: WherePart;
  runsOn: WherePart | null;
  touching: WherePart | null;
}

/** "Anthropic's cloud". */
export function cloudOf(company: string | null | undefined): string {
  return company ? `${company}'s cloud` : "its AI company's cloud";
}

/** The tile a live worker shows on: its own worker tile, or its agent's. */
function tileOf(snapshot: OrgSnapshot, w: LiveWorker): string | null {
  for (const p of snapshot.positions) {
    const worker = p.workers.find((x) => x.taskId === w.taskId);
    if (worker) return workerNodeId(worker.agentId);
  }
  return w.positionId ?? null;
}

function runsOnPart(w: LiveWorker): WherePart | null {
  const r = w.runsOn;
  if (!r) return null;
  if (r.kind === "server") {
    return {
      symbol: "where-server",
      words: `Runs on ${r.name}${r.production ? " · PRODUCTION" : ""}`,
    };
  }
  return { symbol: "where-this-pc", words: `Runs on this PC (${r.what})` };
}

function touchingPart(w: LiveWorker): WherePart | null {
  const t = w.touching;
  if (!t) return null;
  switch (t.kind) {
    case "folder": {
      const folder = t.folder === "" ? "its top folder" : t.folder;
      return {
        symbol: "touch-folder",
        words: `Touching ${t.project ? `${t.project} · ` : ""}${folder}`,
      };
    }
    case "server":
      return {
        symbol: "where-server",
        words: `Touching ${t.name}${t.production ? " · PRODUCTION" : ""}`,
      };
    case "website":
      return { symbol: "touch-website", words: `Touching ${t.host}` };
    case "screen":
      return { symbol: "touch-screen", words: "Touching the screen" };
  }
}

/** The "where" line of each working tile, by tile. */
export function whereLines(snapshot: OrgSnapshot, live: LiveView): Map<string, WhereLine> {
  const companies = new Map(snapshot.runtimes.map((r) => [r.id, r.company]));
  const out = new Map<string, WhereLine>();
  for (const w of live.workers) {
    const tileId = tileOf(snapshot, w);
    if (!tileId || out.has(tileId)) continue;
    out.set(tileId, {
      tileId,
      thinksIn: {
        symbol: "where-cloud",
        words: `Thinks in ${cloudOf(companies.get(w.runtimeId))}`,
      },
      runsOn: runsOnPart(w),
      touching: touchingPart(w),
    });
  }
  return out;
}

/** A hand-off between two shown tiles, as a mark moving along a curve. */
export interface HandoffMark {
  id: string;
  kind: "asked" | "answered";
  d: string;
  from: string;
  to: string;
  mid: Point;
  /** The angle of the curve at its middle (degrees), for a still arrow. */
  angle: number;
}

/** The hand-offs of the last minute between tiles the canvas shows. */
export function handoffMarks(layout: OrgLayout, live: LiveView): HandoffMark[] {
  const out: HandoffMark[] = [];
  const seen = new Set<string>();
  for (const h of live.handoffs) {
    if (live.at - h.at > HANDOFF_SHOWN_MS || !h.fromPositionId || !h.toPositionId) continue;
    if (h.fromPositionId === h.toPositionId || seen.has(h.id)) continue;
    const a = layout.byId.get(h.fromPositionId);
    const b = layout.byId.get(h.toPositionId);
    if (!a || !b) continue;
    seen.add(h.id);
    const line = pathBetween(a, b);
    out.push({
      id: h.id,
      kind: h.kind === "answered" ? "answered" : "asked",
      d: line.d,
      from: a.id,
      to: b.id,
      mid: line.mid,
      angle: (Math.atan2(line.end.y - line.start.y, line.end.x - line.start.x) * 180) / Math.PI,
    });
  }
  return out;
}
