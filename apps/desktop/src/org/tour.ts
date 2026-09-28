/**
 * The canvas's pointer modes and its first-time tour (Phase 18, ADR-053 §1, §21), and what the
 * canvas remembers on this computer: whether the tour was seen and whether the legend is shown.
 */

/** Select (V), Move the view (H, or hold the space bar), and Arrange (A). */
export type PointerMode = "select" | "pan" | "arrange";

export const TOUR_KEY = "plenipo.canvasTour";
export const LEGEND_KEY = "plenipo.canvasLegend";

export interface TourStep {
  title: string;
  glyph: string;
  words: string;
}

export const TOUR_STEPS: readonly TourStep[] = [
  {
    title: "Arrange your organization",
    glyph: "arrange",
    words:
      "Drag any tile to an empty spot and it stays there, even after a restart. A tile moves with its team; hold Alt to move it alone. Tidy up puts everything back in rows, with Undo.",
  },
  {
    title: "Rewire by dragging lines",
    glyph: "link",
    words:
      "Select an agent to see the round ends of its lines. Drag one onto another agent to change who it reports to, or which team a reviewer, QA evaluator, or security auditor checks. The same rules apply as always.",
  },
  {
    title: "Move or lend an agent",
    glyph: "lent",
    words:
      "Drop an agent on another team's lead: Move here for good, or lend an on-call agent for one objective or until you send it home. A lent agent shows a Lent badge and a dashed line.",
  },
  {
    title: "The trash can",
    glyph: "trash",
    words:
      "Drop an agent on the trash can to archive it, with Undo. Click the trash can to open the Archived drawer: bring things back, or delete them for good after a question.",
  },
  {
    title: "Filters and the legend",
    glyph: "filter",
    words:
      "Filters show only the agents you want by department, project, status, AI tool, AI company, rank, or specialty. The legend explains every mark on the canvas.",
  },
  {
    title: "Watch a worker write code",
    glyph: "watch",
    words:
      "While an agent works, Watch opens a tab at the bottom that shows each file its worker changes, line by line, as it writes. It is read-only; Stop stops the worker.",
  },
];

export function readFlag(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writeFlag(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Storage unavailable: it is simply not remembered.
  }
}
