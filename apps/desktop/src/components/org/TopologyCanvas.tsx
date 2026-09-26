/**
 * The organization topology canvas: a pannable, zoomable map of the organization in the style
 * of a network topology view. Nodes are real buttons (focusable, labelled) over an SVG link
 * layer; the whole world moves with one CSS transform.
 *
 * Gestures (camera model adapted from Coastline's plan canvas): wheel zooms at the cursor,
 * shift+wheel pans sideways, dragging the background pans, two fingers pinch-zoom, keys +, −,
 * and 0 zoom and fit. Dragging a position node — or a role card from the hire palette — onto
 * another node drops it there. Drags use pointer events only: HTML5 drag-and-drop is unreliable
 * inside desktop webviews.
 */
import {
  memo,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type Ref,
} from "react";

import {
  DRAG_THRESHOLD,
  FLY_MS,
  ZOOM_STEP,
  clampCamera,
  easeInOutCubic,
  fitCamera,
  focusCamera,
  initialCamera,
  interpolate,
  panBy,
  screenToWorld,
  visibleRect,
  wheelFactor,
  worldTransform,
  zoomAt,
  type Camera,
  type Rect,
  type Size,
} from "../../org/camera";
import { OVERSIGHT_CHIP, OVERSIGHT_LABEL } from "../../org/format";
import { nodeAt, type LayoutNode, type OrgLayout } from "../../org/layout";
import { Glyph } from "./Glyph";
import type { DropState, NodeContext } from "../../org/nodes";
import { OrgNode } from "./OrgNode";

export type DragPayload =
  { kind: "position"; positionId: string } | { kind: "role"; roleId: string };

export interface CanvasHandle {
  /** Begin dragging something from outside the canvas (the hire palette). */
  startDrag: (payload: DragPayload, event: ReactPointerEvent) => void;
  fit: () => void;
}

/** Hit-test result for empty canvas. */
export const EMPTY_CANVAS = "";

const DEFAULT_SIZE: Size = { w: 960, h: 640 };
const CAMERA_KEY = "plenipo.orgCamera";
const EDGE = 40;
const EDGE_SPEED = 10;

interface Props {
  layout: OrgLayout;
  ctx: NodeContext;
  selectedId: string | null;
  /** Search matches; `null` when not searching. */
  matches: ReadonlySet<string> | null;
  showOversight: boolean;
  onToggleOversight: () => void;
  onSelect: (id: string | null) => void;
  onToggle: (id: string) => void;
  /** Why `payload` cannot be dropped on node `target`; `null` when it can. */
  dropRefusal: (payload: DragPayload, target: string) => string | null;
  onDrop: (payload: DragPayload, target: string, at: { x: number; y: number }) => void;
  describeDrag: (payload: DragPayload) => { title: string; glyph: string; hint: string };
  children?: ReactNode;
  ref?: Ref<CanvasHandle>;
}

type Gesture =
  | { kind: "idle" }
  | {
      kind: "press";
      pointerId: number;
      x0: number;
      y0: number;
      x: number;
      y: number;
      nodeId: string | null;
      button: number;
    }
  | { kind: "pan"; pointerId: number; x: number; y: number }
  | { kind: "drag"; pointerId: number; payload: DragPayload; x0: number; y0: number; live: boolean }
  | { kind: "pinch"; distance: number };

interface DragView {
  payload: DragPayload;
  x: number;
  y: number;
  /** Node ID under the pointer, EMPTY_CANVAS, or `null` outside the canvas. */
  over: string | null;
  refusal: string | null;
}

function readCamera(): Camera | null {
  try {
    const raw = sessionStorage.getItem(CAMERA_KEY);
    if (!raw) return null;
    const c = JSON.parse(raw) as Partial<Camera>;
    return typeof c.x === "number" && typeof c.y === "number" && typeof c.z === "number" && c.z > 0
      ? { x: c.x, y: c.y, z: c.z }
      : null;
  } catch {
    return null;
  }
}

function writeCamera(c: Camera) {
  try {
    sessionStorage.setItem(CAMERA_KEY, JSON.stringify(c));
  } catch {
    // Storage unavailable: the view simply isn't restored.
  }
}

