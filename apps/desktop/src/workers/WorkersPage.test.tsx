import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { AgentSession, AgentSessionDetail, AgentTurn, AgentUpdate } from "@plenipo/types";
import { storedKey } from "@plenipo/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsProvider } from "../agents/AgentsProvider";
import * as commands from "../api/commands";
import { ChatProvider } from "../chat/ChatProvider";
import { activity, runtime, session, turn } from "../test/agentFixtures";
import { sampleOrganization } from "../test/orgFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { WorkersPage } from "./WorkersPage";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getPermissions: vi.fn(),
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    startAgentSession: vi.fn(),
    resumeAgentSession: vi.fn(),
    giveObjective: vi.fn(),
    cancelAgentTurn: vi.fn(),
    closeAgentSession: vi.fn(),
    getTaskHandoffs: vi.fn(),
    getChainOrders: vi.fn(),
    getWorkFolder: vi.fn(),
    getRouting: vi.fn(),
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

function emit(update: AgentUpdate) {
  act(() => {
    for (const handler of hub.handlers) handler(update);
  });
}

function finished(id: string, sessionId: string, objective: string, text: string): AgentTurn {
  return turn(id, {
    sessionId,
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

/** Plenipo's conversations: the Website Supervisor's, a Senior Developer's task, and one outside. */
const SESSIONS: Record<string, AgentSession> = {
  "s-web": session("s-web", {
    title: "Website Supervisor",
    metadata: { liaison: { origin: "member" }, workforce: { positionId: "p-web" } },
  }),
  "session-a-dev-1": session("session-a-dev-1", {
    title: "Build the new pricing page",
    activeTaskId: "t-dev",
    metadata: {
      liaison: { origin: "handoff", parentSessionId: "s-web" },
      workforce: { positionId: "p-dev" },
    },
  }),
  "s-own": session("s-own", { title: "Ask about tests" }),
};
const TURNS: Record<string, AgentTurn[]> = {
  "s-web": [finished("t-web", "s-web", "Plan the relaunch", "The plan is ready.")],
  "session-a-dev-1": [
    turn("t-dev", { sessionId: "session-a-dev-1", objective: "Build the new pricing page" }),
  ],
  "s-own": [finished("t-own", "s-own", "Ask about tests", "Tests are good.")],
  "s-new": [turn("t-new", { sessionId: "s-new", objective: "Say hi" })],
};

function detail(id: string): AgentSessionDetail {
  return {
    session: SESSIONS[id] ?? session(id, { title: "Say hi", activeTaskId: "t-new" }),
    turns: TURNS[id] ?? [],
    activity: [],
  };
}

function Harness({ initial = null }: { initial?: string | null }) {
  const [selected, setSelected] = useState<string | null>(initial);
  return (
    <AgentsProvider>
      <ChatProvider>
        <WorkersPage
          selectedSessionId={selected}
          onSelectSession={setSelected}
          onOpenRuntimes={vi.fn()}
          onOpenPosition={vi.fn()}
        />
      </ChatProvider>
    </AgentsProvider>
  );
}

async function draw(initial: string | null = null) {
  render(<Harness initial={initial} />);
  await screen.findByRole("button", { name: /^Website Supervisor/ }, SLOW);
  return within(screen.getByRole("navigation", { name: "Your organization" }));
}

beforeEach(() => {
  localStorage.clear();
  hub.handlers = [];
  const org = sampleOrganization();
  org.positions = org.positions.map((p) =>
    p.id === "p-web" && p.agent ? { ...p, agent: { ...p.agent, sessionId: "s-web" } } : p,
  );
  api.getOrganization.mockResolvedValue(org);
  api.getPermissions.mockRejectedValue(new Error("not needed"));
  api.getAgentOverview.mockResolvedValue({
    runtimes: [runtime("claude-code"), runtime("codex", false)],
    sessions: Object.values(SESSIONS),
    notices: [],
  });
  api.getAgentSession.mockImplementation((id) => Promise.resolve(detail(id)));
  api.getTaskHandoffs.mockImplementation((taskId) =>
    Promise.resolve({ taskId, correlationId: null, depth: null, received: null, sent: [] }),
  );
  api.getChainOrders.mockResolvedValue([]);
  api.getWorkFolder.mockResolvedValue({ path: "C:\\x", plenipoFiles: true, exists: true });
  api.getRouting.mockResolvedValue(sampleRouting());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("the Workers page (I4)", () => {
  it("shows the organization as a tree, and a worker's chat beside it, live", async () => {
    const tree = await draw();
    expect(tree.getByRole("button", { name: "Engineering" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(tree.getByRole("button", { name: "Website Relaunch" })).toBeInTheDocument();
    expect(tree.getByRole("button", { name: /^Senior Developer.*Working/ })).toBeInTheDocument();

    fireEvent.click(tree.getByRole("button", { name: /^Website Supervisor/ }));
    expect(tree.getByRole("button", { name: /^Website Supervisor/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    const log = await screen.findByRole("log", { name: "Conversation with Website Supervisor" });
    expect(await within(log).findByText("The plan is ready.", {}, SLOW)).toBeInTheDocument();

    // Live: its next answer streams in as it is written.
    emit({ kind: "turn", ...turn("t-web-2", { sessionId: "s-web", number: 2, objective: "Go" }) });
    emit({
      kind: "activity",
      ...activity("t-web-2", 1, { type: "textDelta", text: "Starting on it." }),
      sessionId: "s-web",
    });
    expect(await within(log).findByText("Starting on it.", {}, SLOW)).toBeInTheDocument();
    // Shown here, not as a tab in the Chat panel.
    expect(localStorage.getItem(storedKey("plenipo.chat"))).toBeNull();
  });

  it("closes a department, says how many are working in it, and keeps that", async () => {
    const tree = await draw();
    fireEvent.click(tree.getByRole("button", { name: "Engineering" }));
    const engineering = tree.getByRole("button", { name: /^Engineering/ });
    expect(engineering).toHaveAttribute("aria-expanded", "false");
    // The Supervisor waits for its team; the Senior Developer and the QA Engineer work.
    expect(engineering).toHaveTextContent("3 working");
    expect(tree.queryByRole("button", { name: /^Website Supervisor/ })).not.toBeInTheDocument();
    expect(localStorage.getItem(storedKey("plenipo.workers.closed"))).toBe(
      JSON.stringify(["department:d-eng"]),
    );
  });

  it("lists an on-call worker's tasks now; one shows that conversation, watched only", async () => {
    const tree = await draw();
    const tasks = tree.getByRole("list", { name: "Senior Developer's tasks now" });
    // The one waiting for a free slot has no conversation yet.
    expect(within(tasks).queryByText(/Migrate the blog/)).not.toBeInTheDocument();
    fireEvent.click(within(tasks).getByRole("button", { name: "Build the new pricing page" }));
    const log = await screen.findByRole("log", { name: "Conversation with Senior Developer" });
    expect(
      await within(log).findByText("Build the new pricing page", {}, SLOW),
    ).toBeInTheDocument();
    expect(await screen.findByText(/takes its work from its lead/, {}, SLOW)).toBeInTheDocument();
    expect(screen.getByLabelText("Message to Senior Developer")).toBeDisabled();
  });

  it("shows a conversation another page links to, opening the groups that hold its worker", async () => {
    localStorage.setItem(
      storedKey("plenipo.workers.closed"),
      JSON.stringify(["department:d-eng", "project:pr-web"]),
    );
    render(<Harness initial="session-a-dev-1" />);
    const tree = within(await screen.findByRole("navigation", { name: "Your organization" }));
    await waitFor(
      () =>
        expect(tree.getByRole("button", { name: /^Senior Developer/ })).toHaveAttribute(
          "aria-current",
          "true",
        ),
      SLOW,
    );
    expect(tree.getByRole("button", { name: "Engineering" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(
      await screen.findByRole("log", { name: "Conversation with Senior Developer" }, SLOW),
    ).toBeInTheDocument();
  });

  it("lists conversations outside the organization, closes one, and starts one", async () => {
    await draw();
    const others = screen.getByRole("list", { name: "Other conversations" });
    expect(within(others).queryByText("Website Supervisor")).not.toBeInTheDocument();
    fireEvent.click(within(others).getByRole("button", { name: /Ask about tests/ }));
    expect(await screen.findByText("Tests are good.", {}, SLOW)).toBeInTheDocument();
    api.closeAgentSession.mockResolvedValue({ ...SESSIONS["s-own"]!, state: "closed" });
    fireEvent.click(screen.getByRole("button", { name: "Close conversation" }));
    await waitFor(() => expect(api.closeAgentSession).toHaveBeenCalledWith("s-own"));

    fireEvent.click(
      screen.getByRole("button", { name: "Start a conversation outside your organization" }),
    );
    const form = screen.getByRole("form", { name: "Start a conversation" });
    api.startAgentSession.mockResolvedValue(detail("s-new"));
    const user = userEvent.setup();
    await user.type(within(form).getByLabelText("What should it do?"), "Say hi");
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(api.startAgentSession).toHaveBeenCalledWith("claude-code", "Say hi", "", false);
    expect(
      await screen.findByRole("log", { name: "Conversation with Say hi" }, SLOW),
    ).toBeVisible();
  });

  it("says who a worker works with, and goes to them (B5)", async () => {
    const tree = await draw();
    fireEvent.click(tree.getByRole("button", { name: /^Website Supervisor/ }));
    const talks = await screen.findByLabelText("Talks with", {}, SLOW);
    expect(talks).toHaveTextContent("Asked Senior Developer (1 task)");
    fireEvent.click(within(talks).getByRole("button", { name: "Senior Developer" }));
    expect(tree.getByRole("button", { name: /^Senior Developer/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(await screen.findByLabelText("Talks with", {}, SLOW)).toHaveTextContent(
      "Asked by Website Supervisor (1 task)",
    );
  });

  it("says a vacant position has no chat", async () => {
    const tree = await draw();
    fireEvent.click(tree.getByRole("button", { name: /^Marketing Manager/ }));
    expect(
      screen.getByText("Vacant: no one holds this position yet, so it has no chat."),
    ).toBeInTheDocument();
  });
});
