import { describe, expect, it } from "vitest";

import {
  activeIn,
  defaultLayout,
  dockMax,
  dockShown,
  hideDock,
  isLayout,
  moveTo,
  panelShown,
  panelsIn,
  popOut,
  poppedPanels,
  putBack,
  resize,
  show,
  startingLayout,
  toggle,
} from "./layout";

describe("the window's layout (Phase 21, ADR-092)", () => {
  it("starts with the terminal at the bottom and Files on the left, both hidden", () => {
    const l = defaultLayout();
    expect(panelsIn(l, "bottom")).toEqual(["terminal"]);
    expect(panelsIn(l, "left")).toEqual(["files"]);
    expect(panelsIn(l, "right")).toEqual([]);
    expect(panelShown(l, "terminal")).toBe(false);
    expect(panelShown(l, "files")).toBe(false);
    expect(isLayout(l)).toBe(true);
  });

  it("keeps the terminal where it was before Phase 21 (ADR-092 §14)", () => {
    const l = startingLayout(null, { open: true, side: "right", size: 500 });
    expect(l.panels.terminal).toEqual({ dock: "right", popped: false });
    expect(l.docks.right).toEqual({ open: true, size: 500, active: "terminal" });
    expect(panelShown(l, "terminal")).toBe(true);
    // A kept layout wins; nonsense falls back to the start.
    expect(startingLayout(defaultLayout(), { side: "right" }).panels.terminal.dock).toBe("bottom");
    expect(startingLayout({ version: 2 }, "junk")).toEqual(defaultLayout());
  });

  it("shows, hides, and toggles a panel in its dock", () => {
    let l = show(defaultLayout(), "terminal");
    expect(panelShown(l, "terminal")).toBe(true);
    expect(dockShown(l, "bottom")).toBe(true);
    l = toggle(l, "terminal");
    expect(panelShown(l, "terminal")).toBe(false);
    l = hideDock(toggle(l, "terminal"), "bottom");
    expect(panelShown(l, "terminal")).toBe(false);
  });

  it("moves a panel to another dock, shown there, in its tab order", () => {
    let l = moveTo(defaultLayout(), "files", "bottom");
    expect(panelsIn(l, "bottom")).toEqual(["terminal", "files"]);
    expect(activeIn(l, "bottom")).toBe("files");
    expect(panelsIn(l, "left")).toEqual([]);
    expect(dockShown(l, "left")).toBe(false);
    l = moveTo(l, "files", "bottom", "terminal");
    expect(panelsIn(l, "bottom")).toEqual(["files", "terminal"]);
    // The dock a shown panel leaves shows its next one.
    l = moveTo(l, "files", "right");
    expect(activeIn(l, "bottom")).toBe("terminal");
  });

  it("pops a panel out and puts it back in the dock it came from, or another", () => {
    let l = show(defaultLayout(), "terminal");
    l = popOut(l, "terminal");
    expect(l.panels.terminal).toEqual({ dock: "bottom", popped: true });
    expect(panelShown(l, "terminal")).toBe(true);
    expect(panelsIn(l, "bottom")).toEqual([]);
    expect(poppedPanels(l)).toEqual(["terminal"]);
    expect(show(l, "terminal")).toBe(l);
    const home = putBack(l, "terminal");
    expect(home.panels.terminal).toEqual({ dock: "bottom", popped: false });
    expect(panelShown(home, "terminal")).toBe(true);
    const right = putBack(l, "terminal", "right");
    expect(right.panels.terminal.dock).toBe("right");
    // A panel not popped out stays where it is.
    expect(putBack(home, "terminal")).toBe(home);
  });

  it("keeps a dock's size within the room the page leaves", () => {
    const l = show(show(defaultLayout(), "files"), "terminal");
    const room = { width: 1200, height: 800 };
    expect(dockMax(l, "bottom", room)).toBe(620);
    // The other side dock's width counts when it shows.
    const both = moveTo(l, "terminal", "right");
    expect(dockMax(both, "left", room)).toBe(1200 - 180 - both.docks.right.size);
    expect(resize(l, "left", 5, 500).docks.left.size).toBe(120);
    expect(resize(l, "left", 9000, 500).docks.left.size).toBe(500);
    // Before the work area is measured, there is no limit yet.
    expect(dockMax(l, "left", { width: 0, height: 0 })).toBe(4000);
  });

  it("refuses a kept layout that is not whole", () => {
    const l = defaultLayout();
    expect(isLayout({ ...l, order: ["terminal"] })).toBe(false);
    expect(
      isLayout({ ...l, docks: { ...l.docks, left: { open: true, size: 5, active: null } } }),
    ).toBe(false);
    expect(isLayout({ ...l, panels: { ...l.panels, files: { dock: "top", popped: false } } })).toBe(
      false,
    );
    expect(isLayout(null)).toBe(false);
  });
});
