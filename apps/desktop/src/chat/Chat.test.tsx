import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail, AgentTurn, AgentUpdate } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import * as commands from "../api/commands";
import { activity, runtime, session, turn } from "../test/agentFixtures";
import { ChatPanel } from "./ChatPanel";
import { ChatProvider } from "./ChatProvider";
import { useChat } from "./context";
import type { ChatTarget } from "./tabs";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    startAgentSession: vi.fn(),
    closeAgentSession: vi.fn(),
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

/** A live update, to every listener (the agents' store and the chats). */
function emit(update: AgentUpdate) {
  act(() => {
    for (const handler of hub.handlers) handler(update);
  });
}

const MEMBER = { liaison: { origin: "member" }, workforce: { positionId: "p1" } };
/** Several promises chain before each of these shows; slow test machines need the time. */
const SLOW = { timeout: 5000 };
const started = Date.now() - 5_000;

function detail(turns: AgentTurn[], over: Partial<AgentSessionDetail["session"]> = {}) {
  return {
    session: session("s1", { metadata: MEMBER, activeTaskId: "t1", ...over }),
    turns,
    activity: [],
  } satisfies AgentSessionDetail;
}

function done(taskId: string, text: string): AgentTurn {
  return turn(taskId, {
    objective: "Write a plan",
    running: false,
    startedAt: started,
    endedAt: started + 4_000,
    result: {
      outcome: "completed",
      summary: "done",
      text,
      error: null,
      providerSessionId: null,
      model: "sonnet",
      usage: { inputTokens: 900, cachedInputTokens: 0, outputTokens: 300 },
      durationMs: 4000,
      ignoredLines: 0,
    },
  });
}

function Opener({ target }: { target: ChatTarget }) {
  const chat = useChat();
  return (
    <button type="button" onClick={() => chat.open(target)}>
      Open chat
    </button>
  );
}

async function openChat(target: ChatTarget = { positionId: "p1", title: "Development Manager" }) {
  const user = userEvent.setup();
  render(
    <AgentsProvider>
      <ChatProvider>
        <Opener target={target} />
        <ChatPanel go={() => undefined} />
      </ChatProvider>
    </AgentsProvider>,
  );
  await waitFor(() => expect(hub.handlers.length).toBe(2), SLOW);
  await user.click(screen.getByRole("button", { name: "Open chat" }));
  return user;
}