function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function TopologyCanvas({
  layout,
  ctx,
  selectedId,
  matches,
  showOversight,
  onToggleOversight,
  onSelect,
  onToggle,
  dropRefusal,
  onDrop,
  describeDrag,
  children,
  ref,
}: Props) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [measured, setMeasured] = useState<Size | null>(null);
  const size = measured ?? DEFAULT_SIZE;
  const [restored] = useState(readCamera);
  const [camera, setCameraState] = useState<Camera>(
    () => restored ?? initialCamera(layout.bounds, DEFAULT_SIZE),
  );
  const [drag, setDrag] = useState<DragView | null>(null);
  const [now, setNow] = useState(() => Date.now());

  // Latest values for the event handlers (they are registered once).
  const live = useRef({ camera, size, layout, dropRefusal, onDrop, onSelect });
  useLayoutEffect(() => {
    live.current = { camera, size, layout, dropRefusal, onDrop, onSelect };
  });
  const gesture = useRef<Gesture>({ kind: "idle" });
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const fly = useRef(0);
  const gestureEnded = useRef(0);
  /** The selection last brought into view. */
  const revealed = useRef<string | null>(null);
  const edge = useRef({ vx: 0, vy: 0, raf: 0, x: 0, y: 0 });

  const setCamera = useCallback((next: Camera) => {
    const { size: view, layout: l } = live.current;
    const c = clampCamera(next, view, l.bounds);
    live.current.camera = c;
    setCameraState(c);
  }, []);

  const stopFly = useCallback(() => {
    if (fly.current) cancelAnimationFrame(fly.current);
    fly.current = 0;
  }, []);

  const flyTo = useCallback(
    (target: Camera) => {
      stopFly();
      const from = live.current.camera;
      if (prefersReducedMotion() || typeof requestAnimationFrame !== "function") {
        setCamera(target);
        return;
      }
      const start = performance.now();
      const step = (t: number) => {
        const k = Math.min(1, (t - start) / FLY_MS);
        setCamera(interpolate(from, target, easeInOutCubic(k)));
        fly.current = k < 1 ? requestAnimationFrame(step) : 0;
      };
      fly.current = requestAnimationFrame(step);
    },
    [setCamera, stopFly],
  );

  const fit = useCallback(() => {
    const { size: view, layout: l } = live.current;
    flyTo(fitCamera(l.bounds, view));
  }, [flyTo]);

  const zoomBy = useCallback(
    (factor: number) => {
      stopFly();
      const { camera: c, size: view, layout: l } = live.current;
      flyTo(zoomAt(c, view, l.bounds, view.w / 2, view.h / 2, factor));
    },
    [flyTo, stopFly],
  );

  /** Fly to a node unless it is already comfortably in view. */
  const reveal = useCallback(
    (node: LayoutNode, force: boolean) => {
      const { camera: c, size: view } = live.current;
      const seen = visibleRect(c, view);
      const margin = 24 / c.z;
      const inside =
        node.x >= seen.x + margin &&
        node.y >= seen.y + margin &&
        node.x + node.w <= seen.x + seen.w - margin &&
        node.y + node.h <= seen.y + seen.h - margin;
      if (force || !inside) flyTo(focusCamera(c, view, node));
    },
    [flyTo],
  );

  // Viewport size.
  useEffect(() => {
    const el = viewportRef.current;
    if (!el || typeof ResizeObserver !== "function") return;
    const observer = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) setMeasured({ w: r.width, h: r.height });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // First real measurement: fit the organization unless a view was restored.
  const fitted = useRef(false);
  useEffect(() => {
    if (!measured || fitted.current) return;
    fitted.current = true;
    live.current.size = measured;
    setCamera(restored ?? initialCamera(live.current.layout.bounds, measured));
  }, [measured, restored, setCamera]);

  // Keep the camera valid when the organization changes shape.
  useEffect(() => {
    setCamera(live.current.camera);
  }, [layout, setCamera]);

  // An unmount (or StrictMode's rehearsal of one) cancels any flight, so reveal again after it.
  useEffect(
    () => () => {
      revealed.current = null;
    },
    [],
  );

  // Bring a new selection into view (once it is shown: it may be behind a collapsed team).
  useEffect(() => {
    if (selectedId === null) {
      revealed.current = null;
      return;
    }
    const node = layout.byId.get(selectedId);
    if (!node || revealed.current === selectedId) return;
    revealed.current = selectedId;
    if (gesture.current.kind === "idle") reveal(node, false);
  }, [selectedId, layout, reveal]);

  // Remember the view for this session (restored after navigating away and back).
  useEffect(() => {
    const t = setTimeout(() => writeCamera(camera), 250);
    return () => clearTimeout(t);
  }, [camera]);

  // Relative times on worker nodes.
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => () => stopFly(), [stopFly]);

  /** What is under a client point: a node ID, EMPTY_CANVAS, or `null` outside the canvas. */
  const hitTest = useCallback((clientX: number, clientY: number): string | null => {
    const el = viewportRef.current;
    if (!el) return null;
    const r = el.getBoundingClientRect();
    const sized = r.width > 0 && r.height > 0;
    if (sized && (clientX < r.left || clientX > r.right || clientY < r.top || clientY > r.bottom)) {
      return null;
    }
    const { camera: c, size: view, layout: l } = live.current;
    const [wx, wy] = screenToWorld(c, view, clientX - r.left, clientY - r.top);
    return nodeAt(l, wx, wy)?.id ?? EMPTY_CANVAS;
  }, []);

  const updateDrag = useCallback(
    (payload: DragPayload, x: number, y: number) => {
      const over = hitTest(x, y);
      const refusal =
        over !== null && over !== EMPTY_CANVAS ? live.current.dropRefusal(payload, over) : null;
      setDrag({ payload, x, y, over, refusal });
      // Near an edge of the canvas, keep panning so far-away nodes can be reached.
      const el = viewportRef.current;
      const r = el?.getBoundingClientRect();
      const e = edge.current;
      e.x = x;
      e.y = y;
      e.vx = 0;
      e.vy = 0;
      if (r && r.width > 0 && over !== null) {
        if (x < r.left + EDGE) e.vx = -EDGE_SPEED;
        else if (x > r.right - EDGE) e.vx = EDGE_SPEED;
        if (y < r.top + EDGE) e.vy = -EDGE_SPEED;
        else if (y > r.bottom - EDGE) e.vy = EDGE_SPEED;
      }
      if ((e.vx || e.vy) && !e.raf && typeof requestAnimationFrame === "function") {
        const tick = () => {
          const g = gesture.current;
          if (g.kind !== "drag" || (!e.vx && !e.vy)) {
            e.raf = 0;
            return;
          }
          const { camera: c, size: view, layout: l } = live.current;
          setCamera(panBy(c, view, l.bounds, -e.vx, -e.vy));
          const target = hitTest(e.x, e.y);
          setDrag((d) =>
            d
              ? {
                  ...d,
                  over: target,
                  refusal:
                    target !== null && target !== EMPTY_CANVAS
                      ? live.current.dropRefusal(d.payload, target)
                      : null,
                }
              : d,
          );
          e.raf = requestAnimationFrame(tick);
        };
        e.raf = requestAnimationFrame(tick);
      }
    },
    [hitTest, setCamera],
  );

  const endDrag = useCallback(() => {
    const e = edge.current;
    if (e.raf) cancelAnimationFrame(e.raf);
    e.raf = 0;
    e.vx = 0;
    e.vy = 0;
    setDrag(null);
  }, []);

  // Pointer and keyboard handling during gestures (registered once, on the window, so a drag
  // that started in the hire palette or leaves the canvas keeps working). Registered as soon as
  // the canvas is in the document, so no early gesture is missed.
  useLayoutEffect(() => {
    const capture = (pointerId: number) => {
      try {
        viewportRef.current?.setPointerCapture(pointerId);
      } catch {
        // Not supported (tests) or the pointer is gone: window listeners still see it.
      }
    };
    const onMove = (e: PointerEvent) => {
      if (pointers.current.has(e.pointerId)) {
        pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
      }
      const g = gesture.current;
      const { camera: c, size: view, layout: l } = live.current;
      switch (g.kind) {
        case "pinch": {
          const [a, b] = [...pointers.current.values()];
          const el = viewportRef.current;
          if (!a || !b || !el) return;
          const distance = Math.hypot(a.x - b.x, a.y - b.y);
          const r = el.getBoundingClientRect();
          if (g.distance > 0 && distance > 0) {
            setCamera(
              zoomAt(
                c,
                view,
                l.bounds,
                (a.x + b.x) / 2 - r.left,
                (a.y + b.y) / 2 - r.top,
                distance / g.distance,
              ),
            );
          }
          g.distance = distance;
          return;
        }
        case "press": {
          if (e.pointerId !== g.pointerId) return;
          if (Math.hypot(e.clientX - g.x0, e.clientY - g.y0) <= DRAG_THRESHOLD) return;
          capture(e.pointerId);
          const node = g.nodeId ? l.byId.get(g.nodeId) : undefined;
          if (g.button === 0 && node?.kind === "position" && node.position.active) {
            const payload: DragPayload = { kind: "position", positionId: node.id };
            gesture.current = {
              kind: "drag",
              pointerId: g.pointerId,
              payload,
              x0: g.x0,
              y0: g.y0,
              live: true,
            };
            updateDrag(payload, e.clientX, e.clientY);
          } else {
            gesture.current = { kind: "pan", pointerId: g.pointerId, x: e.clientX, y: e.clientY };
            setCamera(panBy(c, view, l.bounds, e.clientX - g.x, e.clientY - g.y));
          }
          return;
        }
        case "pan": {
          if (e.pointerId !== g.pointerId) return;
          setCamera(panBy(c, view, l.bounds, e.clientX - g.x, e.clientY - g.y));
          g.x = e.clientX;
          g.y = e.clientY;
          return;
        }
        case "drag": {
          if (e.pointerId !== g.pointerId) return;
          if (!g.live) {
            if (Math.hypot(e.clientX - g.x0, e.clientY - g.y0) <= DRAG_THRESHOLD) return;
            g.live = true;
          }
          updateDrag(g.payload, e.clientX, e.clientY);
          return;
        }
        case "idle":
          return;
      }
    };
    const onEnd = (e: PointerEvent) => {
      pointers.current.delete(e.pointerId);
      const g = gesture.current;
      if (g.kind === "pinch") {
        if (pointers.current.size < 2) {
          gesture.current = { kind: "idle" };
          gestureEnded.current = performance.now();
        }
        return;
      }
      if (g.kind === "idle" || g.pointerId !== e.pointerId) return;
      gesture.current = { kind: "idle" };
      if (g.kind === "press") {
        // A click on the empty canvas clears the selection.
        if (g.nodeId === null && g.button === 0 && e.type === "pointerup")
          live.current.onSelect(null);
        return;
      }
      gestureEnded.current = performance.now();
      if (g.kind === "drag" && g.live && e.type === "pointerup") {
        const over = hitTest(e.clientX, e.clientY);
        if (over && live.current.dropRefusal(g.payload, over) === null) {
          live.current.onDrop(g.payload, over, { x: e.clientX, y: e.clientY });
        }
      }
      endDrag();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && gesture.current.kind === "drag") {
        e.preventDefault();
        gesture.current = { kind: "idle" };
        endDrag();
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onEnd);
    window.addEventListener("pointercancel", onEnd);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onEnd);
      window.removeEventListener("pointercancel", onEnd);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [endDrag, hitTest, setCamera, updateDrag]);

  // Wheel: zoom at the cursor; shift+wheel pans sideways. (Non-passive, so the page never scrolls.)
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if ((e.target as Element | null)?.closest?.("[data-canvas-scroll]")) return;
      e.preventDefault();
      stopFly();
      const { camera: c, size: view, layout: l } = live.current;
      if (e.shiftKey) {
        const step = (e.deltaY || e.deltaX) * (e.deltaMode === 1 ? 16 : 1);
        setCamera(panBy(c, view, l.bounds, -step, 0));
        return;
      }
      const r = el.getBoundingClientRect();
      setCamera(
        zoomAt(
          c,
          view,
          l.bounds,
          e.clientX - r.left,
          e.clientY - r.top,
          wheelFactor(e.deltaY, e.deltaMode),
        ),
      );
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [setCamera, stopFly]);

  useImperativeHandle(
    ref,
    () => ({
      startDrag(payload, event) {
        if (event.button !== 0) return;
        gesture.current = {
          kind: "drag",
          pointerId: event.pointerId,
          payload,
          x0: event.clientX,
          y0: event.clientY,
          live: false,
        };
      },
      fit,
    }),
    [fit],
  );

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const target = e.target as Element;
    if (target.closest("[data-canvas-ui]")) return;
    if (e.pointerType === "mouse" && e.button > 2) return;
    stopFly();
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size >= 2) {
      const [a, b] = [...pointers.current.values()];
      gesture.current = { kind: "pinch", distance: a && b ? Math.hypot(a.x - b.x, a.y - b.y) : 0 };
      endDrag();
      return;
    }
    const node = target.closest<HTMLElement>("[data-node-id]");
    gesture.current = {
      kind: "press",
      pointerId: e.pointerId,
      x0: e.clientX,
      y0: e.clientY,
      x: e.clientX,
      y: e.clientY,
      nodeId: node?.dataset.nodeId ?? null,
      button: e.button,
    };
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select")) return;
    const { camera: c, size: view, layout: l } = live.current;
    const pan = (dx: number, dy: number) => {
      e.preventDefault();
      setCamera(panBy(c, view, l.bounds, dx, dy));
    };
    switch (e.key) {
      case "+":
      case "=":
        e.preventDefault();
        zoomBy(ZOOM_STEP);
        return;
      case "-":
      case "_":
        e.preventDefault();
        zoomBy(1 / ZOOM_STEP);
        return;
      case "0":
        e.preventDefault();
        fit();
        return;
      case "Escape":
        if (selectedId) {
          e.preventDefault();
          onSelect(null);
        }
        return;
    }
    if (target !== e.currentTarget) return;
    if (e.key === "ArrowLeft") pan(60, 0);
    else if (e.key === "ArrowRight") pan(-60, 0);
    else if (e.key === "ArrowUp") pan(0, 60);
    else if (e.key === "ArrowDown") pan(0, -60);
  };

  // A click right after a drag or pan is part of that gesture, not a selection.
  const select = useCallback(
    (id: string) => {
      if (performance.now() - gestureEnded.current < 250) return;
      onSelect(id);
    },
    [onSelect],
  );
  const onFocusNode = useCallback(
    (id: string) => {
      const node = live.current.layout.byId.get(id);
      if (node && gesture.current.kind === "idle") reveal(node, false);
    },
    [reveal],
  );

  const draggedId = drag?.payload.kind === "position" ? drag.payload.positionId : null;
  const dropTarget =
    drag && drag.over && drag.over !== EMPTY_CANVAS
      ? { id: drag.over, state: (drag.refusal === null ? "valid" : "invalid") as DropState }
      : null;

  return (
    <div
      ref={viewportRef}
      className={`topology${drag ? " is-dragging" : ""}`}
      tabIndex={0}
      role="region"
      aria-label="Organization topology"
      aria-describedby="topology-help"
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      <p id="topology-help" className="visually-hidden">
        Tab through the positions; Enter opens one. Drag a position onto another to change who it
        reports to or to assign it as a reviewer, QA evaluator, or security auditor. Plus and minus
        zoom, zero fits everything, arrow keys pan.
      </p>
      <div className="topology__world" style={{ transform: worldTransform(camera, size) }}>
        <World
          layout={layout}
          ctx={ctx}
          selectedId={selectedId}
          matches={matches}
          showOversight={showOversight}
          draggedId={draggedId}
          dropId={dropTarget?.id ?? null}
          dropState={dropTarget?.state ?? null}
          now={now}
          onSelect={select}
          onFocusNode={onFocusNode}
          onToggle={onToggle}
        />
      </div>
      {children}
      <div
        className="topology__controls"
        data-canvas-ui
        role="toolbar"
        aria-label="Canvas controls"
        aria-orientation="vertical"
      >
        <button
          type="button"
          className="topology__control"
          aria-label="Fit to screen"
          title="Fit to screen (0)"
          onClick={fit}
        >
          <Glyph name="fit" />
        </button>
        <button
          type="button"
          className="topology__control"
          aria-label="Zoom in"
          title="Zoom in (+)"
          onClick={() => zoomBy(ZOOM_STEP)}
        >
          <Glyph name="plus" />
        </button>
        <button
          type="button"
          className="topology__control"
          aria-label="Zoom out"
          title="Zoom out (−)"
          onClick={() => zoomBy(1 / ZOOM_STEP)}
        >
          <Glyph name="minus" />
        </button>
        <span className="topology__zoom" aria-label="Zoom level">
          {Math.round(camera.z * 100)}%
        </span>
        <button
          type="button"
          className="topology__control"
          aria-label="Show oversight links"
          aria-pressed={showOversight}
          title="Show oversight links"
          onClick={onToggleOversight}
        >
          <Glyph name="link" />
        </button>
      </div>
      <div className="topology__legend" data-canvas-ui aria-hidden="true">
        <span className="topology__legend-item topology__legend-item--tree">Reports to</span>
        <span className="topology__legend-item topology__legend-item--worker">Live worker</span>
        <span className="topology__legend-item topology__legend-item--oversight">Oversight</span>
      </div>
      <Minimap
        layout={layout}
        camera={camera}
        size={size}
        onCenter={(x, y) => {
          stopFly();
          setCamera({ ...live.current.camera, x, y });
        }}
      />
      {drag && <DragGhost drag={drag} describe={describeDrag} />}
    </div>
  );
}

