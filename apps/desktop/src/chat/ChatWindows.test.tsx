import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useEffect } from "react";
import type { AgentSessionDetail, AgentTurn, AgentUpdate, PopOutNotice } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import * as commands from "../api/commands";
import { activity, runtime, session, turn } from "../test/agentFixtures";
import { useWorkspace } from "../workspace/context";
import { WorkspaceProvider } from "../workspace/WorkspaceProvider";
import { ChatPanel } from "./ChatPanel";
import { ChatProvider } from "./ChatProvider";
import { ChatWindows } from "./ChatWindows";
import { useChat, type ChatApi } from "./context";
import { TABS_KEY, type ChatTarget } from "./tabs";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    giveObjective: vi.fn(),
    resumeAgentSession: vi.fn(),
    cancelAgentTurn: vi.fn(),
    getWorkFolder: vi.fn(),
    openWorkFolder: vi.fn(),
    getTaskHandoffs: vi.fn(),
    getChainOrders: vi.fn(),
    preparePopOut: vi.fn(),
    focusPopOut: vi.fn(),
    closePopOut: vi.fn(),
    resetPopOuts: vi.fn(),
  };
});

const hub = vi.hoisted(() => ({
  handlers: [] as ((u: unknown) => void)[],
  windows: null as ((n: unknown) => void) | null,
}));
vi.mock("../api/events", () => ({
  subscribeAgentUpdates: vi.fn((handler: (u: unknown) => void) => {
    hub.handlers.push(handler);
    return Promise.resolve(() => {
      hub.handlers = hub.handlers.filter((h) => h !== handler);
    });
  }),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribePopOuts: vi.fn((handler: (n: unknown) => void) => {
    hub.windows = handler;
    return Promise.resolve(() => undefined);
  }),
}));

const api = vi.mocked(commands);
const SLOW = { timeout: 5000 };
const MEMBER = { liaison: { origin: "member" }, workforce: { positionId: "p1" } };
const HANDOFF = { liaison: { origin: "handoff", parentSessionId: "s1" } };
const MANAGER: ChatTarget = { positionId: "p1", sessionId: "s1", title: "Development Manager" };

function emit(update: AgentUpdate) {
  act(() => {
    for (const handler of hub.handlers) handler(update);
  });
}

function notify(notice: PopOutNotice) {
  act(() => hub.windows?.(notice));
}

/**
 * A stand-in for the window `window.open` gives: a frame's own window and page (a real window, as
 * the queries need), with its close noted.
 */
function fakeWindow() {
  const frame = document.createElement("iframe");
  document.body.appendChild(frame);
  const win = frame.contentWindow as Window;
  const close = vi.spyOn(win, "close").mockImplementation(() => undefined);
  return { win, close };
}

function finished(taskId: string, objective: string, text: string): AgentTurn {
  return turn(taskId, {
    objective,
    running: false,
    endedAt: 2,
    result: {
      outcome: "completed",
      summary: "done",
      text,
      error: null,
      providerSessionId: null,
      model: null,
      usage: null,
      durationMs: 1,
      ignoredLines: 0,
    },
  });
}

function detail(
  id: string,
  turns: AgentTurn[],
  metadata: Record<string, unknown> = MEMBER,
): AgentSessionDetail {
  return { session: session(id, { metadata }), turns, activity: [] };
}

/** The chats, for the test to drive (kept once drawn). */
const grabbed: { chat: ChatApi | null } = { chat: null };
const chat = (): ChatApi => {
  if (!grabbed.chat) throw new Error("the chats are not drawn yet");
  return grabbed.chat;
};
function Grab() {
  const api = useChat();
  useEffect(() => {
    grabbed.chat = api;
  });
  const ws = useWorkspace();
  return (
    <button type="button" onClick={() => ws.reset()}>
      Reset layout
    </button>
  );
}

