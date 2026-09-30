import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { WindowPlace } from "@plenipo/types";
import { useElementSize } from "@plenipo/ui";

import { focusPopOut, preparePopOut, resetPopOuts, toCommandError } from "../api/commands";
import { subscribePopOuts } from "../api/events";
import { WorkspaceContext, type PanelDrag, type PopUp, type WorkspaceApi } from "./context";
import * as L from "./layout";
import { dressWindow, whenReady } from "./popout";

function readJson(key: string): unknown {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

function writeJson(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
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

/**
 * An organization window's panels (Phase 21, ADR-092): the layout (kept on this computer), the
 * docks' sizes, and the pop-out windows. Each panel is drawn once, into its own element, and
 * that element is moved to wherever the panel is — its dock, its pop-out window, or (while not
 * shown) a hidden place — so a panel is always the same one: its terminals keep running and
 * keep what they showed.
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
  const parking = useMemo(makeParking, []);
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

  // The pop-out windows.
  const popUpsRef = useRef(new Map<L.PanelId, PopUp>());
  const [popUps, setPopUps] = useState<PopUp[]>([]);
  const syncPopUps = () => setPopUps([...popUpsRef.current.values()]);
  /** Panels whose window Plenipo is closing itself (their "closed" notice changes nothing). */
  const closing = useRef(new Set<L.PanelId>());

  const [hosts, setHosts] = useState<Record<L.PanelId, Window>>(() => ({
    terminal: window,
    files: window,
  }));

  // Put each panel's element where the layout says: its pop-out, its dock (shown and showing
  // it), or the hidden place.
  useLayoutEffect(() => {
    const next = { ...hosts };
    let changed = false;
    for (const panel of L.PANELS) {
      const el = containerOf(panel);
      const place = layout.panels[panel];
      const popUp = popUpsRef.current.get(panel);
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
      const win = target.ownerDocument.defaultView ?? window;
      if (next[panel] !== win) {
        next[panel] = win;
        changed = true;
      }
    }
    if (changed) setHosts(next);
  }, [layout, slotCount, popUps, containerOf, parking, hosts]);

  /** A pop-out window is gone: its panel's element comes home, and (unless Plenipo closed it
   * on purpose) the panel goes back to its dock. */
  const onClosed = useCallback(
    (panel: L.PanelId) => {
      const popUp = popUpsRef.current.get(panel);
      if (popUp) {
        popUpsRef.current.delete(panel);
        popUp.stop();
        parking.appendChild(containerOf(panel));
        syncPopUps();
      }
      if (closing.current.delete(panel)) return;
      setLayout((l) => (l.panels[panel].popped ? L.putBack(l, panel) : l));
    },
    [containerOf, parking, setLayout],
  );

  /** Open a panel's window and move the panel into it. `false` if it could not open. */
  const openWindow = useCallback(
    async (panel: L.PanelId, place?: WindowPlace): Promise<boolean> => {
      if (popUpsRef.current.has(panel)) {
        void focusPopOut(panel).catch(() => undefined);
        return true;
      }
      try {
        await preparePopOut(panel, place ?? null);
      } catch (reason) {
        setProblem(toCommandError(reason).message);
        return false;
      }
      const win = window.open("about:blank", "_blank");
      const doc = win ? await whenReady(win) : null;
      if (!win || !doc) {
        try {
          win?.close();
        } catch {
          // Already gone.
        }
        setProblem(`Plenipo could not open a window for ${L.PANEL_TITLES[panel]}.`);
        return false;
      }
      const { root, stop } = dressWindow(doc, `Plenipo · ${L.PANEL_TITLES[panel]}`);
      const header = doc.createElement("div");
      header.className = "popout__header";
      const body = doc.createElement("div");
      body.className = "popout__body";
      root.append(header, body);
      const popUp: PopUp = { panel, win, header, body, stop };
      win.addEventListener("pagehide", () => {
        if (popUpsRef.current.get(panel) === popUp) onClosed(panel);
      });
      popUpsRef.current.set(panel, popUp);
      syncPopUps();
      setProblem(null);
      return true;
    },
    [onClosed],
  );

  const popOut = useCallback(
    (panel: L.PanelId, place?: WindowPlace) => {
      void openWindow(panel, place).then((ok) => {
        if (ok) setLayout((l) => L.popOut(l, panel));
      });
    },
    [openWindow, setLayout],
  );

  /** Close a panel's window on purpose (the panel stays where the layout puts it next). */
  const closeWindow = useCallback(
    (panel: L.PanelId) => {
      const popUp = popUpsRef.current.get(panel);
      if (!popUp) return;
      closing.current.add(panel);
      popUpsRef.current.delete(panel);
      parking.appendChild(containerOf(panel));
      popUp.stop();
      try {
        popUp.win.close();
      } catch {
        // Already gone.
      }
      syncPopUps();
    },
    [containerOf, parking],
  );

  const putBack = useCallback(
    (panel: L.PanelId, dock?: L.DockSide) => {
      closeWindow(panel);
      setLayout((l) => L.putBack(l, panel, dock));
    },
    [closeWindow, setLayout],
  );

  // A pop-out's window closed (by the owner, or by Plenipo): Plenipo says which.
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    subscribePopOuts((notice) => {
      if (notice.kind === "closed") onClosed(notice.panel);
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
  // again; one that cannot goes back to its dock.
  const restored = useRef(false);
  useEffect(() => {
    if (restored.current) return;
    restored.current = true;
    for (const panel of L.poppedPanels(layoutRef.current)) {
      void openWindow(panel).then((ok) => {
        if (!ok) setLayout((l) => L.putBack(l, panel));
      });
    }
  }, [openWindow, setLayout]);

  // Leaving (the page closes): the pop-outs close with it.
  useEffect(
    () => () => {
      for (const panel of [...popUpsRef.current.keys()]) closeWindow(panel);
    },
    [closeWindow],
  );

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
        void focusPopOut(panel).catch(() => undefined);
        return;
      }
      setLayout((l) => L.show(l, panel));
    },
    toggle: (panel) => {
      if (layoutRef.current.panels[panel].popped) {
        void focusPopOut(panel).catch(() => undefined);
        return;
      }
      setLayout((l) => L.toggle(l, panel));
    },
    hideDock: (dock) => setLayout((l) => L.hideDock(l, dock)),
    moveTo: (panel, dock, before) => {
      closeWindow(panel);
      setLayout((l) => L.moveTo(l, panel, dock, before));
    },
    popOut,
    putBack,
    reset: () => {
      for (const panel of [...popUpsRef.current.keys()]) closeWindow(panel);
      void resetPopOuts().catch(() => undefined);
      setProblem(null);
      setLayout(() => L.defaultLayout());
    },
    popOutProblem: problem,
    windowOf: (panel) => hosts[panel],
    registerSlot,
    measure: setArea,
    drag,
    setDrag,
    dockAt,
    containerOf,
    popUps,
  };

  return <WorkspaceContext.Provider value={api}>{children}</WorkspaceContext.Provider>;
}
