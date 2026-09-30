import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { AccountAction, AgentSession, Environment, LedgerEvent } from "@plenipo/types";

import { AgentsContext } from "../agents/context";
import { getControlStatus, getTaskTimeline } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { tasksUsing } from "../components/aiTools/words";
import { useWorkspaceIfAny } from "../workspace/context";
import { TerminalContext, type AiToolWait, type TerminalApi } from "./context";
import {
  aiToolTitle,
  codeTabId,
  isBusyRefusal,
  openedAt,
  TERMINAL_BUTTON_ID,
  type CodeTab,
  type OwnerTab,
  type TerminalTab,
} from "./panel";
import { applyWatchEvent, applyWatchEvents, type WatchTab } from "./watch";

/** A moment after an AI tool looks free: Plenipo may still be finishing the task's step. */
const FREE_MS = 500;
/** A tab Plenipo refused tries again this often while nothing changes (or when a task does). */
const RETRY_MS = 5000;
/** Refused this many times in a row while nothing changed: it stops trying by itself. */
const MOST_REFUSALS = 3;

const NO_SESSIONS: Readonly<Record<string, AgentSession>> = {};

type Waits = Readonly<Record<string, AiToolWait>>;

/** The waits without those `drop` picks (the same object when it picks none). */
function dropWaits(all: Waits, drop: (runtimeId: string, wait: AiToolWait) => boolean): Waits {
  const kept = Object.entries(all).filter(([id, wait]) => !drop(id, wait));
  return kept.length === Object.keys(all).length ? all : Object.fromEntries(kept);
}

/**
 * The terminal panel (Phase 12, ADR-031): the owner's terminals and the workers' watch tabs,
 * and where the panel is. Watch tabs come from the Ledger: the workers connected to a server
 * when Plenipo starts, then each server event as it is committed. A tab opens by itself when a
 * worker connects to a server, and stays readable after it disconnects, until the owner closes
 * it. Watch tabs for code (Phase 18, ADR-055) open when the owner presses Watch on an agent: one
 * per agent, until closed. An AI tool's sign-in and sign-out tabs (Phase 19, ADR-058) open from
 * its card on the AI tools page; the page learns when their program ends. One pressed while the
 * tool is busy (or that Plenipo would not open for that reason) waits here until the tool is
 * free, then opens: it still does after you leave the page.
 */