async function show() {
  render(
    <AgentsProvider>
      <WorkspaceProvider>
        <ChatProvider>
          <Grab />
          <ChatPanel go={() => undefined} />
          <ChatWindows />
        </ChatProvider>
      </WorkspaceProvider>
    </AgentsProvider>,
  );
  await waitFor(() => expect(hub.handlers.length).toBe(2), SLOW);
}

const kept = () =>
  JSON.parse(localStorage.getItem(TABS_KEY) ?? "null") as { popped: { key: string }[] };

beforeEach(() => {
  localStorage.clear();
  hub.handlers = [];
  hub.windows = null;
  // Plenipo knows every conversation (a worker handed its work by its lead, too).
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code")],
    sessions: [session("s1", { metadata: MEMBER }), session("s2", { metadata: HANDOFF })],
    notices: [],
  });
  api.getAgentSession.mockImplementation((id) =>
    Promise.resolve(
      id === "s1"
        ? detail("s1", [finished("t1", "Write a plan", "Here is the plan.")])
        : detail(
            id,
            [{ ...finished(`t-${id}`, "Check the page", "Checked."), sessionId: id }],
            HANDOFF,
          ),
    ),
  );
  api.getTaskHandoffs.mockImplementation((taskId) =>
    Promise.resolve({ taskId, correlationId: null, depth: null, received: null, sent: [] }),
  );
  api.getChainOrders.mockResolvedValue([]);
  api.getWorkFolder.mockResolvedValue({ path: "C:\\x", plenipoFiles: true, exists: true });
  api.preparePopOut.mockResolvedValue();
  api.focusPopOut.mockResolvedValue(true);
  api.closePopOut.mockResolvedValue(true);
  api.resetPopOuts.mockResolvedValue();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  for (const frame of document.querySelectorAll("iframe")) frame.remove();
});

