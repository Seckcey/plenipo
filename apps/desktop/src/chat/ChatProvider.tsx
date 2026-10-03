import {
  startTransition,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { AgentSessionDetail, AgentUpdate } from "@plenipo/types";
import { storedKey } from "@plenipo/ui";

import { useAgents } from "../agents/useAgents";
import { isRunning, isWaiting, type AgentState } from "../agents/store";
import {
  cancelAgentTurn,
  getAgentSession,
  giveObjective,
  resumeAgentSession,
  toCommandError,
} from "../api/commands";
import { subscribeAgentUpdates } from "../api/events";
import { useWorkspaceIfAny } from "../workspace/context";
import { ChatContext, type ChatApi } from "./context";
import {
  applyActivity,
  applyTurn,
  emptySession,
  isBusy,
  mergeDetail,
  type ChatSession,
} from "./model";
import {
  closeTab,
  isChatTabs,
  NO_TABS,
  openTab,
  positionSession,
  setSession,
  showTab,
  TABS_KEY,
  type ChatTab,
  type ChatTabs,
} from "./tabs";

type Conversations = Record<string, ChatSession>;

function readTabs(): ChatTabs {
  try {
    const raw = localStorage.getItem(storedKey(TABS_KEY));
    const value: unknown = raw === null ? null : JSON.parse(raw);
    return isChatTabs(value) ? value : NO_TABS;
  } catch {
    return NO_TABS;
  }
}

function writeTabs(tabs: ChatTabs) {
  try {
    localStorage.setItem(storedKey(TABS_KEY), JSON.stringify(tabs));
  } catch {
    // Storage unavailable: the chats stay open until the window closes.
  }
}

/** The refusal means only that the agent is busy: the message waits and goes later. */
function saysBusy(message: string): boolean {
  return /already running|waiting to continue|busy with/i.test(message);
}

/** One live update applied to the conversations held (others are not loaded, so skipped). */
function applyUpdate(all: Conversations, update: AgentUpdate): Conversations {
  if (update.kind !== "activity" && update.kind !== "turn") return all;
  const held = all[update.sessionId];
  if (!held) return all;
  const next = update.kind === "activity" ? applyActivity(held, update) : applyTurn(held, update);
  return next === held ? all : { ...all, [update.sessionId]: next };
}

/**
 * The chats with the agents (ADR-200): which are open, each conversation as it streams in (built
 * from the live updates, word by word), and what you send. A message sent while its agent is busy
 * waits, and goes by itself when the agent finishes, as in Claude Code. Drawn once for the
 * window, so a chat keeps everything while its panel is hidden or popped out.
 */
export function ChatProvider({ children }: { children: ReactNode }) {
  const agents = useAgents();
  const agentState = agents.state;
  const ws = useWorkspaceIfAny();
  const show = ws?.show;

  const [tabs, setTabsState] = useState<ChatTabs>(readTabs);
  const tabsRef = useRef(tabs);
  const setTabs = useCallback((change: (t: ChatTabs) => ChatTabs) => {
    const next = change(tabsRef.current);
    if (next === tabsRef.current) return;
    tabsRef.current = next;
    setTabsState(next);
    writeTabs(next);
  }, []);

  const [conversations, setConversationsState] = useState<Conversations>({});
  const conversationsRef = useRef(conversations);
  const setConversations = useCallback((change: (c: Conversations) => Conversations) => {
    const next = change(conversationsRef.current);
    if (next === conversationsRef.current) return;
    conversationsRef.current = next;
    // Words arrive many times a second: typing in the message box comes first.
    startTransition(() => setConversationsState(next));
  }, []);

  const [queues, setQueuesState] = useState<Record<string, string[]>>({});
  const queuesRef = useRef(queues);
  const setQueues = useCallback(
    (change: (q: Record<string, string[]>) => Record<string, string[]>) => {
      const next = change(queuesRef.current);
      queuesRef.current = next;
      setQueuesState(next);
    },
    [],
  );
  const [sendingKeys, setSendingKeys] = useState<Record<string, boolean>>({});
  const sendingRef = useRef<Record<string, boolean>>({});
  const [problems, setProblems] = useState<Record<string, string | null>>({});

  /** Conversations being fetched, and the live updates that came for them meanwhile. */
  const loading = useRef(new Map<string, AgentUpdate[]>());
  const agentStateRef = useRef(agentState);
  useEffect(() => {
    agentStateRef.current = agentState;
  }, [agentState]);

  const sessionOfTab = useCallback(
    (tab: ChatTab, state: AgentState = agentStateRef.current): string | null =>
      tab.sessionId ?? (tab.positionId ? positionSession(state, tab.positionId) : null),
    [],
  );

  /** Busy now, from the newest that is known: the conversation as it streams, or the session. */
  const busyNow = useCallback(
    (tab: ChatTab): boolean => {
      const id = sessionOfTab(tab);
      if (!id) return false;
      const held = conversationsRef.current[id];
      if (held) return isBusy(held);
      const s = agentStateRef.current.sessions[id];
      return isRunning(s) || isWaiting(s);
    },
    [sessionOfTab],
  );

  const take = useCallback(
    (detail: AgentSessionDetail) => {
      const id = detail.session.id;
      setConversations((c) => ({ ...c, [id]: mergeDetail(c[id] ?? emptySession(id), detail) }));
    },
    [setConversations],
  );

  /** Fetch a conversation (once at a time), then apply what came live while it was fetched. */
  const load = useCallback(
    (id: string) => {
      if (loading.current.has(id)) return;
      loading.current.set(id, []);
      getAgentSession(id)
        .then((detail) => {
          const early = loading.current.get(id) ?? [];
          loading.current.delete(id);
          setConversations((c) => {
            let all: Conversations = {
              ...c,
              [id]: mergeDetail(c[id] ?? emptySession(id), detail),
            };
            for (const update of early) all = applyUpdate(all, update);
            return all;
          });
        })
        .catch(() => {
          loading.current.delete(id);
        });
    },
    [setConversations],
  );

  const flushRef = useRef<() => void>(() => undefined);

  // The live stream: each piece goes to its conversation as it arrives.
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    subscribeAgentUpdates((update) => {
      if (disposed) return;
      if (update.kind !== "activity" && update.kind !== "turn") return;
      const early = loading.current.get(update.sessionId);
      if (early) {
        early.push(update);
        return;
      }
      setConversations((c) => applyUpdate(c, update));
      // A finished turn frees its agent: a message that waited can go now.
      if (update.kind === "turn" && update.result) queueMicrotask(() => flushRef.current());
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      stop?.();
    };
  }, [setConversations]);

  /** Send one message now. `false` when it could not go. */
  const deliver = useCallback(
    async (tab: ChatTab, text: string): Promise<boolean> => {
      const sessionId = sessionOfTab(tab);
      if (!tab.positionId && !sessionId) return false;
      sendingRef.current = { ...sendingRef.current, [tab.key]: true };
      setSendingKeys(sendingRef.current);
      try {
        const detail = tab.positionId
          ? await giveObjective(tab.positionId, text)
          : await resumeAgentSession(sessionId ?? "", text);
        setTabs((t) => setSession(t, tab.key, detail.session.id));
        take(detail);
        setProblems((p) => ({ ...p, [tab.key]: null }));
        return true;
      } catch (reason) {
        const message = toCommandError(reason).message;
        if (saysBusy(message)) {
          // Busy after all: it waits for the agent, first in line.
          setQueues((q) => ({ ...q, [tab.key]: [text, ...(q[tab.key] ?? [])] }));
        } else {
          setProblems((p) => ({ ...p, [tab.key]: message }));
        }
        return false;
      } finally {
        sendingRef.current = { ...sendingRef.current, [tab.key]: false };
        setSendingKeys(sendingRef.current);
      }
    },
    [sessionOfTab, setQueues, setTabs, take],
  );

  /** Send each chat's oldest waiting message whose agent is free now. */
  const flush = useCallback(() => {
    for (const tab of tabsRef.current.tabs) {
      const waiting = queuesRef.current[tab.key];
      if (!waiting || waiting.length === 0) continue;
      if (sendingRef.current[tab.key] || busyNow(tab)) continue;
      const [first, ...rest] = waiting;
      if (first === undefined) continue;
      setQueues((q) => ({ ...q, [tab.key]: rest }));
      void deliver(tab, first);
    }
  }, [busyNow, deliver, setQueues]);
  useEffect(() => {
    flushRef.current = flush;
  }, [flush]);

  // The conversations the open chats show are fetched once, then kept live.
  const wanted = useMemo(
    () =>
      tabs.tabs
        .map(
          (tab) =>
            tab.sessionId ?? (tab.positionId ? positionSession(agentState, tab.positionId) : null),
        )
        .filter((id): id is string => id !== null),
    [tabs, agentState],
  );
  useEffect(() => {
    for (const id of wanted) if (!conversationsRef.current[id]) load(id);
  }, [wanted, load]);

  // A turn that ended while its end was missed (the window was asleep): fetched again once, and
  // what waited goes then.
  const stale = useMemo(
    () =>
      wanted.flatMap((id) => {
        const held = conversations[id];
        const s = agentState.sessions[id];
        if (!held || !s || !isBusy(held) || isRunning(s) || isWaiting(s)) return [];
        const last = held.turns[held.turns.length - 1];
        return [{ id, key: `${id}:${last?.taskId ?? ""}` }];
      }),
    [wanted, conversations, agentState],
  );
  const refetched = useRef(new Set<string>());
  useEffect(() => {
    for (const { id, key } of stale) {
      if (refetched.current.has(key)) continue;
      refetched.current.add(key);
      getAgentSession(id)
        .then((detail) => {
          take(detail);
          queueMicrotask(() => flushRef.current());
        })
        .catch(() => undefined);
    }
  }, [stale, take]);

  const api = useMemo<ChatApi>(() => {
    const tabOf = (key: string) => tabs.tabs.find((t) => t.key === key);
    return {
      tabs,
      open: (target) => {
        setTabs((t) => openTab(t, target));
        show?.("chat");
      },
      close: (key) => {
        setTabs((t) => closeTab(t, key));
        setQueues((q) => {
          if (!(key in q)) return q;
          const rest = { ...q };
          delete rest[key];
          return rest;
        });
      },
      show: (key) => setTabs((t) => showTab(t, key)),
      setSideBySide: (on) => setTabs((t) => (t.sideBySide === on ? t : { ...t, sideBySide: on })),
      sessionOf: (tab) => sessionOfTab(tab, agentState),
      conversation: (tab) => {
        const id = sessionOfTab(tab, agentState);
        return id ? (conversations[id] ?? null) : null;
      },
      busy: (tab) => {
        const id = sessionOfTab(tab, agentState);
        if (!id) return false;
        const held = conversations[id];
        if (held && isBusy(held)) return true;
        const s = agentState.sessions[id];
        return isRunning(s) || isWaiting(s);
      },
      send: (key, text) => {
        const tab = tabOf(key);
        const message = text.trim();
        if (!tab || message === "") return;
        const waiting = (queuesRef.current[key]?.length ?? 0) > 0;
        if (waiting || sendingRef.current[key] || busyNow(tab)) {
          setQueues((q) => ({ ...q, [key]: [...(q[key] ?? []), message] }));
          return;
        }
        void deliver(tab, message);
      },
      queued: (key) => queues[key] ?? [],
      unqueue: (key, index) =>
        setQueues((q) => ({ ...q, [key]: (q[key] ?? []).filter((_, i) => i !== index) })),
      stop: async (key) => {
        const tab = tabOf(key);
        const id = tab ? sessionOfTab(tab, agentState) : null;
        if (!tab || !id) return;
        // What waited stays waiting: stopping one task is not cancelling the next.
        try {
          take(await cancelAgentTurn(id));
        } catch (reason) {
          setProblems((p) => ({ ...p, [key]: toCommandError(reason).message }));
        }
      },
      sending: (key) => sendingKeys[key] === true,
      problem: (key) => problems[key] ?? null,
      dismissProblem: (key) => setProblems((p) => ({ ...p, [key]: null })),
    };
  }, [
    tabs,
    conversations,
    agentState,
    queues,
    sendingKeys,
    problems,
    setTabs,
    setQueues,
    show,
    sessionOfTab,
    busyNow,
    deliver,
    take,
  ]);

  return <ChatContext.Provider value={api}>{children}</ChatContext.Provider>;
}
