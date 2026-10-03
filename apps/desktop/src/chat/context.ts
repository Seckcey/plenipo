import { createContext, useContext } from "react";

import type { ChatSession } from "./model";
import type { ChatTab, ChatTabs, ChatTarget } from "./tabs";

/** The Chat panel's state and what it can do (ADR-200), from `ChatProvider`. */
export interface ChatApi {
  tabs: ChatTabs;
  /** Open a chat with an agent (or watch a worker's conversation) in the Chat panel. */
  open: (target: ChatTarget) => void;
  close: (key: string) => void;
  show: (key: string) => void;
  setSideBySide: (on: boolean) => void;
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
  /** Stop what the agent is doing now. */
  stop: (key: string) => Promise<void>;
  sending: (key: string) => boolean;
  /** Where the last message went, when it went somewhere else (an on-call worker's lead). */
  note: (key: string) => string | null;
  /** Why the last message did not go, until it is dismissed or the next one goes. */
  problem: (key: string) => string | null;
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
