import { createContext, useContext } from "react";
import type { WindowPlace } from "@plenipo/types";

import type { DockSide, Layout, PanelId } from "./layout";

/** A panel's tab being dragged (by the pointer), and where it would go if let go now. */
export interface PanelDrag {
  panel: PanelId;
  /** The dock under the pointer (marked), or none. */
  over: DockSide | null;
  /** The pointer is past the window's edges: letting go pops the panel out there. */
  outside: boolean;
}

/** One panel's pop-out window, and where in it the panel and its title bar go. */
export interface PopUp {
  panel: PanelId;
  win: Window;
  header: HTMLElement;
  body: HTMLElement;
  stop: () => void;
}

export interface WorkspaceApi {
  layout: Layout;
  /** Each dock's size now, kept within the room the page leaves. */
  sizes: Record<DockSide, number>;
  maxSize: (dock: DockSide) => number;
  setSize: (dock: DockSide, size: number) => void;
  /** The panel can be seen (popped out, or its dock open and showing it). */
  shown: (panel: PanelId) => boolean;
  /** Show a panel (a popped-out one comes to the front). */
  show: (panel: PanelId) => void;
  /** Show a hidden panel, or hide its dock (a popped-out one comes to the front). */
  toggle: (panel: PanelId) => void;
  hideDock: (dock: DockSide) => void;
  /** Put a panel in a dock (`before`: the panel whose tab it goes in front of). */
  moveTo: (panel: PanelId, dock: DockSide, before?: PanelId | null) => void;
  /** Open a panel in its own window (`place`: where it was dropped). */
  popOut: (panel: PanelId, place?: WindowPlace) => void;
  /** Put a popped-out panel back in a dock (its own, or `dock`). */
  putBack: (panel: PanelId, dock?: DockSide) => void;
  /** Reset layout (ADR-092 §12). */
  reset: () => void;
  /** Why the last pop-out could not open, until the next one does. */
  popOutProblem: string | null;
  /** The window each panel is drawn in now (Plenipo's own, or its pop-out). */
  windowOf: (panel: PanelId) => Window;
  /** The dock places a panel is drawn into (the docks give theirs). */
  registerSlot: (dock: DockSide, el: HTMLElement | null) => void;
  /** Measure the work area (the page and its docks) through this ref. */
  measure: (el: HTMLElement | null) => void;
  /** A panel's tab being dragged now. */
  drag: PanelDrag | null;
  setDrag: (drag: PanelDrag | null) => void;
  /** Where each dock is on screen, for dragging (the work area's parts). */
  dockAt: (clientX: number, clientY: number) => DockSide | null;
  /** The element each panel is drawn into (moved between docks and windows as it is). */
  containerOf: (panel: PanelId) => HTMLElement;
  /** The pop-out windows open now. */
  popUps: readonly PopUp[];
}

export const WorkspaceContext = createContext<WorkspaceApi | null>(null);

/** The organization window's panels (from `WorkspaceProvider`). */
export function useWorkspace(): WorkspaceApi {
  const api = useContext(WorkspaceContext);
  if (!api) throw new Error("useWorkspace needs a WorkspaceProvider");
  return api;
}

/** The workspace, or `null` where there is none (a part shown on its own, as in tests). */
export function useWorkspaceIfAny(): WorkspaceApi | null {
  return useContext(WorkspaceContext);
}

/**
 * The window the panel around this part is drawn in (Plenipo's own window, or the panel's
 * pop-out). Parts that listen to their window (sizes, keys) use it.
 */
export const PanelWindowContext = createContext<Window | null>(null);

export function usePanelWindow(): Window {
  return useContext(PanelWindowContext) ?? window;
}
