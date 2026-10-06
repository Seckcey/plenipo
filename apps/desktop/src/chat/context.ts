import { createContext, useContext } from "react";

import type { ChatSession } from "./model";
import type { ChatTab, ChatTabs, ChatTarget } from "./tabs";

/** What went wrong in a chat: its heading, and why, in plain words. */
export interface ChatProblem {
  heading: string;
  message: string;
}

/** The Chat panel's state and what it can do (ADR-200), from `ChatProvider`. */
export interface ChatApi {
  tabs: ChatTabs;
  /** Open a chat with an agent (or watch a worker's conversation) in the Chat panel. */
  open: (target: ChatTarget) => void;
  close: (key: string) => void;
  show: (key: string) => void;
  setSideBySide: (on: boolean) => void;
  /** Chats can have windows of their own here (ADR-203); a page drawn on its own has none. */
  canPopOut: boolean;
  /**
   * Give an open chat a window of its own (up to six at once); one that has one comes to the
   * front. When all six are taken, it shows in the panel and says why.
   */
  popOut: (key: string) => void;
  /** Open a chat, as `open` does, in a window of its own instead of the panel. */
  openWindow: (target: ChatTarget) => void;
  /** Put a chat back in the Chat panel (its window closes). */
  putBack: (key: string) => void;
  /** Bring a chat's own window to the front. */
  focusWindow: (key: string) => void;
  /** The window a chat is in (1 to 6), or `null` while it is in the panel. */
  windowSlot: (key: string) => number | null;
  /**
   * Show a chat outside the Chat panel (the Workers page): its conversation is fetched and kept
   * live, and it works as in the panel (messages, Stop, waiting messages), without a tab of its
   * own. Returns the function that stops showing it there.
   */
  showElsewhere: (target: ChatTarget) => () => void;
  /** An open chat, in the panel or shown elsewhere. */
  tab: (key: string) => ChatTab | null;
  /** The conversation a chat shows (its ID): the tab's, or its position's agent's. */
  sessionOf: (tab: ChatTab) => string | null;
  /** The conversation a chat shows, as far as Plenipo has it; `null` before the first message. */
  conversation: (tab: ChatTab) => ChatSession | null;
  /** The agent is busy: what you send now waits until it finishes. */
  busy: (tab: ChatTab) => boolean;
  /** Send a message. While the agent is busy it waits its turn, and goes when the agent is free. */
  send: (key: string, text: string) => void;
  /** Messages waiting for the agent to finish, oldest first. */
  queued: (key: string) => readonly string[];
  unqueue: (key: string, index: number) => void;
  /**
   * Stop what the agent is doing now. When Plenipo says nothing is running, the conversation is
   * fetched again, so a chat that missed its turn's end stops showing it as running.
   */
  stop: (key: string) => Promise<void>;
  /** A stop is on its way: its AI tool is asked to stop, and ended if it does not. */
  stopping: (key: string) => boolean;
  /** The agent did not stop when asked, and what to try next; shown while it still works. */
  stopNote: (key: string) => string | null;
  sending: (key: string) => boolean;
  /** Where the last message went, when it went somewhere else (an on-call worker's lead). */
  note: (key: string) => string | null;
  /** Why the last message did not go, or the last stop did not work, until it is dismissed. */
  problem: (key: string) => ChatProblem | null;
  dismissProblem: (key: string) => void;
}

export const ChatContext = createContext<ChatApi | null>(null);

export function useChat(): ChatApi {
  const api = useContext(ChatContext);
  if (!api) throw new Error("useChat needs a ChatProvider");
  return api;
}

/** The chats, where there are any (a page drawn without them, as in a test, has none). */
export function useChatIfAny(): ChatApi | null {
  return useContext(ChatContext);
}
