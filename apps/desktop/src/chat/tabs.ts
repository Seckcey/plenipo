/**
 * The chats open in the Chat panel (ADR-200): a tab for each agent you talk to or watch, kept on
 * this computer so they come back after a restart. A chat can also have a window of its own
 * (ADR-203), up to six at once, and comes back to the panel when it is put back. Every change is
 * a plain function, so it is easy to test.
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

/** A chat in a window of its own (ADR-203): its tab, and which of the windows (1 to 6) it is in. */
export interface PoppedChat {
  key: string;
  slot: number;
}

export interface ChatTabs {
  tabs: ChatTab[];
  active: string | null;
  /** Show the open chats side by side, instead of one at a time. */
  sideBySide: boolean;
  /** The chats in windows of their own. */
  popped: PoppedChat[];
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
/** The most chats with windows of their own at once (the owner's choice, ADR-203). */
export const MAX_CHAT_WINDOWS = 6;

export const NO_TABS: ChatTabs = { tabs: [], active: null, sideBySide: false, popped: [] };

/** A tab's key: its position's, or else its conversation's. */
export function keyOf(target: Pick<ChatTarget, "positionId" | "sessionId">): string | null {
  if (target.positionId) return `position:${target.positionId}`;
  if (target.sessionId) return `session:${target.sessionId}`;
  return null;
}

/** The window a chat is in (1 to 6), or `null` while it is in the Chat panel. */
export function slotOf(state: ChatTabs, key: string): number | null {
  return state.popped.find((p) => p.key === key)?.slot ?? null;
}

/** The chat a window shows, if any. */
export function tabInSlot(state: ChatTabs, slot: number): ChatTab | null {
  const key = state.popped.find((p) => p.slot === slot)?.key;
  return state.tabs.find((t) => t.key === key) ?? null;
}

/** The chat for `target` (`null` when it names neither a position nor a conversation). */
export function tabFor(target: ChatTarget): ChatTab | null {
  const key = keyOf(target);
  if (key === null) return null;
  return {
    key,
    positionId: target.positionId ?? null,
    sessionId: target.sessionId ?? null,
    title: target.title.trim() || "Agent",
  };
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
  // Too many: the first one that is not in front (and has no window of its own) goes.
  while (tabs.length > MAX_TABS) {
    const drop = tabs.findIndex(
      (t) => t.key !== state.active && t.key !== key && slotOf(state, t.key) === null,
    );
    tabs = tabs.filter((_, i) => i !== (drop < 0 ? 0 : drop));
  }
  const popped = state.popped.filter((p) => tabs.some((t) => t.key === p.key));
  return { ...state, tabs, active: key, popped };
}

/** Close a chat (its window too, if it has one); the one beside it comes to the front. */
export function closeTab(state: ChatTabs, key: string): ChatTabs {
  const at = state.tabs.findIndex((t) => t.key === key);
  if (at < 0) return state;
  const tabs = state.tabs.filter((t) => t.key !== key);
  const popped = state.popped.filter((p) => p.key !== key);
  if (state.active !== key) return { ...state, tabs, popped };
  const next = tabs[at] ?? tabs[at - 1] ?? null;
  return { ...state, tabs, popped, active: next?.key ?? null };
}

export function showTab(state: ChatTabs, key: string): ChatTabs {
  return state.tabs.some((t) => t.key === key) && state.active !== key
    ? { ...state, active: key }
    : state;
}

/**
 * Give an open chat a window of its own: the first of the six that is free. A chat that has one
 * keeps it. `null` when the chat is not open, or all six windows are taken.
 */
export function popOutTab(state: ChatTabs, key: string): ChatTabs | null {
  if (!state.tabs.some((t) => t.key === key)) return null;
  if (slotOf(state, key) !== null) return state;
  const taken = new Set(state.popped.map((p) => p.slot));
  for (let slot = 1; slot <= MAX_CHAT_WINDOWS; slot += 1) {
    if (!taken.has(slot)) return { ...state, popped: [...state.popped, { key, slot }] };
  }
  return null;
}

/** Put a chat back in the Chat panel (its window closes), in front. */
export function putBackTab(state: ChatTabs, key: string): ChatTabs {
  if (slotOf(state, key) === null) return state;
  return { ...state, active: key, popped: state.popped.filter((p) => p.key !== key) };
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

/**
 * The chats the panel shows now: the one in front, or (side by side) up to four, the front one
 * first. Side by side leaves out the chats that are in windows of their own; one at a time, the
 * front one shows even then (the panel says where it is).
 */
export function shownTabs(state: ChatTabs): ChatTab[] {
  const front = state.tabs.find((t) => t.key === state.active) ?? state.tabs[0];
  if (!front) return [];
  if (!state.sideBySide) return [front];
  const here = [front, ...state.tabs.filter((t) => t !== front)].filter(
    (t) => slotOf(state, t.key) === null,
  );
  return here.length > 0 ? here.slice(0, SIDE_BY_SIDE_MAX) : [front];
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

function isPopped(v: unknown): v is PoppedChat {
  if (typeof v !== "object" || v === null) return false;
  const p = v as Record<string, unknown>;
  return (
    isText(p.key) &&
    typeof p.slot === "number" &&
    Number.isInteger(p.slot) &&
    p.slot >= 1 &&
    p.slot <= MAX_CHAT_WINDOWS
  );
}

/**
 * Chats read back from this computer, checked part by part. Those kept before chats had windows
 * of their own have none (`popped` is then left out).
 */
export function isChatTabs(v: unknown): v is Omit<ChatTabs, "popped"> & { popped?: PoppedChat[] } {
  if (typeof v !== "object" || v === null) return false;
  const s = v as Record<string, unknown>;
  if (
    !Array.isArray(s.tabs) ||
    s.tabs.length > MAX_TABS ||
    !s.tabs.every(isTab) ||
    !isTextOrNull(s.active) ||
    typeof s.sideBySide !== "boolean"
  ) {
    return false;
  }
  if (s.popped === undefined) return true;
  if (!Array.isArray(s.popped) || s.popped.length > MAX_CHAT_WINDOWS) return false;
  const popped: unknown[] = s.popped;
  const tabs = s.tabs;
  return (
    popped.every(isPopped) &&
    // Each window shows one open chat, and each chat is in one window.
    new Set(popped.map((p) => p.slot)).size === popped.length &&
    new Set(popped.map((p) => p.key)).size === popped.length &&
    popped.every((p) => tabs.some((t) => t.key === p.key))
  );
}

/** Chats read back from this computer, or none when what was kept is not chats. */
export function readChatTabs(v: unknown): ChatTabs {
  return isChatTabs(v) ? { ...v, popped: v.popped ?? [] } : NO_TABS;
}

/** A position's agent's conversation: its newest open one. */
export function positionSession(state: AgentState, positionId: string): string | null {
  for (const id of state.order) {
    const s = state.sessions[id];
    if (s && s.state === "open" && liaisonInfo(s).positionId === positionId) return id;
  }
  return null;
}

/** The most conversations a position's chat shows as one thread. */
export const THREAD_MAX = 10;

/**
 * A position's conversations, oldest first, closed ones too: its chat shows them as one thread
 * (B5). An on-call worker gets a new conversation for each task its lead hands it, so this is
 * everything it was asked and answered. At most the newest `THREAD_MAX`.
 */
export function positionThread(state: AgentState, positionId: string): string[] {
  return state.order
    .filter((id) => {
      const s = state.sessions[id];
      return s !== undefined && liaisonInfo(s).positionId === positionId;
    })
    .slice(0, THREAD_MAX)
    .sort((a, b) => (state.sessions[a]?.createdAt ?? 0) - (state.sessions[b]?.createdAt ?? 0));
}
