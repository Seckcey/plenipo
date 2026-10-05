import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { PopOutTarget, WindowPlace } from "@plenipo/types";
import { storedKey, useElementSize } from "@plenipo/ui";

import {
  closePopOut,
  focusPopOut,
  preparePopOut,
  resetPopOuts,
  toCommandError,
} from "../api/commands";
import { subscribePopOuts } from "../api/events";
import {
  chatTarget,
  panelTarget,
  targetKey,
  WorkspaceAreaContext,
  WorkspaceContext,
  type PanelDrag,
  type PopUp,
  type WorkspaceApi,
} from "./context";
import * as L from "./layout";
import { dressWindow, whenReady } from "./popout";

function readJson(key: string): unknown {
  try {
    const raw = localStorage.getItem(storedKey(key));
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown) {
  try {
    localStorage.setItem(storedKey(key), JSON.stringify(value));
  } catch {
    // Storage unavailable: the layout lasts until the window closes.
  }
}

/** Where a panel's parts wait while it is not shown (they keep running). */
function makeParking(): HTMLElement {
  const el = document.createElement("div");
  el.className = "panel-parking";
  el.hidden = true;
  return el;
}

/** Plenipo's own keys, which a pop-out passes on to its page: Ctrl+` (the terminal) and
 * Ctrl+Shift+E (Files). */
function shortcut(e: KeyboardEvent): boolean {
  if (!e.ctrlKey || e.altKey || e.metaKey) return false;
  if (e.code === "Backquote" || e.key === "`") return !e.shiftKey;
  return e.shiftKey && e.key.toLowerCase() === "e";
}

/**
 * An organization window's panels (Phase 21, ADR-092): the layout (kept on this computer), the
 * docks' sizes, and the pop-out windows. Each panel is drawn once, into its own element, and
 * that element is moved to wherever the panel is — its dock, its pop-out window, or (while not
 * shown) a hidden place — so a panel is always the same one: its terminals keep running and
 * keep what they showed. A chat can have a window of its own too (ADR-203): the Chat keeps which
 * chats do, and draws each into its window; the windows open here, one at a time, with the
 * panels'.
 */
export function WorkspaceProvider({
  children,
  storageKey = L.LAYOUT_KEY,
}: {
  children: ReactNode;
  /** Where this window's layout is kept (each organization's window has its own). */
  storageKey?: string;
}) {
  const [layout, setLayoutState] = useState<L.Layout>(() =>
    L.startingLayout(
      readJson(storageKey),
      storageKey === L.LAYOUT_KEY ? readJson(L.OLD_TERMINAL_KEY) : null,
    ),
  );
  const layoutRef = useRef(layout);
  const setLayout = useCallback(
    (change: (l: L.Layout) => L.Layout) => {
      const next = change(layoutRef.current);
      if (next === layoutRef.current) return;
      layoutRef.current = next;
      setLayoutState(next);
      writeJson(storageKey, next);
    },
    [storageKey],
  );
  const [area, setArea] = useState<HTMLElement | null>(null);
  const room = useElementSize(area);
  const [drag, setDrag] = useState<PanelDrag | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  // Each panel's element, and the hidden place panels wait in while not shown.
  const containers = useMemo(
    () =>
      new Map(
        L.PANELS.map((p) => {
          const el = document.createElement("div");
          el.className = `panel-host panel-host--${p}`;
          return [p, el] as const;
        }),
      ),
    [],
  );
  const parking = useMemo(() => makeParking(), []);
  useEffect(() => {
    document.body.appendChild(parking);
    return () => parking.remove();
  }, [parking]);
  const containerOf = useCallback(
    (panel: L.PanelId) => containers.get(panel) ?? parking,
    [containers, parking],
  );

  // The docks' places for a panel.
  const slots = useRef(new Map<L.DockSide, HTMLElement>());
  const [slotCount, setSlotCount] = useState(0);
  const registerSlot = useCallback((dock: L.DockSide, el: HTMLElement | null) => {
    const before = slots.current.get(dock) ?? null;
    if (before === el) return;
    if (el) slots.current.set(dock, el);
    else slots.current.delete(dock);
    setSlotCount((n) => n + 1);
  }, []);

  // The pop-out windows, by what they show (`targetKey`).
  const popUpsRef = useRef(new Map<string, PopUp>());
  const [popUps, setPopUps] = useState<PopUp[]>([]);
  const syncPopUps = () => setPopUps([...popUpsRef.current.values()]);
  /** Pop-out windows open one at a time: each takes the window Plenipo was told to expect. */
  const opening = useRef<Promise<unknown>>(Promise.resolve());
  /** Who hears that a chat's window closed without the chat asking (ADR-203). */
  const chatListeners = useRef(new Set<(slot: number | null) => void>());
  const tellChats = (slot: number | null) => {
    for (const listener of [...chatListeners.current]) listener(slot);
  };

  /** The window each panel is drawn in: its pop-out's while it is popped out, else this one. */
  const hosts = useMemo(() => {
    const shownIn = (panel: L.PanelId): Window =>
      (layout.panels[panel].popped &&
        popUps.find((u) => u.target.kind === "panel" && u.target.panel === panel)?.win) ||
      window;
    return { terminal: shownIn("terminal"), files: shownIn("files"), chat: shownIn("chat") };
  }, [layout, popUps]);

  // Put each panel's element where the layout says: its pop-out, its dock (shown and showing
  // it), or the hidden place.
  useLayoutEffect(() => {
    for (const panel of L.PANELS) {
      const el = containerOf(panel);
      const place = layout.panels[panel];
      const popUp = popUpsRef.current.get(targetKey(panelTarget(panel)));
      const slot = slots.current.get(place.dock);
      let target: HTMLElement = parking;
      if (place.popped && popUp) target = popUp.body;
      else if (
        !place.popped &&
        slot &&
        L.dockShown(layout, place.dock) &&
        L.activeIn(layout, place.dock) === panel
      ) {
        target = slot;
      }
      if (el.parentNode !== target) target.appendChild(el);
    }
  }, [layout, slotCount, popUps, containerOf, parking]);

  /** A pop-out window is gone: a panel's element comes home and the panel goes back to its
   * dock; a chat goes back to the Chat panel. A window Plenipo closed on purpose is already
   * forgotten, so its notice changes nothing. */
  const onClosed = useCallback(
    (target: PopOutTarget) => {
      const key = targetKey(target);
      const popUp = popUpsRef.current.get(key);
      if (!popUp) return;
      popUpsRef.current.delete(key);
      popUp.stop();
      syncPopUps();
      if (target.kind === "chat") {
        tellChats(target.slot);
        return;
      }
      const panel = target.panel;
      parking.appendChild(containerOf(panel));
      setLayout((l) => (l.panels[panel].popped ? L.putBack(l, panel) : l));
    },
    [containerOf, parking, setLayout],
  );

  /** Open a pop-out's window (titled `title`). `false` if it could not open. */
  const openWindowNow = useCallback(
    async (target: PopOutTarget, title: string, place?: WindowPlace): Promise<boolean> => {
      const key = targetKey(target);
      if (popUpsRef.current.has(key)) {
        void focusPopOut(target).catch(() => undefined);
        return true;
      }
      try {
        await preparePopOut(target, place ?? null);
      } catch (reason) {
        setProblem(toCommandError(reason).message);
        return false;
      }
      const win = window.open("about:blank", "_blank");
      const doc = win ? await whenReady(win) : null;
      if (!win || !doc) {
        const open = win !== null && !win.closed;
        try {
          win?.close();
        } catch {
          // Already gone.
        }
        // Its page closed; Plenipo closes the window too, so none is left behind empty.
        if (open) void closePopOut(target).catch(() => undefined);
        setProblem(`Plenipo could not open a window for ${title}.`);
        return false;
      }
      const { root, stop: undress } = dressWindow(doc, `Plenipo · ${title}`);
      // Plenipo's own keys work in a pop-out too: Ctrl+` and Ctrl+Shift+E reach the page.
      const forward = (e: KeyboardEvent) => {
        if (!shortcut(e)) return;
        e.preventDefault();
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: e.key,
            code: e.code,
            ctrlKey: e.ctrlKey,
            shiftKey: e.shiftKey,
            altKey: e.altKey,
            metaKey: e.metaKey,
            cancelable: true,
          }),
        );
      };
      win.addEventListener("keydown", forward);
      const stop = () => {
        undress();
        win.removeEventListener("keydown", forward);
      };
      const header = doc.createElement("div");
      header.className = "popout__header";
      const body = doc.createElement("div");
      body.className = "popout__body";
      // A chat's window is the chat alone: its own header says whose it is and puts it back.
      if (target.kind === "chat") root.append(body);
      else root.append(header, body);
      const popUp: PopUp = { target, win, header, body, stop };
      win.addEventListener("pagehide", () => {
        if (popUpsRef.current.get(key) === popUp) onClosed(target);
      });
      popUpsRef.current.set(key, popUp);
      syncPopUps();
      setProblem(null);
      return true;
    },
    [onClosed],
  );
  const openWindow = useCallback(
    (target: PopOutTarget, title: string, place?: WindowPlace): Promise<boolean> => {
      const run = opening.current.then(() => openWindowNow(target, title, place));
      opening.current = run.catch(() => undefined);
      return run;
    },
    [openWindowNow],
  );

  const popOut = useCallback(
    (panel: L.PanelId, place?: WindowPlace) => {
      void openWindow(panelTarget(panel), L.PANEL_TITLES[panel], place).then((ok) => {
        if (ok) setLayout((l) => L.popOut(l, panel));
      });
    },
    [openWindow, setLayout],
  );

  /** Close a pop-out's window on purpose (a panel stays where the layout puts it next). */
  const closeWindow = useCallback(
    (target: PopOutTarget) => {
      const key = targetKey(target);
      const popUp = popUpsRef.current.get(key);
      if (!popUp) return;
      popUpsRef.current.delete(key);
      if (target.kind === "panel") parking.appendChild(containerOf(target.panel));
      popUp.stop();
      try {
        popUp.win.close();
      } catch {
        // Already gone.
      }
      // Plenipo closes the window too: a window its page closed may stay behind unseen.
      void closePopOut(target).catch(() => undefined);
      syncPopUps();
    },
    [containerOf, parking],
  );

  const putBack = useCallback(
    (panel: L.PanelId, dock?: L.DockSide) => {
      closeWindow(panelTarget(panel));
      setLayout((l) => L.putBack(l, panel, dock));
    },
    [closeWindow, setLayout],
  );

  // A pop-out's window closed (by the owner, or by Plenipo): Plenipo says which.
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    subscribePopOuts((notice) => {
      if (notice.kind === "closed") onClosed(notice.target);
    })
      .then((s) => {
        if (live) stop = s;
        else s();
      })
      .catch(() => undefined);
    return () => {
      live = false;
      stop?.();
    };
  }, [onClosed]);

  // After a restart (or a reload), the panels that were popped out open in their windows
  // again; one that cannot goes back to its dock. (The Chat opens its chats' windows itself.)
  const restored = useRef(false);
  useEffect(() => {
    if (restored.current) return;
    restored.current = true;
    for (const panel of L.poppedPanels(layoutRef.current)) {
      void openWindow(panelTarget(panel), L.PANEL_TITLES[panel]).then((ok) => {
        if (!ok) setLayout((l) => L.putBack(l, panel));
      });
    }
  }, [openWindow, setLayout]);

  // Leaving (the page closes): the pop-outs close with it.
  useEffect(
    () => () => {
      for (const popUp of [...popUpsRef.current.values()]) closeWindow(popUp.target);
    },
    [closeWindow],
  );

  const openChatWindow = useCallback(
    (slot: number, title: string) => openWindow(chatTarget(slot), title),
    [openWindow],
  );
  const closeChatWindow = useCallback(
    (slot: number) => closeWindow(chatTarget(slot)),
    [closeWindow],
  );
  const focusChatWindow = useCallback((slot: number) => {
    void focusPopOut(chatTarget(slot)).catch(() => undefined);
  }, []);
  const onChatWindowClosed = useCallback((listener: (slot: number | null) => void) => {
    chatListeners.current.add(listener);
    return () => {
      chatListeners.current.delete(listener);
    };
  }, []);

  const maxSize = useCallback((dock: L.DockSide) => L.dockMax(layout, dock, room), [layout, room]);
  const sizes = useMemo(
    () =>
      Object.fromEntries(
        L.DOCKS.map((d) => [d, Math.min(layout.docks[d].size, L.dockMax(layout, d, room))]),
      ) as Record<L.DockSide, number>,
    [layout, room],
  );

  const dockAt = useCallback(
    (x: number, y: number): L.DockSide | null => {
      if (!area) return null;
      const r = area.getBoundingClientRect();
      if (x < r.left || x > r.right || y < r.top || y > r.bottom) return null;
      const side = Math.min(240, r.width * 0.25);
      if (x < r.left + side) return "left";
      if (x > r.right - side) return "right";
      if (y > r.bottom - Math.min(200, r.height * 0.35)) return "bottom";
      return null;
    },
    [area],
  );

  const api: WorkspaceApi = {
    layout,
    sizes,
    maxSize,
    setSize: (dock, size) => setLayout((l) => L.resize(l, dock, size, L.dockMax(l, dock, room))),
    shown: (panel) => L.panelShown(layout, panel),
    show: (panel) => {
      if (layoutRef.current.panels[panel].popped) {
        void focusPopOut(panelTarget(panel)).catch(() => undefined);
        return;
      }
      setLayout((l) => L.show(l, panel));
    },
    toggle: (panel) => {
      if (layoutRef.current.panels[panel].popped) {
        void focusPopOut(panelTarget(panel)).catch(() => undefined);
        return;
      }
      setLayout((l) => L.toggle(l, panel));
    },
    hideDock: (dock) => setLayout((l) => L.hideDock(l, dock)),
    moveTo: (panel, dock, before) => {
      closeWindow(panelTarget(panel));
      setLayout((l) => L.moveTo(l, panel, dock, before));
    },
    popOut,
    putBack,
    reset: () => {
      for (const popUp of [...popUpsRef.current.values()]) closeWindow(popUp.target);
      void resetPopOuts().catch(() => undefined);
      setProblem(null);
      setLayout(() => L.defaultLayout());
      // Every chat's window closed with the rest: the chats are back in the Chat panel.
      tellChats(null);
    },
    popOutProblem: problem,
    windowOf: (panel) => hosts[panel],
    registerSlot,

    drag,
    setDrag,
    dockAt,
    containerOf,
    popUps,

    openChatWindow,
    closeChatWindow,
    focusChatWindow,
    onChatWindowClosed,
  };

  return (
    <WorkspaceContext.Provider value={api}>
      <WorkspaceAreaContext.Provider value={setArea}>{children}</WorkspaceAreaContext.Provider>
    </WorkspaceContext.Provider>
  );
}
