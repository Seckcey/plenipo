import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, TerminalEvent, TerminalSettings } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { sampleServer, sampleServers } from "../test/serverFixtures";
import { PANEL_KEY } from "./panel";
import { TerminalButton, TerminalPanel } from "./TerminalPanel";
import { TerminalProvider } from "./TerminalProvider";

// xterm.js draws on a real screen; a stand-in records what it is given.
const xterm = vi.hoisted(() => {
  const made: {
    written: (string | Uint8Array)[];
    type: (data: string) => void;
    options: Record<string, unknown>;
    disposed: boolean;
    focused: number;
  }[] = [];
  return { made };
});
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown>;
    private handlers: ((d: string) => void)[] = [];
    private record: (typeof xterm.made)[number];
    constructor(options: Record<string, unknown>) {
      this.options = { ...options };
      this.record = {
        written: [],
        type: (d: string) => this.handlers.forEach((h) => h(d)),
        options: this.options,
        disposed: false,
        focused: 0,
      };
      xterm.made.push(this.record);
    }
    loadAddon() {}
    open() {}
    focus() {
      this.record.focused += 1;
    }
    dispose() {
      this.record.disposed = true;
    }
    write(d: string | Uint8Array) {
      this.record.written.push(d);
    }
    attachCustomKeyEventHandler() {}
    onData(h: (d: string) => void) {
      this.handlers.push(h);
      return { dispose: () => undefined };
    }
    onResize() {
      return { dispose: () => undefined };
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getControlStatus: vi.fn(),
    getTaskTimeline: vi.fn(),
    getServers: vi.fn(),
    getTerminalSettings: vi.fn(),
    openTerminal: vi.fn(),
    writeTerminal: vi.fn(),
    resizeTerminal: vi.fn(),
    closeTerminal: vi.fn(),
    stopServerCommand: vi.fn(),
    takeOverControl: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
}));

const api = vi.mocked(commands);
let ledger: (e: LedgerEvent) => void = () => undefined;
let shell: (e: TerminalEvent) => void = () => undefined;

const settings: TerminalSettings = {
  shell: "windowsPowerShell",
  shells: [
    { shell: "windowsPowerShell", label: "Windows PowerShell", installed: true },
    { shell: "powerShell7", label: "PowerShell 7", installed: false },
    { shell: "commandPrompt", label: "Command Prompt", installed: true },
  ],
  serversSwitchedOn: true,
  open: [],
};

let seq = 100;
function ledgerEvent(eventType: string, payload: Record<string, unknown>): LedgerEvent {
  seq += 1;
  return {
    seq,
    id: `e${seq}`,
    taskId: "task-1",
    executionId: null,
    source: "guard",
    destination: null,
    eventType,
    payload,
    createdAt: Date.now(),
  };
}

function Harness() {
  return (
    <TerminalProvider>
      <TerminalButton />
      <TerminalPanel theme="dark" />
    </TerminalProvider>
  );
}

/** The panel (a hidden element has no role to find it by). */
const panel = () => {
  const el = document.querySelector<HTMLElement>('section[aria-label="Terminal"]');
  if (!el) throw new Error("no terminal panel");
  return el;
};

beforeEach(() => {
  cleanup();
  localStorage.clear();
  xterm.made.length = 0;
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    ledger = handler;
    return Promise.resolve(() => undefined);
  });
  api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [], revision: 1 });
  api.getTaskTimeline.mockResolvedValue({ task: null as never, events: [], children: [] });
  api.getServers.mockResolvedValue(sampleServers());
  api.getTerminalSettings.mockResolvedValue(settings);
  api.openTerminal.mockImplementation((place, cols, rows, onEvent) => {
    shell = onEvent;
    return Promise.resolve({
      id: "0f8fad5b-d9cb-469f-a165-70867728950e",
      title: place.kind === "thisPc" ? "This PC" : "Shop",
      place,
      detail: "Windows PowerShell",
      openedAt: Date.now(),
    });
  });
  api.writeTerminal.mockResolvedValue();
  api.resizeTerminal.mockResolvedValue();
  api.closeTerminal.mockResolvedValue();
  api.stopServerCommand.mockResolvedValue();
  api.takeOverControl.mockResolvedValue({ stopped: false, sessions: [], revision: 2 });
});

