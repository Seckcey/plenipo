/**
 * The terminal panel's place on screen (ADR-031 §1): open or hidden, at the bottom or on the
 * right, and its size. Kept on this computer, so it comes back the same after a restart.
 */

import type { Environment, TerminalPlace } from "@plenipo/types";

import type { WatchTab } from "./watch";

export type PanelSide = "bottom" | "right";

export interface PanelState {
  open: boolean;
  side: PanelSide;
  /** Height at the bottom, width on the right (pixels). */
  size: number;
}

export const PANEL_KEY = "plenipo.terminal";
export const DEFAULT_PANEL: PanelState = { open: false, side: "bottom", size: 260 };
/** The panel's smallest size, and the least room it leaves the page. */
export const PANEL_MIN = 120;
export const PAGE_MIN = 180;

export function isPanelState(v: unknown): v is PanelState {
  if (typeof v !== "object" || v === null) return false;
  const p = v as Record<string, unknown>;
  return (
    typeof p.open === "boolean" &&
    (p.side === "bottom" || p.side === "right") &&
    typeof p.size === "number" &&
    Number.isFinite(p.size) &&
    p.size >= PANEL_MIN &&
    p.size <= 4000
  );
}

/** The largest the panel may be in a work area this big (0 until measured: no limit yet). */
export function panelMax(side: PanelSide, width: number, height: number): number {
  const room = side === "bottom" ? height : width;
  return room > 0 ? Math.max(PANEL_MIN, room - PAGE_MIN) : 4000;
}

/** One of the owner's terminals, as its tab knows it. */
export interface OwnerTab {
  kind: "owner";
  /** The tab's own ID (the terminal's ID comes once it opens). */
  id: string;
  place: TerminalPlace;
  title: string;
  environment: Environment | null;
  openedAt: number;
}

export interface WatchEntry {
  kind: "watch";
  id: string;
  watch: WatchTab;
}

export type TerminalTab = OwnerTab | WatchEntry;
