import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useEffect, useState } from "react";
import type { AgentSessionDetail, AgentTurn, AgentUpdate } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import * as commands from "../api/commands";
import { activity, runtime, session, turn } from "../test/agentFixtures";
import { ChatPanel } from "./ChatPanel";
import { ChatProvider } from "./ChatProvider";
import { ChatWindow } from "./ChatWindow";
import { useChat, type ChatApi } from "./context";
import { TABS_KEY, type ChatTarget } from "./tabs";
import { useShownChat } from "./useShownChat";

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
  };
});

const hub = vi.hoisted(() => ({ handlers: [] as ((u: unknown) => void)[] }));
vi.mock("../api/events", () => ({
  subscribeAgentUpdates: vi.fn((handler: (u: unknown) => void) => {
    hub.handlers.push(handler);
    return Promise.resolve(() => {
      hub.handlers = hub.handlers.filter((h) => h !== handler);
    });
  }),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const SLOW = { timeout: 5000 };
const MEMBER = { liaison: { origin: "member" }, workforce: { positionId: "p1" } };
const MANAGER: ChatTarget = { positionId: "p1", sessionId: "s1", title: "Development Manager" };

function emit(update: AgentUpdate) {
  act(() => {
    for (const handler of hub.handlers) handler(update);
  });
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

function detail(turns: AgentTurn[], over: Partial<AgentSessionDetail["session"]> = {}) {
  return {
    session: session("s1", { metadata: MEMBER, ...over }),
    turns,
    activity: [],
  } satisfies AgentSessionDetail;
}

/** A chat drawn outside the Chat panel, as the Workers page draws it. */
function Shown({ target }: { target: ChatTarget }) {
  const tab = useShownChat(target);
  return tab ? <ChatWindow tab={tab} /> : null;
}

const grabbed: { chat: ChatApi | null } = { chat: null };
const chat = (): ChatApi => {
  if (!grabbed.chat) throw new Error("the chats are not drawn yet");
  return grabbed.chat;
};
function Grab() {
  const value = useChat();
  useEffect(() => {
    grabbed.chat = value;
  });
  return null;
}

/** Draws `count` copies of the chat for MANAGER, with buttons to change how many. */
function Page({ withPanel = false }: { withPanel?: boolean }) {
  const [count, setCount] = useState(1);
  return (
    <>
      <button type="button" onClick={() => setCount((n) => n + 1)}>
        One more
      </button>
      <button type="button" onClick={() => setCount((n) => n - 1)}>
        One less
      </button>
      {Array.from({ length: count }, (_, i) => (
        <section key={i} aria-label={`Copy ${i + 1}`}>
          <Shown target={MANAGER} />
        </section>
      ))}
      {withPanel && <ChatPanel go={() => undefined} />}
    </>
  );
}

async function draw(withPanel = false) {
  render(
    <AgentsProvider>
      <ChatProvider>
        <Grab />
        <Page withPanel={withPanel} />
      </ChatProvider>
    </AgentsProvider>,
  );
  await waitFor(() => expect(hub.handlers.length).toBe(2), SLOW);
}

beforeEach(() => {
  localStorage.clear();
  hub.handlers = [];
  grabbed.chat = null;
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code")],
    sessions: [session("s1", { metadata: MEMBER })],
    notices: [],
  });
  api.getAgentSession.mockResolvedValue(
    detail([finished("t1", "Write a plan", "Here is the plan.")]),
  );
  api.getTaskHandoffs.mockImplementation((taskId) =>
    Promise.resolve({ taskId, correlationId: null, depth: null, received: null, sent: [] }),
  );
  api.getChainOrders.mockResolvedValue([]);
  api.getWorkFolder.mockResolvedValue({ path: "C:\\x", plenipoFiles: true, exists: true });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("a chat shown outside the Chat panel (the Workers page)", () => {
  it("is live, with its message box and Stop, and takes no tab in the panel", async () => {
    await draw();
    const copy = screen.getByRole("region", { name: "Copy 1" });
    expect(await within(copy).findByText("Here is the plan.", {}, SLOW)).toBeInTheDocument();
    expect(chat().tabs.tabs).toEqual([]);
    expect(localStorage.getItem(TABS_KEY)).toBeNull();

    // Live: a new answer streams in.
    emit({ kind: "turn", ...turn("t2", { sessionId: "s1", number: 2, objective: "Go on" }) });
    emit({ kind: "activity", ...activity("t2", 1, { type: "textDelta", text: "Going on now." }) });
    expect(await within(copy).findByText("Going on now.", {}, SLOW)).toBeInTheDocument();

    // Stop works from here.
    api.cancelAgentTurn.mockResolvedValue(
      detail([finished("t1", "Write a plan", "Here is the plan.")]),
    );
    fireEvent.click(within(copy).getByRole("button", { name: "Stop Development Manager" }));
    await waitFor(() => expect(api.cancelAgentTurn).toHaveBeenCalledWith("s1"));

    // Its message box gives the agent its next objective once it is free.
    emit({ kind: "turn", ...finished("t2", "Go on", "Went on."), sessionId: "s1", number: 2 });
    api.giveObjective.mockResolvedValue(detail([]));
    const box = within(copy).getByLabelText("Message to Development Manager");
    fireEvent.change(box, { target: { value: "And the budget" } });
    fireEvent.keyDown(box, { key: "Enter" });
    await waitFor(() => expect(api.giveObjective).toHaveBeenCalledWith("p1", "And the budget"));
    expect(chat().tabs.tabs).toEqual([]);
  });

  it("stops being shown when nothing draws it any more; two copies share it", async () => {
    await draw();
    await screen.findByText("Here is the plan.", {}, SLOW);
    expect(chat().tab("position:p1")).not.toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "One more" }));
    expect(
      await within(screen.getByRole("region", { name: "Copy 2" })).findByText(
        "Here is the plan.",
        {},
        SLOW,
      ),
    ).toBeInTheDocument();
    // One copy goes: the other still shows it, live.
    fireEvent.click(screen.getByRole("button", { name: "One less" }));
    expect(chat().tab("position:p1")).not.toBeNull();
    // The last one goes: it is no longer an open chat.
    fireEvent.click(screen.getByRole("button", { name: "One less" }));
    await waitFor(() => expect(chat().tab("position:p1")).toBeNull());
  });

  it("is the same chat as the panel's tab for it, when it has one", async () => {
    await draw(true);
    act(() => chat().open(MANAGER));
    // Both draw the same conversation: here, and in the panel.
    await waitFor(
      () =>
        expect(
          screen.getAllByRole("log", { name: "Conversation with Development Manager" }),
        ).toHaveLength(2),
      SLOW,
    );
    expect(chat().tabs.tabs.map((t) => t.key)).toEqual(["position:p1"]);
    expect(screen.getAllByText("Here is the plan.")).toHaveLength(2);
    expect(chat().tab("position:p1")).toBe(chat().tabs.tabs[0]);
  });
});
