import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type {
  AgentRuntimeInfo,
  AgentUpdate,
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
import { aiPage, aiRuntime, aiTool, idle, route, T0 } from "../../test/aiToolFixtures";
import { sampleRouting } from "../../test/routingFixtures";
import { RuntimesView } from "../../views/RuntimesView";

// xterm.js draws on a real screen; a stand-in records what it is given.
const xterm = vi.hoisted(() => {
  const made: { written: (string | Uint8Array)[]; type: (data: string) => void }[] = [];
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
      this.record = { written: [], type: (d: string) => this.handlers.forEach((h) => h(d)) };
      xterm.made.push(this.record);
    }
    loadAddon() {}
    open() {}
    focus() {}
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

/** The five AI tools, as their checks say. */
function runtimes(patch: Record<string, Partial<AgentRuntimeInfo>> = {}): AgentRuntimeInfo[] {
  return [
    aiRuntime("claude-code", "2.1.283", patch["claude-code"]),
    aiRuntime("codex", "0.50.0", patch.codex),
    aiRuntime("grok", "1.0.41", patch.grok),
    aiRuntime("kimi", "0.34.0", patch.kimi),
    aiRuntime("ollama", "0.34.4", patch.ollama),
  ];
}

function page(patch: Record<string, Parameters<typeof aiTool>[1]> = {}): AiToolsPage {
  return aiPage(
    ["claude-code", "codex", "grok", "kimi", "ollama"].map((id) => aiTool(id, patch[id])),
  );
}

function routing(patch: Record<string, Parameters<typeof route>[1]> = {}) {
  return {
    ...sampleRouting(),
    tools: ["claude-code", "codex", "grok", "kimi", "ollama"].map((id) => route(id, patch[id])),
  };
}

function Page({ toolId = null }: { toolId?: string | null }) {
  return (
    <RuntimeProvider>
      <AgentsProvider>
        <TerminalProvider>
          <RuntimesView selectedId={null} onSelect={() => undefined} toolId={toolId} />
          <TerminalPanel theme="dark" />
        </TerminalProvider>
      </AgentsProvider>
    </RuntimeProvider>
  );
}

/** One AI tool's card. */
const card = (label: string) => screen.getByRole("listitem", { name: `${label} AI tool` });

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
  it("the paid-key switch is locked, and Plenipo never asks for a paid key", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: runtimes({
        codex: { auth: { state: "subscription", method: "ChatGPT sign-in", detail: null } },
      }),
      sessions: [],
      notices: [],
    });
    await show();
    const user = userEvent.setup();
    expect(card("Codex")).toHaveTextContent("Subscription (ChatGPT sign-in)");
    const switches = screen.getAllByRole("switch", { name: "Paid AI key (pay per use)" });
    expect(switches).toHaveLength(5);
    for (const s of switches) {
      expect(s).toBeDisabled();
      expect(s).toHaveAttribute("aria-checked", "false");
      expect(s).toHaveAccessibleDescription("Comes with spending caps in a later version.");
      await user.click(s);
    }
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
      "Codex reports this during a task; nothing reported yet.",
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
      "GPT-6-Sol gpt-6-sol · Effort: Low, Medium, High, Extra high, Max, Ultra",
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
