import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { PopOutNotice } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { useWorkspace } from "./context";
import { Dock, PanelPortals } from "./Dock";
import { LAYOUT_KEY } from "./layout";
import { WorkspaceProvider } from "./WorkspaceProvider";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    preparePopOut: vi.fn(),
    focusPopOut: vi.fn(),
    resetPopOuts: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribePopOuts: vi.fn(),
}));

const api = vi.mocked(commands);
let notify: (n: PopOutNotice) => void = () => undefined;

/** A stand-in for the window `window.open` gives: its own document, and whether it closed. */
function fakeWindow() {
  const doc = document.implementation.createHTMLDocument("");
  const listeners = new Map<string, () => void>();
  const win = {
    document: doc,
    closed: false,
    close: vi.fn(() => {
      win.closed = true;
    }),
    addEventListener: (name: string, fn: () => void) => listeners.set(name, fn),
    fire: (name: string) => listeners.get(name)?.(),
  };
  return win;
}

function Buttons() {
  const ws = useWorkspace();
  return (
    <>
      <button type="button" onClick={() => ws.show("terminal")}>
        Show the terminal
      </button>
      <button type="button" onClick={() => ws.show("files")}>
        Show Files
      </button>
      <button
        type="button"
        onClick={() => ws.popOut("terminal", { x: 1, y: 2, width: 700, height: 400 })}
      >
        Pop it out
      </button>
    </>
  );
}

function WorkArea() {
  const ws = useWorkspace();
  return (
    <div ref={ws.measure} className="shell__work">
      <Dock side="left" />
      <Dock side="right" />
      <Dock side="bottom" />
    </div>
  );
}

function Harness() {
  return (
    <WorkspaceProvider>
      <Buttons />
      <WorkArea />
      <PanelPortals
        render={(panel) => (
          <section aria-label={panel === "terminal" ? "Terminal" : "Files"}>
            {panel === "terminal" ? "the owner's terminals" : "the files"}
          </section>
        )}
      />
    </WorkspaceProvider>
  );
}

const kept = () => JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "null");

beforeEach(() => {
  cleanup();
  localStorage.clear();
  vi.restoreAllMocks();
  api.preparePopOut.mockResolvedValue();
  api.focusPopOut.mockResolvedValue(true);
  api.resetPopOuts.mockResolvedValue();
  vi.mocked(events.subscribePopOuts).mockImplementation((handler) => {
    notify = handler;
    return Promise.resolve(() => undefined);
  });
});

