/**
 * The layout of an organization's window (Phase 21, ADR-092): where each panel is — in the left,
 * right, or bottom dock, or popped out into its own window — which dock is open and which panel
 * it shows, and each dock's size. Kept on this computer, so it comes back the same after a
 * restart. Every change here is a plain function, so it is easy to test.
 */

import type { PanelId } from "@plenipo/types";

export type { PanelId };
export type DockSide = "left" | "right" | "bottom";

export const PANELS: readonly PanelId[] = ["terminal", "files", "chat"];
export const DOCKS: readonly DockSide[] = ["left", "right", "bottom"];

/** Each panel's name on screen. */
export const PANEL_TITLES: Record<PanelId, string> = {
  terminal: "Terminal",
  files: "Files",
  chat: "Chat",
};
/** Each dock's name on screen ("Move to the left"). */
export const DOCK_WORDS: Record<DockSide, string> = {
  left: "the left",
  right: "the right",
  bottom: "the bottom",
};

/** Where a panel is: the dock it sits in (or goes back to), and whether it is popped out. */
export interface PanelPlace {
  dock: DockSide;
  popped: boolean;
}

/** A dock: shown or hidden, its size (width on a side, height at the bottom), and its panel. */
export interface DockState {
  open: boolean;
  size: number;
  active: PanelId | null;
}

export interface Layout {
  version: 1;
  panels: Record<PanelId, PanelPlace>;
  /** The order of the panels' tabs, in whichever dock they are. */
  order: PanelId[];
  docks: Record<DockSide, DockState>;
}

export const LAYOUT_KEY = "plenipo.layout";
/** The terminal panel's place before Phase 21 (ADR-031): its open, side, and size. */
export const OLD_TERMINAL_KEY = "plenipo.terminal";

/** A dock's smallest size, and the least room it leaves the page. */
export const DOCK_MIN = 120;
export const PAGE_MIN = 180;
/** The largest size kept (a larger one would not come back after a restart). */
export const DOCK_LARGEST = 4000;
/** Each dock's first size. */
export const FIRST_SIZE: Record<DockSide, number> = { left: 280, right: 420, bottom: 260 };

/** Where everything starts, and where Reset layout puts it back. */
export function defaultLayout(): Layout {
  return {
    version: 1,
    panels: {
      terminal: { dock: "bottom", popped: false },
      files: { dock: "left", popped: false },
      chat: { dock: "right", popped: false },
    },
    order: ["terminal", "files", "chat"],
    docks: {
      left: { open: false, size: FIRST_SIZE.left, active: "files" },
      right: { open: false, size: FIRST_SIZE.right, active: "chat" },
      bottom: { open: false, size: FIRST_SIZE.bottom, active: "terminal" },
    },
  };
}

const isPanel = (v: unknown): v is PanelId => PANELS.some((p) => p === v);
const isDock = (v: unknown): v is DockSide => DOCKS.some((d) => d === v);
const isSize = (v: unknown): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= DOCK_MIN && v <= DOCK_LARGEST;

/** A layout read back from this computer, checked part by part. */
export function isLayout(v: unknown): v is Layout {
  if (typeof v !== "object" || v === null) return false;
  const l = v as Record<string, unknown>;
  if (l.version !== 1) return false;
  const panels = l.panels as Record<string, unknown> | null;
  const docks = l.docks as Record<string, unknown> | null;
  if (typeof panels !== "object" || panels === null) return false;
  if (typeof docks !== "object" || docks === null) return false;
  const placesOk = PANELS.every((p) => {
    const place = panels[p] as Record<string, unknown> | undefined;
    return !!place && isDock(place.dock) && typeof place.popped === "boolean";
  });
  const docksOk = DOCKS.every((d) => {
    const dock = docks[d] as Record<string, unknown> | undefined;
    return (
      !!dock &&
      typeof dock.open === "boolean" &&
      isSize(dock.size) &&
      (dock.active === null || isPanel(dock.active))
    );
  });
  const order = l.order;
  const orderOk =
    Array.isArray(order) &&
    order.length === PANELS.length &&
    PANELS.every((p) => order.includes(p));
  return placesOk && docksOk && orderOk;
}

/**
 * A layout kept before a panel existed (Chat, ADR-200) gets the new panel where it starts; the
 * others stay where the owner put them.
 */
function withNewPanels(kept: unknown): unknown {
  if (typeof kept !== "object" || kept === null) return kept;
  const l = kept as Record<string, unknown>;
  const panels = l.panels;
  const order = l.order;
  if (typeof panels !== "object" || panels === null || !Array.isArray(order)) return kept;
  const missing = PANELS.filter((p) => !(p in panels) && !order.includes(p));
  if (missing.length === 0) return kept;
  const first = defaultLayout();
  const placed: Record<string, unknown> = { ...(panels as Record<string, unknown>) };
  for (const p of missing) placed[p] = first.panels[p];
  return { ...l, panels: placed, order: [...(order as unknown[]), ...missing] };
}

/**
 * The layout to start with: the one kept (with any panel added since), or (the first time after
 * Phase 21) the terminal panel where it was (ADR-092 §14), or the default.
 */