const World = memo(function World({
  layout,
  ctx,
  selectedId,
  matches,
  showOversight,
  draggedId,
  dropId,
  dropState,
  now,
  onSelect,
  onFocusNode,
  onToggle,
}: {
  layout: OrgLayout;
  ctx: NodeContext;
  selectedId: string | null;
  matches: ReadonlySet<string> | null;
  showOversight: boolean;
  draggedId: string | null;
  dropId: string | null;
  dropState: DropState;
  now: number;
  onSelect: (id: string) => void;
  onFocusNode: (id: string) => void;
  onToggle: (id: string) => void;
}) {
  const oversight = showOversight ? layout.oversight : [];
  return (
    <>
      <svg className="topology__links" width="1" height="1" aria-hidden="true" focusable="false">
        <g className="topo-oversight">
          {oversight.map((o) => (
            <path
              key={o.id}
              d={o.d}
              className={`topo-oversight__path topo-oversight__path--${o.role}${
                selectedId === o.overseerId || selectedId === o.targetId ? " is-highlighted" : ""
              }`}
            />
          ))}
        </g>
        <g className="topo-links">
          {layout.links.map((l) => (
            <path
              key={l.id}
              d={l.d}
              className={`topo-link topo-link--${l.style}${l.active ? " is-active" : ""}`}
            />
          ))}
        </g>
        <g className="topo-flow">
          {layout.links
            .filter((l) => l.active)
            .map((l) => (
              <path key={l.id} d={l.d} className="topo-link__flow" />
            ))}
        </g>
      </svg>
      {layout.chips.map((c) => (
        <span
          key={c.id}
          className={`topo-chip topo-chip--${c.tone}`}
          style={{ left: c.x, top: c.y }}
          title={c.label}
        >
          {c.label}
        </span>
      ))}
      {oversight.map((o) => (
        <span
          key={`chip:${o.id}`}
          className={`topo-chip topo-chip--oversight topo-chip--${o.role}`}
          style={{ left: o.x, top: o.y }}
          title={`${OVERSIGHT_LABEL[o.role]}: ${ctx.title(o.overseerId)} → ${ctx.title(o.targetId)}'s team`}
        >
          {OVERSIGHT_CHIP[o.role]}
        </span>
      ))}
      <div role="group" aria-label="Positions" className="topology__nodes">
        {layout.nodes.map((n) => (
          <OrgNode
            key={n.id}
            node={n}
            ctx={ctx}
            selected={n.id === selectedId}
            dimmed={matches !== null && !matches.has(n.id)}
            lifted={n.id === draggedId}
            drop={n.id === dropId ? dropState : null}
            now={now}
            onSelect={onSelect}
            onFocusNode={onFocusNode}
          />
        ))}
      </div>
      {layout.toggles.map((t) => (
        <button
          key={`toggle:${t.id}`}
          type="button"
          data-canvas-ui
          className={`topo-toggle${t.collapsed ? " is-collapsed" : ""}`}
          style={{ left: t.x, top: t.y }}
          aria-expanded={!t.collapsed}
          aria-label={`${t.collapsed ? "Expand" : "Collapse"} ${nodeName(layout, ctx, t.id)} (${
            t.collapsed ? `${t.hidden} hidden` : `${t.count} below`
          })`}
          onClick={() => onToggle(t.id)}
        >
          <span aria-hidden="true">{t.collapsed ? "+" : "−"}</span>
          {t.collapsed && <span className="topo-toggle__count">{t.hidden}</span>}
        </button>
      ))}
    </>
  );
});