describe("each agent's chat in a window of its own (ADR-203)", () => {
  it("pops a chat out, live and with its message box, and puts it back", async () => {
    const { win, close } = fakeWindow();
    const open = vi.spyOn(window, "open").mockReturnValue(win);
    await show();
    act(() => chat().open(MANAGER));
    await screen.findByRole("log", { name: "Conversation with Development Manager" }, SLOW);

    // From the chat's header in the panel.
    fireEvent.click(
      screen.getByRole("button", { name: "Pop out: Development Manager's chat in its own window" }),
    );
    await waitFor(() =>
      expect(api.preparePopOut).toHaveBeenCalledWith(
        { kind: "chat", slot: 1 },
        null,
        "Development Manager",
      ),
    );
    await waitFor(() => expect(open).toHaveBeenCalledWith("about:blank", "_blank"));
    const own = win.document.body;
    await within(own).findByRole("log", { name: "Conversation with Development Manager" }, SLOW);
    expect(win.document.title).toBe("Plenipo · Development Manager");
    expect(within(own).getByText("Here is the plan.")).toBeInTheDocument();
    // The panel says where it is.
    expect(screen.getByText("Development Manager's chat is in its own window")).toBeInTheDocument();
    expect(kept().popped).toEqual([{ key: "position:p1", slot: 1 }]);

    // Live there: a new answer streams into the window.
    emit({ kind: "turn", ...turn("t2", { sessionId: "s1", number: 2, objective: "Go on" }) });
    emit({ kind: "activity", ...activity("t2", 1, { type: "textDelta", text: "Going on now." }) });
    expect(await within(own).findByText("Going on now.", {}, SLOW)).toBeInTheDocument();

    // Its message box gives the agent its next objective (when it is free).
    emit({
      kind: "turn",
      ...finished("t2", "Go on", "Went on."),
      sessionId: "s1",
      number: 2,
    });
    api.giveObjective.mockResolvedValue(detail("s1", []));
    const box = within(own).getByLabelText("Message to Development Manager");
    fireEvent.change(box, { target: { value: "And the budget" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(api.giveObjective).toHaveBeenCalledWith("p1", "And the budget"));

    // Put back, in the window's own header: the window closes; the chat is in the panel again.
    fireEvent.click(within(own).getByRole("button", { name: "Put back in the Chat panel" }));
    await waitFor(() => expect(close).toHaveBeenCalled());
    expect(api.closePopOut).toHaveBeenCalledWith({ kind: "chat", slot: 1 });
    await screen.findByRole("log", { name: "Conversation with Development Manager" }, SLOW);
    expect(kept().popped).toEqual([]);
  });

  it("goes back to the panel when its window is closed, or when Reset layout closes them all", async () => {
    vi.spyOn(window, "open").mockImplementation(() => fakeWindow().win);
    await show();
    act(() => chat().openWindow(MANAGER));
    act(() => chat().openWindow({ sessionId: "s2", title: "Developer (on call)" }));
    await waitFor(() => expect(kept().popped).toHaveLength(2));
    await waitFor(() => expect(api.preparePopOut).toHaveBeenCalledTimes(2));

    // The owner closed window 1 (its X): that chat is in the panel, the other keeps its window.
    notify({ kind: "closed", target: { kind: "chat", slot: 1 } });
    await waitFor(() => expect(kept().popped).toEqual([{ key: "session:s2", slot: 2 }]));
    // Reset layout: every chat back in the panel.
    fireEvent.click(screen.getByRole("button", { name: "Reset layout" }));
    await waitFor(() => expect(kept().popped).toEqual([]));
    expect(api.resetPopOuts).toHaveBeenCalled();
  });

  it("opens its window again after a restart; one that cannot open goes back to the panel", async () => {
    localStorage.setItem(
      TABS_KEY,
      JSON.stringify({
        tabs: [
          { key: "position:p1", positionId: "p1", sessionId: "s1", title: "Development Manager" },
          { key: "session:s2", positionId: null, sessionId: "s2", title: "Developer (on call)" },
        ],
        active: "position:p1",
        sideBySide: false,
        popped: [
          { key: "position:p1", slot: 3 },
          { key: "session:s2", slot: 5 },
        ],
      }),
    );
    const { win } = fakeWindow();
    vi.spyOn(window, "open").mockReturnValueOnce(win).mockReturnValueOnce(null);
    await show();
    await waitFor(() =>
      expect(api.preparePopOut).toHaveBeenCalledWith(
        { kind: "chat", slot: 3 },
        null,
        "Development Manager",
      ),
    );
    await within(win.document.body).findByRole(
      "log",
      { name: "Conversation with Development Manager" },
      SLOW,
    );
    // Window 5 could not open: that chat is back in the panel.
    await waitFor(() => expect(kept().popped).toEqual([{ key: "position:p1", slot: 3 }]));
  });

  it("gives six chats windows at most; the seventh opens in the panel and says why", async () => {
    vi.spyOn(window, "open").mockImplementation(() => fakeWindow().win);
    await show();
    for (let i = 1; i <= 7; i += 1) {
      act(() => chat().openWindow({ sessionId: `w${i}`, title: `Worker ${i}` }));
    }
    await waitFor(() => expect(kept().popped).toHaveLength(6));
    expect(chat().windowSlot("session:w7")).toBeNull();
    expect(
      await screen.findByText(/6 chats have windows of their own already/, {}, SLOW),
    ).toBeInTheDocument();
    await waitFor(() => expect(api.preparePopOut).toHaveBeenCalledTimes(6));
  });

  it("says a handed-off worker's messages are its lead's", async () => {
    await show();
    act(() => chat().open({ sessionId: "s2", title: "Developer (on call)" }));
    const log = await screen.findByRole(
      "log",
      { name: "Conversation with Developer (on call)" },
      SLOW,
    );
    expect(await within(log).findByText("From its lead", {}, SLOW)).toBeInTheDocument();
    expect(within(log).getByText("Check the page")).toBeInTheDocument();
    // Your own chat with an agent says no such thing.
    act(() => chat().open(MANAGER));
    const mine = await screen.findByRole(
      "log",
      { name: "Conversation with Development Manager" },
      SLOW,
    );
    expect(within(mine).queryByText("From its lead")).toBeNull();
  });
});
