import { createContext, useContext } from "react";
import type { PopOutTarget, WindowPlace } from "@plenipo/types";

import type { DockSide, Layout, PanelId } from "./layout";

/** A panel's pop-out. */
export function panelTarget(panel: PanelId): PopOutTarget {
  return { kind: "panel", panel };
}

/** One chat's own window (ADR-203), `slot` 1 to 6. */
export function chatTarget(slot: number): PopOutTarget {
  return { kind: "chat", slot };
}

/** A pop-out's name among the window's pop-outs. */
export function targetKey(target: PopOutTarget): string {
  return target.kind === "panel" ? `panel:${target.panel}` : `chat:${target.slot}`;
}

/** A panel's tab being dragged (by the pointer), and where it would go if let go now. */
export interface PanelDrag {
  panel: PanelId;
  /** The dock under the pointer (marked), or none. */
  over: DockSide | null;
  /** The pointer is past the window's edges: letting go pops the panel out there. */
  outside: boolean;
}

/** One pop-out window (a panel's, or a chat's), and where in it its parts and title bar go. */
export interface PopUp {
  target: PopOutTarget;
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
  /** A panel's tab being dragged now. */
  drag: PanelDrag | null;
  setDrag: (drag: PanelDrag | null) => void;
  /** Where each dock is on screen, for dragging (the work area's parts). */
  dockAt: (clientX: number, clientY: number) => DockSide | null;
  /** The element each panel is drawn into (moved between docks and windows as it is). */
  containerOf: (panel: PanelId) => HTMLElement;
  /** The pop-out windows open now (panels' and chats'). */
  popUps: readonly PopUp[];

  /**
   * Open the window of one chat (ADR-203): window `slot` (1 to 6), titled with its agent. `false`
   * if it could not open. The chat draws itself into the window's body.
   */
  openChatWindow: (slot: number, title: string) => Promise<boolean>;
  /** Close a chat's window on purpose (the chat goes back to the Chat panel). */
  closeChatWindow: (slot: number) => void;
  /** Bring a chat's window to the front. */
  focusChatWindow: (slot: number) => void;
  /**
   * Hear when a chat's window closes without the chat asking: the owner closed it (`slot`), or
   * Reset layout closed every one (`null`). Returns a function that stops listening.
   */
  onChatWindowClosed: (listener: (slot: number | null) => void) => () => void;
}

export const WorkspaceContext = createContext<WorkspaceApi | null>(null);

/** The organization window's panels (from `WorkspaceProvider`). */
export function useWorkspace(): WorkspaceApi {
  const api = useContext(WorkspaceContext);
  if (!api) throw new Error("useWorkspace needs a WorkspaceProvider");
  return api;
}

/**
 * The work area (the page and its docks), measured through this callback ref: the docks' sizes
 * keep within it. On its own, apart from the rest, so the ref is only ever a ref.
 */
export const WorkspaceAreaContext = createContext<((el: HTMLElement | null) => void) | null>(null);

export function useWorkspaceArea(): (el: HTMLElement | null) => void {
  const measure = useContext(WorkspaceAreaContext);
  if (!measure) throw new Error("useWorkspaceArea needs a WorkspaceProvider");
  return measure;
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
