import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail, AgentTurn, HandoffView, TaskHandoffs } from "@plenipo/types";
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
    giveObjective: vi.fn(),
    resumeAgentSession: vi.fn(),
    cancelAgentTurn: vi.fn(),
    getWorkFolder: vi.fn(),
    getTaskHandoffs: vi.fn(),
    getChainOrders: vi.fn(),
  };
});

const hub = vi.hoisted(() => ({
  handlers: [] as ((u: unknown) => void)[],
  ledger: [] as ((e: unknown) => void)[],
}));
vi.mock("../api/events", () => ({
  subscribeAgentUpdates: vi.fn((handler: (u: unknown) => void) => {
    hub.handlers.push(handler);
    return Promise.resolve(() => {
      hub.handlers = hub.handlers.filter((h) => h !== handler);
    });
  }),
  subscribeLedgerEvents: vi.fn((handler: (e: unknown) => void) => {
    hub.ledger.push(handler);
    return Promise.resolve(() => {
      hub.ledger = hub.ledger.filter((h) => h !== handler);
    });
  }),
}));

const api = vi.mocked(commands);
const SLOW = { timeout: 5000 };

/** The Website Supervisor's own conversation, and the one its Senior Developer worked in. */
const LEAD = session("s-lead", {
  title: "Website Supervisor",
  metadata: { liaison: { enabled: true, origin: "member" }, workforce: { positionId: "p-lead" } },
});
const WORKER = session("s-worker", {
  title: "Review the parser",
  metadata: {
    liaison: { enabled: true, origin: "handoff", parentSessionId: "s-lead" },
    workforce: { positionId: "p-dev" },
  },
});

const handoff = (patch: Partial<HandoffView> = {}): HandoffView => ({
  messageId: "m1",
  correlationId: "c1",
  state: "answered",
  requesterTaskId: "t-lead",
  requester: "session:s-lead",
  requesterRuntimeId: "claude-code",
  step: 1,
  destination: "role:Senior Developer",
  destinationLabel: "Senior Developer",
  objective: "Review the parser",
  acceptanceCriteria: "Say whether it is correct.",
  priority: 2,
  depth: 1,
  context: [{ kind: "answer", title: "The requester's answer", chars: 120 }],
  artifacts: [],
  capabilitiesRequested: [],
  rejection: null,
  childTaskId: "t-worker",
  childSessionId: "s-worker",
  childState: "succeeded",
  reply: {
    messageId: "r1",
    state: "delivered",
    outcome: "completed",
    summary: "Looks correct",
    text: "The parser looks correct.",
    error: null,
    source: "session:s-worker",
    createdAt: 2_000,
  },
  createdAt: 1_500,
  updatedAt: 2_000,
  ...patch,
});

const none = (taskId: string): TaskHandoffs => ({
  taskId,
  correlationId: null,
  depth: null,
  received: null,
  sent: [],
});