export function TerminalProvider({ children }: { children: ReactNode }) {
  // Where the panel is comes from the window's layout (Phase 21, ADR-092); a terminal shown on
  // its own (some tests) keeps its own open or hidden.
  const workspace = useWorkspaceIfAny();
  const [ownOpen, setOwnOpen] = useState(false);
  const open = workspace ? workspace.shown("terminal") : ownOpen;
  const [owners, setOwners] = useState<OwnerTab[]>([]);
  const [watches, setWatches] = useState<WatchTab[]>([]);
  const [codes, setCodes] = useState<CodeTab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [unseen, setUnseen] = useState(0);
  const [signInEnded, setSignInEnded] = useState<Record<string, number>>({});
  const [aiToolWaits, setAiToolWaits] = useState<Waits>({});
  const counter = useRef(0);
  // The owner's tabs whose program ended (or never started), and the tabs open now.
  const endedTabs = useRef(new Set<string>());
  const ownersRef = useRef<OwnerTab[]>([]);
  useEffect(() => {
    ownersRef.current = owners;
  }, [owners]);
  // The latest values, for the Ledger feed's callback (it outlives each render).
  const watchesRef = useRef<WatchTab[]>([]);
  const openRef = useRef(open);
  useEffect(() => {
    openRef.current = open;
  }, [open]);
  const workspaceRef = useRef(workspace);
  useEffect(() => {
    workspaceRef.current = workspace;
  }, [workspace]);
  // The conversations (none without the agents, as in some tests): a waiting AI tool's tab opens
  // when no task is using the tool.
  const sessions = useContext(AgentsContext)?.state.sessions ?? NO_SESSIONS;
  const sessionsRef = useRef(sessions);
  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);
  // The waits, known at once (a tab's answer may come before the next render), and each AI
  // tool's name.
  const waitsRef = useRef<Waits>({});
  const aiToolLabels = useRef(new Map<string, string>());
  const changeWaits = useCallback((change: (all: Waits) => Waits) => {
    const next = change(waitsRef.current);
    if (next === waitsRef.current) return;
    waitsRef.current = next;
    setAiToolWaits(next);
  }, []);

  const show = useCallback(() => {
    const ws = workspaceRef.current;
    if (ws) ws.show("terminal");
    else setOwnOpen(true);
    openRef.current = true;
    setUnseen(0);
  }, []);
  const hide = useCallback(() => {
    const ws = workspaceRef.current;
    if (ws) {
      const place = ws.layout.panels.terminal;
      if (!place.popped && ws.shown("terminal")) ws.hideDock(place.dock);
    } else setOwnOpen(false);
    openRef.current = false;
  }, []);
  const toggle = useCallback(() => {
    const ws = workspaceRef.current;
    if (ws?.layout.panels.terminal.popped) {
      ws.show("terminal");
      return;
    }
    if (openRef.current) hide();
    else show();
  }, [hide, show]);

  /** A worker connected to a server: its tab opens (the panel too, if it was hidden). */
  const announce = useCallback(
    (id: string) => {
      if (!openRef.current) {
        show();
        setActive(id);
      } else {
        setActive((current) => current ?? id);
        setUnseen((n) => n + 1);
      }
    },
    [show],
  );

  const onEvent = useCallback(
    (e: LedgerEvent) => {
      const before = watchesRef.current;
      const after = applyWatchEvent(before, e);
      if (after === before) return;
      watchesRef.current = after;
      setWatches(after);
      const fresh = after.find((t) => !before.some((b) => b.id === t.id));
      if (fresh) announce(fresh.id);
    },
    [announce],
  );

  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    // Live events wait until the tabs of workers connected now are rebuilt, then those not
    // already read go on top: none is lost, and none is shown twice.
    let waiting: LedgerEvent[] | null = [];
    const release = (loaded: ReadonlySet<number>) => {
      const held = waiting ?? [];
      waiting = null;
      for (const e of held) if (!loaded.has(e.seq)) onEvent(e);
    };
    subscribeLedgerEvents((e) => {
      if (disposed) return;
      if (waiting) waiting.push(e);
      else onEvent(e);
    })
      .then((s) => {
        if (disposed) s();
        else stop = s;
      })
      .catch(() => undefined)
      // Listen first, then read what happened so far (after a restart or a reload).
      .finally(() => {
        getControlStatus()
          .then(async (status) => {
            const sessions = status.sessions.filter((s) => s.kind === "server");
            const timelines = await Promise.all(
              sessions.map((s) => getTaskTimeline(s.taskId).catch(() => null)),
            );
            if (disposed) return;
            const events = timelines.flatMap((t, i) =>
              t
                ? t.events.filter(
                    (e) =>
                      (e.payload as Record<string, unknown> | null)?.grantId ===
                        sessions[i]?.grantId || e.eventType === "ssh.output",
                  )
                : [],
            );
            const loaded = applyWatchEvents(watchesRef.current, events);
            watchesRef.current = loaded;
            setWatches(loaded);
            release(new Set(events.map((e) => e.seq)));
          })
          .catch(() => {
            if (!disposed) release(new Set());
          });
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [onEvent]);

  // Ctrl+` (the key left of 1) shows and hides the panel, as in Visual Studio Code.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.altKey && !e.metaKey && (e.code === "Backquote" || e.key === "`")) {
        e.preventDefault();
        // Hidden while the keyboard was in it: the keyboard goes back to the Terminal button.
        const inPanel =
          openRef.current &&
          document.activeElement?.closest('section[aria-label="Terminal"]') != null;
        toggle();
        if (inPanel) document.getElementById(TERMINAL_BUTTON_ID)?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);

  /** One of the owner's terminals: a new tab, shown. Its ID. */
  const openOwner = useCallback(
    (tab: Omit<OwnerTab, "id" | "kind" | "openedAt">): string => {
      counter.current += 1;
      const id = `owner-${counter.current}`;
      const added: OwnerTab = { ...tab, kind: "owner", id, openedAt: Date.now() };
      // Known at once (a second press before the next render finds it).
      ownersRef.current = [...ownersRef.current, added];
      setOwners((all) => [...all, added]);
      setActive(id);
      show();
      return id;
    },
    [show],
  );

  /**
   * An AI tool's sign-in or sign-out tab: opened (its ID), or the one still running that command
   * (`null`). `quiet`: opened by itself, so the keyboard stays where it is.
   */
  const openAiToolTab = useCallback(
    (runtimeId: string, label: string, action: AccountAction, quiet: boolean): string | null => {
      aiToolLabels.current.set(runtimeId, label);
      const running = ownersRef.current.find(
        (t) =>
          t.place.kind === "aiTool" &&
          t.place.runtimeId === runtimeId &&
          t.place.action === action &&
          !endedTabs.current.has(t.id),
      );
      if (running) {
        setActive(running.id);
        show();
        return null;
      }
      return openOwner({
        place: { kind: "aiTool", runtimeId, action },
        title: aiToolTitle(label, action),
        environment: null,
        ...(quiet ? { quiet } : {}),
      });
    },
    [openOwner, show],
  );

  /** Pressed on the AI tools page: the tab opens now, and the tool's wait (if any) is over. */
  const openAiTool = useCallback(
    (runtimeId: string, label: string, action: AccountAction) => {
      changeWaits((all) => dropWaits(all, (id) => id === runtimeId));
      openAiToolTab(runtimeId, label, action, false);
    },
    [changeWaits, openAiToolTab],
  );

  const waitForAiTool = useCallback(
    (runtimeId: string, label: string, action: AccountAction) => {
      aiToolLabels.current.set(runtimeId, label);
      changeWaits((all) => ({
        ...all,
        [runtimeId]: {
          action,
          label,
          refused: null,
          sessions: null,
          refusals: 0,
          opening: null,
          stopped: false,
          at: Date.now(),
        },
      }));
    },
    [changeWaits],
  );

  const cancelAiToolWait = useCallback(
    (runtimeId: string) => changeWaits((all) => dropWaits(all, (id) => id === runtimeId)),
    [changeWaits],
  );

  /** A waiting tab's AI tool is free: it opens, and the wait ends once it has opened. */
  const openWaiting = useCallback(
    (runtimeId: string) => {
      const wait = waitsRef.current[runtimeId];
      if (!wait || wait.opening !== null || wait.stopped) return;
      const opening = openAiToolTab(runtimeId, wait.label, wait.action, true);
      changeWaits((all) => {
        const now = all[runtimeId];
        if (!now) return all;
        if (opening === null) return dropWaits(all, (id) => id === runtimeId);
        return { ...all, [runtimeId]: { ...now, opening } };
      });
    },
    [changeWaits, openAiToolTab],
  );

  // A waiting tab opens a moment after no task is using its AI tool. One Plenipo refused tries
  // again soon after the conversations change, or else every few seconds, until it has been
  // refused too many times. A timer starts again only when its reason changes (not each time a
  // conversation does).
  const waitTimers = useRef(
    new Map<string, { key: string; timer: ReturnType<typeof setTimeout> }>(),
  );
  useEffect(() => {
    const timers = waitTimers.current;
    const due = new Set<string>();
    for (const [runtimeId, wait] of Object.entries(aiToolWaits)) {
      if (wait.opening !== null || wait.stopped || tasksUsing(sessions, runtimeId) > 0) continue;
      const changed = wait.sessions !== null && wait.sessions !== sessions;
      const ms = wait.refused === null || changed ? FREE_MS : RETRY_MS;
      const key = `${wait.at}:${ms}`;
      due.add(runtimeId);
      const before = timers.get(runtimeId);
      if (before?.key === key) continue;
      if (before) clearTimeout(before.timer);
      const timer = setTimeout(() => {
        timers.delete(runtimeId);
        openWaiting(runtimeId);
      }, ms);
      timers.set(runtimeId, { key, timer });
    }
    for (const [runtimeId, { timer }] of timers) {
      if (due.has(runtimeId)) continue;
      clearTimeout(timer);
      timers.delete(runtimeId);
    }
  }, [aiToolWaits, sessions, openWaiting]);
  useEffect(() => {
    const timers = waitTimers.current;
    return () => {
      for (const { timer } of timers.values()) clearTimeout(timer);
      timers.clear();
    };
  }, []);

  /** An agent's Watch tab for code: opened, or the one already open (renamed if it was). */
  const openWatch = useCallback(
    (positionId: string, title: string) => {
      const id = codeTabId(positionId);
      const now = Date.now();
      setCodes((all) =>
        all.some((c) => c.id === id)
          ? all.map((c) => (c.id === id && c.title !== title ? { ...c, title } : c))
          : [...all, { kind: "code", id, positionId, title, openedAt: now }],
      );
      setActive(id);
      show();
    },
    [show],
  );

  const tabs: TerminalTab[] = useMemo(
    () =>
      [
        ...owners,
        ...watches.map((w) => ({ kind: "watch" as const, id: w.id, watch: w })),
        ...codes,
      ].sort((a, b) => openedAt(a) - openedAt(b)),
    [owners, watches, codes],
  );

  const close = useCallback(
    (id: string) => {
      const i = tabs.findIndex((t) => t.id === id);
      const next = tabs[i + 1] ?? tabs[i - 1];
      // A waiting AI tool's tab closed before it opened: that wait is over.
      changeWaits((all) => dropWaits(all, (_, wait) => wait.opening === id));
      setOwners((all) => all.filter((t) => t.id !== id));
      setCodes((all) => all.filter((t) => t.id !== id));
      if (watchesRef.current.some((w) => w.id === id)) {
        watchesRef.current = watchesRef.current.filter((w) => w.id !== id);
        setWatches(watchesRef.current);
      }
      setActive((current) => (current === id ? (next?.id ?? null) : current));
    },
    [tabs, changeWaits],
  );

  /**
   * A terminal opened (again, after Try again: it is running, so an AI tool's tab is found and
   * shown rather than opened twice). An AI tool's tab that waited is open: the wait is over.
   */
  const tabOpened = useCallback(
    (tab: OwnerTab) => {
      endedTabs.current.delete(tab.id);
      changeWaits((all) => dropWaits(all, (_, wait) => wait.opening === tab.id));
    },
    [changeWaits],
  );

  /** A terminal's program ended: for a sign-in tab, the AI tools page shows "Checking…". */
  const tabEnded = useCallback(
    (tab: OwnerTab) => {
      endedTabs.current.add(tab.id);
      changeWaits((all) => dropWaits(all, (_, wait) => wait.opening === tab.id));
      if (tab.place.kind !== "aiTool") return;
      const { runtimeId } = tab.place;
      setSignInEnded((all) => ({ ...all, [runtimeId]: Date.now() }));
    },
    [changeWaits],
  );

  /**
   * A terminal could not open. An AI tool's tab refused because the tool is busy (a task is
   * using it, or it is being updated) closes and waits until the tool is free, then opens again;
   * refused too often while nothing changes, it stops trying by itself. Refused for another
   * reason, it stays and says why.
   */
  const tabFailed = useCallback(
    (tab: OwnerTab, message: string) => {
      endedTabs.current.add(tab.id);
      if (tab.place.kind !== "aiTool") return;
      const { runtimeId, action } = tab.place;
      if (!isBusyRefusal(message)) {
        changeWaits((all) => dropWaits(all, (_, wait) => wait.opening === tab.id));
        return;
      }
      const sessionsNow = sessionsRef.current;
      const before = waitsRef.current[runtimeId];
      const refusals =
        before && before.refused !== null && before.sessions === sessionsNow
          ? before.refusals + 1
          : 1;
      const label = before?.label ?? aiToolLabels.current.get(runtimeId) ?? runtimeId;
      changeWaits((all) => ({
        ...all,
        [runtimeId]: {
          action,
          label,
          refused: message,
          sessions: sessionsNow,
          refusals,
          opening: null,
          stopped: refusals >= MOST_REFUSALS,
          at: Date.now(),
        },
      }));
      close(tab.id);
    },
    [close, changeWaits],
  );

  const place = workspace?.layout.panels.terminal;
  const api: TerminalApi = {
    open,
    panel: {
      open,
      side: place && !place.popped && place.dock === "right" ? "right" : "bottom",
    },
    toggle,
    show,
    hide,
    tabs,
    active: active !== null && tabs.some((t) => t.id === active) ? active : (tabs[0]?.id ?? null),
    setActive: (id) => {
      setActive(id);
      setUnseen(0);
    },
    openHere: () => openOwner({ place: { kind: "thisPc" }, title: "This PC", environment: null }),
    openServer: (serverId: string, name: string, environment: Environment) =>
      openOwner({ place: { kind: "server", serverId }, title: name, environment }),
    openWatch,
    openAiTool,
    waitForAiTool,
    cancelAiToolWait,
    aiToolWaits,
    signInEnded,
    tabOpened,
    tabEnded,
    tabFailed,
    close,
    unseen,
  };
  return <TerminalContext.Provider value={api}>{children}</TerminalContext.Provider>;
}
