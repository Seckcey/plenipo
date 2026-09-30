/**
 * The terminal panel's tabs (ADR-031, ADR-055, ADR-058). Where the panel is (a dock, or its own
 * window) is the window's layout (Phase 21, `workspace/layout.ts`).
 */

import type { AccountAction, Environment, TerminalPlace } from "@plenipo/types";

import type { WatchTab } from "./watch";

/** The Terminal button in the top bar (the keyboard goes back to it when the panel hides). */
export const TERMINAL_BUTTON_ID = "terminal-button";

/** One of the owner's terminals, as its tab knows it. */
export interface OwnerTab {
  kind: "owner";
  /** The tab's own ID (the terminal's ID comes once it opens). */
  id: string;
  place: TerminalPlace;
  title: string;
  environment: Environment | null;
  openedAt: number;
  /**
   * Opened by itself (an AI tool's sign-in tab that waited until the tool was free): the
   * keyboard stays where it is.
   */
  quiet?: boolean;
}

/** An AI tool's sign-in or sign-out tab's name: "Sign in · Codex" (ADR-058). */
export function aiToolTitle(label: string, action: AccountAction): string {
  return `${action === "signIn" ? "Sign in" : "Sign out"} · ${label}`;
}

/**
 * Plenipo would not open an AI tool's sign-in or sign-out tab because the tool is busy for now: a
 * task is using it ("1 task is using Codex. Plenipo waits until it finishes."), or it is being
 * updated ("Codex is being updated. Plenipo waits until it's done.").
 */
export function isBusyRefusal(message: string): boolean {
  return /\btasks? (?:is|are) using\b|\bis being updated\b/.test(message);
}

export interface WatchEntry {
  kind: "watch";
  id: string;
  watch: WatchTab;
}

/**
 * A Watch tab for code (Phase 18, ADR-055): what one agent's workers change in its objective,
 * read-only. One tab per agent; it stays, and readable, until the owner closes it.
 */
export interface CodeTab {
  kind: "code";
  /** `code:` and the agent's position ID. */
  id: string;
  positionId: string;
  /** The agent's title ("Senior Developer"). */
  title: string;
  openedAt: number;
}

/** The ID of an agent's Watch tab for code. */
export function codeTabId(positionId: string): string {
  return `code:${positionId}`;
}

export type TerminalTab = OwnerTab | WatchEntry | CodeTab;

/** When a tab opened (tabs are shown in that order). */
export function openedAt(tab: TerminalTab): number {
  return tab.kind === "watch" ? tab.watch.openedAt : tab.openedAt;
}
