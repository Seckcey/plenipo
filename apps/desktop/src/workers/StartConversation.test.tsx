import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail, AgentUpdate } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import * as commands from "../api/commands";
import * as events from "../api/events";
import { runtime, session, turn } from "../test/agentFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { StartConversation } from "./StartConversation";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    startAgentSession: vi.fn(),
    getRouting: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeAgentUpdates: vi.fn(),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
let emit: (update: AgentUpdate) => void = () => undefined;
const started = vi.fn();
const openRuntimes = vi.fn();

function detail(): AgentSessionDetail {
  return {
    session: session("s1", { activeTaskId: "t1", title: "Say hello" }),
    turns: [turn("t1")],
    activity: [],
  };
}

async function draw() {
  render(
    <AgentsProvider>
      <StartConversation onStarted={started} onOpenRuntimes={openRuntimes} />
    </AgentsProvider>,
  );
  return screen.findByRole("form", { name: "Start a conversation" });
}

beforeEach(() => {
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code"), runtime("codex", false)],
    sessions: [],
    notices: [],
  });
  api.getRouting.mockResolvedValue(sampleRouting());
  vi.mocked(events.subscribeAgentUpdates).mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

/**
 * Starting a conversation outside the organization (the Workers page, I4): the old Workers tab's
 * New task form, moved, with the same choices.
 */
describe("Start a conversation outside your organization", () => {
  it("starts on a ready AI tool, on a model chosen from its models, and shows it", async () => {
    api.startAgentSession.mockResolvedValue(detail());
    const form = await draw();
    const user = userEvent.setup();
    expect(await within(form).findByText("Ready")).toBeInTheDocument();
    await user.type(within(form).getByRole("textbox", { name: "What should it do?" }), "Say hello");
    await user.click(within(form).getByText("Advanced"));
    const model = within(form).getByRole("combobox", { name: "Model" });
    await within(model).findByRole("option", { name: "haiku — made by Anthropic" });
    await user.selectOptions(model, "haiku");
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Say hello", "haiku", false);
    expect(started).toHaveBeenCalledWith("s1");
  });

  it("explains why an AI tool is not ready and does not let it start", async () => {
    const form = await draw();
    const user = userEvent.setup();
    await user.click(await within(form).findByRole("radio", { name: /Codex/ }));
    expect(within(form).getByRole("note")).toHaveTextContent("Codex is not ready.");
    expect(within(form).getByRole("note")).toHaveTextContent("Run the login command.");
    await user.type(within(form).getByRole("textbox", { name: "What should it do?" }), "Hi");
    expect(within(form).getByRole("button", { name: "Start" })).toBeDisabled();
    await user.click(within(form).getByRole("button", { name: "Open AI tools" }));
    // At Codex's card (Phase 19).
    expect(openRuntimes).toHaveBeenCalledWith("codex");
  });

  it("says a new conversation waits while its AI tool is being updated, and still lets it start", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: [{ ...runtime("claude-code"), held: "update" }, runtime("codex")],
      sessions: [],
      notices: [],
    });
    const form = await draw();
    const user = userEvent.setup();
    expect(await within(form).findByRole("status")).toHaveTextContent(
      "Waiting: Claude Code is being updated. A new task starts on it when that's done.",
    );
    await user.type(within(form).getByRole("textbox", { name: "What should it do?" }), "Hi");
    expect(within(form).getByRole("button", { name: "Start" })).toBeEnabled();
    // Its sign-in tab open: it waits a while at most.
    act(() =>
      emit({
        kind: "runtimes",
        runtimes: [{ ...runtime("claude-code"), held: "signIn" }, runtime("codex")],
      }),
    );
    expect(within(form).getByRole("status")).toHaveTextContent(
      "Claude Code's sign-in tab is open. A new task waits until it closes, or 10 minutes at most.",
    );
    // Free again: nothing more is said.
    act(() => emit({ kind: "runtimes", runtimes: [runtime("claude-code"), runtime("codex")] }));
    expect(within(form).queryByRole("status")).toBeNull();
  });

  it("says why Plenipo refused to start it", async () => {
    api.startAgentSession.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "Claude Code is not signed in."),
    );
    const form = await draw();
    const user = userEvent.setup();
    await within(form).findByText("Ready");
    await user.type(within(form).getByRole("textbox", { name: "What should it do?" }), "Hi");
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(await within(form).findByRole("alert")).toHaveTextContent("not signed in");
    expect(started).not.toHaveBeenCalled();
  });

  it("starts with Enter; Shift+Enter starts a new line, and a word being put together is not sent", async () => {
    api.startAgentSession.mockResolvedValue(detail());
    const form = await draw();
    const user = userEvent.setup();
    await within(form).findByText("Ready");
    const box = within(form).getByRole("textbox", { name: "What should it do?" });
    expect(box).toHaveAccessibleDescription("Enter sends. Shift and Enter start a new line.");
    // Nothing written yet: Enter starts nothing, and adds no line.
    await user.type(box, "{Enter}");
    expect(box).toHaveValue("");
    await user.type(box, "Say{Shift>}{Enter}{/Shift}hello");
    // A word still being put together (an IME) is only finished.
    fireEvent.keyDown(box, { key: "Enter", isComposing: true });
    expect(api.startAgentSession).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Say\nhello", "", false);
  });

  it("allows handoffs to other workers only when you say so", async () => {
    api.startAgentSession.mockResolvedValue(detail());
    const form = await draw();
    const user = userEvent.setup();
    await within(form).findByText("Ready");
    const allow = within(form).getByRole("checkbox", { name: /Allow handoffs/ });
    expect(allow).not.toBeChecked();
    await user.click(allow);
    await user.type(
      within(form).getByRole("textbox", { name: "What should it do?" }),
      "Write a parser",
    );
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Write a parser", "", true);
  });
});
