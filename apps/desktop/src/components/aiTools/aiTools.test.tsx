import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type {
  AgentRuntimeInfo,
  AgentUpdate,
  AiToolState,
  AiToolsPage,
  AiToolUsage,
  LedgerEvent,
  TerminalEvent,
  UsageModel,
} from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import * as events from "../../api/events";
import { AgentsProvider } from "../../agents/AgentsProvider";
import { RuntimeProvider } from "../../runtime/RuntimeProvider";
import { TerminalPanel } from "../../terminal/TerminalPanel";
import { TerminalProvider } from "../../terminal/TerminalProvider";
import { a11yProblems } from "../../test/a11y";
import { session } from "../../test/agentFixtures";
import { AI_TOOL_IDS, aiPage, aiRuntime, aiTool, idle, route, T0 } from "../../test/aiToolFixtures";
import { sampleRouting } from "../../test/routingFixtures";
import { RuntimesView } from "../../views/RuntimesView";
import { firstSentence, isNewerVersion, planLeft, planWindowName } from "./words";

// xterm.js draws on a real screen; a stand-in records what it is given.
const xterm = vi.hoisted(() => {
  const made: {
    written: (string | Uint8Array)[];
    type: (data: string) => void;
    /** How often the keyboard was put in it. */
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
        focused: 0,
      };
      xterm.made.push(this.record);
    }
    loadAddon() {}
    open() {}
    focus() {
      this.record.focused += 1;
    }
    dispose() {}
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

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRuntimeOverview: vi.fn(),
    getAgentOverview: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    getAiTools: vi.fn(),
    checkAiTool: vi.fn(),
    checkAiToolVersions: vi.fn(),
    getAiToolUsage: vi.fn(),
    updateAiTool: vi.fn(),
    cancelAiToolUpdate: vi.fn(),
    setAiToolsAutoUpdate: vi.fn(),
    setAiToolPayment: vi.fn(),
    savePaidKey: vi.fn(),
    removePaidKey: vi.fn(),
    getRouting: vi.fn(),
    clearUsageLimit: vi.fn(),
    getControlStatus: vi.fn(),
    getTaskTimeline: vi.fn(),
    getServers: vi.fn(),
    getTerminalSettings: vi.fn(),
    getLiveView: vi.fn(),
    openTerminal: vi.fn(),
    writeTerminal: vi.fn(),
    resizeTerminal: vi.fn(),
    closeTerminal: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeRuntimeEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(),
}));

const api = vi.mocked(commands);
let shell: (e: TerminalEvent) => void = () => undefined;
const ledgerListeners = new Set<(e: LedgerEvent) => void>();
const agentListeners = new Set<(u: AgentUpdate) => void>();

/** Everyone listening hears each event (the page, the Router's settings, the terminal). */
function ledger(eventType: string, payload: Record<string, unknown> = {}) {
  const e: LedgerEvent = {
    seq: Math.floor(Math.random() * 1e9),
    id: `e-${eventType}`,
    taskId: null,
    executionId: null,
    source: "plenipo",
    destination: null,
    eventType,
    payload,
    createdAt: Date.now(),
  };
  act(() => ledgerListeners.forEach((h) => h(e)));
}

function agents(update: AgentUpdate) {
  act(() => agentListeners.forEach((h) => h(update)));
}

/** The seven AI tools, as their checks say. */
function runtimes(patch: Record<string, Partial<AgentRuntimeInfo>> = {}): AgentRuntimeInfo[] {
  return [
    aiRuntime("claude-code", "2.1.283", patch["claude-code"]),
    aiRuntime("codex", "0.50.0", patch.codex),
    aiRuntime("grok", "1.0.41", patch.grok),
    aiRuntime("kimi", "0.34.0", patch.kimi),
    aiRuntime("ollama", "0.34.4", patch.ollama),
    aiRuntime("antigravity", "1.2.13", patch.antigravity),
    aiRuntime("copilot", "1.0.89", patch.copilot),
  ];
}

function page(patch: Record<string, Parameters<typeof aiTool>[1]> = {}): AiToolsPage {
  return aiPage(AI_TOOL_IDS.map((id) => aiTool(id, patch[id])));
}

function routing(patch: Record<string, Parameters<typeof route>[1]> = {}) {
  return {
    ...sampleRouting(),
    tools: AI_TOOL_IDS.map((id) => route(id, patch[id])),
  };
}

const go = vi.fn();

function Page({ toolId = null }: { toolId?: string | null }) {
  return (
    <RuntimeProvider>
      <AgentsProvider>
        <TerminalProvider>
          <RuntimesView
            selectedId={null}
            onSelect={() => undefined}
            toolId={toolId}
            onOpenPage={go}
          />
          <TerminalPanel theme="dark" />
        </TerminalProvider>
      </AgentsProvider>
    </RuntimeProvider>
  );
}

/** A card, opened: cards start closed (Phase 25, item 2.1). */
const card = (label: string) => {
  const item = screen.getByRole("listitem", { name: `${label} AI tool` });
  const toggle = within(item).queryByRole("button", { name: label, expanded: false });
  if (toggle) fireEvent.click(toggle);
  return item;
};
/** A card as it is, open or closed. */
const cardAsIs = (label: string) => screen.getByRole("listitem", { name: `${label} AI tool` });

async function show(toolId: string | null = null) {
  const view = render(<Page toolId={toolId} />);
  await screen.findByRole("listitem", { name: "Codex AI tool" });
  // The page's own part has loaded.
  await waitFor(() => expect(screen.getByText(/^Last looked for new versions:/)).toBeVisible());
  await waitFor(() => expect(within(card("Codex")).queryAllByText("Loading…")).toHaveLength(0));
  return view;
}

/** The terminal panel (a hidden element has no role to find it by). */
const panel = () => document.querySelector<HTMLElement>('section[aria-label="Terminal"]')!;

const noUsage = (runtimeId: string, starts: number[]): AiToolUsage => ({
  runtimeId,
  days: starts.slice(0, -1).map((start) => ({ start, models: [] })),
});

