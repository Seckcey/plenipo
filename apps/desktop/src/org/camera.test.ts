import { describe, expect, it } from "vitest";

import {
  MAX_ZOOM,
  READABLE_ZOOM,
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
  worldToScreen,
  worldTransform,
  zoomAt,
  type Camera,
} from "./camera";

const view = { w: 1000, h: 600 };
const content = { x: 0, y: 0, w: 2000, h: 1000 };

describe("camera", () => {
  it("maps world and screen coordinates both ways around the viewport center", () => {
    const c: Camera = { x: 500, y: 250, z: 2 };
    expect(worldToScreen(c, view, 500, 250)).toEqual([500, 300]);
    expect(worldToScreen(c, view, 510, 260)).toEqual([520, 320]);
    const [wx, wy] = screenToWorld(c, view, 123, 456);
    expect(worldToScreen(c, view, wx, wy)).toEqual([123, 456]);
    expect(worldTransform(c, view)).toBe("translate(-500px, -200px) scale(2)");
    expect(visibleRect(c, view)).toEqual({ x: 250, y: 100, w: 500, h: 300 });
  });

  it("zooms at the cursor, keeping the point under it still", () => {
    const c: Camera = { x: 1000, y: 500, z: 0.5 };
    const before = screenToWorld(c, view, 800, 150);
    const zoomed = zoomAt(c, view, content, 800, 150, 1.6);
    expect(zoomed.z).toBeCloseTo(0.8);
    const after = screenToWorld(zoomed, view, 800, 150);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);
  });

  it("limits zoom and keeps the content reachable", () => {
    expect(clampCamera({ x: 1000, y: 500, z: 99 }, view, content).z).toBe(MAX_ZOOM);
    expect(clampCamera({ x: 1000, y: 500, z: 0.0001 }, view, content).z).toBeGreaterThan(0.05);
    const far = clampCamera({ x: 1e6, y: -1e6, z: 1 }, view, content);
    // The content's edge can come to the middle of the screen, but not scroll out of reach.
    expect(far.x).toBeLessThanOrEqual(content.x + content.w + view.w);
    expect(far.y).toBeGreaterThanOrEqual(content.y - view.h);
  });

  it("pans with the pointer: dragging right moves the content right", () => {
    const c: Camera = { x: 1000, y: 500, z: 2 };
    const moved = panBy(c, view, content, 100, -40);
    expect(moved.x).toBe(950);
    expect(moved.y).toBe(520);
  });

  it("fits everything but never zooms in past 100%", () => {
    const fit = fitCamera(content, view);
    expect(fit.x).toBe(1000);
    expect(fit.y).toBe(500);
    expect(fit.z).toBeCloseTo(0.45);
    expect(fitCamera({ x: 0, y: 0, w: 100, h: 50 }, view).z).toBe(1);
  });

  it("opens small organizations whole and large ones at their readable top-left", () => {
    expect(initialCamera({ x: 0, y: 0, w: 800, h: 400 }, view)).toEqual(
      fitCamera({ x: 0, y: 0, w: 800, h: 400 }, view),
    );
    const big = initialCamera(content, view);
    expect(big.z).toBe(READABLE_ZOOM);
    const seen = visibleRect(big, view);
    expect(seen.x).toBeCloseTo(content.x);
    expect(seen.y).toBeCloseTo(content.y);
  });

  it("focuses a node without zooming in", () => {
    const c: Camera = { x: 0, y: 0, z: 0.6 };
    expect(focusCamera(c, view, { x: 100, y: 200, w: 200, h: 80 })).toEqual({
      x: 200,
      y: 240,
      z: 0.6,
    });
  });

  it("eases and interpolates zoom on a log scale", () => {
    expect(easeInOutCubic(0)).toBe(0);
    expect(easeInOutCubic(0.5)).toBe(0.5);
    expect(easeInOutCubic(1)).toBe(1);
    const mid = interpolate({ x: 0, y: 0, z: 0.5 }, { x: 100, y: 50, z: 2 }, 0.5);
    expect(mid).toEqual({ x: 50, y: 25, z: 1 });
    expect(wheelFactor(-100, 0)).toBeGreaterThan(1);
    expect(wheelFactor(100, 0)).toBeLessThan(1);
    expect(wheelFactor(3, 1)).toBeCloseTo(wheelFactor(48, 0));
  });
});
