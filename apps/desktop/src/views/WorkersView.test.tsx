import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type {
  AgentSessionDetail,
  AgentUpdate,
  HandoffView,
  LedgerEvent,
  TaskHandoffs,
  TurnResult,
} from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import { activity, runtime, session, turn } from "../test/agentFixtures";
import * as commands from "../api/commands";
import * as events from "../api/events";
import { WorkersView } from "./WorkersView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    startAgentSession: vi.fn(),
    resumeAgentSession: vi.fn(),
    cancelAgentTurn: vi.fn(),
    closeAgentSession: vi.fn(),
    getTaskHandoffs: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeAgentUpdates: vi.fn(),
  subscribeLedgerEvents: vi.fn(),
}));

const api = vi.mocked(commands);
let emit: (update: AgentUpdate) => void = () => undefined;
let emitLedger: (event: LedgerEvent) => void = () => undefined;
const showExecution = vi.fn();
const openRuntimes = vi.fn();

function send(update: AgentUpdate) {
  act(() => emit(update));
}

const openPosition = vi.fn();

function Harness({ initial = null }: { initial?: string | null }) {
  const [selected, setSelected] = useState<string | null>(initial);
  return (
    <AgentsProvider>
      <WorkersView
        selectedSessionId={selected}
        onSelectSession={setSelected}
        onShowExecution={showExecution}
        onOpenRuntimes={openRuntimes}
        onOpenPosition={openPosition}
      />
    </AgentsProvider>
  );
}

const completed = (text: string): TurnResult => ({
  outcome: "completed",
  summary: text,
  text,
  error: null,
  providerSessionId: "p-1",
  model: "model-x",
  usage: { inputTokens: 1200, cachedInputTokens: 200, outputTokens: 34 },
  durationMs: 2500,
  ignoredLines: 0,
});

function detail(patch: Partial<AgentSessionDetail> = {}): AgentSessionDetail {
  return {
    session: session("s1", { activeTaskId: "t1", title: "Say hello" }),
    turns: [turn("t1")],
    activity: [],
    ...patch,
  };
}

