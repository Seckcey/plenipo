import { cleanup, render, screen, within } from "@testing-library/react";
import type { AgentActivity, AgentEvent } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AgentsContext, type AgentsContextValue } from "../agents/context";
import { initialAgentState } from "../agents/store";
import { position, worker } from "../test/orgFixtures";
import { LiveConversation } from "./LiveConversation";
import { NowLine } from "./NowLine";
import { liveProgress, liveWork, nowWords, progressWords, stepWords } from "./words";

let seq = 0;
const at = (event: AgentEvent, taskId = "t-1"): AgentActivity => ({
  sessionId: "s-1",
  taskId,
  seq: ++seq,
  ts: 1_000 + seq,
  event,
});

const plan = (done: number, total: number): AgentEvent => ({
  type: "plan",
  steps: Array.from({ length: total }, (_, i) => ({
    text: `Step ${i + 1}`,
    status: i < done ? "done" : i === done ? "inProgress" : "pending",
  })),
});

function provide(activity: Record<string, AgentActivity[]>, loadSession = vi.fn()) {
  const value = {
    state: { ...initialAgentState, activity, loaded: { "s-1": true } },
    loadSession,
  } as unknown as AgentsContextValue;
  return ({ children }: { children: React.ReactNode }) => (
    <AgentsContext.Provider value={value}>{children}</AgentsContext.Provider>
  );
}

afterEach(cleanup);

describe("the live conversation's words (Phase 25, item 3.1)", () => {
  it("says each step in plain words", () => {
    expect(stepWords("Read", "/repo/src/index.ts")).toBe("Reading index.ts");
    expect(stepWords("plenipo/write_file", "src/app.tsx")).toBe("Writing app.tsx");
    expect(stepWords("Edit", "C:\\code\\app.tsx")).toBe("Changing app.tsx");
    expect(stepWords("shell", "npm test")).toBe("Running `npm test`");
    expect(stepWords("plenipo/run_command", "git --version")).toBe("Running `git --version`");
    expect(stepWords("Grep", "TODO")).toBe("Searching for TODO");
    expect(stepWords("web search", "tauri windows")).toBe("Searching the web for tauri windows");
    expect(stepWords("WebFetch", "https://example.com")).toBe("Opening https://example.com");
    expect(stepWords("something_new", "")).toBe("Using something_new");
  });

  it("reads how far along it is from its plan, its steps, and its tokens", () => {
    const activity = [
      at(plan(1, 3)),
      at({ type: "toolUse", tool: "Read", summary: "a.ts" }),
      at({ type: "toolUse", tool: "Bash", summary: "npm test" }),
      at({ type: "usage", usage: { inputTokens: 3000, cachedInputTokens: 0, outputTokens: 210 } }),
      at(plan(2, 3)),
    ];
    const p = liveProgress(activity);
    expect(p).toEqual({ done: 2, total: 3, current: "Step 3", steps: 2, tokens: 3210 });
    expect(progressWords(p, 0, 4 * 60_000 + 5_000)).toBe(
      "Step 3 of 3 · 4 min · 2 steps · 3,210 tokens",
    );
    expect(progressWords(liveProgress([]), 0, 10_000)).toBe("under a minute");
    expect(progressWords(liveProgress([at(plan(3, 3))]), null, 0)).toBe("All 3 steps done");
  });

  it("says what it is doing now, for a tile", () => {
    expect(nowWords([at({ type: "toolUse", tool: "Bash", summary: "npm test" })])).toBe(
      "Running `npm test`",
    );
    expect(
      nowWords([
        at({ type: "toolUse", tool: "Bash", summary: "npm test" }),
        at({ type: "textDelta", text: "All" }),
      ]),
    ).toBe("Writing its answer");
    expect(nowWords([at(plan(0, 2))])).toBe("Step 1");
    expect(nowWords([])).toBeNull();
  });

  it("finds the work going on now, with its conversation", () => {
    const sup = position("sup", "Website Supervisor", "r-coord", null, {
      currentTask: {
        id: "t-1",
        objective: "Build it",
        state: "blocked",
        positionId: "sup",
        positionTitle: "Website Supervisor",
        projectId: null,
        parentTaskId: null,
        sessionId: "s-1",
        createdAt: 1,
        startedAt: 2,
        completedAt: null,
      },
    });
    expect(liveWork(sup).map((w) => w.taskId)).toEqual(["t-1"]);
    const dev = position("dev", "Senior Developer", "r-dev", "sup", {
      workers: [worker("a"), worker("b", { state: "queued", sessionId: null })],
    });
    expect(liveWork(dev).map((w) => w.taskId)).toEqual(["task-a"]);
  });
});

