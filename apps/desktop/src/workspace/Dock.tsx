import {
  useCallback,
  useRef,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { createPortal } from "react-dom";
import { Button, IconButton, MenuButton, ResizeHandle, cx, type MenuItem } from "@plenipo/ui";

import { PanelWindowContext, useWorkspace, type WorkspaceApi } from "./context";
import {
  activeIn,
  DOCK_MIN,
  DOCK_WORDS,
  DOCKS,
  dockShown,
  PANEL_TITLES,
  PANELS,
  panelsIn,
  type DockSide,
  type PanelId,
} from "./layout";
import { dropPlace, insideWindow, outsideWindow } from "./popout";

/** Each panel's button in the top bar (the keyboard goes back to it when its dock hides). */
const PANEL_BUTTONS: Record<PanelId, string> = {
  terminal: "terminal-button",
  files: "files-button",
  chat: "chat-button",
};

/** How far the pointer moves before a press on a tab becomes a drag. */
const DRAG_THRESHOLD = 5;
const RESET = "reset-layout";
const POP_OUT = "pop-out";
const PUT_BACK = "put-back";
const MOVE = "move:";

/** The panel menu's choices for one panel. */
function menuItems(ws: WorkspaceApi, panel: PanelId): MenuItem[] {
  const place = ws.layout.panels[panel];
  const moves: MenuItem[] = DOCKS.filter((d) => place.popped || d !== place.dock).map((d) => ({
    id: `${MOVE}${d}`,
    label: `Move to ${DOCK_WORDS[d]}`,
    icon: d === "bottom" ? "panelClose" : "panelOpen",
  }));
  return [
    ...moves,
    place.popped
      ? { id: PUT_BACK, label: "Put back", icon: "panelClose" }
      : { id: POP_OUT, label: "Pop out", hint: "Its own window", icon: "external" },
    { id: RESET, label: "Reset layout", icon: "refresh" },
  ];
}

function pickFromMenu(ws: WorkspaceApi, panel: PanelId, id: string) {
  if (id === RESET) ws.reset();
  else if (id === POP_OUT) ws.popOut(panel);
  else if (id === PUT_BACK) ws.putBack(panel);
  else if (id.startsWith(MOVE)) ws.moveTo(panel, id.slice(MOVE.length) as DockSide);
}

/** The menu of one panel: move it, pop it out or put it back, or reset the layout. */
export function PanelMenu({ panel }: { panel: PanelId }) {
  const ws = useWorkspace();
  return (
    <MenuButton
      label={`${PANEL_TITLES[panel]} panel`}
      icon="more"
      variant="quiet"
      align="end"
      items={menuItems(ws, panel)}
      onSelect={(id) => pickFromMenu(ws, panel, id)}
    />
  );
}

/**
 * Pointer handlers that let a panel's tab be dragged (ADR-092 §5–§7, pointer events and the
 * window's edges, ADR-009 §12): onto another dock to move it there, past the window's edge to
 * pop it out there, or (from a pop-out) back into Plenipo's window. A press that does not move is
 * a click.
 */
function useTabDrag(
  panel: PanelId,
  onClick: () => void,
  size: () => { width: number; height: number },
) {
  const ws = useWorkspace();
  const start = useRef<{ x: number; y: number; dragging: boolean } | null>(null);
  const cancel = (el: Element, pointerId: number) => {
    start.current = null;
    ws.setDrag(null);
    if (el.hasPointerCapture?.(pointerId)) el.releasePointerCapture(pointerId);
  };
  return {
    onPointerDown: (e: ReactPointerEvent<HTMLElement>) => {
      if (e.button !== 0) return;
      e.currentTarget.setPointerCapture?.(e.pointerId);
      start.current = { x: e.clientX, y: e.clientY, dragging: false };
    },
    onPointerMove: (e: ReactPointerEvent<HTMLElement>) => {
      const s = start.current;
      if (!s) return;
      if (!s.dragging && Math.hypot(e.clientX - s.x, e.clientY - s.y) < DRAG_THRESHOLD) return;
      s.dragging = true;
      const win = e.currentTarget.ownerDocument.defaultView ?? window;
      const outside = outsideWindow(win, e.clientX, e.clientY);
      const popped = ws.layout.panels[panel].popped;
      ws.setDrag({
        panel,
        outside: outside && !popped,
        over: popped || outside ? null : ws.dockAt(e.clientX, e.clientY),
      });
    },
    onPointerUp: (e: ReactPointerEvent<HTMLElement>) => {
      const s = start.current;
      const el = e.currentTarget;
      cancel(el, e.pointerId);
      if (!s) return;
      if (!s.dragging) {
        onClick();
        return;
      }
      const win = el.ownerDocument.defaultView ?? window;
      const place = ws.layout.panels[panel];
      if (place.popped) {
        // Dragged from its own window back into Plenipo's: it goes back where it was let go.
        if (win !== window && insideWindow(window, e.screenX, e.screenY)) {
          const frameX = (window.outerWidth - window.innerWidth) / 2;
          const frameY = window.outerHeight - window.innerHeight - frameX;
          const x = e.screenX - window.screenX - frameX;
          const y = e.screenY - window.screenY - frameY;
          ws.putBack(panel, ws.dockAt(x, y) ?? place.dock);
        }
        return;
      }
      if (outsideWindow(win, e.clientX, e.clientY)) {
        ws.popOut(panel, dropPlace(e.screenX, e.screenY, size()));
        return;
      }
      const over = ws.dockAt(e.clientX, e.clientY);
      if (over && over !== place.dock) ws.moveTo(panel, over);
    },
    onPointerCancel: (e: ReactPointerEvent<HTMLElement>) => cancel(e.currentTarget, e.pointerId),
    onKeyDown: (e: KeyboardEvent<HTMLElement>) => {
      if (e.key === "Escape" && start.current) {
        e.preventDefault();
        cancel(e.currentTarget, 0);
      }
    },
  };
}

/** One panel's tab in a dock (or a pop-out's title bar). */
function PanelTab({
  panel,
  selected,
  onSelect,
  size,
}: {
  panel: PanelId;
  selected: boolean;
  onSelect: () => void;
  size: () => { width: number; height: number };
}) {
  const handlers = useTabDrag(panel, onSelect, size);
  return (
    <button
      type="button"
      role="tab"
      id={`panel-tab-${panel}`}
      aria-selected={selected}
      tabIndex={selected ? 0 : -1}
      className={cx("panel-tab", selected && "panel-tab--selected")}
      title={`${PANEL_TITLES[panel]}: drag to move it, or past the window's edge to pop it out`}
      {...handlers}
      onClick={(e) => {
        // A click from the keyboard (Enter or Space); a pointer's click is handled on release.
        if (e.detail === 0) onSelect();
      }}
    >
      {PANEL_TITLES[panel]}
    </button>
  );
}

/**
 * One dock (ADR-092 §2): its panels' tabs, the shown panel's menu, a Hide button, and the edge to
 * resize it. The panel itself is drawn elsewhere and moved into the dock's place.
 */
export function Dock({ side }: { side: DockSide }) {
  const ws = useWorkspace();
  const here = panelsIn(ws.layout, side);
  const active = activeIn(ws.layout, side);
  const shown = dockShown(ws.layout, side);
  const body = useRef<HTMLDivElement | null>(null);
  const { registerSlot } = ws;
  // The dock's place for its panel, given to the workspace once (not on every draw).
  const slot = useCallback(
    (el: HTMLDivElement | null) => {
      body.current = el;
      registerSlot(side, el);
    },
    [registerSlot, side],
  );
  if (here.length === 0 || active === null) return null;
  const size = () => ({
    width: body.current?.clientWidth ?? 0,
    height: body.current?.clientHeight ?? 0,
  });
  const onTabsKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const at = here.indexOf(active);
    const next =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? here[(at + 1) % here.length]
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? here[(at - 1 + here.length) % here.length]
          : undefined;
    if (!next) return;
    e.preventDefault();
    ws.show(next);
    e.currentTarget.ownerDocument.getElementById(`panel-tab-${next}`)?.focus();
  };
  const edge = side === "bottom" ? "top" : side === "left" ? "right" : "left";
  return (
    <section
      className={cx("dock", `dock--${side}`)}
      hidden={!shown}
      aria-label={`Panels on ${DOCK_WORDS[side]}`}
      data-dock={side}
      style={side === "bottom" ? { height: ws.sizes[side] } : { width: ws.sizes[side] }}
    >
      <ResizeHandle
        label={`Resize the panels on ${DOCK_WORDS[side]}`}
        value={ws.sizes[side]}
        min={DOCK_MIN}
        max={ws.maxSize(side)}
        edge={edge}
        onChange={(n) => ws.setSize(side, n)}
      />
      <div className="dock__main">
        <div className="dock__bar">
          <div className="dock__tabs" role="tablist" aria-label="Panels" onKeyDown={onTabsKey}>
            {here.map((p) => (
              <PanelTab
                key={p}
                panel={p}
                selected={p === active}
                onSelect={() => ws.show(p)}
                size={size}
              />
            ))}
          </div>
          <div className="dock__actions">
            <PanelMenu panel={active} />
            <IconButton
              icon="close"
              label={`Hide ${PANEL_TITLES[active]}`}
              onClick={() => {
                ws.hideDock(side);
                // The keyboard goes back to the panel's button in the top bar.
                document.getElementById(PANEL_BUTTONS[active])?.focus();
              }}
            />
          </div>
        </div>
        <div
          className="dock__body"
          role="tabpanel"
          aria-labelledby={`panel-tab-${active}`}
          ref={slot}
        />
      </div>
    </section>
  );
}