beforeEach(() => {
  localStorage.clear();
  xterm.made.length = 0;
  ledgerListeners.clear();
  agentListeners.clear();
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    ledgerListeners.add(handler);
    return Promise.resolve(() => {
      ledgerListeners.delete(handler);
    });
  });
  vi.mocked(events.subscribeAgentUpdates).mockImplementation((handler) => {
    agentListeners.add(handler);
    return Promise.resolve(() => {
      agentListeners.delete(handler);
    });
  });
  api.getRuntimeOverview.mockResolvedValue({
    profiles: [],
    executions: [],
    activeCount: 0,
    notices: [],
  });
  api.getAgentOverview.mockResolvedValue({ runtimes: runtimes(), sessions: [], notices: [] });
  api.refreshAgentRuntimes.mockResolvedValue(runtimes());
  api.getAiTools.mockResolvedValue(page());
  api.getAiToolUsage.mockImplementation((runtimeId, starts) =>
    Promise.resolve(noUsage(runtimeId, starts)),
  );
  api.getRouting.mockResolvedValue(routing());
  api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [], revision: 1 });
  api.getTaskTimeline.mockResolvedValue({ task: null as never, events: [], children: [] });
  api.getServers.mockResolvedValue({ servers: [], switchedOn: false } as never);
  api.getTerminalSettings.mockResolvedValue({
    shell: "windowsPowerShell",
    shells: [{ shell: "windowsPowerShell", label: "Windows PowerShell", installed: true }],
    runsAs: "as your own Windows user — never as administrator",
    serversSwitchedOn: false,
    open: [],
  });
  api.getLiveView.mockResolvedValue({ workers: [] } as never);
  api.openTerminal.mockImplementation((place, _cols, _rows, onEvent) => {
    shell = onEvent;
    return Promise.resolve({
      id: "0f8fad5b-d9cb-469f-a165-70867728950e",
      title: "Sign in · Grok",
      place,
      detail: "grok login",
      openedAt: Date.now(),
    });
  });
  api.writeTerminal.mockResolvedValue();
  api.resizeTerminal.mockResolvedValue();
  api.closeTerminal.mockResolvedValue();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe("the AI tools page: signing in (ADR-058)", () => {
  it("shows Sign in, Reconnect, and Sign out by sign-in state; Kimi has no Sign out", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        grok: { auth: { state: "signedOut", method: null, detail: null }, ready: false },
        antigravity: { auth: { state: "signedOut", method: null, detail: null }, ready: false },
        ollama: {
          installation: {
            state: "notInstalled",
            executable: null,
            version: null,
            detail: "Not found",
          },
          auth: { state: "signedOut", method: null, detail: null },
          ready: false,
        },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    // Signed in: Reconnect and Sign out.
    const codex = card("Codex");
    expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toBeEnabled();
    expect(within(codex).getByRole("button", { name: "Sign out of Codex" })).toBeEnabled();
    expect(within(codex).queryByRole("button", { name: "Sign in to Codex" })).toBeNull();
    expect(codex).toHaveTextContent(
      "Reconnect opens a tab at the bottom that runs codex login, and Sign out one that runs codex logout. You sign in there, and in your browser; Plenipo never sees it.",
    );
    // Signed out: Sign in only.
    const grok = card("Grok");
    expect(within(grok).getByRole("button", { name: "Sign in to Grok" })).toBeEnabled();
    expect(within(grok).queryByRole("button", { name: "Sign out of Grok" })).toBeNull();
    expect(within(grok).queryByRole("button", { name: "Reconnect Grok" })).toBeNull();
    expect(grok).toHaveTextContent(
      "Opens a tab at the bottom that runs grok login. You sign in there, and in your browser; Plenipo never sees it.",
    );
    // Kimi has no sign-out command of its own.
    const kimi = card("Kimi");
    expect(within(kimi).getByRole("button", { name: "Reconnect Kimi" })).toBeEnabled();
    expect(within(kimi).queryByRole("button", { name: "Sign out of Kimi" })).toBeNull();
    expect(within(kimi).getByText("Kimi has no sign-out command.")).toBeInTheDocument();
    // Antigravity signs in when started on its own (ADR-082), and has no sign-out command.
    const antigravity = card("Antigravity");
    expect(
      within(antigravity).getByRole("button", { name: "Sign in to Antigravity" }),
    ).toBeEnabled();
    expect(antigravity).toHaveTextContent(
      "Opens a tab at the bottom that runs agy. You sign in there, and in your browser; Plenipo never sees it.",
    );
    expect(
      within(antigravity).queryByRole("button", { name: "Sign out of Antigravity" }),
    ).toBeNull();
    // Not installed: the button is there, off, with the reason.
    const ollama = card("Ollama");
    expect(within(ollama).getByRole("button", { name: "Sign in to Ollama" })).toBeDisabled();
    expect(ollama).toHaveTextContent("Ollama is not installed, so you can't sign in to it here.");
  });

  it("Sign in opens a tab running the tool's own sign-in, and nothing is written to it", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        grok: { auth: { state: "signedOut", method: null, detail: null }, ready: false },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const user = userEvent.setup();
    expect(panel()).not.toBeVisible();
    await user.click(within(card("Grok")).getByRole("button", { name: "Sign in to Grok" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(1));
    expect(api.openTerminal.mock.calls[0]![0]).toEqual({
      kind: "aiTool",
      runtimeId: "grok",
      action: "signIn",
    });
    expect(await screen.findByRole("tab", { name: "Sign in · Grok" })).toBeInTheDocument();
    expect(panel()).toBeVisible();
    // The program asks a question: Plenipo never answers it.
    act(() => shell({ kind: "output", data: btoa("Press Enter to open your browser: ") }));
    await new Promise((r) => setTimeout(r, 50));
    expect(api.writeTerminal).not.toHaveBeenCalled();
    // Only the owner's own keys reach it.
    act(() => xterm.made[0]!.type("\r"));
    await waitFor(() => expect(api.writeTerminal).toHaveBeenCalledTimes(1));
    expect(api.writeTerminal).toHaveBeenCalledWith("0f8fad5b-d9cb-469f-a165-70867728950e", "\r");
    // The New terminal menu never offers a sign-in tab.
    await user.click(screen.getByRole("button", { name: "New terminal" }));
    const items = (await screen.findAllByRole("menuitem")).map((i) => i.textContent ?? "");
    expect(items.some((t) => /sign/i.test(t))).toBe(false);
  });

  it("after the sign-in tab ends, the card says Checking… until the new check arrives", async () => {
    const out = { state: "signedOut", method: null, detail: null } as const;
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({ grok: { auth: out, ready: false } }),
      sessions: [],
      notices: [],
    });
    await show();
    const user = userEvent.setup();
    await user.click(within(card("Grok")).getByRole("button", { name: "Sign in to Grok" }));
    await screen.findByRole("tab", { name: "Sign in · Grok" });
    act(() => shell({ kind: "ended", why: "the program ended", code: 0 }));
    await waitFor(() => expect(within(card("Grok")).getAllByText("Checking…").length).toBe(2));
    expect(within(card("Grok")).getByRole("button", { name: "Sign in to Grok" })).toBeDisabled();
    // Plenipo checked Grok again by itself: signed in now.
    agents({
      kind: "runtimes",
      runtimes: runtimes({
        grok: {
          auth: { state: "subscription", method: "SuperGrok", detail: null },
          checkedAt: Date.now(),
        },
      }),
    });
    const grok = card("Grok");
    expect(await within(grok).findByRole("button", { name: "Reconnect Grok" })).toBeInTheDocument();
    expect(within(grok).getByText("Signed in (subscription) · SuperGrok")).toBeInTheDocument();
    expect(within(grok).queryByText("Checking…")).toBeNull();
  });

  it("Sign out waits while a task is using the tool, opens when it is free, and Cancel stops waiting", async () => {
    const busy = session("s1", { runtimeId: "codex", activeTaskId: "t1" });
    api.getAgentOverview.mockResolvedValue({ runtimes: runtimes(), sessions: [busy], notices: [] });
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Sign out of Codex" }));
    expect(within(codex).getByRole("status")).toHaveTextContent(
      "Waiting: 1 task is using Codex. The sign-out tab opens when Codex is free.",
    );
    expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toBeDisabled();
    await new Promise((r) => setTimeout(r, 600));
    expect(api.openTerminal).not.toHaveBeenCalled();
    // The task finishes: the tab opens by itself.
    agents({ kind: "session", ...busy, activeTaskId: null });
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(1));
    expect(api.openTerminal.mock.calls[0]![0]).toEqual({
      kind: "aiTool",
      runtimeId: "codex",
      action: "signOut",
    });
    expect(await screen.findByRole("tab", { name: "Sign out · Codex" })).toBeInTheDocument();

    // Busy again: Reconnect waits, and Cancel stops it.
    agents({ kind: "session", ...busy });
    await user.click(await within(codex).findByRole("button", { name: "Reconnect Codex" }));
    expect(within(codex).getByRole("status")).toHaveTextContent("Waiting: 1 task is using Codex.");
    await user.click(within(codex).getByRole("button", { name: "Cancel Codex's sign-in" }));
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
    agents({ kind: "session", ...busy, activeTaskId: null });
    await new Promise((r) => setTimeout(r, 800));
    expect(api.openTerminal).toHaveBeenCalledTimes(1);
  });

  it("a tab Plenipo refused because a task was using the tool waits, then opens again", async () => {
    const refused = "1 task is using Codex. Plenipo waits until it finishes.";
    api.openTerminal.mockRejectedValueOnce({ kind: "invalidInput", message: refused });
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Reconnect Codex" }));
    expect(await within(codex).findByRole("status")).toHaveTextContent(`Waiting: ${refused}`);
    // The refused tab closed by itself.
    await waitFor(() => expect(screen.queryByRole("tab", { name: "Sign in · Codex" })).toBeNull());
    // Something changed in the conversations: it tries again.
    agents({ kind: "session", ...session("s2", { runtimeId: "codex" }) });
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole("tab", { name: "Sign in · Codex" })).toBeInTheDocument();
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
  });

  it("a waiting sign-out still opens after the card shows another tab (the wait is kept with the terminal panel)", async () => {
    const busy = session("s1", { runtimeId: "codex", activeTaskId: "t1" });
    api.getAgentOverview.mockResolvedValue({ runtimes: runtimes(), sessions: [busy], notices: [] });
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Sign out of Codex" }));
    expect(within(codex).getByRole("status")).toHaveTextContent("Waiting: 1 task is using Codex.");
    // Usage, then Overview again: still waiting.
    await user.click(within(codex).getByRole("tab", { name: "Usage" }));
    await user.click(within(codex).getByRole("tab", { name: "Overview" }));
    expect(within(codex).getByRole("status")).toHaveTextContent("Waiting: 1 task is using Codex.");
    // Usage again (the Sign-in part is gone from the screen), and the task finishes.
    await user.click(within(codex).getByRole("tab", { name: "Usage" }));
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
    agents({ kind: "session", ...busy, activeTaskId: null });
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(1));
    expect(api.openTerminal.mock.calls[0]![0]).toEqual({
      kind: "aiTool",
      runtimeId: "codex",
      action: "signOut",
    });
    expect(await screen.findByRole("tab", { name: "Sign out · Codex" })).toBeInTheDocument();
    await user.click(within(codex).getByRole("tab", { name: "Overview" }));
    await waitFor(() => expect(within(codex).queryByText(/^Waiting:/)).toBeNull());
  });

  it("puts the keyboard on Cancel while it waits, and back on the card's first button after", async () => {
    const busy = session("s1", { runtimeId: "codex", activeTaskId: "t1" });
    api.getAgentOverview.mockResolvedValue({ runtimes: runtimes(), sessions: [busy], notices: [] });
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Sign out of Codex" }));
    await waitFor(() =>
      expect(within(codex).getByRole("button", { name: "Cancel Codex's sign-out" })).toHaveFocus(),
    );
    await user.keyboard("{Enter}");
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
    await waitFor(() =>
      expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toHaveFocus(),
    );
    // It waits again, and the tab opens by itself: the keyboard stays on the card, not in the
    // new tab.
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(codex).getByRole("button", { name: "Cancel Codex's sign-in" })).toHaveFocus(),
    );
    agents({ kind: "session", ...busy, activeTaskId: null });
    expect(await screen.findByRole("tab", { name: "Sign in · Codex" })).toBeInTheDocument();
    await waitFor(() =>
      expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toHaveFocus(),
    );
    expect(xterm.made.at(-1)!.focused).toBe(0);
  });

  it("stops trying by itself after Plenipo refused the tab three times while nothing changed", async () => {
    const updating = "Codex is being updated. Plenipo waits until it's done.";
    await show();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    api.openTerminal.mockRejectedValue({ kind: "invalidInput", message: updating });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Reconnect Codex" }));
    expect(await within(codex).findByRole("status")).toHaveTextContent(`Waiting: ${updating}`);
    // Pressed: the keyboard went into the tab.
    expect(xterm.made[0]!.focused).toBe(1);
    // Nothing changes: it tries again every 5 seconds, without taking the keyboard.
    await act(() => vi.advanceTimersByTimeAsync(5000));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(2));
    expect(xterm.made[1]!.focused).toBe(0);
    await act(() => vi.advanceTimersByTimeAsync(5000));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(3));
    // Refused three times: it stops, and says why.
    expect(
      await within(codex).findByText(
        /^The sign-in tab didn't open: Codex is being updated\. Plenipo stopped trying by itself\./,
      ),
    ).toBeVisible();
    await act(() => vi.advanceTimersByTimeAsync(60_000));
    expect(api.openTerminal).toHaveBeenCalledTimes(3);
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
    expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toBeEnabled();
    // Try again: one more try, and it waits again.
    await user.click(within(codex).getByRole("button", { name: "Try Codex's sign-in again" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(4));
    expect(await within(codex).findByText(/^Waiting:/)).toBeVisible();
    // Cancel stops it.
    await user.click(within(codex).getByRole("button", { name: "Cancel Codex's sign-in" }));
    expect(within(codex).queryByText(/^Waiting:|^The sign-in tab/)).toBeNull();
  });

  it("a tab that opened after Try again is shown again, not opened twice", async () => {
    // Not busy: this refusal stays in its tab, and nothing waits.
    const open = "Codex's sign-in tab is open. Close it first; Plenipo waits until then.";
    api.openTerminal.mockRejectedValueOnce({ kind: "invalidInput", message: open });
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("button", { name: "Reconnect Codex" }));
    expect(await screen.findByText("Sign in · Codex could not open")).toBeVisible();
    expect(screen.getByText(open)).toBeVisible();
    expect(within(codex).queryByText(/^Waiting:/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.queryByText("Sign in · Codex could not open")).toBeNull());
    // Reconnect again: the tab running now is shown.
    await user.click(within(codex).getByRole("button", { name: "Reconnect Codex" }));
    await new Promise((r) => setTimeout(r, 50));
    expect(api.openTerminal).toHaveBeenCalledTimes(2);
    expect(screen.getAllByRole("tab", { name: "Sign in · Codex" })).toHaveLength(1);
  });

  it("a tool that stopped working says so, not that it is not installed", async () => {
    const why = "Codex didn't answer when Plenipo asked for its version.";
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        codex: {
          installation: { state: "broken", executable: "/bin/codex", version: null, detail: why },
          ready: false,
        },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const codex = card("Codex");
    expect(codex).toHaveTextContent("Not working · Checked by Plenipo 0.50.0");
    expect(within(codex).getByText(why)).toBeVisible();
    expect(codex).toHaveTextContent("Plenipo can't update Codex now (not working).");
    expect(codex).toHaveTextContent(
      `Not working: you can't sign in to Codex here now. ${why} Install Codex.`,
    );
    expect(codex).not.toHaveTextContent(/not installed/i);
    expect(within(codex).getByRole("button", { name: "Reconnect Codex" })).toBeDisabled();
  });

  it("goes straight to a card another page asked for", async () => {
    await show("codex");
    await waitFor(() => expect(card("Codex")).toHaveFocus());
  });
});