describe("the live conversation on screen (Phase 25, item 3.1)", () => {
  it("shows the words as they are typed, the steps, and how far along it is", () => {
    const activity = {
      "t-1": [
        at(plan(0, 2)),
        at({ type: "message", text: "I'll look at the page first." }),
        at({ type: "toolUse", tool: "Read", summary: "src/index.ts" }),
        at({ type: "toolResult", tool: "Read", isError: true, summary: "not found" }),
        at({ type: "textDelta", text: "The page " }),
        at({ type: "textDelta", text: "needs a title" }),
      ],
    };
    render(
      <LiveConversation
        taskId="t-1"
        sessionId="s-1"
        startedAt={Date.now() - 2 * 60_000}
        running
        who="Senior Developer"
      />,
      { wrapper: provide(activity) },
    );
    const live = screen.getByRole("region", { name: "Senior Developer's live conversation" });
    expect(within(live).getByLabelText("How far along")).toHaveTextContent(
      "Step 1 of 2 · 2 min · 1 step",
    );
    const log = within(live).getByRole("log", { name: "What Senior Developer says and does" });
    const lines = within(log)
      .getAllByRole("listitem")
      .map((l) => l.textContent);
    expect(lines).toEqual([
      "I'll look at the page first.",
      "Reading index.ts",
      "That didn't work: not found",
      "The page needs a title",
    ]);
    // Still typing: the last words have a caret.
    expect(log.querySelector(".live__caret")).not.toBeNull();
  });

  it("shows only the last lines in the details panel, and reads a conversation not loaded yet", () => {
    const load = vi.fn(() => Promise.resolve());
    const activity = {
      "t-2": [
        at({ type: "toolUse", tool: "Read", summary: "a.ts" }, "t-2"),
        at({ type: "toolUse", tool: "Read", summary: "b.ts" }, "t-2"),
        at({ type: "toolUse", tool: "Read", summary: "c.ts" }, "t-2"),
        at({ type: "toolUse", tool: "Read", summary: "d.ts" }, "t-2"),
      ],
    };
    render(
      <LiveConversation
        taskId="t-2"
        sessionId="s-9"
        startedAt={null}
        running
        who="Alex"
        lines={3}
      />,
      { wrapper: provide(activity, load) },
    );
    expect(
      within(screen.getByRole("log"))
        .getAllByRole("listitem")
        .map((l) => l.textContent),
    ).toEqual(["Reading b.ts", "Reading c.ts", "Reading d.ts"]);
    expect(load).toHaveBeenCalledWith("s-9");
  });

  it("puts what a working tile is doing now in its one line", () => {
    const dev = position("dev", "Senior Developer", "r-dev", null, {
      workers: [worker("a")],
    });
    const activity = {
      "task-a": [at({ type: "toolUse", tool: "Bash", summary: "npm test" }, "task-a")],
    };
    const { rerender } = render(<NowLine p={dev} className="line" otherwise="On call" />, {
      wrapper: provide(activity),
    });
    expect(screen.getByText("now: Running `npm test`")).toBeInTheDocument();
    rerender(<NowLine p={{ ...dev, workers: [] }} className="line" otherwise="On call" />);
    expect(screen.getByText("On call")).toBeInTheDocument();
  });
});