describe("panels in docks and windows (Phase 21, ADR-092)", () => {
  it("shows the terminal in its dock, moves it with the panel menu, and keeps where it is", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Show the terminal" }));
    const bottom = document.querySelector('section[data-dock="bottom"]')!;
    expect(bottom).toBeVisible();
    expect(bottom.querySelector('section[aria-label="Terminal"]')).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Terminal panel" }));
    await user.click(screen.getByRole("menuitem", { name: /Move to the left/ }));
    const left = document.querySelector('section[data-dock="left"]')!;
    await waitFor(() =>
      expect(left.querySelector('section[aria-label="Terminal"]')).not.toBeNull(),
    );
    expect(kept().panels.terminal).toEqual({ dock: "left", popped: false });
    // The left dock's edge is on its right: the right arrow widens it.
    const edge = screen.getByRole("separator", { name: "Resize the panels on the left" });
    edge.focus();
    await user.keyboard("{ArrowRight}");
    expect(kept().docks.left.size).toBe(296);
  });

  it("pops a panel out into its own window, the same element, and puts it back", async () => {
    const user = userEvent.setup();
    const win = fakeWindow();
    const open = vi.spyOn(window, "open").mockReturnValue(win as unknown as Window);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Show the terminal" }));
    const terminal = document.querySelector(".panel-host--terminal")!;
    await user.click(screen.getByRole("button", { name: "Pop it out" }));
    // Plenipo is asked first, where it was dropped; then the page opens the window.
    expect(api.preparePopOut).toHaveBeenCalledWith("terminal", {
      x: 1,
      y: 2,
      width: 700,
      height: 400,
    });
    await waitFor(() => expect(open).toHaveBeenCalledWith("about:blank", "_blank"));
    await waitFor(() => expect(win.document.querySelector(".panel-host--terminal")).toBe(terminal));
    expect(document.querySelector(".panel-host--terminal")).toBeNull();
    expect(win.document.title).toBe("Plenipo · Terminal");
    expect(kept().panels.terminal.popped).toBe(true);
    // The window's own title bar: its tab and Put back.
    await waitFor(() =>
      expect(win.document.querySelector(".popout-bar")?.textContent).toContain("Put back"),
    );
    const putBack = [...win.document.querySelectorAll(".popout-bar button")].find(
      (b) => b.textContent === "Put back",
    ) as HTMLButtonElement;
    act(() => putBack.click());
    await waitFor(() => expect(document.querySelector(".panel-host--terminal")).toBe(terminal));
    expect(win.close).toHaveBeenCalled();
    expect(kept().panels.terminal.popped).toBe(false);
  });

  it("puts a panel back when its window is closed", async () => {
    const user = userEvent.setup();
    const win = fakeWindow();
    vi.spyOn(window, "open").mockReturnValue(win as unknown as Window);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Pop it out" }));
    await waitFor(() => expect(kept().panels.terminal.popped).toBe(true));
    act(() => notify({ kind: "closed", panel: "terminal" }));
    await waitFor(() => expect(kept().panels.terminal.popped).toBe(false));
    expect(
      document.querySelector('section[data-dock="bottom"] .panel-host--terminal'),
    ).not.toBeNull();
  });

  it("says so, and keeps the panel docked, when its window cannot open", async () => {
    const user = userEvent.setup();
    vi.spyOn(window, "open").mockReturnValue(null);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Pop it out" }));
    await waitFor(() => expect(api.preparePopOut).toHaveBeenCalled());
    expect(kept()?.panels.terminal.popped ?? false).toBe(false);
  });

  it("opens a popped-out panel's window again after a restart", async () => {
    localStorage.setItem(
      LAYOUT_KEY,
      JSON.stringify({
        version: 1,
        panels: {
          terminal: { dock: "bottom", popped: true },
          files: { dock: "left", popped: false },
        },
        order: ["terminal", "files"],
        docks: {
          left: { open: false, size: 280, active: "files" },
          right: { open: false, size: 420, active: null },
          bottom: { open: false, size: 260, active: "terminal" },
        },
      }),
    );
    const win = fakeWindow();
    vi.spyOn(window, "open").mockReturnValue(win as unknown as Window);
    render(<Harness />);
    await waitFor(() => expect(win.document.querySelector(".panel-host--terminal")).not.toBeNull());
    expect(api.preparePopOut).toHaveBeenCalledWith("terminal", null);
  });

  it("drags a panel's tab to another dock, and past the window's edge to pop it out there", async () => {
    const user = userEvent.setup();
    const win = fakeWindow();
    vi.spyOn(window, "open").mockReturnValue(win as unknown as Window);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Show the terminal" }));
    const tab = screen.getByRole("tab", { name: "Terminal" });
    // jsdom has no layout: the work area is where the pointer is.
    const area = document.querySelector(".shell__work") as HTMLElement;
    vi.spyOn(area, "getBoundingClientRect").mockReturnValue({
      left: 0,
      top: 0,
      right: 1000,
      bottom: 800,
      width: 1000,
      height: 800,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    Object.assign(tab, { setPointerCapture: () => undefined, hasPointerCapture: () => false });
    fireEvent.pointerDown(tab, { button: 0, clientX: 500, clientY: 700, pointerId: 1 });
    fireEvent.pointerMove(tab, { clientX: 980, clientY: 300, pointerId: 1 });
    fireEvent.pointerUp(tab, { clientX: 980, clientY: 300, pointerId: 1 });
    await waitFor(() => expect(kept()?.panels.terminal.dock).toBe("right"));
    // Past the window's edge: its own window, where it was let go.
    const moved = screen.getByRole("tab", { name: "Terminal" });
    Object.assign(moved, { setPointerCapture: () => undefined, hasPointerCapture: () => false });
    fireEvent.pointerDown(moved, { button: 0, clientX: 900, clientY: 100, pointerId: 2 });
    fireEvent.pointerMove(moved, { clientX: -40, clientY: 100, pointerId: 2 });
    fireEvent.pointerUp(moved, {
      clientX: -40,
      clientY: 100,
      screenX: 1800,
      screenY: 300,
      pointerId: 2,
    });
    await waitFor(() =>
      expect(api.preparePopOut).toHaveBeenCalledWith("terminal", {
        x: 1740,
        y: 284,
        width: 480,
        height: 300,
      }),
    );
    await waitFor(() => expect(kept().panels.terminal.popped).toBe(true));
  });

  it("Reset layout closes the pop-outs, forgets their places, and starts again", async () => {
    const user = userEvent.setup();
    const win = fakeWindow();
    vi.spyOn(window, "open").mockReturnValue(win as unknown as Window);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Pop it out" }));
    await waitFor(() => expect(kept().panels.terminal.popped).toBe(true));
    // From the Files panel's menu (its dock shows it).
    await user.click(screen.getByRole("button", { name: "Show Files" }));
    await user.click(screen.getByRole("button", { name: "Files panel" }));
    await user.click(screen.getByRole("menuitem", { name: /Reset layout/ }));
    expect(api.resetPopOuts).toHaveBeenCalled();
    expect(win.close).toHaveBeenCalled();
    expect(kept().panels.terminal).toEqual({ dock: "bottom", popped: false });
  });
});
