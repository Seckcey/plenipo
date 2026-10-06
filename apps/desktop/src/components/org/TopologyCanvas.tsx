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
 *
 * Phase 18 (ADR-053): three ways to use the pointer (Select, Move the view, Arrange); dropping a
 * tile on an empty spot places it (with its team; Alt: alone), on the trash can archives it; the
 * selected agent's line ends are handles to drag onto another agent; the toolbar, lent lines,
 * hand-offs moving along the lines (still with reduce motion), and each working tile's "where".
 */
import {
  memo,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type Ref,
} from "react";
import { Icon } from "@plenipo/ui";

import {
  DRAG_THRESHOLD,
  FLY_MS,
  MAX_ZOOM,
  ZOOM_STEP,
  clampCamera,
  easeInOutCubic,
  fitCamera,
  focusCamera,
  forInset,
  initialCamera,
  interpolate,
  panBy,
  screenToWorld,
  uncovered,
  visibleRect,
  wheelFactor,
  worldTransform,
  zoomAt,
  TOOLBAR_ROOM,
  type Camera,
  type Rect,
  type Size,
} from "../../org/camera";
import { OVERSIGHT_CHIP, OVERSIGHT_LABEL, OVERSIGHT_NOUN } from "../../org/format";
import { nodeAt, type LayoutNode, type OrgLayout, type Point } from "../../org/layout";
import type { HandoffMark, WhereLine } from "../../org/live";
import type { DropState, NodeContext } from "../../org/nodes";
import type { PointerMode } from "../../org/tour";
import { workToStop } from "../stop/whatToStop";
import { canTakeObjective } from "../../org/rules";
import { CanvasControlsContext } from "./canvasContext";
import { Glyph } from "./Glyph";
import { OrgNode } from "./OrgNode";

/** One end of a line, grabbed to rewire it (ADR-053 §7–§8). */
export type LineEnd =
  | { kind: "reports"; positionId: string }
  | { kind: "oversight"; oversightId: string; end: "overseer" | "target" };

export type DragPayload =
  | { kind: "position"; positionId: string }
  | { kind: "role"; roleId: string }
  | { kind: "line"; line: LineEnd };

/** A tile moved by hand: by how much, and whether its team comes along. */
export interface ArrangeMove {
  tileId: string;
  dx: number;
  dy: number;
  alone: boolean;
}

/** What the live view shows on the canvas. */
export interface CanvasLive {
  /** Each working tile's "where" line, when Where is on. */
  where: ReadonlyMap<string, WhereLine> | null;
  handoffs: readonly HandoffMark[];
  reducedMotion: boolean;
}

export interface CanvasHandle {
  /** Begin dragging something from outside the canvas (the hire palette). */
  startDrag: (payload: DragPayload, event: ReactPointerEvent) => void;
  fit: () => void;
}

/** Hit-test result for empty canvas. */
export const EMPTY_CANVAS = "";
/** Hit-test result for the trash can. */
export const TRASH = "trash:";
/** How far Alt + an arrow key moves the selected tile. */
const NUDGE = 20;

const WHERE_GLYPH: Record<string, string> = {
  "where-cloud": "cloud",
  "where-this-pc": "pc",
  "where-server": "server",
  "touch-folder": "folder",
  "touch-website": "web",
  "touch-screen": "screen",
};

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
  /** Leads shown only to keep a filter's matches in place (faded). */
  faded?: ReadonlySet<string> | null;
  showOversight: boolean;
  mode?: PointerMode;
  onMode?: (mode: PointerMode) => void;
  /** A tile moved by hand (`null`: the move was cancelled); `final` when it was dropped. */
  onArrange?: (move: ArrangeMove | null, final: boolean) => void;
  /** A line end chosen without dragging (Enter, or a click): offer where it can go. */
  onLineMenu?: (line: LineEnd, at: { x: number; y: number }) => void;
  /** A lent agent's badge chosen: offer Send home. */
  onLent?: (positionId: string, at: { x: number; y: number }) => void;
  /** Watch a working agent write code (`null` when there is no terminal panel). */
  onWatch?: ((positionId: string) => void) | null;
  /** Stop a tile's work now, after a question (Phase 25, item 3.3). */
  onStop?: ((positionId: string) => void) | null;
  /**
   * Chat with an agent, or watch an on-call worker's chat (ADR-200; `null` when there is no Chat
   * panel). Its button shows on the chosen tile, and on each one at work.
   */
  onChat?: ((node: LayoutNode) => void) | null;
  /**
   * The same chat in a window of its own (ADR-203): a double-click on a tile that has a chat
   * (`null` when chats cannot have windows here). Any other tile zooms in on a double-click.
   */
  onChatWindow?: ((node: LayoutNode) => void) | null;
  live?: CanvasLive | null;
  /** The toolbar (it reads the zoom and the trash can's drop state from the canvas). */
  toolbar?: ReactNode;
  onSelect: (id: string | null) => void;
  onToggle: (id: string) => void;
  /** Why `payload` cannot be dropped on node `target`; `null` when it can. */
  dropRefusal: (payload: DragPayload, target: string) => string | null;
  onDrop: (payload: DragPayload, target: string, at: { x: number; y: number }) => void;
  /**
   * The ghost's title, glyph, and words for `payload` over `over` (a node ID, TRASH, EMPTY_CANVAS,
   * or `null`). The canvas shows a refusal instead of the words, and its own away from any tile.
   */
  describeDrag: (
    payload: DragPayload,
    over: string | null,
  ) => { title: string; glyph: string; hint: string };
  /** Pixels on the right covered by a panel (the details panel floats over the canvas). */
  insetRight?: number;
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
      /** The world point pressed, with the camera as it was then. */
      start: Point;
      x: number;
      y: number;
      nodeId: string | null;
      /** A line end pressed (ADR-053 §7). */
      line: LineEnd | null;
      button: number;
      /** The pointer's mode when pressed (the space bar may be held). */
      mode: PointerMode;
    }
  | { kind: "pan"; pointerId: number; x: number; y: number }
  | {
      kind: "drag";
      pointerId: number;
      payload: DragPayload;
      x0: number;
      y0: number;
      /** The world point where the drag started: the camera may move during it. */
      start: Point;
      live: boolean;
    }
  | {
      kind: "arrange";
      pointerId: number;
      tileId: string;
      /** The world point where the tile was picked up. */
      start: Point;
      alone: boolean;
      moved: ArrangeMove | null;
      raf: number;
    }
  | { kind: "pinch"; distance: number };

