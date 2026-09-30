import { createContext } from "react";
import type { AccountAction, AgentSession, Environment } from "@plenipo/types";

import type { OwnerTab, TerminalTab } from "./panel";

/**
 * An AI tool's sign-in or sign-out tab waiting for the tool to be free (ADR-058 §5). It is kept
 * here, not on the AI tools page, so it still opens after the page or the card's tab changes.
 */
export interface AiToolWait {
  action: AccountAction;
  /** The tool's name ("Codex"). */
  label: string;
  /**
   * Plenipo would not open it because the tool was busy: its words ("1 task is using Codex.
   * Plenipo waits until it finishes.").
   */
  refused: string | null;
  /** The conversations when it was last refused: it tries again soon after they change. */
  sessions: Readonly<Record<string, AgentSession>> | null;
  /** Refusals in a row while the conversations stayed the same. */
  refusals: number;
  /** The tab opening now: the wait ends when it opens, and goes on if it is refused. */
  opening: string | null;
  /** Refused too many times while nothing changed: it no longer tries by itself (Try again). */
  stopped: boolean;
  /** When it started or was last refused (ms). */
  at: number;
}

export interface TerminalApi {
  /** The panel can be seen (its dock open and showing it, or popped out; ADR-092). */
  open: boolean;
  /**
   * Where the panel is, as the AI tools page words it ("on the right", "at the bottom"; it is
   * left as it was while Phase 16's second wave changes that page). Anything but the right side
   * reads as the bottom.
   */
  panel: { open: boolean; side: "bottom" | "right" };
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
   * running the same command is shown instead of a second one. It ends the tool's wait, if any.
   */
  openAiTool: (runtimeId: string, label: string, action: AccountAction) => void;
  /**
   * Open that tab when the AI tool is free (no task is using it). A tab Plenipo refuses because
   * the tool is busy waits the same way by itself.
   */
  waitForAiTool: (runtimeId: string, label: string, action: AccountAction) => void;
  /** Stop waiting to open an AI tool's tab. */
  cancelAiToolWait: (runtimeId: string) => void;
  /** Each AI tool's tab waiting for the tool to be free, by runtime ID. */
  aiToolWaits: Readonly<Record<string, AiToolWait>>;
  /** When each AI tool's sign-in or sign-out tab last ended (ms), by runtime ID. */
  signInEnded: Readonly<Record<string, number>>;
  /** The panel's own: one of the owner's terminals opened, ended, or could not open. */
  tabOpened: (tab: OwnerTab) => void;
  tabEnded: (tab: OwnerTab) => void;
  tabFailed: (tab: OwnerTab, message: string) => void;
  close: (id: string) => void;
  /** Watch tabs that opened while the owner looked elsewhere. */
  unseen: number;
}

export const TerminalContext = createContext<TerminalApi | null>(null);