beforeEach(() => {
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code"), runtime("codex", false)],
    sessions: [],
    notices: [],
  });
  vi.mocked(events.subscribeAgentUpdates).mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    emitLedger = handler;
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Workers view", () => {
  it("starts a task on a ready runtime and streams its activity to a normalized result", async () => {
    api.startAgentSession.mockResolvedValue(detail());
    api.getAgentSession.mockResolvedValue(detail());
    render(<Harness />);
    const user = userEvent.setup();
    const form = await screen.findByRole("form", { name: "New task" });
    expect(await within(form).findByText("Ready")).toBeInTheDocument();

    await user.type(within(form).getByRole("textbox", { name: "Objective" }), "Say hello");
    await user.click(within(form).getByRole("button", { name: "Start task" }));
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Say hello", "", false);

    // The new session is selected and shows its running turn.
    const turns = await screen.findByRole("list", { name: "Turns" });
    expect(within(turns).getByText("Working…")).toBeInTheDocument();

    send({
      kind: "activity",
      ...activity("t1", 1, { type: "sessionStarted", providerSessionId: "p-1", model: "model-x" }),
    });
    send({ kind: "activity", ...activity("t1", 2, { type: "textDelta", text: "Hel" }) });
    send({ kind: "activity", ...activity("t1", 3, { type: "textDelta", text: "lo!" }) });
    const log = screen.getByRole("list", { name: "Turn 1 activity" });
    expect(within(log).getByText("Hello!")).toBeInTheDocument();

    send({ kind: "activity", ...activity("t1", 4, { type: "message", text: "Hello!" }) });
    send({
      kind: "turn",
      ...turn("t1", { running: false, result: completed("Hello!"), endedAt: 5 }),
    });
    send({
      kind: "session",
      ...session("s1", {
        title: "Say hello",
        providerSessionId: "p-1",
        providerSessionConfirmed: true,
      }),
    });

    const result = await screen.findByLabelText("Turn 1 result");
    expect(within(result).getByText("Hello!")).toBeInTheDocument();
    expect(within(result).getByText(/1,200 in \(200 cached\) · 34 out/)).toBeInTheDocument();
    expect(within(turns).getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("Provider session p-1")).toBeInTheDocument();

    await user.click(within(turns).getByRole("button", { name: "Raw output" }));
    expect(showExecution).toHaveBeenCalledWith("e1");
  });

  it("explains why a runtime is not ready and does not let it start", async () => {
    render(<Harness />);
    const user = userEvent.setup();
    const form = await screen.findByRole("form", { name: "New task" });
    await user.click(await within(form).findByRole("radio", { name: /Codex/ }));
    expect(within(form).getByRole("note")).toHaveTextContent("Codex is not ready.");
    expect(within(form).getByRole("note")).toHaveTextContent("Run the login command.");
    await user.type(within(form).getByRole("textbox", { name: "Objective" }), "Hi");
    expect(within(form).getByRole("button", { name: "Start task" })).toBeDisabled();
    await user.click(within(form).getByRole("button", { name: "Open Runtimes" }));
    expect(openRuntimes).toHaveBeenCalled();
  });

  it("shows refusals from Core", async () => {
    api.startAgentSession.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Claude Code is not signed in."),
    );
    render(<Harness />);
    const user = userEvent.setup();
    const form = await screen.findByRole("form", { name: "New task" });
    await within(form).findByText("Ready");
    await user.type(within(form).getByRole("textbox", { name: "Objective" }), "Hi");
    await user.click(within(form).getByRole("button", { name: "Start task" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not signed in");
  });

  it("cancels a running turn and continues (resumes) the session", async () => {
    const running = detail();
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [running.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(running);
    const cancelled = detail({
      session: session("s1", { title: "Say hello" }),
      turns: [
        turn("t1", {
          running: false,
          result: {
            ...completed(""),
            outcome: "cancelled",
            summary: "Cancelled by user",
            text: null,
          },
        }),
      ],
    });
    api.cancelAgentTurn.mockResolvedValue(cancelled);
    api.resumeAgentSession.mockResolvedValue(
      detail({
        session: session("s1", { title: "Say hello", activeTaskId: "t2", turnCount: 2 }),
        turns: [...cancelled.turns, turn("t2", { number: 2, objective: "Try again" })],
      }),
    );
    render(<Harness initial="s1" />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: "Cancel turn" }));
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("s1");
    expect(await screen.findByText("Cancelled by user")).toBeInTheDocument();

    const followUp = screen.getByRole("form", { name: "Continue session" });
    await user.type(within(followUp).getByRole("textbox"), "Try again");
    await user.click(within(followUp).getByRole("button", { name: "Send" }));
    expect(api.resumeAgentSession).toHaveBeenCalledWith("s1", "Try again");
    const turns = screen.getByRole("list", { name: "Turns" });
    await waitFor(() => expect(within(turns).getByText("Try again")).toBeInTheDocument());
    // One turn at a time: Send is disabled while turn 2 runs.
    expect(within(followUp).getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("loads a session's full history when selected, even after partial live updates", async () => {
    const full = detail({
      session: session("s1", { title: "Say hello", turnCount: 2 }),
      turns: [
        turn("t1", { running: false, result: completed("First answer") }),
        turn("t2", {
          number: 2,
          objective: "Second",
          running: false,
          result: completed("Second answer"),
        }),
      ],
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [full.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(full);
    render(<Harness />);
    await screen.findByRole("list", { name: "Sessions" });
    // A live update for the latest turn arrives before the session is ever opened.
    send({ kind: "turn", ...full.turns[1]! });
    await userEvent.setup().click(screen.getByRole("button", { name: /Say hello/ }));
    expect(api.getAgentSession).toHaveBeenCalledWith("s1");
    expect(await screen.findByText("First answer")).toBeInTheDocument();
    expect(screen.getByText("Second answer")).toBeInTheDocument();
  });

  it("shows normalized failures with their explanation", async () => {
    const failed = detail({
      session: session("s1", { title: "Say hello" }),
      turns: [
        turn("t1", {
          running: false,
          result: {
            ...completed(""),
            outcome: "usageLimited",
            summary: "Claude Code reported a usage limit; resume this session later",
            text: null,
            error: "Claude AI usage limit reached",
          },
        }),
      ],
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [failed.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(failed);
    render(<Harness initial="s1" />);
    expect(await screen.findByText("Usage limit reached")).toBeInTheDocument();
    expect(screen.getByText(/resume this session later/)).toBeInTheDocument();
    expect(screen.getByText("Claude AI usage limit reached")).toBeInTheDocument();
    // Sessions without handoffs never ask Liaison for any.
    expect(api.getTaskHandoffs).not.toHaveBeenCalled();
  });
});

// ---- Phase 4: handoffs through Liaison ------------------------------------------------------

const REQUESTER = "s1";
const WORKER = "w1";

const liaisonOwner = { liaison: { enabled: true, origin: "owner", protocol: "plenipo-liaison/1" } };
const liaisonWorker = {
  liaison: {
    enabled: true,
    origin: "handoff",
    parentTaskId: "t1",
    parentSessionId: REQUESTER,
    depth: 1,
  },
};

const handoff = (patch: Partial<HandoffView> = {}): HandoffView => ({
  messageId: "m1",
  correlationId: "c1",
  state: "answered",
  requesterTaskId: "t1",
  requester: `session:${REQUESTER}`,
  requesterRuntimeId: "codex",
  step: 1,
  destination: "runtime:claude-code",
  destinationLabel: "Claude Code",
  objective: "Review the parser",
  acceptanceCriteria: "Say whether it is correct.",
  priority: 2,
  depth: 1,
  context: [{ kind: "answer", title: "The requester's answer", chars: 120 }],
  artifacts: [],
  capabilitiesRequested: [],
  rejection: null,
  childTaskId: "c-task",
  childSessionId: WORKER,
  childState: "succeeded",
  reply: {
    messageId: "r1",
    state: "delivered",
    outcome: "completed",
    summary: "Looks correct",
    text: "The parser looks correct.",
    error: null,
    source: `session:${WORKER}`,
    createdAt: 5,
  },
  createdAt: 2,
  updatedAt: 5,
  ...patch,
});

const handoffs = (patch: Partial<TaskHandoffs> = {}): TaskHandoffs => ({
  taskId: "t1",
  correlationId: "c1",
  depth: 0,
  received: null,
  sent: [],
  ...patch,
});

const step = (number: number, result: TurnResult | null, running = false) => ({
  number,
  executionId: `e${number}`,
  running,
  result,
  startedAt: number,
  endedAt: result ? number + 1 : null,
});

function ledgerEvent(eventType: string, taskId = "t1"): LedgerEvent {
  return {
    seq: 1,
    id: "ev",
    taskId,
    executionId: null,
    source: "liaison",
    destination: null,
    eventType,
    payload: {},
    createdAt: 1,
  };
}

describe("Workers view — handoffs", () => {
  it("starts a task with handoffs only when the owner allows them", async () => {
    api.startAgentSession.mockResolvedValue(detail());
    api.getAgentSession.mockResolvedValue(detail());
    render(<Harness />);
    const user = userEvent.setup();
    const form = await screen.findByRole("form", { name: "New task" });
    await within(form).findByText("Ready");
    const allow = within(form).getByRole("checkbox", { name: /Allow handoffs/ });
    expect(allow).not.toBeChecked();
    await user.click(allow);
    await user.type(within(form).getByRole("textbox", { name: "Objective" }), "Write a parser");
    await user.click(within(form).getByRole("button", { name: "Start task" }));
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Write a parser", "", true);
  });

  it("shows a waiting turn's handoffs and lets the owner cancel it, but not send more", async () => {
    const waiting = detail({
      session: session(REQUESTER, {
        runtimeId: "codex",
        title: "Write a parser",
        waitingTaskId: "t1",
        metadata: liaisonOwner,
      }),
      turns: [
        turn("t1", {
          objective: "Write a parser",
          running: false,
          waiting: true,
          steps: [step(1, completed("Here is the parser.\n```plenipo-handoff\n{}\n```"))],
        }),
      ],
      activity: [activity("t1", 1, { type: "message", text: "Here is the parser." })],
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex")],
      sessions: [waiting.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(waiting);
    api.getTaskHandoffs.mockResolvedValue(
      handoffs({ sent: [handoff({ state: "dispatched", childState: "running", reply: null })] }),
    );
    api.cancelAgentTurn.mockResolvedValue(
      detail({
        session: session(REQUESTER, { runtimeId: "codex", metadata: liaisonOwner }),
        turns: [
          turn("t1", {
            running: false,
            result: { ...completed(""), outcome: "cancelled", summary: "Cancelled", text: null },
          }),
        ],
      }),
    );
    render(<Harness initial={REQUESTER} />);
    const user = userEvent.setup();

    const turns = await screen.findByRole("list", { name: "Turns" });
    expect(await within(turns).findByText("Waiting for replies")).toBeInTheDocument();
    expect(screen.getByText("Handoffs allowed")).toBeInTheDocument();
    expect(screen.getAllByText("Waiting").length).toBeGreaterThan(0);
    const card = await within(turns).findByRole("listitem", {
      name: "Handoff to Claude Code: Review the parser",
    });
    expect(within(card).getByText("Worker running")).toBeInTheDocument();
    expect(within(card).getByText(/Context: The requester's answer/)).toBeInTheDocument();
    expect(within(turns).getByText("Waiting for 1 handoff reply…")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("waiting for replies to its handoffs");

    // No follow-up while waiting; the owner can cancel.
    const followUp = screen.getByRole("form", { name: "Continue session" });
    await user.type(within(followUp).getByRole("textbox"), "More");
    expect(within(followUp).getByRole("button", { name: "Send" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Cancel turn" }));
    expect(api.cancelAgentTurn).toHaveBeenCalledWith(REQUESTER);
  });

  it("shows each step, the reply it continued with, and opens the worker's session", async () => {
    const done = detail({
      session: session(REQUESTER, {
        runtimeId: "codex",
        title: "Write a parser",
        metadata: liaisonOwner,
      }),
      turns: [
        turn("t1", {
          objective: "Write a parser",
          running: false,
          result: completed("Final parser, reviewed."),
          steps: [
            step(1, completed("Draft parser.\n```plenipo-handoff\n{}\n```")),
            step(2, completed("Final parser, reviewed.")),
          ],
          endedAt: 9,
        }),
      ],
      activity: [
        activity("t1", 1, { type: "message", text: "Draft parser." }),
        activity("t1", 1_000_001, { type: "message", text: "Final parser, reviewed." }),
      ],
    });
    const worker = session(WORKER, {
      title: "Review the parser",
      metadata: liaisonWorker,
      state: "closed",
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex")],
      sessions: [done.session, worker],
      notices: [],
    });
    api.getAgentSession.mockImplementation((id) =>
      Promise.resolve(
        id === WORKER
          ? detail({
              session: worker,
              turns: [
                turn("c-task", {
                  sessionId: WORKER,
                  objective: "Review the parser",
                  running: false,
                  result: completed("The parser looks correct."),
                }),
              ],
            })
          : done,
      ),
    );
    api.getTaskHandoffs.mockImplementation((taskId) =>
      Promise.resolve(
        taskId === "c-task"
          ? handoffs({ taskId: "c-task", depth: 1, received: handoff() })
          : handoffs({ sent: [handoff()] }),
      ),
    );
    render(<Harness initial={REQUESTER} />);
    const user = userEvent.setup();

    const steps = await screen.findByRole("list", { name: "Turn 1 steps" });
    expect(within(steps).getByText(/^Step 1/)).toBeInTheDocument();
    expect(within(steps).getByText(/^Step 2 · continued with handoff replies/)).toBeInTheDocument();
    // Each step's activity is shown under its own step.
    expect(
      within(screen.getByRole("list", { name: "Turn 1 step 2 activity" })).getByText(
        "Final parser, reviewed.",
      ),
    ).toBeInTheDocument();
    const card = await within(steps).findByRole("listitem", {
      name: "Handoff to Claude Code: Review the parser",
    });
    expect(within(card).getByText("Answered")).toBeInTheDocument();
    expect(within(card).getByText("The parser looks correct.")).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Turn 1 result")).getByText("Final parser, reviewed."),
    ).toBeInTheDocument();

    // The worker's own session shows the request it was started for and links back.
    await user.click(within(card).getByRole("button", { name: "Open worker session" }));
    const request = await screen.findByRole("generic", { name: "Handoff request" });
    expect(request).toHaveTextContent("Asked by Codex through Plenipo Liaison · depth 1");
    expect(request).toHaveTextContent("Acceptance criteria: Say whether it is correct.");
    expect(screen.getByText("Handoff worker")).toBeInTheDocument();
    expect(screen.getByText(/takes work only through Liaison/)).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Continue session" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open requester session" }));
    expect(await screen.findByRole("list", { name: "Turn 1 steps" })).toBeInTheDocument();
  });

  it("marks an organization member's session and sends the owner to the Organization view", async () => {
    const member = session("m1", {
      title: "Website Coordinator",
      metadata: {
        liaison: { enabled: true, origin: "member", protocol: "plenipo-liaison/1" },
        workforce: { positionId: "p-web", agentId: "agent-1", projectId: "pr-web" },
      },
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code")],
      sessions: [member],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(
      detail({
        session: member,
        turns: [turn("t1", { running: false, result: completed("Planned."), endedAt: 9 })],
      }),
    );
    api.getTaskHandoffs.mockResolvedValue(handoffs());
    render(<Harness initial="m1" />);

    expect(await screen.findByText("Organization member")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Sessions" })).toHaveTextContent("organization member");
    // Its objectives come from its position, not from here.
    expect(screen.queryByRole("form", { name: "Continue session" })).not.toBeInTheDocument();
    expect(screen.getByText(/Give it objectives from the/)).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Open in Organization" }));
    expect(openPosition).toHaveBeenCalledWith("p-web");
  });

  it("shows refusals with Liaison's reason", async () => {
    const refused = detail({
      session: session(REQUESTER, { runtimeId: "codex", metadata: liaisonOwner }),
      turns: [
        turn("t1", {
          running: false,
          result: completed("Did it myself."),
          steps: [step(1, completed("```plenipo-handoff\n{}\n```")), step(2, completed("x"))],
        }),
      ],
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex")],
      sessions: [refused.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(refused);
    api.getTaskHandoffs.mockResolvedValue(
      handoffs({
        sent: [
          handoff({
            state: "rejected",
            destination: "runtime:gemini",
            destinationLabel: "gemini",
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
    render(<Harness initial={REQUESTER} />);
    const card = await screen.findByRole("listitem", { name: /Handoff to gemini/ });
    expect(within(card).getByText("Refused")).toBeInTheDocument();
    expect(within(card).getByText(/no AI tool named "gemini"/)).toBeInTheDocument();
    expect(within(card).queryByRole("button", { name: "Open worker session" })).toBeNull();
  });

  it("refreshes handoffs when Liaison records progress, until they settle", async () => {
    const waiting = detail({
      session: session(REQUESTER, {
        runtimeId: "codex",
        waitingTaskId: "t1",
        metadata: liaisonOwner,
      }),
      turns: [turn("t1", { running: false, waiting: true, steps: [step(1, completed("asked"))] })],
    });
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex")],
      sessions: [waiting.session],
      notices: [],
    });
    api.getAgentSession.mockResolvedValue(waiting);
    api.getTaskHandoffs.mockResolvedValueOnce(
      handoffs({ sent: [handoff({ state: "accepted", childState: "queued", reply: null })] }),
    );
    api.getTaskHandoffs.mockResolvedValue(
      handoffs({ sent: [handoff({ state: "dispatched", childState: "running", reply: null })] }),
    );
    render(<Harness initial={REQUESTER} />);
    expect(await screen.findByText("Waiting for a worker")).toBeInTheDocument();
    act(() => emitLedger(ledgerEvent("liaison.dispatched", "c-task")));
    expect(await screen.findByText("Worker running")).toBeInTheDocument();
    expect(api.getTaskHandoffs).toHaveBeenCalledWith("t1");
  });
});