/** While a panel's tab is dragged: where it can go, marked, and what letting go does. */
export function DropMarks() {
  const ws = useWorkspace();
  const drag = ws.drag;
  if (!drag) return null;
  return (
    <div className="drop-marks" aria-hidden="true">
      {DOCKS.map((d) => (
        <div
          key={d}
          className={cx(
            "drop-marks__zone",
            `drop-marks__zone--${d}`,
            drag.over === d && "drop-marks__zone--over",
          )}
        >
          <span>{`${PANEL_TITLES[drag.panel]} to ${DOCK_WORDS[d]}`}</span>
        </div>
      ))}
      {drag.outside && (
        <div className="drop-marks__out">{`Let go to open ${PANEL_TITLES[drag.panel]} in its own window`}</div>
      )}
    </div>
  );
}

/** A pop-out window's title bar: its panel's tab (drag it back into Plenipo) and Put back. */
function PopOutHeader({ panel }: { panel: PanelId }) {
  const ws = useWorkspace();
  const size = () => ({ width: 0, height: 0 });
  return (
    <div className="popout-bar">
      <div className="dock__tabs" role="tablist" aria-label="Panel">
        <PanelTab panel={panel} selected onSelect={() => undefined} size={size} />
      </div>
      <div className="dock__actions">
        <PanelMenu panel={panel} />
        <Button size="sm" variant="quiet" icon="panelClose" onClick={() => ws.putBack(panel)}>
          Put back
        </Button>
      </div>
    </div>
  );
}

/**
 * Each panel drawn once, into its own element (moved to its dock or window by the workspace),
 * with the window it is in; and each pop-out window's title bar.
 */
export function PanelPortals({ render }: { render: (panel: PanelId) => React.ReactNode }) {
  const ws = useWorkspace();
  return (
    <>
      {PANELS.map((p) =>
        createPortal(
          <PanelWindowContext.Provider value={ws.windowOf(p)}>
            {render(p)}
          </PanelWindowContext.Provider>,
          ws.containerOf(p),
          `panel-${p}`,
        ),
      )}
      {ws.popUps.map((u) => {
        // A chat's own window (ADR-203) has no panel bar: the chat's header is its bar.
        const t = u.target;
        if (t.kind !== "panel") return null;
        return createPortal(
          <PanelWindowContext.Provider value={u.win}>
            <PopOutHeader panel={t.panel} />
          </PanelWindowContext.Provider>,
          u.header,
          `popout-${t.panel}`,
        );
      })}
    </>
  );
}
