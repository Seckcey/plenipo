import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { AgentSessionDetail, AgentUpdate, TurnResult } from "@plenipo/types";
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
  };
});
vi.mock("../api/events", () => ({ subscribeAgentUpdates: vi.fn() }));

const api = vi.mocked(commands);
let emit: (update: AgentUpdate) => void = () => undefined;
const showExecution = vi.fn();
const openRuntimes = vi.fn();

function send(update: AgentUpdate) {
  act(() => emit(update));
}

function Harness({ initial = null }: { initial?: string | null }) {
  const [selected, setSelected] = useState<string | null>(initial);
  return (
    <AgentsProvider>
      <WorkersView
        selectedSessionId={selected}
        onSelectSession={setSelected}
        onShowExecution={showExecution}
        onOpenRuntimes={openRuntimes}
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
  });
});