describe("the AI tools page: versions and updates (ADR-059)", () => {
  it("shows installed and checked versions, and a notice when they differ", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        codex: { checkedVersion: "0.49.0" },
        grok: { checkedVersion: "1.0.43" },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    expect(card("Claude Code")).toHaveTextContent("Installed 2.1.283 · Checked by Plenipo 2.1.283");
    expect(card("Claude Code")).not.toHaveTextContent("This version is");
    expect(card("Codex")).toHaveTextContent(
      "This version is newer than the one Plenipo was checked with (0.49.0). It should work; if something looks wrong, check for a new version of Plenipo.",
    );
    expect(card("Grok")).toHaveTextContent(
      "This version is older than the one Plenipo was checked with (1.0.43). Update it to get every model.",
    );
  });

  it("waits while a task is using the tool, then updates, checks, and says it is updated", async () => {
    api.getAiTools.mockResolvedValue(
      page({ grok: { newest: "1.0.43", update: idle({ state: "waiting", tasksUsing: 1 }) } }),
    );
    api.cancelAiToolUpdate.mockResolvedValue(page({ grok: { newest: "1.0.43" } }));
    await show();
    const user = userEvent.setup();
    const grok = card("Grok");
    expect(grok).toHaveTextContent(
      "Waiting: 1 task is using Grok. Plenipo updates it when it finishes.",
    );
    await user.click(within(grok).getByRole("button", { name: "Cancel Grok's update" }));
    expect(api.cancelAiToolUpdate).toHaveBeenCalledWith("grok");
    expect(await within(grok).findByText(/^Newest version: 1\.0\.43 \(looked at/)).toBeVisible();
    api.updateAiTool.mockResolvedValue(
      page({ grok: { newest: "1.0.43", update: idle({ state: "updating", from: "1.0.41" }) } }),
    );
    await user.click(within(grok).getByRole("button", { name: "Update Grok to 1.0.43" }));
    expect(api.updateAiTool).toHaveBeenCalledWith("grok");
    expect(await within(grok).findAllByText("Updating…")).toHaveLength(2);
    // The Ledger says what happens next; the page reads itself again.
    api.getAiTools.mockResolvedValue(
      page({ grok: { newest: "1.0.43", update: idle({ state: "checking", from: "1.0.41" }) } }),
    );
    ledger("ai_tool.update_started", { runtime: "grok", from: "1.0.41", by: "owner" });
    expect(await within(grok).findByText("Checking the new version…")).toBeVisible();
    // Plenipo checked Grok again after its update: the version it has now.
    api.getAiTools.mockResolvedValue(
      page({
        grok: {
          newest: "1.0.43",
          update: idle({ state: "updated", from: "1.0.41", to: "1.0.43", automatic: true }),
        },
      }),
    );
    agents({ kind: "runtimes", runtimes: runtimes({ grok: aiRuntime("grok", "1.0.43") }) });
    expect(await within(grok).findByText("Updated to 1.0.43 by itself.")).toBeVisible();
    expect(grok).toHaveTextContent("Installed 1.0.43 · Checked by Plenipo 1.0.43");
    expect(within(grok).queryByRole("button", { name: /^Update Grok/ })).toBeNull();
  });

  it("says when an update failed, can't update itself, or updates from the tray or with Update", async () => {
    const shown = page({
      grok: {
        newest: "1.0.43",
        update: idle({
          state: "failed",
          from: "1.0.41",
          oldStillWorks: true,
          message: "The update stopped: exit code 1",
        }),
      },
      codex: {
        update: idle({ state: "failed", from: "0.50.0", oldStillWorks: false }),
        outOfService:
          "After its update, Codex does not answer the way Plenipo reads it. Install it again: npm install -g @openai/codex",
      },
      "claude-code": {
        update: idle({
          state: "byHand",
          message:
            "Claude Code was installed with WinGet. To update it, type: winget upgrade Anthropic.ClaudeCode",
        }),
      },
      ollama: { newest: "0.35.0" },
    });
    api.getAiTools.mockResolvedValue(shown);
    api.checkAiTool.mockResolvedValue(shown);
    api.updateAiTool.mockResolvedValue(shown);
    await show();
    const user = userEvent.setup();
    // Failed, and the old version still works.
    expect(card("Grok")).toHaveTextContent(
      "The update didn't finish — your old version (1.0.41) still works.",
    );
    expect(within(card("Grok")).getByText("The update stopped: exit code 1")).toBeVisible();
    // Failed with no way back: Codex gets no tasks until it answers again.
    const codex = card("Codex");
    expect(within(codex).getByText("Plenipo is not giving Codex tasks for now.")).toBeVisible();
    expect(
      within(codex).getByText(/Install it again: npm install -g @openai\/codex/),
    ).toBeVisible();
    expect(within(codex).getByText("No tasks for now")).toBeVisible();
    await user.click(within(codex).getByRole("button", { name: "Check Codex again" }));
    expect(api.checkAiTool).toHaveBeenCalledWith("codex");
    // Can't update itself: its own command, and a terminal where you type it.
    const claude = card("Claude Code");
    expect(
      within(claude).getByText(
        "Claude Code was installed with WinGet. To update it, type: winget upgrade Anthropic.ClaudeCode",
      ),
    ).toBeVisible();
    await user.click(within(claude).getByRole("button", { name: "Open a terminal" }));
    await waitFor(() => expect(api.openTerminal).toHaveBeenCalledTimes(1));
    expect(api.openTerminal.mock.calls[0]![0]).toEqual({ kind: "thisPc" });
    await new Promise((r) => setTimeout(r, 50));
    expect(api.writeTerminal).not.toHaveBeenCalled();
    // Ollama updates from its tray icon.
    const ollama = card("Ollama");
    expect(ollama).toHaveTextContent("Newest version: 0.35.0");
    expect(ollama).toHaveTextContent(
      "Ollama updates itself: when a new version is ready, use Ollama's icon in the tray to restart it.",
    );
    expect(within(ollama).queryByRole("button", { name: /^Update Ollama/ })).toBeNull();
    // Kimi's newest version is not published: Update checks and installs.
    const kimi = card("Kimi");
    expect(kimi).toHaveTextContent(
      "Kimi's newest version is not published in a list Plenipo can read. Press Update: Kimi checks and installs a new version if there is one.",
    );
    await user.click(within(kimi).getByRole("button", { name: "Update Kimi" }));
    expect(api.updateAiTool).toHaveBeenCalledWith("kimi");
  });

  it("never offers a pre-release as an update, as Plenipo's own look doesn't count it newer", async () => {
    api.getAiTools.mockResolvedValue(
      page({ codex: { newest: "0.51.0-beta.1" }, grok: { newest: "1.0.43" } }),
    );
    await show();
    const codex = card("Codex");
    expect(within(codex).queryByRole("button", { name: /^Update Codex/ })).toBeNull();
    expect(codex).toHaveTextContent("Up to date (looked at");
    expect(codex).not.toHaveTextContent("Newest version");
    // A release is newer.
    expect(
      within(card("Grok")).getByRole("button", { name: "Update Grok to 1.0.43" }),
    ).toBeVisible();
    expect(isNewerVersion("1.0.44-beta.1", "1.0.43")).toBe(false);
    expect(isNewerVersion("1.0.44+build.5", "1.0.43")).toBe(false);
    expect(isNewerVersion("1.0.44", "1.0.43")).toBe(true);
    expect(isNewerVersion("1.0.44", "1.0.44-beta.1")).toBe(false);
  });

  it("looks for new versions, checks again, and switches Update AI tools by themselves", async () => {
    api.checkAiToolVersions.mockResolvedValue(page({ grok: { newest: "1.0.43" } }));
    api.setAiToolsAutoUpdate.mockResolvedValue({ ...page(), autoUpdate: true });
    await show();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Check for new versions" }));
    expect(api.checkAiToolVersions).toHaveBeenCalled();
    expect(
      await within(card("Grok")).findByRole("button", { name: "Update Grok to 1.0.43" }),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(api.refreshAgentRuntimes).toHaveBeenCalled();
    const auto = screen.getByRole("switch", { name: "Update AI tools by themselves" });
    expect(auto).toHaveAttribute("aria-checked", "false");
    expect(auto).toHaveAccessibleDescription(
      "Off (the default): Plenipo tells you when a new version is ready and updates only when you press Update. On: it updates each AI tool by itself, only when no task is using it.",
    );
    await user.click(auto);
    expect(api.setAiToolsAutoUpdate).toHaveBeenCalledWith(true);
    await waitFor(() => expect(auto).toHaveAttribute("aria-checked", "true"));
  });
});

describe("the AI tools page: usage, plan, payment, and models (ADR-060)", () => {
  it("a subscription AI tool always uses its subscription; with no paid AI tool, no card asks for a key", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        codex: { auth: { state: "subscription", method: "ChatGPT sign-in", detail: null } },
      }),
      sessions: [],
      notices: [],
    });
    const { container } = await show();
    expect(card("Codex")).toHaveTextContent("Subscription (ChatGPT sign-in)");
    for (const id of AI_TOOL_IDS) {
      const label = aiRuntime(id, "1").label;
      expect(card(label)).toHaveTextContent(`${label} always uses your subscription.`);
      expect(within(card(label)).queryByRole("group", { name: "Pay per use instead" })).toBeNull();
    }
    expect(screen.queryByRole("switch", { name: /Paid AI key/ })).toBeNull();
    expect(container.querySelector('input[type="password"]')).toBeNull();
    expect(api.setAiToolPayment).not.toHaveBeenCalled();
  });

  it("adds up the usage by model for today, this week, and last week; Kimi counts tasks only", async () => {
    // Wednesday, September 30, 2026, 10 AM: this week began Monday the 28th, last week the 21st.
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 8, 30, 10, 0));
    const day = (d: number) => new Date(2026, 8, d).getTime();
    const used = (
      model: string | null,
      tasks: number,
      read: number,
      reused: number,
      written: number,
    ): UsageModel => ({
      model,
      tasks,
      steps: tasks,
      countedSteps: tasks,
      read,
      reused,
      written,
    });
    const codex: Record<number, UsageModel[]> = {
      [day(30)]: [used("gpt-6-sol", 1, 1000, 400, 100)],
      [day(29)]: [used("gpt-6-sol", 2, 2000, 0, 200), used(null, 1, 500, 0, 50)],
      [day(24)]: [used("gpt-6-sol", 3, 5000, 1000, 500)],
      [day(18)]: [used("gpt-6-sol", 4, 9000, 0, 900)],
    };
    const kimi: Record<number, UsageModel[]> = {
      [day(30)]: [{ ...used(null, 2, 0, 0, 0), steps: 4, countedSteps: 0 }],
    };
    api.getAiToolUsage.mockImplementation((runtimeId, starts) => {
      const kept = runtimeId === "codex" ? codex : runtimeId === "kimi" ? kimi : {};
      return Promise.resolve({
        runtimeId,
        days: starts.slice(0, -1).map((start) => ({ start, models: kept[start] ?? [] })),
      });
    });
    await show();
    const user = userEvent.setup();
    // Asked for the 14 days from Thursday the 17th to tomorrow's midnight.
    const starts = api.getAiToolUsage.mock.calls.find(([id]) => id === "codex")![1];
    expect(starts).toHaveLength(15);
    expect(starts[0]).toBe(day(17));
    expect(starts.at(-1)).toBe(new Date(2026, 9, 1).getTime());
    const codexCard = card("Codex");
    // This week's tokens in one line, on the Overview.
    expect(codexCard).toHaveTextContent("3,500 read (400 reused) · 350 written · 4 tasks");
    await user.click(within(codexCard).getByRole("tab", { name: "Usage" }));
    expect(codexCard).toHaveTextContent(
      "Tokens are pieces of words: what the AI tool read and what it wrote.",
    );
    // Every model together, then each model.
    const inAll = within(codexCard).getByRole("list", { name: "Codex's usage in all" });
    expect(
      within(inAll)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual([
      "Today: 1,000 read (400 reused) · 100 written · 1 task",
      "This week: 3,500 read (400 reused) · 350 written · 4 tasks",
      "Last week: 5,000 read (1,000 reused) · 500 written · 3 tasks",
    ]);
    // What caching saved (Phase 25, item 4.1).
    const saved = within(codexCard).getByRole("list", { name: "What caching saved on Codex" });
    expect(
      within(saved)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual([
      "Today: 400 of 1,000 read came from the cache (40%)",
      "This week: 400 of 3,500 read came from the cache (11%)",
      "Last week: 1,000 of 5,000 read came from the cache (20%)",
    ]);
    const byModel = within(codexCard).getByRole("table", { name: "Codex's usage by model" });
    const row = (name: string) =>
      within(byModel).getByRole("rowheader", { name }).closest("tr")!.querySelectorAll("td");
    expect([...row("gpt-6-sol")].map((c) => c.textContent)).toEqual([
      "1,000 read (400 reused) · 100 written · 1 task",
      "3,000 read (400 reused) · 300 written · 3 tasks",
      "5,000 read (1,000 reused) · 500 written · 3 tasks",
    ]);
    expect([...row("The AI tool's default")].map((c) => c.textContent)).toEqual([
      "—",
      "500 read · 50 written · 1 task",
      "—",
    ]);
    const days = within(codexCard).getByRole("table", {
      name: "Codex's usage in the last 14 days",
    });
    expect(within(days).getAllByRole("row")).toHaveLength(15);
    const today = within(days).getByRole("rowheader", { name: "Today" }).closest("tr")!;
    expect([...today.querySelectorAll("td")].map((c) => c.textContent)).toEqual([
      "1,000",
      "400",
      "100",
      "1",
    ]);

    // Kimi reports no token counts: tasks only.
    const kimiCard = card("Kimi");
    expect(kimiCard).toHaveTextContent("2 tasks (Kimi doesn't report token counts)");
    await user.click(within(kimiCard).getByRole("tab", { name: "Usage" }));
    expect(
      within(kimiCard).getByText(
        "Kimi doesn't report token counts, so only its tasks are counted.",
      ),
    ).toBeVisible();
    const kimiRow = within(within(kimiCard).getByRole("table", { name: "Kimi's usage by model" }))
      .getByRole("rowheader", { name: "The AI tool's default" })
      .closest("tr")!;
    expect([...kimiRow.querySelectorAll("td")].map((c) => c.textContent)).toEqual([
      "2 tasks",
      "2 tasks",
      "—",
    ]);
    expect(
      within(within(kimiCard).getByRole("table", { name: /last 14 days/ })).queryByRole(
        "columnheader",
        { name: "Read" },
      ),
    ).toBeNull();
  });

  it("reads the usage again after midnight, so Today and This week move on", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date(2026, 8, 30, 23, 59, 50));
    await show();
    // The last day asked for ends at the next midnight.
    const ends = () =>
      api.getAiToolUsage.mock.calls.filter(([id]) => id === "codex").map(([, s]) => s.at(-1));
    expect(ends()).toEqual([new Date(2026, 9, 1).getTime()]);
    await act(() => vi.advanceTimersByTimeAsync(20_000));
    await waitFor(() => expect(ends()).toContain(new Date(2026, 9, 2).getTime()));
  });

  it("shows the usage limit with Try again now, and what is left of the plan where the tool reports it", async () => {
    const resets = T0 + 2 * 3_600_000;
    api.getRouting.mockResolvedValue(
      routing({
        codex: {
          available: false,
          usageLimit: {
            model: null,
            since: T0,
            resetsAt: resets,
            until: resets,
            detail: "You've hit your usage limit.",
          },
        },
      }),
    );
    api.clearUsageLimit.mockResolvedValue(routing());
    api.getAiTools.mockResolvedValue(
      page({
        "claude-code": {
          plan: {
            windows: [{ minutes: 300, usedPercent: 9, resetsAt: resets }],
            limited: false,
            warning: false,
            plan: null,
            reportedAt: T0,
          },
        },
        codex: {
          plan: {
            windows: [
              { minutes: 300, usedPercent: 25, resetsAt: resets },
              { minutes: 10080, usedPercent: 40, resetsAt: null },
            ],
            limited: false,
            warning: false,
            plan: "plus",
            reportedAt: T0,
          },
        },
      }),
    );
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    expect(codex).toHaveTextContent("Usage limit reached — resets at");
    expect(card("Claude Code")).toHaveTextContent("No usage limit reached");
    await user.click(within(codex).getByRole("button", { name: "Try Codex again now" }));
    expect(api.clearUsageLimit).toHaveBeenCalledWith("codex");
    await waitFor(() => expect(codex).toHaveTextContent("No usage limit reached"));
    // Left of your plan, as each tool reported it.
    const claudePlan = within(card("Claude Code")).getByRole("list", {
      name: "Left of your plan with Claude Code",
    });
    expect(claudePlan).toHaveTextContent("5-hour limit: 91% of your plan left · resets at");
    expect(card("Claude Code")).toHaveTextContent("Reported by Claude Code at");
    const codexPlan = within(codex).getByRole("list", { name: "Left of your plan with Codex" });
    expect(
      within(codexPlan)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual([
      expect.stringMatching(/^5-hour limit: 75% of your plan left · resets at /),
      "Weekly limit: 60% of your plan left",
    ]);
    expect(codex).toHaveTextContent("your plan: plus");
    expect(card("Grok")).toHaveTextContent("Grok doesn't report how much of your plan is left.");
    // A new report during a task: the page reads it again.
    api.getAiTools.mockResolvedValue(
      page({
        "claude-code": {
          plan: { windows: [], limited: true, warning: false, plan: null, reportedAt: T0 },
        },
      }),
    );
    agents({
      kind: "plan",
      runtimeId: "claude-code",
      report: { windows: [], limited: true, warning: false, plan: null, reportedAt: T0 },
    });
    expect(await within(card("Claude Code")).findByText("Limit reached")).toBeVisible();
    expect(card("Codex")).toHaveTextContent(
      "Codex hasn't reported it yet. Plenipo asks when it checks Codex.",
    );
    // Asked now with Check plan (Phase 25, item 1.2); Claude Code tells it only during a task.
    expect(
      within(card("Claude Code")).queryByRole("button", { name: "Check Claude Code's plan now" }),
    ).toBeNull();
    api.checkAiTool.mockResolvedValue(page());
    await userEvent
      .setup()
      .click(within(card("Codex")).getByRole("button", { name: "Check Codex's plan now" }));
    expect(api.checkAiTool).toHaveBeenCalledWith("codex");
  });

  it("says Limit reached once for a whole report, and keeps each window's share left", async () => {
    api.getAiTools.mockResolvedValue(
      page({
        // Codex says "limited" once, for all its windows: the weekly one is used up.
        codex: {
          plan: {
            windows: [
              { minutes: 300, usedPercent: 20, resetsAt: null },
              { minutes: 10080, usedPercent: 100, resetsAt: null },
            ],
            limited: true,
            warning: false,
            plan: "plus",
            reportedAt: T0,
          },
        },
        // Limited, with no window used up: said once, above the windows.
        "claude-code": {
          plan: {
            windows: [{ minutes: 300, usedPercent: 95, resetsAt: null }],
            limited: true,
            warning: false,
            plan: null,
            reportedAt: T0,
          },
        },
      }),
    );
    await show();
    const codex = card("Codex");
    const codexPlan = within(codex).getByRole("list", { name: "Left of your plan with Codex" });
    expect(
      within(codexPlan)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual(["5-hour limit: 80% of your plan left", "Weekly limit: Limit reached"]);
    expect(within(codex).getAllByText(/Limit reached/)).toHaveLength(1);
    const claude = card("Claude Code");
    expect(within(claude).getAllByText(/Limit reached/)).toHaveLength(1);
    expect(claude).toHaveTextContent("Limit reached5-hour limit: 5% of your plan left");
  });

  it("says where each tool's plan left comes from before it has reported any", async () => {
    await show();
    expect(card("Claude Code")).toHaveTextContent(
      "Claude Code reports this during a task; nothing reported yet.",
    );
    expect(card("Codex")).toHaveTextContent(
      "Codex hasn't reported it yet. Plenipo asks when it checks Codex.",
    );
  });

  it("marks new models and models this version no longer offers; Claude Code's come with Plenipo", async () => {
    const terra = { name: "gpt-6-terra", label: "GPT-6-Terra", effortLevels: [] };
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        codex: {
          reportedModels: {
            models: [
              { name: "gpt-6-sol", label: "GPT-6-Sol", effortLevels: ["low", "high"] },
              terra,
            ],
            complete: true,
            checkedAt: T0,
          },
        },
      }),
      sessions: [],
      notices: [],
    });
    api.getRouting.mockResolvedValue(
      routing({ codex: { newModels: [terra], unlistedModels: ["gpt-6-luna"] } }),
    );
    api.checkAiTool.mockResolvedValue(page());
    await show();
    const user = userEvent.setup();
    const codex = card("Codex");
    await user.click(within(codex).getByRole("tab", { name: "Models" }));
    const models = within(codex).getByRole("list", { name: "Codex's models" });
    const item = (label: string) =>
      within(models)
        .getAllByRole("listitem")
        .find((i) => i.textContent?.startsWith(label))!;
    expect(item("GPT-6-Sol")).toHaveTextContent(
      "GPT-6-Sol gpt-6-sol · made by OpenAI · Effort: Low, Medium, High, Extra high, Max, Ultra",
    );
    expect(item("GPT-6-Sol")).not.toHaveTextContent("not offered");
    expect(item("GPT-6-Luna")).toHaveTextContent("not offered by this version");
    expect(item("GPT-6-Terra")).toHaveTextContent("new — not checked yet");
    expect(codex).toHaveTextContent("Plenipo last asked Codex at");
    await user.click(within(codex).getByRole("button", { name: "Check Codex again" }));
    expect(api.checkAiTool).toHaveBeenCalledWith("codex");

    const claude = card("Claude Code");
    await user.click(within(claude).getByRole("tab", { name: "Models" }));
    expect(
      within(claude).getByText(
        "Claude Code has no list of its own. Its models come with Plenipo's updates.",
      ),
    ).toBeVisible();
    const kimi = card("Kimi");
    await user.click(within(kimi).getByRole("tab", { name: "Models" }));
    expect(kimi).toHaveTextContent(
      "Asking Kimi for its models leaves an empty conversation in Kimi's own history, so Plenipo asks only after an update and when you press Check again.",
    );
  });
  it("names the exact model a short name points to now, and who made each of Ollama's models (ADR-081)", async () => {
    const anthropic = { id: "anthropic", label: "Anthropic" };
    const deepseek = { id: "deepseek", label: "DeepSeek" };
    const claudeCaps = aiRuntime("claude-code", "2.1.283").capabilities;
    const ollamaCaps = aiRuntime("ollama", "0.34.4").capabilities;
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        "claude-code": {
          capabilities: {
            ...claudeCaps,
            knownModels: [
              {
                name: "opus",
                label: "Opus",
                effortLevels: ["low", "high"],
                maker: anthropic,
                pointsTo: "claude-opus-5-5",
              },
              {
                name: "claude-opus-5-5",
                label: "Opus 5.5",
                effortLevels: ["low", "high"],
                maker: anthropic,
              },
            ],
          },
        },
        ollama: {
          capabilities: {
            ...ollamaCaps,
            knownModels: [
              {
                name: "deepseek-v4-pro:cloud",
                label: "DeepSeek V4 Pro",
                effortLevels: [],
                maker: deepseek,
              },
            ],
          },
        },
      }),
      sessions: [],
      notices: [],
    });
    api.getRouting.mockResolvedValue(
      routing({
        ollama: {
          newModels: [{ name: "mystery:cloud", label: "mystery:cloud", effortLevels: [] }],
        },
      }),
    );
    await show();
    const user = userEvent.setup();
    const claude = card("Claude Code");
    await user.click(within(claude).getByRole("tab", { name: "Models" }));
    const claudeModels = within(claude).getByRole("list", { name: "Claude Code's models" });
    expect(
      within(claudeModels)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual([
      // Each says who made it; a short name also says what it is now.
      "Opus opus · now Opus 5.5 · made by Anthropic · Effort: Low, High",
      "Opus 5.5 claude-opus-5-5 · made by Anthropic · Effort: Low, High",
    ]);
    const ollama = card("Ollama");
    await user.click(within(ollama).getByRole("tab", { name: "Models" }));
    const ollamaModels = within(ollama).getByRole("list", { name: "Ollama's models" });
    expect(
      within(ollamaModels)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual([
      "DeepSeek V4 Pro deepseek-v4-pro:cloud · made by DeepSeek · No effort setting",
      "mystery:cloud · who made it is not known · No effort setting new — not checked yet",
    ]);
  });
});

