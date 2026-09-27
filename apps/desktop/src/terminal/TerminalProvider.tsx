import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Environment, LedgerEvent } from "@plenipo/types";
import { useElementSize, useStoredState } from "@plenipo/ui";

import { getControlStatus, getTaskTimeline } from "../api/commands";
import { subscribeLedgerEvents } from "../api/events";
import { TerminalContext, type TerminalApi } from "./context";
import {
  DEFAULT_PANEL,
  isPanelState,
  PANEL_KEY,
  PANEL_MIN,
  panelMax,
  TERMINAL_BUTTON_ID,
  type OwnerTab,
  type PanelState,
  type TerminalTab,
} from "./panel";
import { applyWatchEvent, applyWatchEvents, type WatchTab } from "./watch";

/**
 * The terminal panel (Phase 12, ADR-031): the owner's terminals and the workers' watch tabs,
 * and where the panel is. Watch tabs come from the Ledger: the workers connected to a server
 * when Plenipo starts, then each server event as it is committed. A tab opens by itself when a
 * worker connects to a server, and stays readable after it disconnects, until the owner closes
 * it.
 */
export function TerminalProvider({ children }: { children: ReactNode }) {
  const [panel, setPanel] = useStoredState<PanelState>(PANEL_KEY, DEFAULT_PANEL, isPanelState);
  const [area, setArea] = useState<HTMLElement | null>(null);
  const room = useElementSize(area);
  const maxSize = panelMax(panel.side, room.width, room.height);
  const size = Math.min(panel.size, maxSize);
  const [owners, setOwners] = useState<OwnerTab[]>([]);
  const [watches, setWatches] = useState<WatchTab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [unseen, setUnseen] = useState(0);
  const counter = useRef(0);
  // The latest values, for the Ledger feed's callback (it outlives each render).
  const watchesRef = useRef<WatchTab[]>([]);
  const panelRef = useRef(panel);
  useEffect(() => {
    panelRef.current = panel;
  }, [panel]);

  const show = useCallback(() => {
    if (!panelRef.current.open) setPanel({ ...panelRef.current, open: true });
    setUnseen(0);
  }, [setPanel]);
  const hide = useCallback(() => {
    if (panelRef.current.open) setPanel({ ...panelRef.current, open: false });
  }, [setPanel]);
  const toggle = useCallback(() => {
    if (panelRef.current.open) hide();
    else show();
  }, [hide, show]);

  /** A worker connected to a server: its tab opens (the panel too, if it was hidden). */
  const announce = useCallback(
    (id: string) => {
      if (!panelRef.current.open) {
        setPanel({ ...panelRef.current, open: true });
        setActive(id);
      } else {
        setActive((current) => current ?? id);
        setUnseen((n) => n + 1);
      }
    },
    [setPanel],
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
          panelRef.current.open &&
          document.activeElement?.closest('section[aria-label="Terminal"]') != null;
        toggle();
        if (inPanel) document.getElementById(TERMINAL_BUTTON_ID)?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);

  const openOwner = useCallback(
    (tab: Omit<OwnerTab, "id" | "kind" | "openedAt">) => {
      counter.current += 1;
      const id = `owner-${counter.current}`;
      setOwners((all) => [...all, { ...tab, kind: "owner", id, openedAt: Date.now() }]);
      setActive(id);
      show();
    },
    [show],
  );

  const tabs: TerminalTab[] = useMemo(
    () =>
      [...owners, ...watches.map((w) => ({ kind: "watch" as const, id: w.id, watch: w }))].sort(
        (a, b) =>
          (a.kind === "owner" ? a.openedAt : a.watch.openedAt) -
          (b.kind === "owner" ? b.openedAt : b.watch.openedAt),
      ),
    [owners, watches],
  );

  const close = useCallback(
    (id: string) => {
      const i = tabs.findIndex((t) => t.id === id);
      const next = tabs[i + 1] ?? tabs[i - 1];
      setOwners((all) => all.filter((t) => t.id !== id));
      if (watchesRef.current.some((w) => w.id === id)) {
        watchesRef.current = watchesRef.current.filter((w) => w.id !== id);
        setWatches(watchesRef.current);
      }
      setActive((current) => (current === id ? (next?.id ?? null) : current));
    },
    [tabs],
  );

  const api: TerminalApi = {
    panel,
    size,
    maxSize,
    setSize: (next) =>
      setPanel({ ...panel, size: Math.max(PANEL_MIN, Math.min(maxSize, Math.round(next))) }),
    setSide: (side) =>
      setPanel({
        ...panel,
        side,
        size: side === panel.side ? panel.size : side === "right" ? 420 : 260,
      }),
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
    close,
    unseen,
    measure: setArea,
  };
  return <TerminalContext.Provider value={api}>{children}</TerminalContext.Provider>;
}