function finished(id: string, sessionId: string, objective: string, text: string): AgentTurn {
  return turn(id, {
    sessionId,
    objective,
    running: false,
    endedAt: 4_000,
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

const REQUEST =
  'I will get a review.\n```plenipo-handoff\n{"to": "role:Senior Developer", "objective": "Review the parser"}\n```';

const DETAILS: Record<string, AgentSessionDetail> = {
  // The lead's turn: step 1 asks for the review; step 2 goes on once the reply is back.
  "s-lead": {
    session: LEAD,
    turns: [finished("t-lead", "s-lead", "Ship the parser", "It is correct: shipping it.")],
    activity: [
      {
        ...activity("t-lead", 1, { type: "textDelta", text: REQUEST }),
        sessionId: "s-lead",
        ts: 1_000,
      },
      {
        ...activity("t-lead", 1_000_001, {
          type: "textDelta",
          text: "It is correct: shipping it.",
        }),
        sessionId: "s-lead",
        ts: 3_000,
      },
    ],
  },
  "s-worker": {
    session: WORKER,
    turns: [finished("t-worker", "s-worker", "Review the parser", "The parser looks correct.")],
    activity: [],
  },
};

function Opener({ target }: { target: ChatTarget }) {
  const chat = useChat();
  return (
    <button type="button" onClick={() => chat.open(target)}>
      Open chat
    </button>
  );
}

async function openChat(target: ChatTarget) {
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
  // Wait until Plenipo's conversations are known, so the chat can name who asked.
  await waitFor(() => expect(api.getAgentOverview).toHaveBeenCalled(), SLOW);
  await act(async () => {
    await Promise.resolve();
  });
  await user.click(screen.getByRole("button", { name: "Open chat" }));
  return user;
}

beforeEach(() => {
  localStorage.clear();
  hub.handlers = [];
  hub.ledger = [];
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code")],
    sessions: [LEAD, WORKER],
    notices: [],
  });
  api.getAgentSession.mockImplementation((id) => {
    const found = DETAILS[id];
    return found ? Promise.resolve(found) : Promise.reject(new Error("not in this test"));
  });
  api.getTaskHandoffs.mockImplementation((taskId) =>
    Promise.resolve(
      taskId === "t-lead"
        ? { ...none(taskId), sent: [handoff()] }
        : taskId === "t-worker"
          ? { ...none(taskId), depth: 1, received: handoff() }
          : none(taskId),
    ),
  );
  api.getChainOrders.mockResolvedValue([]);
  api.getWorkFolder.mockResolvedValue({ path: "C:\\x", plenipoFiles: true, exists: true });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("what agents say to each other, in their chats (B5)", () => {
  it("shows a lead's request as a card, and its worker's reply where its turn went on", async () => {
    const user = await openChat({
      positionId: "p-lead",
      sessionId: "s-lead",
      title: "Website Supervisor",
    });
    const log = await screen.findByRole("log", { name: "Conversation with Website Supervisor" });
    const card = await within(log).findByRole(
      "listitem",
      { name: "Handoff to Senior Developer: Review the parser" },
      SLOW,
    );
    expect(card).toHaveTextContent("→ Senior Developer");
    expect(card).toHaveTextContent("Answered");
    // Its words, not the raw request.
    expect(within(log).getByText("I will get a review.")).toBeInTheDocument();
    expect(log).not.toHaveTextContent("plenipo-handoff");
    // Its turn went on in a second step, with the reply that came back.
    expect(within(log).getByText("Step 1")).toBeInTheDocument();
    expect(within(log).getByText("Step 2 · continued with handoff replies")).toBeInTheDocument();
    const replies = within(log).getByRole("list", { name: "Replies from its team" });
    expect(replies).toHaveTextContent("Senior Developer replied");
    expect(replies).toHaveTextContent("The parser looks correct.");
    expect(within(log).getByText("It is correct: shipping it.")).toBeInTheDocument();

    // The worker's own chat, from the card.
    await user.click(within(card).getByRole("button", { name: "Open worker conversation" }));
    expect(
      await screen.findByRole("log", { name: "Conversation with Senior Developer" }, SLOW),
    ).toBeInTheDocument();
  });

  it("shows in a worker's chat who asked it, and that its answer reached them", async () => {
    const user = await openChat({ sessionId: "s-worker", title: "Senior Developer" });
    const log = await screen.findByRole("log", { name: "Conversation with Senior Developer" });
    const asked = await within(log).findByLabelText("Request from Website Supervisor", {}, SLOW);
    expect(asked).toHaveTextContent("Website Supervisor asked Senior Developer");
    expect(asked).toHaveTextContent("Done when: Say whether it is correct.");
    expect(asked).toHaveTextContent("With: The requester's answer");
    expect(within(log).getByText("Review the parser")).toBeInTheDocument();
    expect(within(log).getByText("The parser looks correct.")).toBeInTheDocument();
    expect(within(log).getByLabelText("Reply to Website Supervisor")).toHaveTextContent(
      "Senior Developer replied to Website Supervisor · Website Supervisor got it",
    );

    // The lead's side, from here.
    await user.click(
      within(asked).getByRole("button", { name: "Open Website Supervisor's conversation" }),
    );
    expect(
      await screen.findByRole("log", { name: "Conversation with Website Supervisor" }, SLOW),
    ).toBeInTheDocument();
  });

  it("shows an on-call worker's whole thread: each task it was handed, oldest first", async () => {
    // Its first task's conversation was closed once it answered; its second one is new.
    const first = { ...WORKER, state: "closed" as const, createdAt: 1_000 };
    const second = session("s-worker-2", {
      title: "Fix the header",
      createdAt: 5_000,
      metadata: WORKER.metadata,
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [LEAD, second, first],
      notices: [],
    });
    api.getAgentSession.mockImplementation((id) =>
      id === "s-worker-2"
        ? Promise.resolve({
            session: second,
            turns: [finished("t-worker-2", "s-worker-2", "Fix the header", "Header fixed.")],
            activity: [],
          })
        : id === "s-worker"
          ? Promise.resolve({ ...DETAILS["s-worker"]!, session: first })
          : Promise.reject(new Error("not in this test")),
    );
    api.getTaskHandoffs.mockImplementation((taskId) =>
      Promise.resolve(
        taskId === "t-worker"
          ? { ...none(taskId), received: handoff() }
          : taskId === "t-worker-2"
            ? {
                ...none(taskId),
                received: handoff({
                  messageId: "m2",
                  objective: "Fix the header",
                  childTaskId: "t-worker-2",
                  childSessionId: "s-worker-2",
                  reply: { ...handoff().reply!, messageId: "r2", state: "pending" },
                }),
              }
            : none(taskId),
      ),
    );
    await openChat({ positionId: "p-dev", title: "Senior Developer" });
    const log = await screen.findByRole("log", { name: "Conversation with Senior Developer" });
    const one = await within(log).findByRole("article", { name: "Message 1" }, SLOW);
    const two = await within(log).findByRole("article", { name: "Message 2" }, SLOW);
    expect(one).toHaveTextContent("Review the parser");
    expect(two).toHaveTextContent("Fix the header");
    expect(await within(one).findByText(/got it/, {}, SLOW)).toBeInTheDocument();
    expect(
      await within(two).findByText(/on its way to Website Supervisor/, {}, SLOW),
    ).toBeVisible();
  });

  /** The lead's conversation with one turn that asked for `objective` from `to`. */
  function leadAsked(to: string, objective: string, over: Partial<AgentTurn> = {}) {
    const text = `I will ask.\n\`\`\`plenipo-handoff\n{"to": "${to}", "objective": "${objective}"}\n\`\`\``;
    const waiting = over.waiting === true;
    const lead = { ...LEAD, waitingTaskId: waiting ? "t-lead" : null };
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [lead, WORKER],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue({
      session: lead,
      turns: [
        waiting
          ? turn("t-lead", { sessionId: "s-lead", objective: "Ship the parser", ...over })
          : { ...finished("t-lead", "s-lead", "Ship the parser", "Did it myself."), ...over },
      ],
      activity: [
        { ...activity("t-lead", 1, { type: "textDelta", text }), sessionId: "s-lead", ts: 1_000 },
      ],
    });
  }

  it("shows a refused request with Liaison's reason, and no worker to open", async () => {
    leadAsked("gemini", "Ask around");
    api.getTaskHandoffs.mockImplementation((taskId) =>
      Promise.resolve({
        ...none(taskId),
        sent: [
          handoff({
            state: "rejected",
            destination: "runtime:gemini",
            destinationLabel: "gemini",
            objective: "Ask around",
            rejection: 'missing destination: there is no AI tool named "gemini"',
            childTaskId: null,
            childSessionId: null,
            childState: null,
            context: [],
            reply: { ...handoff().reply!, source: "liaison", outcome: "rejected" },
          }),
        ],
      }),
    );
    await openChat({ positionId: "p-lead", sessionId: "s-lead", title: "Website Supervisor" });
    const log = await screen.findByRole("log", { name: "Conversation with Website Supervisor" });
    const card = await within(log).findByRole(
      "listitem",
      { name: "Handoff to gemini: Ask around" },
      SLOW,
    );
    expect(await within(card).findByText("Refused", {}, SLOW)).toBeInTheDocument();
    expect(within(card).getByText(/no AI tool named "gemini"/)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Open worker conversation" })).toBeNull();
  });

  it("shows a lead waiting for its team, which you can stop", async () => {
    leadAsked("role:Senior Developer", "Review the parser", { running: false, waiting: true });
    api.getTaskHandoffs.mockImplementation((taskId) =>
      Promise.resolve({
        ...none(taskId),
        sent: [handoff({ state: "dispatched", childState: "running", reply: null })],
      }),
    );
    api.cancelAgentTurn.mockResolvedValue({
      session: LEAD,
      turns: [finished("t-lead", "s-lead", "Ship the parser", "")],
      activity: [],
    });
    const user = await openChat({
      positionId: "p-lead",
      sessionId: "s-lead",
      title: "Website Supervisor",
    });
    const log = await screen.findByRole("log", { name: "Conversation with Website Supervisor" });
    const card = await within(log).findByRole(
      "listitem",
      { name: "Handoff to Senior Developer: Review the parser" },
      SLOW,
    );
    expect(await within(card).findByText("Worker running", {}, SLOW)).toBeInTheDocument();
    expect(within(card).getByText(/Context: The requester's answer/)).toBeInTheDocument();
    expect(screen.getByText("Waiting for its team")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop Website Supervisor" }));
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("s-lead");
  });

  it("refreshes a request as Liaison records its progress", async () => {
    leadAsked("role:Senior Developer", "Review the parser", { running: false, waiting: true });
    api.getTaskHandoffs.mockResolvedValueOnce({
      ...none("t-lead"),
      sent: [handoff({ state: "accepted", childState: "queued", reply: null })],
    });
    api.getTaskHandoffs.mockResolvedValue({
      ...none("t-lead"),
      sent: [handoff({ state: "dispatched", childState: "running", reply: null })],
    });
    await openChat({ positionId: "p-lead", sessionId: "s-lead", title: "Website Supervisor" });
    const log = await screen.findByRole("log", { name: "Conversation with Website Supervisor" });
    expect(await within(log).findByText("Waiting for a worker", {}, SLOW)).toBeInTheDocument();
    // Liaison records that a worker took it: the card follows.
    act(() => {
      for (const handler of hub.ledger) handler({ eventType: "liaison.dispatched" });
    });
    expect(await within(log).findByText("Worker running", {}, SLOW)).toBeInTheDocument();
    expect(api.getTaskHandoffs).toHaveBeenCalledWith("t-lead");
  });
});