describe("the AI tools page: GitHub Copilot (ADR-083)", () => {
  it("signs in with copilot login, has no sign-out, and says why paid extra use stops it", async () => {
    const why =
      "GitHub may charge for extra use once your Copilot allowance runs out. Plenipo never lets a task cost money, so no task runs. On github.com, open Settings → Billing and licensing → Budgets and alerts, set the budget for AI Credits to $0 with Stop usage on, then check GitHub Copilot again in Plenipo.";
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        copilot: {
          auth: { state: "unverified", method: "GitHub CLI sign-in", detail: why },
          ready: false,
        },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const copilot = card("GitHub Copilot");
    expect(copilot).toHaveTextContent("Signed in (billing unverified) · GitHub CLI sign-in");
    // The check's reason, with what to do, is on the card.
    expect(within(copilot).getByText(why)).toBeVisible();
    expect(within(copilot).getByRole("button", { name: "Reconnect GitHub Copilot" })).toBeEnabled();
    expect(copilot).toHaveTextContent("runs copilot login");
    expect(within(copilot).getByText("GitHub Copilot has no sign-out command.")).toBeVisible();
    expect(
      within(copilot).queryByRole("button", { name: "Sign out of GitHub Copilot" }),
    ).toBeNull();
  });

  it("shows the check's reason only when the sign-in check is why the tool can't take work", async () => {
    const why = (text: string) => ({
      auth: { state: "unverified" as const, method: null, detail: text },
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        // Ready (checked per task): its reason is not why it can't work, so it isn't shown.
        "claude-code": { ...why("Each turn is checked before it runs."), ready: true },
        // Given no tasks for now: the reason is its version's, shown under Version.
        codex: {
          ...why("Codex did not report its sign-in status."),
          ready: false,
          installation: {
            state: "installed",
            executable: "/bin/codex",
            version: "0.50.0",
            detail: "Its update left it not answering.",
          },
        },
        // Not installed: nothing to sign in to.
        grok: {
          ...why("The sign-in check timed out."),
          ready: false,
          installation: { state: "notInstalled", executable: null, version: null, detail: null },
        },
        // Not ready because of its sign-in check: shown.
        kimi: { ...why("Kimi's plan does not say it is yours."), ready: false },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    for (const label of ["Claude Code", "Codex", "Grok"]) {
      expect(card(label).querySelector(".ai-tool__why"), label).toBeNull();
    }
    expect(card("Kimi").querySelector(".ai-tool__why")).toHaveTextContent(
      "Kimi's plan does not say it is yours.",
    );
  });

  it("always uses its subscription like every subscription tool, and plan left from its own check", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        copilot: { auth: { state: "subscription", method: "Copilot sign-in", detail: null } },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const copilot = card("GitHub Copilot");
    expect(copilot).toHaveTextContent("Subscription (Copilot sign-in)");
    expect(copilot).toHaveTextContent("GitHub Copilot always uses your subscription.");
    expect(copilot.querySelector('input[type="password"]')).toBeNull();
    expect(copilot.querySelector(".ai-tool__why")).toBeNull();
    expect(copilot).toHaveTextContent(
      "GitHub Copilot hasn't reported it yet. Plenipo asks when it checks GitHub Copilot.",
    );
  });

  it("lists the models it reports, each new and with who made it not known", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        copilot: {
          reportedModels: {
            models: [{ name: "auto", label: "Auto", effortLevels: [] }],
            complete: true,
            checkedAt: T0,
          },
        },
      }),
      sessions: [],
      notices: [],
    });
    api.getRouting.mockResolvedValue(
      routing({ copilot: { newModels: [{ name: "auto", label: "Auto", effortLevels: [] }] } }),
    );
    await show();
    const user = userEvent.setup();
    const copilot = card("GitHub Copilot");
    await user.click(within(copilot).getByRole("tab", { name: "Models" }));
    const models = within(copilot).getByRole("list", { name: "GitHub Copilot's models" });
    expect(
      within(models)
        .getAllByRole("listitem")
        .map((i) => i.textContent),
    ).toEqual(["Auto auto · who made it is not known · No effort setting new — not checked yet"]);
  });
});