function nodeName(layout: OrgLayout, ctx: NodeContext, id: string): string {
  const n = layout.byId.get(id);
  if (!n) return "team";
  if (n.kind === "position") return `${n.position.title}'s team`;
  if (n.kind === "organization") return ctx.snapshot.name;
  return "team";
}

function Minimap({
  layout,
  camera,
  size,
  onCenter,
}: {
  layout: OrgLayout;
  camera: Camera;
  size: Size;
  onCenter: (x: number, y: number) => void;
}) {
  const W = 184;
  const H = 116;
  const b = layout.bounds;
  const scale = Math.min(W / Math.max(1, b.w), H / Math.max(1, b.h)) * 0.92;
  const ox = (W - b.w * scale) / 2 - b.x * scale;
  const oy = (H - b.h * scale) / 2 - b.y * scale;
  const seen: Rect = visibleRect(camera, size);
  const toWorld = (e: ReactPointerEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    return [(e.clientX - r.left - ox) / scale, (e.clientY - r.top - oy) / scale] as const;
  };
  const onPointer = (e: ReactPointerEvent<SVGSVGElement>) => {
    if (e.type === "pointermove" && e.buttons === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const [x, y] = toWorld(e);
    onCenter(x, y);
  };
  return (
    <div className="topology__minimap" data-canvas-ui aria-hidden="true">
      <svg width={W} height={H} onPointerDown={onPointer} onPointerMove={onPointer}>
        {layout.nodes.map((n) => (
          <rect
            key={n.id}
            x={ox + n.x * scale}
            y={oy + n.y * scale}
            width={Math.max(2, n.w * scale)}
            height={Math.max(2, n.h * scale)}
            rx={1.5}
            className={`topo-mini topo-mini--${n.kind}`}
            data-status={n.kind === "position" ? n.position.status : undefined}
          />
        ))}
        <rect
          className="topo-mini__view"
          x={ox + seen.x * scale}
          y={oy + seen.y * scale}
          width={seen.w * scale}
          height={seen.h * scale}
          rx={2}
        />
      </svg>
    </div>
  );
}

function DragGhost({ drag, describe }: { drag: DragView; describe: Props["describeDrag"] }) {
  const d = describe(drag.payload);
  const hint =
    drag.over === null || drag.over === EMPTY_CANVAS
      ? "Drop on a position · Esc cancels"
      : (drag.refusal ?? d.hint);
  return (
    <div
      className={`topo-ghost${drag.refusal ? " is-refused" : ""}`}
      style={{ left: drag.x, top: drag.y }}
      role="status"
      aria-live="polite"
    >
      <span className="topo-ghost__glyph">
        <Glyph name={d.glyph} size={16} />
      </span>
      <span>
        <span className="topo-ghost__title">{d.title}</span>
        <span className="topo-ghost__hint">{hint}</span>
      </span>
    </div>
  );
}