export function startingLayout(kept: unknown, oldTerminal: unknown): Layout {
  const upgraded = withNewPanels(kept);
  if (isLayout(upgraded)) return upgraded;
  const layout = defaultLayout();
  if (typeof oldTerminal !== "object" || oldTerminal === null) return layout;
  const old = oldTerminal as Record<string, unknown>;
  const side = old.side === "right" ? "right" : old.side === "bottom" ? "bottom" : null;
  if (side === null) return layout;
  const moved = moveTo(layout, "terminal", side);
  const size = isSize(old.size) ? old.size : FIRST_SIZE[side];
  return {
    ...moved,
    docks: { ...moved.docks, [side]: { open: old.open === true, size, active: "terminal" } },
  };
}

/** The panels sitting in a dock (not popped out), in their tabs' order. */
export function panelsIn(layout: Layout, dock: DockSide): PanelId[] {
  return layout.order.filter((p) => {
    const place = layout.panels[p];
    return place.dock === dock && !place.popped;
  });
}

/** The panel a dock shows (its chosen one, or its first). */
export function activeIn(layout: Layout, dock: DockSide): PanelId | null {
  const here = panelsIn(layout, dock);
  const chosen = layout.docks[dock].active;
  return chosen !== null && here.includes(chosen) ? chosen : (here[0] ?? null);
}

/** A dock takes room: it is open and holds a panel. */
export function dockShown(layout: Layout, dock: DockSide): boolean {
  return layout.docks[dock].open && panelsIn(layout, dock).length > 0;
}

/** The panel can be seen: popped out, or the panel its open dock shows. */
export function panelShown(layout: Layout, panel: PanelId): boolean {
  const place = layout.panels[panel];
  if (place.popped) return true;
  return layout.docks[place.dock].open && activeIn(layout, place.dock) === panel;
}

function withDock(layout: Layout, dock: DockSide, change: Partial<DockState>): Layout {
  return { ...layout, docks: { ...layout.docks, [dock]: { ...layout.docks[dock], ...change } } };
}

/** Show a panel in its dock (a popped-out panel is already shown). */
export function show(layout: Layout, panel: PanelId): Layout {
  const place = layout.panels[panel];
  if (place.popped) return layout;
  return withDock(layout, place.dock, { open: true, active: panel });
}

/** Hide a dock (its panels keep running). */
export function hideDock(layout: Layout, dock: DockSide): Layout {
  return withDock(layout, dock, { open: false });
}

/** Show a hidden panel, or hide the dock that shows it. */
export function toggle(layout: Layout, panel: PanelId): Layout {
  const place = layout.panels[panel];
  if (place.popped) return layout;
  return panelShown(layout, panel) ? hideDock(layout, place.dock) : show(layout, panel);
}

/** The dock a panel leaves shows its next panel. */
function leave(layout: Layout, panel: PanelId): Layout {
  const from = layout.panels[panel];
  if (from.popped || layout.docks[from.dock].active !== panel) return layout;
  const rest = panelsIn(layout, from.dock).filter((p) => p !== panel);
  return withDock(layout, from.dock, { active: rest[0] ?? null });
}

/**
 * Put a panel in a dock (from another dock, or back from its own window), shown there. `before`:
 * the panel whose tab it goes in front of.
 */
export function moveTo(
  layout: Layout,
  panel: PanelId,
  dock: DockSide,
  before?: PanelId | null,
): Layout {
  const left = leave(layout, panel);
  const order = left.order.filter((p) => p !== panel);
  const at = before ? order.indexOf(before) : -1;
  if (at >= 0) order.splice(at, 0, panel);
  else order.push(panel);
  const placed: Layout = {
    ...left,
    order,
    panels: { ...left.panels, [panel]: { dock, popped: false } },
  };
  return withDock(placed, dock, { open: true, active: panel });
}

/** The panel is in its own window now; it goes back to the same dock when put back. */
export function popOut(layout: Layout, panel: PanelId): Layout {
  const left = leave(layout, panel);
  return {
    ...left,
    panels: { ...left.panels, [panel]: { dock: layout.panels[panel].dock, popped: true } },
  };
}

/** Its window closed: the panel goes back to its dock (or `dock`), shown. */
export function putBack(layout: Layout, panel: PanelId, dock?: DockSide): Layout {
  if (!layout.panels[panel].popped && dock === undefined) return layout;
  return moveTo(layout, panel, dock ?? layout.panels[panel].dock);
}

/** The largest a dock may be in a work area this big (0 until measured: no limit yet). */
export function dockMax(
  layout: Layout,
  dock: DockSide,
  room: { width: number; height: number },
): number {
  if (dock === "bottom") {
    return room.height > 0
      ? Math.min(DOCK_LARGEST, Math.max(DOCK_MIN, room.height - PAGE_MIN))
      : DOCK_LARGEST;
  }
  if (room.width <= 0) return DOCK_LARGEST;
  const other: DockSide = dock === "left" ? "right" : "left";
  const taken = dockShown(layout, other) ? layout.docks[other].size : 0;
  return Math.min(DOCK_LARGEST, Math.max(DOCK_MIN, room.width - PAGE_MIN - taken));
}

/** A dock's new size, kept within its limits. */
export function resize(layout: Layout, dock: DockSide, size: number, max: number): Layout {
  const next = Math.max(DOCK_MIN, Math.min(max, Math.round(size)));
  return withDock(layout, dock, { size: next });
}

/** The panels popped out now. */
export function poppedPanels(layout: Layout): PanelId[] {
  return PANELS.filter((p) => layout.panels[p].popped);
}
