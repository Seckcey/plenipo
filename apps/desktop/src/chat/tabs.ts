/**
 * The chats open in the Chat panel (ADR-200): a tab for each agent you talk to or watch, kept on
 * this computer so they come back after a restart. Every change is a plain function, so it is
 * easy to test.
 */
import { liaisonInfo, type AgentState } from "../agents/store";

/** One open chat: a full-time agent's position (you talk to it), or a conversation (you watch). */
export interface ChatTab {
  key: string;
  /** The position you talk to: what you send goes to its agent. */
  positionId: string | null;
  /** The conversation it shows, once there is one. */
  sessionId: string | null;
  title: string;
}

export interface ChatTabs {
  tabs: ChatTab[];
  active: string | null;
  /** Show the open chats side by side, instead of one at a time. */
  sideBySide: boolean;
}

/** What to open: a position, a conversation, or both (a position's conversation). */
export interface ChatTarget {
  positionId?: string | null;
  sessionId?: string | null;
  title: string;
}

export const TABS_KEY = "plenipo.chat";
/** The most chats kept open; opening one more closes the one you looked at longest ago. */
export const MAX_TABS = 12;
/** The most chats shown side by side. */
export const SIDE_BY_SIDE_MAX = 4;

export const NO_TABS: ChatTabs = { tabs: [], active: null, sideBySide: false };

/** A tab's key: its position's, or else its conversation's. */
export function keyOf(target: Pick<ChatTarget, "positionId" | "sessionId">): string | null {
  if (target.positionId) return `position:${target.positionId}`;
  if (target.sessionId) return `session:${target.sessionId}`;
  return null;
}

/** Open a chat (or show it, when it is open already), in front. */
export function openTab(state: ChatTabs, target: ChatTarget): ChatTabs {
  const key = keyOf(target);
  if (key === null) return state;
  const title = target.title.trim() || "Agent";
  // A conversation that a position's tab shows already opens that tab.
  const same = state.tabs.find(
    (t) =>
      t.key === key || (target.sessionId && !target.positionId && t.sessionId === target.sessionId),
  );
  if (same) {
    const tabs = state.tabs.map((t) =>
      t === same ? { ...t, title, sessionId: target.sessionId ?? t.sessionId } : t,
    );
    return { ...state, tabs, active: same.key };
  }
  const tab: ChatTab = {
    key,
    positionId: target.positionId ?? null,
    sessionId: target.sessionId ?? null,
    title,
  };
  let tabs = [...state.tabs, tab];
  // Too many: the first one that is not in front goes.
  while (tabs.length > MAX_TABS) {
    const drop = tabs.findIndex((t) => t.key !== state.active && t.key !== key);
    tabs = tabs.filter((_, i) => i !== (drop < 0 ? 0 : drop));
  }
  return { ...state, tabs, active: key };
}

/** Close a chat; the one beside it comes to the front. */
export function closeTab(state: ChatTabs, key: string): ChatTabs {
  const at = state.tabs.findIndex((t) => t.key === key);
  if (at < 0) return state;
  const tabs = state.tabs.filter((t) => t.key !== key);
  if (state.active !== key) return { ...state, tabs };
  const next = tabs[at] ?? tabs[at - 1] ?? null;
  return { ...state, tabs, active: next?.key ?? null };
}

export function showTab(state: ChatTabs, key: string): ChatTabs {
  return state.tabs.some((t) => t.key === key) && state.active !== key
    ? { ...state, active: key }
    : state;
}

/** The conversation a chat shows now (a position's agent's first one, or a new one). */
export function setSession(state: ChatTabs, key: string, sessionId: string): ChatTabs {
  const tab = state.tabs.find((t) => t.key === key);
  if (!tab || tab.sessionId === sessionId) return state;
  return {
    ...state,
    tabs: state.tabs.map((t) => (t.key === key ? { ...t, sessionId } : t)),
  };
}

/** The chats shown now: the one in front, or (side by side) up to four, the front one first. */
export function shownTabs(state: ChatTabs): ChatTab[] {
  const front = state.tabs.find((t) => t.key === state.active) ?? state.tabs[0];
  if (!front) return [];
  if (!state.sideBySide) return [front];
  return [front, ...state.tabs.filter((t) => t !== front)].slice(0, SIDE_BY_SIDE_MAX);
}

const isText = (v: unknown): v is string => typeof v === "string" && v.length <= 300;
const isTextOrNull = (v: unknown): v is string | null => v === null || isText(v);

function isTab(v: unknown): v is ChatTab {
  if (typeof v !== "object" || v === null) return false;
  const t = v as Record<string, unknown>;
  return (
    isText(t.key) &&
    isTextOrNull(t.positionId) &&
    isTextOrNull(t.sessionId) &&
    isText(t.title) &&
    (t.positionId !== null || t.sessionId !== null)
  );
}

/** Chats read back from this computer, checked part by part. */
export function isChatTabs(v: unknown): v is ChatTabs {
  if (typeof v !== "object" || v === null) return false;
  const s = v as Record<string, unknown>;
  return (
    Array.isArray(s.tabs) &&
    s.tabs.length <= MAX_TABS &&
    s.tabs.every(isTab) &&
    isTextOrNull(s.active) &&
    typeof s.sideBySide === "boolean"
  );
}

/** A position's agent's conversation: its newest open one. */
export function positionSession(state: AgentState, positionId: string): string | null {
  for (const id of state.order) {
    const s = state.sessions[id];
    if (s && s.state === "open" && liaisonInfo(s).positionId === positionId) return id;
  }
  return null;
}
