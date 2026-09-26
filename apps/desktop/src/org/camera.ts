/**
 * The Organization canvas camera: pure math, no DOM.
 *
 * Adapted from Coastline's plan canvas engine (`plan-canvas-engine.ts`): a center-anchored camera
 * (world coordinates of the viewport center plus zoom in screen pixels per world unit), cursor-
 * anchored wheel zoom, clamping that keeps the content reachable, fit-to-content, and eased
 * fly-to with logarithmic zoom interpolation.
 */

export interface Camera {
  /** World x at the viewport center. */
  x: number;
  /** World y at the viewport center. */
  y: number;
  /** Zoom: screen pixels per world unit. */
  z: number;
}

export interface Size {
  w: number;
  h: number;
}

/** A world-space rectangle. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export const MIN_ZOOM = 0.2;
export const MAX_ZOOM = 2.5;
/** Breathing room around "fit everything". */
export const FIT_INSET = 0.9;
/** Fly-to animation duration. */
export const FLY_MS = 480;
export const WHEEL_ZOOM_SPEED = 0.0022;
/** Zoom step of the + and − controls. */
export const ZOOM_STEP = 1.25;
/** Pointer travel (screen px) that turns a press into a drag. */
export const DRAG_THRESHOLD = 5;

export function worldToScreen(
  camera: Camera,
  view: Size,
  wx: number,
  wy: number,
): [number, number] {
  return [(wx - camera.x) * camera.z + view.w / 2, (wy - camera.y) * camera.z + view.h / 2];
}

export function screenToWorld(
  camera: Camera,
  view: Size,
  sx: number,
  sy: number,
): [number, number] {
  return [(sx - view.w / 2) / camera.z + camera.x, (sy - view.h / 2) / camera.z + camera.y];
}

/** The CSS transform that draws world coordinates at their screen positions. */
export function worldTransform(camera: Camera, view: Size): string {
  const tx = view.w / 2 - camera.x * camera.z;
  const ty = view.h / 2 - camera.y * camera.z;
  return `translate(${round(tx)}px, ${round(ty)}px) scale(${round(camera.z, 4)})`;
}

/** The world rectangle the viewport shows. */
export function visibleRect(camera: Camera, view: Size): Rect {
  const [x, y] = screenToWorld(camera, view, 0, 0);
  return { x, y, w: view.w / camera.z, h: view.h / camera.z };
}

/**
 * Keep the zoom within limits and the content reachable: the camera center may wander a little
 * past the content (so an edge node can be brought to the middle of the screen), never so far
 * that everything scrolls out of view.
 */
export function clampCamera(camera: Camera, view: Size, content: Rect): Camera {
  const floor = Math.min(MIN_ZOOM, fitZoom(content, view) * 0.5);
  const z = Math.min(MAX_ZOOM, Math.max(floor, camera.z));
  const slackX = content.w * 0.5 + view.w / z / 2;
  const slackY = content.h * 0.5 + view.h / z / 2;
  const cx = content.x + content.w / 2;
  const cy = content.y + content.h / 2;
  return {
    x: Math.min(cx + slackX, Math.max(cx - slackX, camera.x)),
    y: Math.min(cy + slackY, Math.max(cy - slackY, camera.y)),
    z,
  };
}

function fitZoom(content: Rect, view: Size): number {
  return Math.min(view.w / Math.max(1, content.w), view.h / Math.max(1, content.h)) * FIT_INSET;
}

/** The camera that shows all of `content` (never zoomed in past 100%). */
export function fitCamera(content: Rect, view: Size): Camera {
  return {
    x: content.x + content.w / 2,
    y: content.y + content.h / 2,
    z: Math.min(1, Math.max(MIN_ZOOM * 0.5, fitZoom(content, view))),
  };
}

/** Below this zoom, node text is hard to read. */
export const READABLE_ZOOM = 0.72;

/**
 * The first view of the organization: all of it when that stays readable; otherwise its top-left
 * corner — where the chain of command starts — at a readable zoom (the minimap shows the rest).
 */
export function initialCamera(content: Rect, view: Size): Camera {
  const fit = fitCamera(content, view);
  if (fit.z >= READABLE_ZOOM) return fit;
  const w = view.w / READABLE_ZOOM;
  const h = view.h / READABLE_ZOOM;
  return {
    x: content.x + Math.min(content.w, w) / 2,
    y: content.y + Math.min(content.h, h) / 2,
    z: READABLE_ZOOM,
  };
}

/** Zoom by `factor`, keeping the world point under screen point (sx, sy) where it is. */
export function zoomAt(
  camera: Camera,
  view: Size,
  content: Rect,
  sx: number,
  sy: number,
  factor: number,
): Camera {
  const [wx, wy] = screenToWorld(camera, view, sx, sy);
  const zoomed = clampCamera({ ...camera, z: camera.z * factor }, view, content);
  return clampCamera(
    {
      x: wx - (sx - view.w / 2) / zoomed.z,
      y: wy - (sy - view.h / 2) / zoomed.z,
      z: zoomed.z,
    },
    view,
    content,
  );
}

/** The zoom factor for a wheel event (`deltaMode` 1 counts lines, not pixels). */
export function wheelFactor(deltaY: number, deltaMode: number): number {
  return Math.exp(-deltaY * (deltaMode === 1 ? 16 : 1) * WHEEL_ZOOM_SPEED);
}

/** Move the camera by a screen-space drag of (dx, dy): the content follows the pointer. */
export function panBy(camera: Camera, view: Size, content: Rect, dx: number, dy: number): Camera {
  return clampCamera(
    { x: camera.x - dx / camera.z, y: camera.y - dy / camera.z, z: camera.z },
    view,
    content,
  );
}

/** The camera that centers `rect`, keeping the current zoom unless it would not fit. */
export function focusCamera(camera: Camera, view: Size, rect: Rect): Camera {
  const fits = Math.min(view.w / (rect.w + 160), view.h / (rect.h + 160));
  return {
    x: rect.x + rect.w / 2,
    y: rect.y + rect.h / 2,
    z: Math.min(camera.z, Math.max(MIN_ZOOM, fits)),
  };
}

export function easeInOutCubic(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/** The camera `k` (0‥1) of the way from `from` to `to`; zoom moves evenly on a log scale. */
export function interpolate(from: Camera, to: Camera, k: number): Camera {
  return {
    x: from.x + (to.x - from.x) * k,
    y: from.y + (to.y - from.y) * k,
    z: Math.exp(Math.log(from.z) + (Math.log(to.z) - Math.log(from.z)) * k),
  };
}

export function sameCamera(a: Camera, b: Camera): boolean {
  return Math.abs(a.x - b.x) < 0.01 && Math.abs(a.y - b.y) < 0.01 && Math.abs(a.z - b.z) < 1e-4;
}

export function contains(rect: Rect, x: number, y: number): boolean {
  return x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h;
}

function round(n: number, digits = 2): number {
  const f = 10 ** digits;
  return Math.round(n * f) / f;
}
