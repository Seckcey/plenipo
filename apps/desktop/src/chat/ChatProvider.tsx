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
import { isRunning, isWaiting, liaisonInfo, type AgentState } from "../agents/store";
import {
  getAgentSession,
  giveObjective,
  resumeAgentSession,
  toCommandError,
} from "../api/commands";
import { subscribeAgentUpdates } from "../api/events";
import { useWorkspaceIfAny } from "../workspace/context";
import { ChatContext, type ChatApi, type ChatProblem } from "./context";
import {
  applyActivity,
  applyTurn,
  emptySession,
  endUnfinished,
  isBusy,
  isOver,
  mergeDetail,
  stuckTurns,
  type ChatSession,
} from "./model";
import {
  closeTab,
  keyOf,
  MAX_CHAT_WINDOWS,
  NO_TABS,
  openTab,
  popOutTab,
  positionSession,
  putBackTab,
  readChatTabs,
  setSession,
  showTab,
  slotOf,
  tabFor,
  TABS_KEY,
  type ChatTab,
  type ChatTabs,
  type ChatTarget,
} from "./tabs";

type Conversations = Record<string, ChatSession>;

function readTabs(): ChatTabs {
  try {
    const raw = localStorage.getItem(storedKey(TABS_KEY));
    const value: unknown = raw === null ? null : JSON.parse(raw);
    return readChatTabs(value);
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

/** The same chat, so showing it again changes nothing. */
function sameTab(a: ChatTab | undefined, b: ChatTab): boolean {
  return (
    a !== undefined &&
    a.key === b.key &&
    a.positionId === b.positionId &&
    a.sessionId === b.sessionId &&
    a.title === b.title
  );
}

function withoutKey<T>(all: Record<string, T>, key: string): Record<string, T> {
  if (!(key in all)) return all;
  const rest = { ...all };
  delete rest[key];
  return rest;
}

/** Stop's answer when nothing runs: the chat missed its turn's end. */
function saysNothingRuns(message: string): boolean {
  return /no turn is running/i.test(message);
}

/** What a chat says when its agent goes on working after Stop. */
function notStoppedYet(title: string): string {
  return `${title} has not stopped yet. Try Stop all, the red button on the map.`;
}

/** Every open chat: the panel's tabs, then those shown only elsewhere (the Workers page). */
function openChats(panel: ChatTabs, elsewhere: Record<string, ChatTab>): ChatTab[] {
  return [
    ...panel.tabs,
    ...Object.values(elsewhere).filter((s) => !panel.tabs.some((t) => t.key === s.key)),
  ];
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
  // Through the agents' store, so every page sees what Plenipo answered (not only the chats).
  const { cancel: cancelTurn, loadSession } = agents;
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

  // Chats shown outside the Chat panel (the Workers page), by key, while something shows them.
  const [shown, setShownState] = useState<Record<string, ChatTab>>({});
  const shownRef = useRef(shown);
  const shownCount = useRef(new Map<string, number>());
  const setShown = useCallback(
    (change: (s: Record<string, ChatTab>) => Record<string, ChatTab>) => {
      const next = change(shownRef.current);
      if (next === shownRef.current) return;
      shownRef.current = next;
      setShownState(next);
    },
    [],
  );

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
  const [problems, setProblems] = useState<Record<string, ChatProblem | null>>({});
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [stoppingKeys, setStoppingKeys] = useState<Record<string, boolean>>({});
  const stoppingRef = useRef<Record<string, boolean>>({});
  const setStopping = useCallback((key: string, on: boolean) => {
    stoppingRef.current = { ...stoppingRef.current, [key]: on };
    setStoppingKeys(stoppingRef.current);
  }, []);
  /** An agent that did not stop when asked: the turn it went on with, and what to try next. */
  const [stopNotes, setStopNotes] = useState<Record<string, { taskId: string; text: string }>>({});

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
        take(detail);
        setProblems((p) => ({ ...p, [tab.key]: null }));
        const lead = liaisonInfo(detail.session).positionId;
        if (tab.positionId && lead && lead !== tab.positionId) {
          // An on-call position takes its work from its lead (ADR-202): the lead's chat opens,
          // and this one says where the message went.
          const leadTitle = detail.session.title;
          setNotes((n) => ({
            ...n,
            [tab.key]: `Sent to ${leadTitle}, who hands it to ${tab.title} and reports back. Follow it in ${leadTitle}'s chat.`,
          }));
          setTabs((t) =>
            openTab(t, { positionId: lead, sessionId: detail.session.id, title: leadTitle }),
          );
          return true;
        }
        setNotes((n) => {
          if (!(tab.key in n)) return n;
          const rest = { ...n };
          delete rest[tab.key];
          return rest;
        });
        setTabs((t) => setSession(t, tab.key, detail.session.id));
        return true;
      } catch (reason) {
        const message = toCommandError(reason).message;
        if (saysBusy(message)) {
          // Busy after all: it waits for the agent, first in line.
          setQueues((q) => ({ ...q, [tab.key]: [text, ...(q[tab.key] ?? [])] }));
        } else {
          setProblems((p) => ({
            ...p,
            [tab.key]: { heading: "That message did not go", message },
          }));
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
    for (const tab of openChats(tabsRef.current, shownRef.current)) {
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

  // The conversations the open chats show (in the panel, or elsewhere) are fetched once, then
  // kept live.
  const wanted = useMemo(
    () =>
      openChats(tabs, shown)
        .map(
          (tab) =>
            tab.sessionId ?? (tab.positionId ? positionSession(agentState, tab.positionId) : null),
        )
        .filter((id): id is string => id !== null),
    [tabs, shown, agentState],
  );
  useEffect(() => {
    for (const id of wanted) if (!conversationsRef.current[id]) load(id);
  }, [wanted, load]);

  /**
   * Fetch a conversation again: Plenipo says no turn runs in it while the chat shows one, so the
   * turn's end was missed. A turn whose own record now says it is over without saying how shows
   * as stopped; one the record says is running or waiting stays live (the session's summary is
   * read apart from the turns, so it can be a moment behind them). What waited goes then.
   */
  const refresh = useCallback(
    async (id: string) => {
      const at = Date.now();
      let detail: AgentSessionDetail;
      try {
        detail = await loadSession(id);
      } catch {
        // Not reachable now: the chat stays as it is until Plenipo next tells it something.
        return;
      }
      const stuck = stuckTurns(detail.turns);
      setConversations((c) => {
        const merged = mergeDetail(c[id] ?? emptySession(id), detail);
        return { ...c, [id]: endUnfinished(merged, at, stuck) };
      });
      queueMicrotask(() => flushRef.current());
    },
    [setConversations, loadSession],
  );

  // A turn that ended while its end was missed (the window was asleep): fetched again once.
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
      void refresh(id);
    }
  }, [stale, refresh]);

  // Each chat with a window of its own (ADR-203) has its window open: after a restart too, and
  // one that cannot open goes back to the panel. A chat put back, or closed, has its window
  // closed. The workspace opens the windows one at a time.
  const opening = useRef(new Set<number>());
  const chatWindows = useMemo(
    () => (ws?.popUps ?? []).flatMap((u) => (u.target.kind === "chat" ? [u.target.slot] : [])),
    [ws?.popUps],
  );
  const openChatWindow = ws?.openChatWindow;
  const closeChatWindow = ws?.closeChatWindow;
  useEffect(() => {
    if (!openChatWindow || !closeChatWindow) return;
    for (const { key, slot } of tabs.popped) {
      if (chatWindows.includes(slot) || opening.current.has(slot)) continue;
      const tab = tabs.tabs.find((t) => t.key === key);
      if (!tab) continue;
      opening.current.add(slot);
      void openChatWindow(slot, tab.title).then((ok) => {
        opening.current.delete(slot);
        if (!ok) setTabs((t) => (slotOf(t, key) === slot ? putBackTab(t, key) : t));
      });
    }
    for (const slot of chatWindows) {
      if (!tabs.popped.some((p) => p.slot === slot)) closeChatWindow(slot);
    }
  }, [tabs, chatWindows, openChatWindow, closeChatWindow, setTabs]);

  // The owner closed a chat's window (or Reset layout closed them all): back to the panel.
  const onChatWindowClosed = ws?.onChatWindowClosed;
  const focusChatWindow = ws?.focusChatWindow;
  useEffect(
    () =>
      onChatWindowClosed?.((slot) =>
        setTabs((t) => {
          if (slot === null) return t.popped.length === 0 ? t : { ...t, popped: [] };
          const key = t.popped.find((p) => p.slot === slot)?.key;
          return key ? putBackTab(t, key) : t;
        }),
      ),
    [onChatWindowClosed, setTabs],
  );

  /** Show a chat elsewhere while something draws it; each one shown counts, so two can share. */
  const showElsewhere = useCallback(
    (target: ChatTarget) => {
      const tab = tabFor(target);
      if (!tab) return () => undefined;
      const key = tab.key;
      shownCount.current.set(key, (shownCount.current.get(key) ?? 0) + 1);
      setShown((s) => (sameTab(s[key], tab) ? s : { ...s, [key]: tab }));
      let done = false;
      return () => {
        if (done) return;
        done = true;
        const left = (shownCount.current.get(key) ?? 1) - 1;
        if (left > 0) {
          shownCount.current.set(key, left);
          return;
        }
        shownCount.current.delete(key);
        setShown((s) => withoutKey(s, key));
      };
    },
    [setShown],
  );

  const api = useMemo<ChatApi>(() => {
    const tabOf = (key: string) => openChats(tabs, shown).find((t) => t.key === key);
    /** Give the chat `key` of `state` a window of its own, or say why it stays in the panel. */
    const popOutIn = (state: ChatTabs, key: string) => {
      const slot = slotOf(state, key);
      if (slot !== null) {
        setTabs(() => state);
        focusChatWindow?.(slot);
        return;
      }
      const next = popOutTab(state, key);
      if (next) {
        setTabs(() => next);
        return;
      }
      // All six windows are taken: it opens in the panel instead.
      setTabs(() => showTab(state, key));
      setNotes((n) => ({
        ...n,
        [key]: `${MAX_CHAT_WINDOWS} chats have windows of their own already, so this one opened here. Put one of them back to give this one its own window.`,
      }));
      show?.("chat");
    };
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
      canPopOut: focusChatWindow !== undefined,
      popOut: (key) => popOutIn(tabsRef.current, key),
      openWindow: (target) => {
        // Nothing to open (no position, no conversation): no other chat pops out instead.
        if (keyOf(target) === null) return;
        const opened = openTab(tabsRef.current, target);
        if (opened.active) popOutIn(opened, opened.active);
      },
      putBack: (key) => {
        setTabs((t) => putBackTab(t, key));
        show?.("chat");
      },
      focusWindow: (key) => {
        const slot = slotOf(tabsRef.current, key);
        if (slot !== null) focusChatWindow?.(slot);
      },
      windowSlot: (key) => slotOf(tabs, key),
      showElsewhere,
      tab: (key) => tabOf(key) ?? null,
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
        if (!tab || !id || stoppingRef.current[key]) return;
        // The turn being stopped: a message that waited may start the next one meanwhile.
        const s = agentState.sessions[id];
        const shown =
          conversationsRef.current[id]?.turns.find((t) => !isOver(t))?.taskId ??
          s?.activeTaskId ??
          s?.waitingTaskId ??
          null;
        setStopping(key, true);
        setStopNotes((n) => withoutKey(n, key));
        setProblems((p) => ({ ...p, [key]: null }));
        // What waited stays waiting: stopping one task is not cancelling the next. Plenipo asks
        // the AI tool to stop, ends it if it does not, and answers once the turn is recorded.
        try {
          const detail = await cancelTurn(id);
          take(detail);
          const still = detail.session.activeTaskId ?? detail.session.waitingTaskId;
          if (still && (shown === null || still === shown)) {
            setStopNotes((n) => ({
              ...n,
              [key]: { taskId: still, text: notStoppedYet(tab.title) },
            }));
          }
        } catch (reason) {
          const message = toCommandError(reason).message;
          if (saysNothingRuns(message)) {
            await refresh(id);
          } else {
            setProblems((p) => ({
              ...p,
              [key]: { heading: `Could not stop ${tab.title}`, message },
            }));
          }
        } finally {
          setStopping(key, false);
        }
      },
      stopping: (key) => stoppingKeys[key] === true,
      stopNote: (key) => {
        const note = stopNotes[key];
        const tab = tabOf(key);
        const id = tab ? sessionOfTab(tab, agentState) : null;
        const turn = id ? conversations[id]?.turns.find((t) => t.taskId === note?.taskId) : null;
        // Only while the turn it went on with is still running.
        return note && turn && !isOver(turn) ? note.text : null;
      },
      sending: (key) => sendingKeys[key] === true,
      note: (key) => notes[key] ?? null,
      problem: (key) => problems[key] ?? null,
      dismissProblem: (key) => setProblems((p) => ({ ...p, [key]: null })),
    };
  }, [
    tabs,
    shown,
    conversations,
    agentState,
    queues,
    sendingKeys,
    stoppingKeys,
    stopNotes,
    problems,
    notes,
    setTabs,
    showElsewhere,
    setStopping,
    refresh,
    cancelTurn,
    setQueues,
    show,
    focusChatWindow,
    sessionOfTab,
    busyNow,
    deliver,
    take,
  ]);

  return <ChatContext.Provider value={api}>{children}</ChatContext.Provider>;
}
