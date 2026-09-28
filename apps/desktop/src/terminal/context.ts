import { createContext } from "react";
import type { AccountAction, Environment } from "@plenipo/types";

import type { OwnerTab, PanelState, TerminalTab } from "./panel";

/** An AI tool's sign-in or sign-out tab that Plenipo would not open while a task used the tool. */
export interface AiToolRefusal {
  action: AccountAction;
  /** Plenipo's words ("1 task is using Codex. Plenipo waits until it finishes."). */
  message: string;
  /** When it was refused (ms). */
  at: number;
}

export interface TerminalApi {
  panel: PanelState;
  /** The panel's size, kept within the room the page leaves. */
  size: number;
  maxSize: number;
  setSize: (size: number) => void;
  setSide: (side: PanelState["side"]) => void;
  toggle: () => void;
  show: () => void;
  hide: () => void;
  tabs: TerminalTab[];
  active: string | null;
  setActive: (id: string) => void;
  /** A new terminal on this PC, or on a server from Settings → Servers. */
  openHere: () => void;
  openServer: (serverId: string, name: string, environment: Environment) => void;
  /**
   * Watch an agent write code (Phase 18, ADR-055): opens its Watch tab, or goes to the one
   * already open, and shows the panel.
   */
  openWatch: (positionId: string, title: string) => void;
  /**
   * An AI tool's sign-in or sign-out tab (Phase 19, ADR-058): it runs that tool's own command,
   * from a fixed list, and the panel shows. `label` is the tool's name ("Codex"). A tab already
   * running the same command is shown instead of a second one.
   */
  openAiTool: (runtimeId: string, label: string, action: AccountAction) => void;
  /** When each AI tool's sign-in or sign-out tab last ended (ms), by runtime ID. */
  signInEnded: Readonly<Record<string, number>>;
  /**
   * The last sign-in or sign-out tab refused because a task was using the AI tool, by runtime ID
   * (that tab closes by itself: the AI tools page waits, then opens it again).
   */
  signInRefused: Readonly<Record<string, AiToolRefusal>>;
  /** The panel's own: one of the owner's terminals ended, or could not open. */
  tabEnded: (tab: OwnerTab) => void;
  tabFailed: (tab: OwnerTab, message: string) => void;
  close: (id: string) => void;
  /** Watch tabs that opened while the owner looked elsewhere. */
  unseen: number;
  /** Measure the work area (the page and the panel) through this ref. */
  measure: (el: HTMLElement | null) => void;
}

export const TerminalContext = createContext<TerminalApi | null>(null);