describe("the terminal panel", () => {
  it("shows and hides with the Terminal button and Ctrl+`, and comes back the same after a restart", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Harness />);
    expect(panel()).not.toBeVisible();
    const button = screen.getByRole("button", { name: /^Terminal/ });
    expect(button).toHaveAttribute("aria-pressed", "false");
    await user.click(button);
    expect(panel()).toBeVisible();
    expect(button).toHaveAttribute("aria-pressed", "true");
    await user.keyboard("{Control>}`{/Control}");
    expect(panel()).not.toBeVisible();
    await user.keyboard("{Control>}`{/Control}");
    expect(panel()).toBeVisible();
    // Move it to the right, and make it bigger with the keyboard.
    await user.click(screen.getByRole("button", { name: "Move the terminal to the right" }));
    const edge = screen.getByRole("separator", { name: "Resize the terminal panel" });
    expect(edge).toHaveAttribute("aria-orientation", "vertical");
    edge.focus();
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(edge).toHaveAttribute("aria-valuenow", "452");
    expect(JSON.parse(localStorage.getItem(PANEL_KEY) ?? "{}")).toEqual({
      open: true,
      side: "right",
      size: 452,
    });
    unmount();
    // A restart: the panel comes back open, on the right, the same size.
    render(<Harness />);
    expect(panel()).toBeVisible();
    expect(screen.getByRole("separator")).toHaveAttribute("aria-valuenow", "452");
    expect(screen.getByRole("button", { name: "Move the terminal to the bottom" })).toBeVisible();
  });

  it("opens a terminal on this PC: output, typing, and closing reach the shell", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /^Terminal/ }));
    expect(screen.getByText("No terminal open")).toBeInTheDocument();
    expect(panel().querySelector("[data-pip=coding]")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Open a terminal on this PC" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(1));
    const [place, cols, rows] = api.openTerminal.mock.calls[0]!;
    expect(place).toEqual({ kind: "thisPc" });
    expect([cols, rows]).toEqual([80, 24]);
    const term = xterm.made[0]!;
    expect(term.options.fontFamily).toContain("Cascadia Mono");
    expect((term.options.theme as Record<string, string>).background).toBeTruthy();
    act(() => shell({ kind: "output", data: btoa("PS C:\\Users\\owner> ") }));
    const out = term.written.at(-1) as Uint8Array;
    expect(new TextDecoder().decode(out)).toBe("PS C:\\Users\\owner> ");
    await waitFor(() => expect(screen.getByRole("tab", { name: "This PC" })).toBeInTheDocument());
    act(() => term.type("dir\r"));
    await waitFor(() =>
      expect(api.writeTerminal).toHaveBeenCalledWith(
        "0f8fad5b-d9cb-469f-a165-70867728950e",
        "dir\r",
      ),
    );
    await user.click(screen.getByRole("button", { name: "Close the terminal on this PC" }));
    expect(api.closeTerminal).toHaveBeenCalledWith("0f8fad5b-d9cb-469f-a165-70867728950e");
    expect(term.disposed).toBe(true);
    expect(screen.getByText("No terminal open")).toBeInTheDocument();
  });

  it("says why a terminal could not open, and tries again", async () => {
    const user = userEvent.setup();
    api.openTerminal.mockRejectedValueOnce({
      kind: "invalidInput",
      message: "This server's ID changed. Shop now shows SHA256:other",
    });
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /^Terminal/ }));
    await user.click(await screen.findByRole("button", { name: "New terminal" }));
    await user.click(await screen.findByRole("menuitem", { name: /Shop/ }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The terminal on Shop could not open");
    expect(alert).toHaveTextContent("This server's ID changed");
    expect(screen.getByRole("tab", { name: /Shop/ })).toHaveTextContent("PRODUCTION");
    await user.click(within(alert).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(2));
    expect(api.openTerminal.mock.calls[1]![0]).toEqual({ kind: "server", serverId: "srv-shop" });
  });

  it("lists the servers under New terminal, and says why one cannot be used", async () => {
    const user = userEvent.setup();
    api.getServers.mockResolvedValue({
      ...sampleServers([
        sampleServer(),
        sampleServer({ id: "srv-new", name: "New box", hostKey: undefined }),
      ]),
    });
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /^Terminal/ }));
    await user.click(await screen.findByRole("button", { name: "New terminal" }));
    expect(await screen.findByRole("menuitem", { name: /This PC/ })).toHaveTextContent(
      "Windows PowerShell",
    );
    expect(screen.getByRole("menuitem", { name: /Shop/ })).toHaveTextContent("PRODUCTION");
    expect(
      screen.getByRole("menuitem", { name: /New box \(its server ID is not pinned\)/ }),
    ).toBeDisabled();
  });

  it("opens a watch tab by itself when a worker connects: live, read-only, with Stop and Disconnect", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await waitFor(() => expect(events.subscribeLedgerEvents).toHaveBeenCalled());
    expect(panel()).not.toBeVisible();
    act(() =>
      ledger(
        ledgerEvent("ssh.connected", {
          grantId: "g1",
          worker: "Operations Engineer",
          serverId: "srv-shop",
          server: "Shop",
          environment: "production",
          address: "shop@203.0.113.10:22",
        }),
      ),
    );
    expect(panel()).toBeVisible();
    const tab = screen.getByRole("tab", { name: /Operations Engineer · Shop/ });
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(tab).toHaveTextContent("PRODUCTION");
    act(() => {
      ledger(
        ledgerEvent("ssh.command_started", {
          grantId: "g1",
          serverId: "srv-shop",
          commandId: "c1",
          command: "tail -f /var/log/app.log",
        }),
      );
      ledger(ledgerEvent("ssh.output", { commandId: "c1", stream: "out", lines: ["ready"] }));
    });
    const log = screen.getByRole("log", { name: /Operations Engineer · Shop/ });
    expect(log).toHaveTextContent("$ tail -f /var/log/app.log");
    expect(log).toHaveTextContent("ready");
    // Read-only: there is nowhere to type.
    const body = screen.getByRole("tabpanel");
    expect(within(body).queryByRole("textbox")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Stop" }));
    expect(api.stopServerCommand).toHaveBeenCalledWith("c1");
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(api.takeOverControl).toHaveBeenCalledWith("server:g1");
    // It stays readable after the worker disconnects, until closed.
    act(() =>
      ledger(
        ledgerEvent("ssh.disconnected", {
          grantId: "g1",
          serverId: "srv-shop",
          why: "you disconnected the worker",
        }),
      ),
    );
    expect(log).toHaveTextContent("Disconnected: you disconnected the worker");
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Close Operations Engineer · Shop" }));
    expect(screen.queryByRole("tab")).toBeNull();
  });
});