interface DragView {
  payload: DragPayload;
  x: number;
  y: number;
  /** Node ID under the pointer, EMPTY_CANVAS, TRASH, or `null` outside the canvas. */
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
  faded = null,
  showOversight,
  mode = "select",
  onMode,
  onArrange,
  onLineMenu,
  onLent,
  onWatch = null,
  onStop = null,
  onChat = null,
  onChatWindow = null,
  live: liveView = null,
  toolbar,
  onSelect,
  onToggle,
  dropRefusal,
  onDrop,
  describeDrag,
  insetRight = 0,
  children,
  ref,
}: Props) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [measured, setMeasured] = useState<Size | null>(null);
  const size = measured ?? DEFAULT_SIZE;
  const [restored] = useState(readCamera);
  /** The toolbar covers a strip along the top: fitting leaves it clear. */
  const top = toolbar ? TOOLBAR_ROOM : 0;
  const [camera, setCameraState] = useState<Camera>(
    () =>
      restored ??
      forInset(
        initialCamera(layout.bounds, uncovered(DEFAULT_SIZE, insetRight, top)),
        insetRight,
        top,
      ),
  );
  const [drag, setDrag] = useState<DragView | null>(null);
  const [now, setNow] = useState(() => Date.now());
  /** The space bar is held: the pointer moves the view, whatever the mode. */
  const [spaceHeld, setSpaceHeld] = useState(false);
  const pointerMode: PointerMode = spaceHeld ? "pan" : mode;

  // Latest values for the event handlers (they are registered once).
  const current = () => ({
    camera,
    size,
    layout,
    dropRefusal,
    onDrop,
    onSelect,
    onArrange,
    mode: pointerMode,
    inset: insetRight,
    top,
  });
  const live = useRef(current());
  useLayoutEffect(() => {
    live.current = current();
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

  // Set once the viewport has been measured (the first measurement fits the organization).
  const fitted = useRef(false);

  /**
   * The viewport's size now. The resize observer reports only with the next frame, and a command
   * can come first (right after a start, or a window resize), so camera commands measure for
   * themselves; their camera then takes the place of the first fit.
   */
  const measure = useCallback((): Size => {
    const r = viewportRef.current?.getBoundingClientRect();
    const { size } = live.current;
    if (!r || r.width <= 0 || r.height <= 0) return size;
    const next = { w: r.width, h: r.height };
    if (next.w !== size.w || next.h !== size.h) {
      fitted.current = true;
      live.current.size = next;
      setMeasured(next);
    }
    return next;
  }, []);

  const fit = useCallback(() => {
    const view = measure();
    const { layout: l, inset, top: t } = live.current;
    flyTo(forInset(fitCamera(l.bounds, uncovered(view, inset, t)), inset, t));
  }, [flyTo, measure]);

  const zoomBy = useCallback(
    (factor: number) => {
      stopFly();
      const view = measure();
      const { camera: c, layout: l, inset } = live.current;
      flyTo(zoomAt(c, view, l.bounds, (view.w - inset) / 2, view.h / 2, factor));
    },
    [flyTo, measure, stopFly],
  );

  /** Fly to a node unless it is already comfortably in view (and not under the panel). */
  const reveal = useCallback(
    (node: LayoutNode, force: boolean) => {
      const view = measure();
      const { camera: c, inset, top: t } = live.current;
      const full = visibleRect(c, view);
      const seen = { ...full, y: full.y + t / c.z, w: full.w - inset / c.z, h: full.h - t / c.z };
      const margin = 24 / c.z;
      const inside =
        node.x >= seen.x + margin &&
        node.y >= seen.y + margin &&
        node.x + node.w <= seen.x + seen.w - margin &&
        node.y + node.h <= seen.y + seen.h - margin;
      if (force || !inside) {
        flyTo(forInset(focusCamera(c, uncovered(view, inset, t), node), inset, t));
      }
    },
    [flyTo, measure],
  );

  /** Zoom in on a node (double-click). */
  const zoomTo = useCallback(
    (node: LayoutNode) => {
      const { camera: c, inset } = live.current;
      const z = Math.max(1, Math.min(MAX_ZOOM, c.z * 1.5));
      flyTo(forInset({ x: node.x + node.w / 2, y: node.y + node.h / 2, z }, inset));
    },
    [flyTo],
  );

  /** The world point under a client point, with the camera as it is now. */
  const worldAt = useCallback(
    (clientX: number, clientY: number): Point => {
      const view = measure();
      const r = viewportRef.current?.getBoundingClientRect();
      const [x, y] = screenToWorld(
        live.current.camera,
        view,
        clientX - (r?.left ?? 0),
        clientY - (r?.top ?? 0),
      );
      return { x, y };
    },
    [measure],
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
  useEffect(() => {
    if (!measured || fitted.current) return;
    fitted.current = true;
    live.current.size = measured;
    const { inset, top: t } = live.current;
    setCamera(
      restored ??
        forInset(
          initialCamera(live.current.layout.bounds, uncovered(measured, inset, t)),
          inset,
          t,
        ),
    );
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

  /**
   * What is under a client point: a node ID, EMPTY_CANVAS, TRASH, or `null` where nothing can be
   * dropped (outside the canvas, under the details panel, or on the toolbar, a panel, the tour,
   * or the minimap: a tile hidden under them is not a target).
   */
  const hitTest = useCallback(
    (clientX: number, clientY: number): string | null => {
      const el = viewportRef.current;
      if (!el) return null;
      const r = el.getBoundingClientRect();
      const sized = r.width > 0 && r.height > 0;
      const view = measure();
      const { camera: c, layout: l, inset } = live.current;
      // The panel over the right-hand side is not canvas (even where the trash can is under it).
      const right = r.right - inset;
      if (sized && (clientX < r.left || clientX > right || clientY < r.top || clientY > r.bottom)) {
        return null;
      }
      const inside = (box: DOMRect | undefined) =>
        !!box &&
        box.width > 0 &&
        box.height > 0 &&
        clientX >= box.left &&
        clientX <= box.right &&
        clientY >= box.top &&
        clientY <= box.bottom;
      if (inside(el.querySelector('[data-drop="trash"]')?.getBoundingClientRect())) return TRASH;
      // What floats over the canvas (only the viewport's own children: the badges, Watch
      // buttons, and toggles in the world stay part of the canvas).
      for (const child of el.children) {
        if (child.hasAttribute("data-canvas-ui") && inside(child.getBoundingClientRect())) {
          return null;
        }
      }
      const [wx, wy] = screenToWorld(c, view, clientX - r.left, clientY - r.top);
      return nodeAt(l, wx, wy)?.id ?? EMPTY_CANVAS;
    },
    [measure],
  );

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
      // Not over the trash can or anything floating over the canvas (the toolbar is in the edge).
      if (r && r.width > 0 && over !== null && over !== TRASH) {
        if (x < r.left + EDGE) e.vx = -EDGE_SPEED;
        else if (x > r.right - live.current.inset - EDGE) e.vx = EDGE_SPEED;
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
          const how = g.mode;
          const placeable =
            node !== undefined &&
            node.kind !== "worker" &&
            (node.kind !== "position" || node.position.active);
          if (g.button === 0 && g.line && how === "select") {
            const payload: DragPayload = { kind: "line", line: g.line };
            gesture.current = {
              kind: "drag",
              pointerId: g.pointerId,
              payload,
              x0: g.x0,
              y0: g.y0,
              start: g.start,
              live: true,
            };
            updateDrag(payload, e.clientX, e.clientY);
          } else if (
            g.button === 0 &&
            node &&
            placeable &&
            live.current.onArrange &&
            (how === "arrange" || (how === "select" && node.kind !== "position"))
          ) {
            // Arrange: the tile (and its team) follows the pointer; nothing else happens.
            gesture.current = {
              kind: "arrange",
              pointerId: g.pointerId,
              tileId: node.id,
              start: g.start,
              alone: e.altKey,
              moved: null,
              raf: 0,
            };
            arrangeTo(e.clientX, e.clientY, e.altKey);
          } else if (how === "select" && g.button === 0 && node?.kind === "position" && placeable) {
            const payload: DragPayload = { kind: "position", positionId: node.id };
            gesture.current = {
              kind: "drag",
              pointerId: g.pointerId,
              payload,
              x0: g.x0,
              y0: g.y0,
              start: g.start,
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
        case "arrange": {
          if (e.pointerId !== g.pointerId) return;
          arrangeTo(e.clientX, e.clientY, e.altKey);
          return;
        }
        case "idle":
          return;
      }
    };
    /** Where an arrange gesture has the tile now (in the world), shown on the next frame. */
    const arrangeTo = (clientX: number, clientY: number, alt: boolean) => {
      const g = gesture.current;
      if (g.kind !== "arrange") return;
      // Measured in the world, so a zoom or a scroll during the gesture keeps the tile under the
      // pointer.
      const at = worldAt(clientX, clientY);
      g.alone = g.alone || alt;
      g.moved = {
        tileId: g.tileId,
        dx: at.x - g.start.x,
        dy: at.y - g.start.y,
        alone: g.alone,
      };
      if (g.raf) return;
      const show = () => {
        const now = gesture.current;
        if (now.kind !== "arrange") return;
        now.raf = 0;
        live.current.onArrange?.(now.moved, false);
      };
      if (typeof requestAnimationFrame === "function") g.raf = requestAnimationFrame(show);
      else show();
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
        if (g.nodeId === null && !g.line && g.button === 0 && e.type === "pointerup")
          live.current.onSelect(null);
        return;
      }
      gestureEnded.current = performance.now();
      if (g.kind === "arrange") {
        if (g.raf) cancelAnimationFrame(g.raf);
        const dropped = e.type === "pointerup" && g.moved;
        live.current.onArrange?.(dropped ? g.moved : null, !!dropped);
        return;
      }
      if (g.kind === "drag" && g.live && e.type === "pointerup") {
        const over = hitTest(e.clientX, e.clientY);
        if (over === EMPTY_CANVAS && g.payload.kind === "position") {
          // Select: an agent dropped on an empty spot is placed there (measured in the world:
          // the view may have scrolled or zoomed during the drag).
          const at = worldAt(e.clientX, e.clientY);
          live.current.onArrange?.(
            {
              tileId: g.payload.positionId,
              dx: at.x - g.start.x,
              dy: at.y - g.start.y,
              alone: e.altKey,
            },
            true,
          );
        } else if (
          over &&
          over !== EMPTY_CANVAS &&
          live.current.dropRefusal(g.payload, over) === null
        ) {
          live.current.onDrop(g.payload, over, { x: e.clientX, y: e.clientY });
        }
      }
      endDrag();
    };
    const onKey = (e: KeyboardEvent) => {
      const g = gesture.current;
      if (e.key === "Escape" && (g.kind === "drag" || g.kind === "arrange")) {
        e.preventDefault();
        gesture.current = { kind: "idle" };
        if (g.kind === "arrange") {
          if (g.raf) cancelAnimationFrame(g.raf);
          live.current.onArrange?.(null, false);
        }
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
  }, [endDrag, hitTest, setCamera, updateDrag, worldAt]);

  // Wheel: zoom at the cursor; shift+wheel pans sideways. (Non-passive, so the page never scrolls.)
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if ((e.target as Element | null)?.closest?.("[data-canvas-scroll]")) return;
      e.preventDefault();
      stopFly();
      const view = measure();
      const { camera: c, layout: l } = live.current;
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
  }, [measure, setCamera, stopFly]);

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
          start: worldAt(event.clientX, event.clientY),
          live: false,
        };
      },
      fit,
    }),
    [fit, worldAt],
  );

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const target = e.target as Element;
    if (target.closest("[data-canvas-ui]")) return;
    if (e.pointerType === "mouse" && e.button > 2) return;
    stopFly();
    measure();
    pointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.current.size >= 2) {
      const [a, b] = [...pointers.current.values()];
      // A second finger ends what the first was doing: a tile being arranged goes back.
      const was = gesture.current;
      if (was.kind === "arrange") {
        if (was.raf) cancelAnimationFrame(was.raf);
        live.current.onArrange?.(null, false);
      }
      gesture.current = { kind: "pinch", distance: a && b ? Math.hypot(a.x - b.x, a.y - b.y) : 0 };
      endDrag();
      return;
    }
    const node = target.closest<HTMLElement>("[data-node-id]");
    const handle = target.closest<HTMLElement>("[data-line-end]");
    gesture.current = {
      kind: "press",
      pointerId: e.pointerId,
      x0: e.clientX,
      y0: e.clientY,
      start: worldAt(e.clientX, e.clientY),
      x: e.clientX,
      y: e.clientY,
      nodeId: node?.dataset.nodeId ?? null,
      line: handle ? lineOf(handle) : null,
      button: e.button,
      // Read now: pressing a tile moves the focus, and the first move comes after that.
      mode: live.current.mode,
    };
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    // Already handled: Escape that cancelled a drag, or closed a menu, keeps the selection.
    if (e.key === "Escape" && e.nativeEvent.defaultPrevented) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select")) return;
    if (target.closest("[data-canvas-ui]") && e.key !== "Escape") return;
    // Alt + arrow keys move the selected tile (the keyboard way to arrange).
    const nudge: Record<string, [number, number]> = {
      ArrowLeft: [-NUDGE, 0],
      ArrowRight: [NUDGE, 0],
      ArrowUp: [0, -NUDGE],
      ArrowDown: [0, NUDGE],
    };
    const step = nudge[e.key];
    if (e.altKey && step && selectedId && onArrange) {
      e.preventDefault();
      onArrange({ tileId: selectedId, dx: step[0], dy: step[1], alone: false }, true);
      return;
    }
    if (!e.altKey && !e.ctrlKey && !e.metaKey && onMode) {
      const key = e.key.toLowerCase();
      const next: PointerMode | null =
        key === "v" ? "select" : key === "h" ? "pan" : key === "a" ? "arrange" : null;
      if (next) {
        e.preventDefault();
        onMode(next);
        return;
      }
    }
    // The space bar held moves the view, with a tile focused too (it does not press the tile
    // then: Enter does).
    if (e.key === " ") {
      e.preventDefault();
      if (!e.repeat) setSpaceHeld(true);
      return;
    }
    const view = measure();
    const { camera: c, layout: l } = live.current;
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

  /** A line end chosen without dragging it (not right after a gesture). */
  const lineMenu = useCallback(
    (line: LineEnd, at: { x: number; y: number }) => {
      if (performance.now() - gestureEnded.current < 250) return;
      onLineMenu?.(line, at);
    },
    [onLineMenu],
  );

  const draggedId = drag?.payload.kind === "position" ? drag.payload.positionId : null;
  const dropTarget =
    drag && drag.over && drag.over !== EMPTY_CANVAS && drag.over !== TRASH
      ? { id: drag.over, state: (drag.refusal === null ? "valid" : "invalid") as DropState }
      : null;

  return (
    <div
      ref={viewportRef}
      className={`topology topology--${pointerMode}${drag ? " is-dragging" : ""}`}
      tabIndex={0}
      role="region"
      aria-label="Organization topology"
      aria-describedby="topology-help"
      onPointerDown={onPointerDown}
      onDoubleClick={(e) => {
        const id = (e.target as Element).closest<HTMLElement>("[data-node-id]")?.dataset.nodeId;
        const node = id ? live.current.layout.byId.get(id) : undefined;
        if (!node) return;
        // An agent with a chat: its chat in a window of its own (the owner's choice, ADR-203).
        if (onChatWindow && chatTarget(node, ctx)) onChatWindow(node);
        else zoomTo(node);
      }}
      style={{ "--canvas-inset": `${insetRight}px` } as CSSProperties}
      onKeyDown={onKeyDown}
      onKeyUp={(e) => {
        if (e.key !== " ") return;
        setSpaceHeld(false);
        const target = e.target as HTMLElement;
        if (!target.closest("input, textarea, select, [data-canvas-ui]")) e.preventDefault();
      }}
      onBlur={(e) => {
        // Focus moving to a tile inside the canvas keeps the space bar held.
        if (!e.currentTarget.contains(e.relatedTarget)) setSpaceHeld(false);
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      <p id="topology-help" className="visually-hidden">
        Tab through the agents; Enter opens one. Drag an agent onto another to move it, lend it, or
        make it that team&apos;s reviewer, QA evaluator, or security auditor; onto the trash can to
        archive it; or onto an empty spot to place it. V selects, H moves the view, A arranges; Alt
        and the arrow keys move the selected tile. Plus and minus zoom, zero fits everything, arrow
        keys move the view.
      </p>
      <div className="topology__world" style={{ transform: worldTransform(camera, size) }}>
        <World
          layout={layout}
          ctx={ctx}
          selectedId={selectedId}
          matches={matches}
          faded={faded}
          showOversight={showOversight}
          draggedId={draggedId}
          dropId={dropTarget?.id ?? null}
          dropState={dropTarget?.state ?? null}
          now={now}
          live={liveView}
          onSelect={select}
          onFocusNode={onFocusNode}
          onToggle={onToggle}
          onLineMenu={lineMenu}
          onLent={onLent}
          onWatch={onWatch}
          onStop={onStop}
          onChat={onChat}
        />
      </div>
      {children}
      <CanvasControlsContext.Provider
        value={{
          zoom: {
            level: camera.z,
            zoomIn: () => zoomBy(ZOOM_STEP),
            zoomOut: () => zoomBy(1 / ZOOM_STEP),
            fit,
          },
          trash: drag?.over === TRASH ? (drag.refusal === null ? "valid" : "invalid") : null,
        }}
      >
        {toolbar}
      </CanvasControlsContext.Provider>
      {layout.nodes.length > 2 && (
        <Minimap
          layout={layout}
          camera={camera}
          size={size}
          insetRight={insetRight}
          onCenter={(x, y) => {
            stopFly();
            setCamera(forInset({ ...live.current.camera, x, y }, insetRight));
          }}
        />
      )}
      {drag && <DragGhost drag={drag} describe={describeDrag} canPlace={!!onArrange} />}
    </div>
  );
}

const World = memo(function World({
  layout,
  ctx,
  selectedId,
  matches,
  faded,
  showOversight,
  draggedId,
  dropId,
  dropState,
  now,
  live,
  onSelect,
  onFocusNode,
  onToggle,
  onLineMenu,
  onLent,
  onWatch,
  onStop,
  onChat,
}: {
  layout: OrgLayout;
  ctx: NodeContext;
  selectedId: string | null;
  matches: ReadonlySet<string> | null;
  faded: ReadonlySet<string> | null;
  showOversight: boolean;
  draggedId: string | null;
  dropId: string | null;
  dropState: DropState;
  now: number;
  live: CanvasLive | null;
  onSelect: (id: string) => void;
  onFocusNode: (id: string) => void;
  onToggle: (id: string) => void;
  onLineMenu: (line: LineEnd, at: { x: number; y: number }) => void;
  onLent: ((positionId: string, at: { x: number; y: number }) => void) | undefined;
  onWatch: ((positionId: string) => void) | null;
  onStop: ((positionId: string) => void) | null;
  onChat: ((node: LayoutNode) => void) | null;
}) {
  const oversight = showOversight ? layout.oversight : [];
  const handoffs = live?.handoffs ?? [];
  const still = live?.reducedMotion ?? false;
  const handles = handlesFor(layout, ctx, selectedId, oversight);
  const at = (e: { currentTarget: Element }) => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  };
  return (
    <>
      <svg className="topology__links" width="1" height="1" aria-hidden="true" focusable="false">
        <g className="topo-oversight">
          {oversight.map((o) => (
            <path
              key={o.id}
              d={o.d}
              data-symbol={`line-${o.role}`}
              className={`topo-oversight__path topo-oversight__path--${o.role}${
                selectedId === o.overseerId || selectedId === o.targetId ? " is-highlighted" : ""
              }`}
            />
          ))}
        </g>
        <g className="topo-lent">
          {layout.lent.map((l) => (
            <path
              key={l.id}
              d={l.d}
              data-symbol="line-lent"
              className={`topo-lent__path${
                selectedId === l.positionId || selectedId === l.toLeadId ? " is-highlighted" : ""
              }`}
            />
          ))}
        </g>
        {/* A wide, faint stroke under each link makes the glow (cheaper than a blur filter). */}
        <g className="topo-halo">
          {layout.links.map((l) => (
            <path key={l.id} d={l.d} className={`topo-halo__path${l.active ? " is-active" : ""}`} />
          ))}
        </g>
        <g className="topo-links">
          {layout.links.map((l) => (
            <path
              key={l.id}
              d={l.d}
              data-symbol={l.style === "worker" ? "line-worker" : "line-reports"}
              className={`topo-link topo-link--${l.style}${l.active ? " is-active" : ""}`}
            />
          ))}
        </g>
        <g className="topo-flow">
          {layout.links
            .filter((l) => l.active)
            .map((l) => (
              <path key={l.id} d={l.d} data-symbol="line-active" className="topo-link__flow" />
            ))}
        </g>
        <g className="topo-handoffs">
          {handoffs.map((h) => (
            <g
              key={h.id}
              className={`topo-handoff topo-handoff--${h.kind}`}
              data-symbol="line-handoff"
            >
              <path d={h.d} className="topo-handoff__path" />
              {still ? (
                <path
                  className="topo-handoff__arrow"
                  d="M -7 -6 L 7 0 L -7 6 Z"
                  transform={`translate(${h.mid.x} ${h.mid.y}) rotate(${h.angle})`}
                />
              ) : (
                <path className="topo-handoff__arrow" d="M -7 -6 L 7 0 L -7 6 Z">
                  <animateMotion dur="1.6s" repeatCount="indefinite" rotate="auto" path={h.d} />
                </path>
              )}
            </g>
          ))}
        </g>
      </svg>
      {layout.chips.map((c) => (
        <span
          key={c.id}
          className={`topo-chip topo-chip--${c.tone}`}
          data-symbol={`chip-${c.tone}`}
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
          data-symbol={`line-${o.role}`}
          style={{ left: o.x, top: o.y }}
          title={`${OVERSIGHT_LABEL[o.role]}: ${ctx.title(o.overseerId)} → ${ctx.title(o.targetId)}'s team`}
        >
          {OVERSIGHT_CHIP[o.role]}
        </span>
      ))}
      {layout.lent.map((l) => (
        <span
          key={`chip:${l.id}`}
          className="topo-chip topo-chip--lent"
          data-symbol="line-lent"
          style={{ left: l.x, top: l.y }}
          title={`${ctx.title(l.positionId)} is lent to ${ctx.title(l.toLeadId)}'s team`}
        >
          Lent
        </span>
      ))}
      {handoffs.map((h) => (
        <span
          key={`chip:${h.id}`}
          className={`topo-chip topo-chip--handoff topo-chip--${h.kind}`}
          data-symbol="line-handoff"
          style={{ left: h.mid.x, top: h.mid.y }}
          title={`${ctx.title(h.from)} → ${ctx.title(h.to)}`}
        >
          {h.kind === "asked" ? "Hand-off" : "Answer"}
        </span>
      ))}
      <div role="group" aria-label="Positions" className="topology__nodes">
        {layout.nodes.map((n) => (
          <OrgNode
            key={n.id}
            node={n}
            ctx={ctx}
            selected={n.id === selectedId}
            dimmed={(matches !== null && !matches.has(n.id)) || (faded?.has(n.id) ?? false)}
            lifted={n.id === draggedId}
            drop={n.id === dropId ? dropState : null}
            now={now}
            onSelect={onSelect}
            onFocusNode={onFocusNode}
          />
        ))}
      </div>
      {live?.where &&
        layout.nodes.map((n) => {
          const w = live.where?.get(n.id);
          if (!w) return null;
          // Keyed by its slot: where it runs and what it touches can both be a server.
          const parts = [
            { slot: "thinks", part: w.thinksIn },
            { slot: "runs", part: w.runsOn },
            { slot: "touch", part: w.touching },
          ].flatMap(({ slot, part }) => (part ? [{ slot, part }] : []));
          return (
            <div
              key={`where:${n.id}`}
              className="topo-where"
              style={{ left: n.x, top: n.y + n.h + 6, width: n.w }}
              title={parts.map((p) => p.part.words).join(" · ")}
            >
              {parts.map(({ slot, part }) => (
                <span key={slot} className="topo-where__part" data-symbol={part.symbol}>
                  <Glyph name={WHERE_GLYPH[part.symbol] ?? "where"} size={12} />
                  {part.words}
                </span>
              ))}
            </div>
          );
        })}
      {onChat &&
        layout.nodes.map((n) => {
          const target = chatTarget(n, ctx);
          if (!target || (n.id !== selectedId && !target.working)) return null;
          return (
            <button
              key={`chat:${n.id}`}
              type="button"
              data-canvas-ui
              className="topo-watch topo-chat"
              style={{ left: n.x, top: n.y + n.h }}
              aria-label={
                target.watch ? `Watch ${target.title}'s chat` : `Chat with ${target.title}`
              }
              title={
                target.watch
                  ? `Watch ${target.title} work, live`
                  : `Chat with ${target.title}: talk to it and watch it work, live`
              }
              onClick={() => onChat(n)}
            >
              <Icon name="chat" size={13} />
              Chat
            </button>
          );
        })}
      {layout.nodes.map((n) => {
        if (n.kind !== "position") return null;
        const p = n.position;
        const loan = p.loan;
        const working = p.status === "working" || p.workers.some((w) => w.state === "running");
        return (
          <span key={`extras:${n.id}`}>
            {loan && (
              <button
                type="button"
                data-canvas-ui
                data-symbol="badge-lent"
                className="topo-badge topo-badge--lent topo-lent-badge"
                style={{ left: n.x + n.w, top: n.y }}
                aria-label={`${p.title} is lent to ${loan.to}'s team${
                  loan.goingHome ? ", going home after this task" : ""
                }. Choose to send it home.`}
                title={`Lent to ${loan.to}'s team${loan.project ? ` (${loan.project})` : ""}${
                  loan.until === "objective" ? " for one objective" : " until you send it home"
                }${loan.goingHome ? " · going home after this task" : ""}`}
                onClick={(e) => onLent?.(p.id, at(e))}
              >
                <Glyph name="lent" size={12} />
                {loan.goingHome ? "Going home" : `Lent → ${loan.to}`}
              </button>
            )}
            {/* On every active tile, quieter while it isn't working (Phase 25, item 1.8). */}
            {onWatch && p.active && (
              <button
                type="button"
                data-canvas-ui
                className={working ? "topo-watch" : "topo-watch topo-watch--quiet"}
                data-tour={working ? "watch" : "watch-quiet"}
                style={{ left: n.x + n.w, top: n.y + n.h }}
                aria-label={`Watch ${p.title} write code`}
                title={`Watch ${p.title} write code`}
                onClick={() => onWatch(p.id)}
              >
                <Glyph name="watch" size={13} />
                Watch
              </button>
            )}
            {/* Stop, while it has work to stop (Phase 25, item 3.3): in the middle of the
                tile's bottom edge, between Chat (left) and Watch (right). */}
            {onStop && p.active && workToStop(p).length > 0 && (
              <button
                type="button"
                data-canvas-ui
                className="topo-stop"
                style={{ left: n.x + n.w / 2, top: n.y + n.h }}
                aria-label={`Stop ${p.title}`}
                title={`Stop ${p.title}'s work now`}
                onClick={() => onStop(p.id)}
              >
                <Glyph name="stop" size={11} />
                Stop
              </button>
            )}
          </span>
        );
      })}
      {handles.map((h) => (
        <button
          key={h.key}
          type="button"
          className={`topo-handle topo-handle--${h.kind}`}
          data-symbol="handle"
          data-line-end={JSON.stringify(h.line)}
          style={{ left: h.at.x, top: h.at.y }}
          aria-label={h.label}
          title={h.label}
          onClick={(e) => onLineMenu(h.line, at(e))}
        />
      ))}
      {layout.toggles.map((t) => (
        <button
          key={`toggle:${t.id}`}
          type="button"
          data-canvas-ui
          data-symbol="toggle"
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

interface Handle {
  key: string;
  kind: "reports" | "overseer" | "target";
  line: LineEnd;
  at: Point;
  label: string;
}

/** The selected agent's line ends (ADR-053 §7): buttons to drag, or to choose where they go. */
/**
 * Who a tile's Chat button reaches (ADR-200): a full-time agent you can talk to; an on-call
 * position, which you talk to directly too (ADR-208); or an on-call worker you can watch.
 * `null` for a tile with no chat.
 */
function chatTarget(
  n: LayoutNode,
  ctx: NodeContext,
): { title: string; watch: boolean; working: boolean } | null {
  if (n.kind === "position") {
    const p = n.position;
    const onCall = p.active && p.staffing === "onDemand";
    if (!canTakeObjective(p) && !onCall) return null;
    return {
      title: p.title,
      watch: false,
      working: p.status === "working" || p.status === "waiting",
    };
  }
  if (n.kind === "worker" && n.worker.sessionId) {
    return { title: ctx.title(n.positionId), watch: true, working: n.worker.state === "running" };
  }
  return null;
}

function handlesFor(
  layout: OrgLayout,
  ctx: NodeContext,
  selectedId: string | null,
  oversight: OrgLayout["oversight"],
): Handle[] {
  const node = selectedId ? layout.byId.get(selectedId) : undefined;
  if (!node || node.kind !== "position" || !node.position.active) return [];
  const p = node.position;
  const out: Handle[] = [];
  const link = layout.links.find((l) => l.childId === p.id);
  if (link?.from && !p.loan) {
    const lead = p.reportsTo ? ctx.title(p.reportsTo) : "you";
    out.push({
      key: `handle:reports:${p.id}`,
      kind: "reports",
      line: { kind: "reports", positionId: p.id },
      at: link.from,
      label: `Line end: ${p.title} reports to ${lead}. Drag it onto another agent, or press Enter to choose.`,
    });
  }
  for (const o of oversight) {
    if (o.overseerId !== p.id && o.targetId !== p.id) continue;
    const noun = OVERSIGHT_NOUN[o.role];
    const overseer = ctx.title(o.overseerId);
    const team = ctx.title(o.targetId);
    out.push({
      key: `handle:overseer:${o.id}`,
      kind: "overseer",
      line: { kind: "oversight", oversightId: o.id, end: "overseer" },
      at: o.start,
      label: `Line end: ${overseer}, the ${noun} for ${team}'s team. Drag it onto another on-call agent to hand the job over.`,
    });
    out.push({
      key: `handle:target:${o.id}`,
      kind: "target",
      line: { kind: "oversight", oversightId: o.id, end: "target" },
      at: o.end,
      label: `Line end: ${team}'s team, checked by ${overseer} as its ${noun}. Drag it onto another lead to check that team instead.`,
    });
  }
  return out;
}

/** The line end a handle stands for. */
function lineOf(el: HTMLElement): LineEnd | null {
  try {
    const v = JSON.parse(el.dataset.lineEnd ?? "null") as LineEnd | null;
    if (v && v.kind === "reports" && typeof v.positionId === "string") return v;
    if (
      v &&
      v.kind === "oversight" &&
      typeof v.oversightId === "string" &&
      (v.end === "overseer" || v.end === "target")
    ) {
      return v;
    }
  } catch {
    // Not a handle.
  }
  return null;
}

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
  insetRight,
  onCenter,
}: {
  layout: OrgLayout;
  camera: Camera;
  size: Size;
  insetRight: number;
  onCenter: (x: number, y: number) => void;
}) {
  // Smaller on a narrow canvas, so it never covers much of the map.
  const W = size.w - insetRight < 760 ? 136 : 184;
  const H = Math.round(W * 0.63);
  const b = layout.bounds;
  const scale = Math.min(W / Math.max(1, b.w), H / Math.max(1, b.h)) * 0.92;
  const ox = (W - b.w * scale) / 2 - b.x * scale;
  const oy = (H - b.h * scale) / 2 - b.y * scale;
  const full: Rect = visibleRect(camera, size);
  // What the owner sees: not the part under the details panel.
  const seen: Rect = { ...full, w: full.w - insetRight / camera.z };
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
    <div
      className="topology__minimap"
      data-canvas-ui
      aria-hidden="true"
      style={{ right: 14 + insetRight }}
    >
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

/** What releasing does away from any tile. */
function spareHint(payload: DragPayload, over: string | null, canPlace: boolean): string {
  if (payload.kind === "line") return "Drop the line's end on an agent · Esc cancels";
  if (payload.kind === "position" && over === EMPTY_CANVAS && canPlace) {
    return "Release to place it here · Alt: alone · Esc cancels";
  }
  return "Drop on a position · Esc cancels";
}

function DragGhost({
  drag,
  describe,
  canPlace,
}: {
  drag: DragView;
  describe: Props["describeDrag"];
  canPlace: boolean;
}) {
  const d = describe(drag.payload, drag.over);
  const hint =
    drag.refusal ??
    (drag.over === null || drag.over === EMPTY_CANVAS
      ? spareHint(drag.payload, drag.over, canPlace)
      : d.hint);
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