describe("the AI tools page: a paid AI tool's key (Phase 16 Wave 3, ADR-085)", () => {
  /** A made-up key: never a real one in the tests. */
  const KEY = "sk-or-v1-made-up-for-the-tests-0123456789";
  const SAVED = {
    id: "paid-key-0123",
    runtimeId: "openrouter",
    name: "Office key",
    createdAt: T0,
    updatedAt: T0,
  };
  const noKey = "No paid key is saved for this AI tool: add one on its card on the AI tools page.";

  /** The seven AI tools and OpenRouter, with its check and page part. */
  function withOpenRouter(info: Partial<AgentRuntimeInfo> = {}, tool: Partial<AiToolState> = {}) {
    const openRouter = aiRuntime("openrouter", "1.16.0", {
      ready: false,
      auth: { state: "signedOut", method: null, detail: noKey },
      ...info,
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [...runtimes(), openRouter],
      sessions: [],
      notices: [],
    });
    const tools = [...page().tools, aiTool("openrouter", tool)];
    api.getAiTools.mockResolvedValue(aiPage(tools));
    api.getRouting.mockResolvedValue({
      ...routing(),
      tools: [...routing().tools, route("openrouter", { auth: "signedOut" })],
    });
    return tools;
  }

  it("with no key: it comes with Plenipo, has no plan, and asks for a key", async () => {
    withOpenRouter();
    const { container } = await show();
    const openRouter = card("OpenRouter");
    expect(openRouter).toHaveTextContent("AI company: OpenRouter");
    // The card's pill and its key check.
    expect(within(openRouter).getAllByText("No key yet")).toHaveLength(2);
    expect(openRouter).toHaveTextContent("Key check");
    expect(openRouter).toHaveTextContent(noKey);
    expect(openRouter).toHaveTextContent("Comes with Plenipo 1.16.0");
    expect(openRouter).toHaveTextContent(
      "OpenRouter comes with Plenipo: it is updated when Plenipo is.",
    );
    expect(openRouter).toHaveTextContent(
      "Paid per use with your key. A worker on OpenRouter answers in text only.",
    );
    expect(openRouter).toHaveTextContent(
      "No plan: OpenRouter is paid per use. A spending limit is up to you, in Spending caps.",
    );
    // What is not checked yet, and where to make a key (ADR-087).
    expect(openRouter).toHaveTextContent("Plenipo has not checked OpenRouter with a real key yet.");
    expect(openRouter).toHaveTextContent("make a key at openrouter.ai → Keys");
    // Nothing to sign in to, and no update button of its own.
    expect(within(openRouter).queryByRole("button", { name: /Sign in|Update/ })).toBeNull();
    const form = within(openRouter).getByRole("form", { name: "Add a key for OpenRouter" });
    expect(within(form).getByLabelText("Name for the key")).toHaveValue("OpenRouter key");
    expect(within(form).getByLabelText("OpenRouter key")).toHaveAttribute("type", "password");
    expect(within(form).getByRole("button", { name: "Save and check" })).toBeDisabled();
    // A spending limit is up to you: no cap is needed to add a key (2026-09-30).
    expect(form).toHaveTextContent(
      "You can set a spending limit in Settings → Spending caps if you want; it is not required.",
    );
    expect(a11yProblems(container)).toEqual([]);
  });

  it("Save and check sends the key once, then the card shows its name and never the key", async () => {
    const tools = withOpenRouter();
    api.savePaidKey.mockResolvedValue(
      aiPage(tools.map((t) => (t.runtimeId === "openrouter" ? { ...t, paidKey: SAVED } : t))),
    );
    await show();
    const user = userEvent.setup();
    const openRouter = card("OpenRouter");
    const form = within(openRouter).getByRole("form", { name: "Add a key for OpenRouter" });
    const name = within(form).getByLabelText("Name for the key");
    await user.clear(name);
    await user.type(name, "Office key");
    await user.type(within(form).getByLabelText("OpenRouter key"), KEY);
    await user.click(within(form).getByRole("button", { name: "Save and check" }));
    await waitFor(() => expect(openRouter).toHaveTextContent("Key saved: Office key"));
    expect(openRouter).toHaveTextContent(
      "You can set a spending limit in Settings → Spending caps if you want; it is not required.",
    );
    expect(api.savePaidKey).toHaveBeenCalledTimes(1);
    expect(api.savePaidKey).toHaveBeenCalledWith("openrouter", "Office key", KEY);
    expect(openRouter.querySelector('input[type="password"]')).toBeNull();
    expect(screen.queryByDisplayValue(KEY)).toBeNull();
    expect(
      within(openRouter).getByRole("button", { name: "Replace key for OpenRouter" }),
    ).toBeEnabled();
  });

  it("a refused key leaves the form, and the refusal says why", async () => {
    withOpenRouter();
    api.savePaidKey.mockRejectedValue({
      kind: "invalidInput",
      message: "OpenRouter did not accept the key: check it and try again.",
    });
    await show();
    const user = userEvent.setup();
    const openRouter = card("OpenRouter");
    const key = within(openRouter).getByLabelText("OpenRouter key");
    await user.type(key, KEY);
    await user.click(within(openRouter).getByRole("button", { name: "Save and check" }));
    expect(await within(openRouter).findByRole("alert")).toHaveTextContent(
      "OpenRouter did not accept the key: check it and try again.",
    );
    expect(key).toHaveValue("");
    expect(screen.queryByDisplayValue(KEY)).toBeNull();
  });

  it("switched off: the notice says why once, opens Switches, and locks the form", async () => {
    const off =
      "Paid AI keys are switched off: turn on Settings → Switches → Let workers use paid AI keys.";
    withOpenRouter(
      { auth: { state: "signedOut", method: null, detail: off } },
      { paidBlocked: off },
    );
    await show();
    const user = userEvent.setup();
    const openRouter = card("OpenRouter");
    expect(within(openRouter).getAllByText(off)).toHaveLength(1);
    expect(within(openRouter).getByLabelText("OpenRouter key")).toBeDisabled();
    await user.click(within(openRouter).getByRole("button", { name: "Open Switches" }));
    expect(go).toHaveBeenLastCalledWith({ view: "settings", id: "switches" });
    // The spending limit's own way there, beside the form.
    await user.click(within(openRouter).getByRole("button", { name: "Open Spending caps" }));
    expect(go).toHaveBeenLastCalledWith({ view: "settings", id: "spending" });
    expect(api.savePaidKey).not.toHaveBeenCalled();
  });

  it("the paid AI tools have their own part of the page, after the ones you sign in to", async () => {
    withOpenRouter();
    await show();
    const user = userEvent.setup();
    const signedIn = screen.getByRole("list", { name: "AI tools" });
    const paid = screen.getByRole("list", { name: "AI tools paid per use" });
    expect(within(signedIn).queryByRole("listitem", { name: "OpenRouter AI tool" })).toBeNull();
    expect(within(paid).getByRole("listitem", { name: "OpenRouter AI tool" })).toBeVisible();
    expect(within(paid).queryByRole("listitem", { name: "Codex AI tool" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Paid per use with your key" })).toBeVisible();
    const scroll = vi.fn();
    const heading = document.getElementById("paid-ai-tools")!;
    heading.scrollIntoView = scroll;
    await user.click(screen.getByRole("button", { name: "Go to the AI tools paid per use" }));
    expect(scroll).toHaveBeenCalledTimes(1);
    // The keyboard goes there too.
    expect(heading).toHaveFocus();
  });

  it("starts each card closed with its light, one line, and Sign in or Reconnect; one that needs you opens (Phase 25, item 2.1)", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        grok: { ready: false, auth: { state: "signedOut", method: null, detail: null } },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const kimi = cardAsIs("Kimi");
    const toggle = within(kimi).getByRole("button", { name: "Kimi" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(kimi).toHaveTextContent("Subscription connected");
    expect(kimi).toHaveTextContent("AI company: Moonshot AI · Kimi subscription");
    expect(within(kimi).queryByRole("tab", { name: "Overview" })).toBeNull();
    // Closed, it still offers Reconnect; open, the button inside does (only one of them shows).
    expect(within(kimi).getByRole("button", { name: "Reconnect Kimi" })).toBeVisible();
    fireEvent.click(toggle);
    expect(within(kimi).getByRole("tab", { name: "Overview" })).toBeVisible();
    expect(within(kimi).getAllByRole("button", { name: "Reconnect Kimi" })).toHaveLength(1);
    // Grok isn't signed in: its card opened by itself, with Sign in.
    const grok = cardAsIs("Grok");
    expect(within(grok).getByRole("button", { name: "Grok" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(within(grok).getAllByRole("button", { name: "Sign in to Grok" })).toHaveLength(1);
    // Antigravity is fine: closed.
    expect(
      within(cardAsIs("Antigravity")).getByRole("button", { name: "Antigravity" }),
    ).toHaveAttribute("aria-expanded", "false");
  });

  it("a link to a card opens it; a link to a company's key opens its subscription card (Phase 25, item 2.1)", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: [
        ...runtimes(),
        aiRuntime("anthropic-key", "1.17.0", {
          ready: false,
          auth: { state: "signedOut", method: null, detail: null },
        }),
      ],
      sessions: [],
      notices: [],
    });
    api.getAiTools.mockResolvedValue(aiPage([...page().tools, aiTool("anthropic-key")]));
    await show("anthropic-key");
    const claude = cardAsIs("Claude Code");
    await waitFor(() =>
      expect(within(claude).getByRole("button", { name: "Claude Code" })).toHaveAttribute(
        "aria-expanded",
        "true",
      ),
    );
    expect(screen.queryByRole("listitem", { name: "Anthropic AI tool" })).toBeNull();
    expect(within(cardAsIs("Kimi")).getByRole("button", { name: "Kimi" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("a subscription card is green when its subscription or its key works, and says which (Phase 25, item 1.3)", async () => {
    const keyReady = aiRuntime("anthropic-key", "1.17.0", {
      ready: true,
      auth: { state: "paidKey", method: "Your key", detail: null },
    });
    const keyTool = aiTool("anthropic-key", { paidKey: { ...SAVED, runtimeId: "anthropic-key" } });
    // The subscription ended, the key works: still green.
    api.getAgentOverview.mockResolvedValue({
      runtimes: [
        ...runtimes({
          "claude-code": {
            ready: false,
            auth: { state: "signedOut", method: null, detail: null },
          },
        }),
        keyReady,
      ],
      sessions: [],
      notices: [],
    });
    api.getAiTools.mockResolvedValue(aiPage([...page().tools, keyTool]));
    const view = await show();
    await waitFor(() => expect(card("Claude Code")).toHaveTextContent("API key connected"));
    view.unmount();
    // Both work.
    api.getAgentOverview.mockResolvedValue({
      runtimes: [...runtimes(), keyReady],
      sessions: [],
      notices: [],
    });
    const both = await show();
    await waitFor(() =>
      expect(card("Claude Code")).toHaveTextContent("Subscription and API key connected"),
    );
    // Codex has no key here: its subscription alone.
    expect(card("Codex")).toHaveTextContent("Subscription connected");
    both.unmount();
  });

  it("a card you sign in to has a key box that saves its AI company's own key", async () => {
    const anthropic = aiRuntime("anthropic-key", "1.17.0", {
      ready: false,
      auth: { state: "signedOut", method: null, detail: noKey },
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [...runtimes(), anthropic],
      sessions: [],
      notices: [],
    });
    const tools = [...page().tools, aiTool("anthropic-key")];
    api.getAiTools.mockResolvedValue(aiPage(tools));
    const saved = { ...SAVED, runtimeId: "anthropic-key", name: "Anthropic key" };
    api.savePaidKey.mockResolvedValue(
      aiPage(tools.map((t) => (t.runtimeId === "anthropic-key" ? { ...t, paidKey: saved } : t))),
    );
    await show();
    const user = userEvent.setup();
    const claude = card("Claude Code");
    expect(claude).toHaveTextContent("Claude Code always uses your subscription.");
    const box = within(claude).getByRole("group", { name: "Pay per use instead" });
    expect(box).toHaveTextContent("Your Anthropic key goes here.");
    // The Anthropic key's own card is folded into Claude Code's (Phase 25, item 2.1).
    expect(box).not.toHaveTextContent("under Paid per use with your key");
    expect(screen.queryByRole("listitem", { name: "Anthropic AI tool" })).toBeNull();
    expect(box).toHaveTextContent("Claude Code itself keeps using your subscription.");
    expect(box).toHaveTextContent("it is not required");
    const form = within(box).getByRole("form", {
      name: "Add a key for Anthropic on Claude Code's card",
    });
    await user.type(within(form).getByLabelText("Anthropic key"), KEY);
    await user.click(within(form).getByRole("button", { name: "Save and check" }));
    // The Anthropic AI tool's key, once: the same one its own card shows. Claude Code never
    // gets it.
    expect(api.savePaidKey).toHaveBeenCalledTimes(1);
    expect(api.savePaidKey).toHaveBeenCalledWith("anthropic-key", "Anthropic key", KEY);
    await waitFor(() => expect(box).toHaveTextContent("Key saved: Anthropic key"));
    // The closed card's line names the saved key.
    expect(claude).toHaveTextContent("Your Anthropic key: Anthropic key");
    expect(screen.queryByDisplayValue(KEY)).toBeNull();
    // Codex's AI company has no paid AI tool here: no key box.
    expect(within(card("Codex")).queryByRole("group", { name: "Pay per use instead" })).toBeNull();
  });

  it("Ollama and GitHub Copilot offer an OpenRouter key, which reaches the same kinds of models", async () => {
    const tools = withOpenRouter();
    api.savePaidKey.mockResolvedValue(
      aiPage(tools.map((t) => (t.runtimeId === "openrouter" ? { ...t, paidKey: SAVED } : t))),
    );
    await show();
    const user = userEvent.setup();
    for (const label of ["Ollama", "GitHub Copilot"]) {
      const box = within(card(label)).getByRole("group", { name: "Pay per use instead" });
      expect(box).toHaveTextContent(
        `${label} has no key of its own for paying per use; an OpenRouter key reaches the same kinds of models, and many more.`,
      );
      expect(
        within(box).getByRole("form", { name: `Add a key for OpenRouter on ${label}'s card` }),
      ).toBeVisible();
    }
    // Each form has its own name: the three OpenRouter forms are told apart.
    card("OpenRouter");
    expect(screen.getAllByRole("form", { name: /^Add a key for OpenRouter/ })).toHaveLength(3);
    const box = within(card("Ollama")).getByRole("group", { name: "Pay per use instead" });
    await user.type(within(box).getByLabelText("OpenRouter key"), KEY);
    await user.click(within(box).getByRole("button", { name: "Save and check" }));
    expect(api.savePaidKey).toHaveBeenCalledTimes(1);
    expect(api.savePaidKey).toHaveBeenCalledWith("openrouter", "OpenRouter key", KEY);
    await waitFor(() => expect(card("OpenRouter")).toHaveTextContent("Key saved: Office key"));
    expect(card("GitHub Copilot")).toHaveTextContent("Key saved: Office key");
  });

  it("a saved key works: Replace opens the form, and Remove asks first", async () => {
    const tools = withOpenRouter(
      { ready: true, auth: { state: "paidKey", method: null, detail: null }, checkedAt: T0 },
      { paidKey: SAVED },
    );
    api.removePaidKey.mockResolvedValue(aiPage(tools.map((t) => ({ ...t, paidKey: null }))));
    await show();
    const user = userEvent.setup();
    const openRouter = card("OpenRouter");
    expect(openRouter).toHaveTextContent("Ready");
    expect(openRouter).toHaveTextContent("Your key works");
    expect(openRouter).toHaveTextContent("Key saved: Office key");
    await user.click(
      within(openRouter).getByRole("button", { name: "Replace key for OpenRouter" }),
    );
    const form = within(openRouter).getByRole("form", { name: "Replace key for OpenRouter" });
    expect(within(form).getByLabelText("Name for the key")).toHaveValue("Office key");
    expect(within(form).getByLabelText("OpenRouter key")).toHaveFocus();
    await user.click(within(form).getByRole("button", { name: "Cancel" }));
    // Keep it: nothing is removed.
    await user.click(within(openRouter).getByRole("button", { name: "Remove key for OpenRouter" }));
    await user.click(within(openRouter).getByRole("button", { name: "Keep it" }));
    expect(within(openRouter).queryByRole("group", { name: "Remove Office key?" })).toBeNull();
    // A refused removal says why, and the key stays.
    api.removePaidKey.mockRejectedValueOnce({
      kind: "internal",
      message: "Windows Credential Manager could not be reached.",
    });
    await user.click(within(openRouter).getByRole("button", { name: "Remove key for OpenRouter" }));
    await user.click(
      within(within(openRouter).getByRole("group", { name: "Remove Office key?" })).getByRole(
        "button",
        { name: "Remove key" },
      ),
    );
    expect(await within(openRouter).findByRole("alert")).toHaveTextContent(
      "Windows Credential Manager could not be reached.",
    );
    expect(openRouter).toHaveTextContent("Key saved: Office key");
    await user.click(within(openRouter).getByRole("button", { name: "Keep it" }));
    expect(within(openRouter).queryByRole("alert")).toBeNull();
    // Removed: the form for a new key, with no trace of the old one's name.
    await user.click(within(openRouter).getByRole("button", { name: "Remove key for OpenRouter" }));
    expect(api.removePaidKey).toHaveBeenCalledTimes(1);
    const ask = within(openRouter).getByRole("group", { name: "Remove Office key?" });
    await user.click(within(ask).getByRole("button", { name: "Remove key" }));
    expect(api.removePaidKey).toHaveBeenLastCalledWith("openrouter");
    const fresh = await within(openRouter).findByRole("form", { name: "Add a key for OpenRouter" });
    expect(within(fresh).getByLabelText("Name for the key")).toHaveValue("OpenRouter key");
  });

  it("paid keys switched off since the last check: the card no longer says the key works", async () => {
    const off =
      "Paid AI keys are switched off: turn on Settings → Switches → Let workers use paid AI keys.";
    withOpenRouter(
      { ready: true, auth: { state: "paidKey", method: null, detail: null }, checkedAt: T0 },
      { paidKey: SAVED, paidBlocked: off },
    );
    await show();
    const openRouter = card("OpenRouter");
    expect(openRouter).not.toHaveTextContent("Your key works");
    expect(within(openRouter).getAllByText("Key not in use")).toHaveLength(2);
    expect(within(openRouter).getAllByText(off)).toHaveLength(1);
  });

  it("the same model on other AI tools says where else it runs (ADR-036 §4)", async () => {
    const k3 = (name: string, label: string) => ({
      name,
      label,
      effortLevels: [],
      maker: { id: "moonshot", label: "Moonshot AI" },
      same: "kimi-k3",
    });
    withOpenRouter({
      capabilities: {
        ...aiRuntime("openrouter", "1.16.0").capabilities,
        knownModels: [k3("moonshotai/kimi-k3", "Kimi K3")],
      },
    });
    const others = runtimes({
      kimi: {
        capabilities: {
          ...aiRuntime("kimi", "0.34.0").capabilities,
          knownModels: [k3("kimi-code/k3", "K3")],
        },
      },
      ollama: {
        capabilities: {
          ...aiRuntime("ollama", "0.34.4").capabilities,
          knownModels: [k3("kimi-k3:cloud", "Kimi K3 (paid plan)")],
        },
      },
    });
    const openRouter = (await api.getAgentOverview()).runtimes.at(-1)!;
    api.getAgentOverview.mockResolvedValue({
      runtimes: [...others, openRouter],
      sessions: [],
      notices: [],
    });
    await show();
    const user = userEvent.setup();
    await user.click(within(card("Kimi")).getByRole("tab", { name: "Models" }));
    expect(within(card("Kimi")).getByRole("list", { name: "Kimi's models" })).toHaveTextContent(
      "also on Ollama, OpenRouter",
    );
    await user.click(within(card("OpenRouter")).getByRole("tab", { name: "Models" }));
    expect(
      within(card("OpenRouter")).getByRole("list", { name: "OpenRouter's models" }),
    ).toHaveTextContent("also on Kimi, Ollama");
  });

  it("each priced model says what it costs", async () => {
    withOpenRouter();
    await show();
    const user = userEvent.setup();
    const openRouter = card("OpenRouter");
    await user.click(within(openRouter).getByRole("tab", { name: "Models" }));
    const models = within(openRouter).getByRole("list", { name: "OpenRouter's models" });
    expect(models).toHaveTextContent("$3.00 a million tokens read, $15.00 a million written");
  });
});

describe("the AI tools page: accessibility smoke", () => {
  it("names every control, with one main heading and no skipped level", async () => {
    const { container } = await show();
    const user = userEvent.setup();
    expect(a11yProblems(container)).toEqual([]);
    for (const label of ["Codex", "Kimi"]) {
      await user.click(within(card(label)).getByRole("tab", { name: "Usage" }));
    }
    await user.click(within(card("Grok")).getByRole("tab", { name: "Models" }));
    await waitFor(() => expect(container.querySelector('[role="status"][aria-busy]')).toBeNull());
    expect(a11yProblems(container)).toEqual([]);
  });
});

describe("left of your plan, in words", () => {
  it("names a window by its length, and says nothing twice when the tool gave none", () => {
    expect(planWindowName(300)).toBe("5-hour limit");
    expect(planWindowName(1440)).toBe("Daily limit");
    expect(planWindowName(10080)).toBe("Weekly limit");
    expect(planWindowName(4320)).toBe("3-day limit");
    expect(planWindowName(90)).toBe("90-minute limit");
    // Claude Code does not say how long its window is: "91% of your plan left", never
    // "Your plan: 91% of your plan left".
    expect(planWindowName(null)).toBeNull();
    const ok = { limited: false, warning: false };
    expect(planLeft({ minutes: null, usedPercent: 9, resetsAt: null }, ok)).toBe(
      "91% of your plan left",
    );
  });

  it("keeps a window's share left when the report says limited; a used-up window says Limit reached", () => {
    const limited = { limited: true, warning: false };
    expect(planLeft({ minutes: 300, usedPercent: 20, resetsAt: null }, limited)).toBe(
      "80% of your plan left",
    );
    expect(planLeft({ minutes: 10080, usedPercent: 100, resetsAt: null }, limited)).toBe(
      "Limit reached",
    );
    expect(planLeft({ minutes: null, usedPercent: null, resetsAt: null }, limited)).toBe(
      "Limit reached",
    );
  });

  it("takes a refusal's first sentence", () => {
    expect(firstSentence("Codex is being updated. Plenipo waits until it's done.")).toBe(
      "Codex is being updated",
    );
    expect(firstSentence("Codex 1.0.4 is busy")).toBe("Codex 1.0.4 is busy");
  });
});