beforeEach(() => {
  localStorage.clear();
  hub.handlers = [];
  vi.mocked(commands.getAgentOverview).mockResolvedValue({
    runtimes: [runtime("claude-code")],
    sessions: [],
    notices: [],
  });
  vi.mocked(commands.getTaskHandoffs).mockImplementation((taskId) =>
    Promise.resolve({ taskId, correlationId: null, depth: null, received: null, sent: [] }),
  );
  vi.mocked(commands.getWorkFolder).mockResolvedValue({
    path: "C:\\Users\\frankie\\Documents\\Plenipo\\8 West Ventures\\Development Manager",
    plenipoFiles: true,
    exists: true,
  });
  vi.mocked(commands.openWorkFolder).mockResolvedValue(undefined);
  vi.mocked(commands.getChainOrders).mockResolvedValue([]);
  // Fetching a conversation again finds nothing new unless a test says what Plenipo has.
  vi.mocked(commands.getAgentSession).mockRejectedValue(
    new commands.PlenipoCommandError("invalidInput", "Not in this test."),
  );
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("a chat with an agent (ADR-200)", () => {
  it("streams the answer as it is written, says what it is doing, and shows where files went", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "Write a plan", startedAt: started })]),
    );
    const user = await openChat();
    expect(screen.getByText(/Say what you need/)).toBeInTheDocument();

    await user.type(screen.getByLabelText("Message to Development Manager"), "Write a plan{Enter}");
    expect(commands.giveObjective).toHaveBeenCalledWith("p1", "Write a plan");
    const log = await screen.findByRole(
      "log",
      { name: "Conversation with Development Manager" },
      SLOW,
    );
    // The conversation is drawn as a low-priority update (typing comes first): wait for it.
    expect(await within(log).findByText("Write a plan", {}, SLOW)).toBeInTheDocument();
    expect(await within(log).findByText("Starting", {}, SLOW)).toBeInTheDocument();

    // A wait that would have looked like nothing happening says why.
    emit({
      kind: "activity",
      ...activity("t1", 1, {
        type: "status",
        phase: "waiting",
        text: "Claude's servers are busy. Trying again in 4 s (try 2 of 10).",
      }),
    });
    expect(within(log).getByText(/Trying again in 4 s/)).toBeInTheDocument();

    // Words appear as they are written.
    emit({ kind: "activity", ...activity("t1", 2, { type: "textDelta", text: "Here is " }) });
    emit({ kind: "activity", ...activity("t1", 3, { type: "textDelta", text: "the plan." }) });
    expect(within(log).getByText("Here is the plan.")).toBeInTheDocument();
    expect(within(log).queryByText(/Trying again/)).not.toBeInTheDocument();

    // A tool call is one line, open while it runs, with what it does now.
    emit({
      kind: "activity",
      ...activity("t1", 4, {
        type: "toolUse",
        id: "call-1",
        tool: "mcp__plenipo__write_file",
        summary: "plan.md",
      }),
    });
    expect(within(log).getByRole("button", { name: /Saving plan\.md/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(within(log).getAllByText("Saving a file").length).toBeGreaterThan(0);
    emit({
      kind: "activity",
      ...activity("t1", 5, {
        type: "toolResult",
        id: "call-1",
        tool: "mcp__plenipo__write_file",
        summary: "Created plan.md",
        isError: false,
      }),
    });

    // It finishes: the answer stays, and the file it saved shows with its folder.
    emit({ kind: "turn", ...done("t1", "Here is the plan.") });
    expect(await within(log).findByText("1 file saved", {}, SLOW)).toBeInTheDocument();
    expect(within(log).getByText(/in Plenipo's folder/)).toBeInTheDocument();
    expect(within(log).getByText(/Documents\\Plenipo\\8 West Ventures/)).toBeInTheDocument();
    expect(within(log).getByRole("button", { name: /Saved plan\.md/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    await user.click(within(log).getByRole("button", { name: "Open folder" }));
    expect(commands.openWorkFolder).toHaveBeenCalledWith("t1");
    expect(within(log).getByText(/sonnet · 1\.2k tokens/)).toBeInTheDocument();
  });

  it("holds a message sent while the agent works, and sends it when it finishes", async () => {
    vi.mocked(commands.giveObjective)
      .mockResolvedValueOnce(detail([turn("t1", { objective: "First", startedAt: started })]))
      .mockResolvedValueOnce(
        detail([
          done("t1", "One."),
          turn("t2", { number: 2, objective: "Second", startedAt: Date.now() }),
        ]),
      );
    const user = await openChat();
    const box = screen.getByLabelText("Message to Development Manager");
    await user.type(box, "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.type(box, "Second{Enter}");
    expect(commands.giveObjective).toHaveBeenCalledTimes(1);
    const waiting = screen.getByRole("list", { name: "Waiting to send" });
    expect(within(waiting).getByText("Second")).toBeInTheDocument();

    emit({ kind: "turn", ...done("t1", "One.") });
    await waitFor(() => expect(commands.giveObjective).toHaveBeenCalledTimes(2), SLOW);
    expect(commands.giveObjective).toHaveBeenLastCalledWith("p1", "Second");
    await waitFor(
      () => expect(screen.queryByRole("list", { name: "Waiting to send" })).not.toBeInTheDocument(),
      SLOW,
    );
  });

  it("takes back a waiting message, and Stop stops the work now", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    vi.mocked(commands.cancelAgentTurn).mockResolvedValue(
      detail([turn("t1", { objective: "First", running: false, endedAt: Date.now() })], {
        activeTaskId: null,
      }),
    );
    const user = await openChat();
    const box = screen.getByLabelText("Message to Development Manager");
    await user.type(box, "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.type(box, "Never mind{Enter}");
    await user.click(screen.getByRole("button", { name: "Do not send this" }));
    expect(screen.queryByRole("list", { name: "Waiting to send" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));
    expect(commands.cancelAgentTurn).toHaveBeenCalledWith("s1");
    emit({ kind: "turn", ...done("t1", "") });
    expect(commands.giveObjective).toHaveBeenCalledTimes(1);
  });

  it("says Stopping while the stop goes, and says so when the agent goes on working", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    let answer: (d: AgentSessionDetail) => void = () => undefined;
    vi.mocked(commands.cancelAgentTurn).mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));

    // Plenipo asks its AI tool to stop, and ends it if it does not: that takes a moment.
    const stopping = screen.getByRole("button", { name: "Stopping Development Manager…" });
    expect(stopping).toBeDisabled();
    expect(screen.getByText("Stopping")).toBeInTheDocument();
    await user.click(stopping);
    expect(commands.cancelAgentTurn).toHaveBeenCalledTimes(1);

    // Its AI tool went on: the chat says so, and Stop is there again.
    await act(async () => {
      answer(detail([turn("t1", { objective: "First", startedAt: started })]));
      await Promise.resolve();
    });
    expect(
      await screen.findByText(
        "Development Manager has not stopped yet. Try Stop all, the red button on the map.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop Development Manager" })).toBeEnabled();

    // When it ends after all, the note goes.
    emit({ kind: "turn", ...done("t1", "") });
    await waitFor(() => expect(screen.queryByText(/has not stopped yet/)).not.toBeInTheDocument());
  });

  it("shows a turn whose end was missed as stopped, when Stop finds nothing running", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    vi.mocked(commands.cancelAgentTurn).mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "No turn is running in this session."),
    );
    // Plenipo has no end for it either, and nothing runs in the conversation.
    vi.mocked(commands.getAgentSession).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started, running: false })], {
        activeTaskId: null,
      }),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));

    expect(await screen.findByText(/Stopped\. It was no longer running\./, {}, SLOW)).toBeVisible();
    expect(commands.getAgentSession).toHaveBeenCalledWith("s1");
    expect(screen.queryByRole("button", { name: /^Stop/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps a turn its record says is running, even when the session reads idle a moment behind", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    vi.mocked(commands.cancelAgentTurn).mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "No turn is running in this session."),
    );
    // The session's summary was read before the turn was claimed; the turn's own record runs.
    vi.mocked(commands.getAgentSession).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started, running: true })], {
        activeTaskId: null,
      }),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));

    await waitFor(() => expect(commands.getAgentSession).toHaveBeenCalledWith("s1"), SLOW);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Stop Development Manager" })).toBeEnabled(),
    );
    expect(screen.queryByText(/It was no longer running/)).not.toBeInTheDocument();
  });

  it("clears a stuck turn while another turn in the conversation runs on", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    vi.mocked(commands.cancelAgentTurn).mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "No turn is running in this session."),
    );
    // The first turn's record is over without saying how; a second one runs now.
    vi.mocked(commands.getAgentSession).mockResolvedValue(
      detail(
        [
          turn("t1", { objective: "First", startedAt: started, running: false }),
          turn("t2", { objective: "Second", number: 2, startedAt: started + 1_000 }),
        ],
        { activeTaskId: "t2" },
      ),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));

    expect(await screen.findByText(/Stopped\. It was no longer running\./, {}, SLOW)).toBeVisible();
    expect(screen.getAllByText(/It was no longer running/)).toHaveLength(1);
    // The second turn is still working: Stop is there for it.
    expect(screen.getByRole("button", { name: "Stop Development Manager" })).toBeEnabled();
  });

  it("says why a stop did not work, under its own heading", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue(
      detail([turn("t1", { objective: "First", startedAt: started })]),
    );
    vi.mocked(commands.cancelAgentTurn).mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "The turn is still starting; try again in a moment.",
      ),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "First{Enter}");
    await screen.findByRole("log", {}, SLOW);
    await user.click(screen.getByRole("button", { name: "Stop Development Manager" }));

    const alert = await screen.findByRole("alert", {}, SLOW);
    expect(alert).toHaveTextContent("Could not stop Development Manager");
    expect(alert).toHaveTextContent("The turn is still starting; try again in a moment.");
    expect(alert).not.toHaveTextContent("That message did not go");
    expect(screen.getByRole("button", { name: "Stop Development Manager" })).toBeEnabled();
  });

  it("says why a message did not go, and keeps the box ready", async () => {
    vi.mocked(commands.giveObjective).mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Development Manager is vacant."),
    );
    const user = await openChat();
    await user.type(screen.getByLabelText("Message to Development Manager"), "Hello{Enter}");
    const alert = await screen.findByRole("alert", {}, SLOW);
    expect(alert).toHaveTextContent("That message did not go");
    expect(alert).toHaveTextContent("Development Manager is vacant.");
    await user.click(within(alert).getByRole("button", { name: /dismiss/i }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("lets you watch an on-call worker, but sends nothing to it", async () => {
    vi.mocked(commands.getAgentSession).mockResolvedValue({
      session: session("s9", { metadata: { liaison: { origin: "handoff" } } }),
      turns: [turn("t9", { sessionId: "s9", objective: "Write the script", startedAt: started })],
      activity: [],
    });
    vi.mocked(commands.getAgentOverview).mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [session("s9", { metadata: { liaison: { origin: "handoff" } } })],
      notices: [],
    });
    await openChat({ sessionId: "s9", title: "Developer (on call)" });
    expect(await screen.findByText("Write the script", {}, SLOW)).toBeInTheDocument();
    expect(await screen.findByText(/takes its work from its lead/, {}, SLOW)).toBeInTheDocument();
    expect(screen.getByLabelText("Message to Developer (on call)")).toBeDisabled();
  });

  it("shows what an on-call worker did last, after its conversation was closed (B5)", async () => {
    // A new conversation for each task it was handed, closed once its answer was written.
    const handed = (id: string) =>
      session(id, {
        state: "closed",
        metadata: {
          liaison: { origin: "handoff", parentSessionId: "s-lead" },
          workforce: { positionId: "p-dev" },
        },
      });
    vi.mocked(commands.getAgentOverview).mockResolvedValue({
      runtimes: [runtime("claude-code")],
      // Newest first: its last task's conversation, then an older one.
      sessions: [handed("s-new"), handed("s-old")],
      notices: [],
    });
    vi.mocked(commands.getAgentSession).mockImplementation((id) =>
      Promise.resolve({
        session: handed(id),
        turns: [
          {
            ...done(`t-${id}`, id === "s-new" ? "Built the contact page." : "Fixed the header."),
            sessionId: id,
            objective: id === "s-new" ? "Build the contact page" : "Fix the header",
          },
        ],
        activity: [],
      }),
    );
    await openChat({ positionId: "p-dev", title: "Senior Developer" });
    expect(await screen.findByText("Build the contact page", {}, SLOW)).toBeInTheDocument();
    expect(screen.getByText("Built the contact page.")).toBeInTheDocument();
    expect(screen.queryByText("Fix the header")).not.toBeInTheDocument();
    // Watched, not messaged: its work comes from its lead.
    expect(await screen.findByText(/takes its work from its lead/, {}, SLOW)).toBeInTheDocument();
    expect(screen.getByLabelText("Message to Senior Developer")).toBeDisabled();
  });

  it("sends an on-call worker's message through its lead, and opens the lead's chat", async () => {
    vi.mocked(commands.giveObjective).mockResolvedValue({
      session: session("s5", {
        title: "Cloudline Supervisor",
        activeTaskId: "t5",
        metadata: { liaison: { origin: "member" }, workforce: { positionId: "p-lead" } },
      }),
      turns: [
        turn("t5", {
          sessionId: "s5",
          objective:
            "Write the tests\n\n(The owner asks this of your Senior Developer, through you)",
          startedAt: started,
        }),
      ],
      activity: [],
    });
    const user = await openChat({ positionId: "p-dev", title: "Senior Developer" });
    await user.type(screen.getByLabelText("Message to Senior Developer"), "Write the tests{Enter}");
    expect(commands.giveObjective).toHaveBeenCalledWith("p-dev", "Write the tests");
    expect(await screen.findByRole("tab", { name: /Cloudline Supervisor/ }, SLOW)).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(await screen.findByText(/through you/, {}, SLOW)).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Senior Developer/ }));
    expect(
      screen.getByText(/Sent to Cloudline Supervisor, who hands it to Senior Developer/),
    ).toBeInTheDocument();
  });

  it("shows your orders that went past this agent, and what came back up", async () => {
    vi.mocked(commands.getChainOrders).mockResolvedValue([
      {
        taskId: "t7",
        at: 1,
        positionId: "p-sup",
        position: "Cloudline Supervisor",
        via: null,
        words: "Fix the login page",
        leads: ["Development Manager"],
        part: "told",
        standing: "done",
        result: "Fixed: the button works.",
        reportedAt: 2,
      },
    ]);
    const user = await openChat();
    await user.click(screen.getByRole("button", { name: "Show its tasks" }));
    const list = await screen.findByRole("list", { name: "Chain of command" }, SLOW);
    expect(
      within(list).getByText("You asked Cloudline Supervisor directly: “Fix the login page”."),
    ).toBeInTheDocument();
    expect(within(list).getByText("Reported: Fixed: the button works.")).toBeInTheDocument();
    expect(within(list).getByRole("img", { name: "Done" })).toBeInTheDocument();
    expect(commands.getChainOrders).toHaveBeenCalledWith("p1");
  });

  it("closes a chat with its tab", async () => {
    const user = await openChat();
    expect(screen.getByRole("tab", { name: /Development Manager/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.click(
      screen.getByRole("button", { name: "Close the chat with Development Manager" }),
    );
    expect(screen.getByText("No chats open")).toBeInTheDocument();
  });
});
