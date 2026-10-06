import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import type { TaskCost, TaskCostPart, TaskTreeCost } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { TaskCostSummary } from "./TaskCostSummary";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getTaskTreeCost: vi.fn() };
});

const hub = vi.hoisted(() => ({ handlers: [] as ((e: unknown) => void)[] }));
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn((handler: (e: unknown) => void) => {
    hub.handlers.push(handler);
    return Promise.resolve(() => {
      hub.handlers = hub.handlers.filter((h) => h !== handler);
    });
  }),
}));

const api = vi.mocked(commands);

const cost = (patch: Partial<TaskCost> = {}): TaskCost => ({
  taskId: "t1",
  read: 0,
  reused: 0,
  written: 0,
  runs: 0,
  counted: 0,
  atLeast: false,
  spentMicros: 0,
  setAsideMicros: 0,
  notPriced: 0,
  notPricedMicros: 0,
  pricedBy: null,
  running: false,
  ...patch,
});

const part = (taskId: string, patch: Partial<TaskCostPart>, c: Partial<TaskCost>) => ({
  taskId,
  positionTitle: null,
  runtime: null,
  ...patch,
  cost: cost({ taskId, ...c }),
});

/** A lead on a paid key, a worker on a subscription, and a task no AI tool has run yet. */
function tree(total: Partial<TaskCost> = {}, more = false): TaskTreeCost {
  return {
    taskId: "t1",
    total: cost({
      read: 18_000,
      reused: 7_000,
      written: 6_000,
      runs: 3,
      counted: 3,
      spentMicros: 40_000,
      pricedBy: "service",
      ...total,
    }),
    parts: [
      part(
        "t1",
        { positionTitle: "Development Manager", runtime: "claude-code" },
        { read: 9_000, written: 3_000, runs: 1, counted: 1, spentMicros: 40_000 },
      ),
      part("t2", { runtime: "kimi" }, { runs: 2 }),
      part("t3", { positionTitle: "Developer" }, {}),
    ],
    more,
  };
}

beforeEach(() => {
  hub.handlers = [];
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("what a task cost (I2)", () => {
  it("adds up its workers' tokens and its paid keys' money, and lists each worker", async () => {
    api.getTaskTreeCost.mockResolvedValue(tree());
    render(<TaskCostSummary taskId="t1" />);
    const box = await screen.findByLabelText("Tokens and cost");
    expect(api.getTaskTreeCost).toHaveBeenCalledWith("t1");
    expect(
      within(box).getByText("Used 24K tokens · across 2 workers · $0.04 on paid keys"),
    ).toBeInTheDocument();
    expect(within(box).getByText("18K read (7,000 reused) · 6,000 written")).toBeInTheDocument();
    expect(
      within(box).getByText("spent $0.04, priced by the AI company's own bill"),
    ).toBeInTheDocument();
    // One line per worker that ran: its position, or its AI tool for one outside the org.
    const workers = within(box).getAllByRole("listitem");
    expect(workers.map((w) => w.textContent)).toEqual([
      "Development Manager · 12K tokens · $0.04",
      "kimi · no tokens reported",
    ]);
  });

  it("says so far once while it runs, and catches up when the Ledger records more", async () => {
    api.getTaskTreeCost.mockResolvedValue(tree({ running: true, setAsideMicros: 500_000 }));
    render(<TaskCostSummary taskId="t1" />);
    expect(
      await screen.findByText("Used 24K tokens · across 2 workers · $0.04 on paid keys so far"),
    ).toBeInTheDocument();
    expect(screen.getByText(/still running: up to \$0\.50 set aside/)).toBeInTheDocument();

    // The run finishes: it reads its cost again, then stops listening.
    api.getTaskTreeCost.mockResolvedValue(tree({ spentMicros: 90_000 }));
    await waitFor(() => expect(hub.handlers.length).toBe(1));
    act(() => hub.handlers[0]!({ eventType: "agent.result" }));
    expect(
      await screen.findByText("Used 24K tokens · across 2 workers · $0.09 on paid keys"),
    ).toBeInTheDocument();
    await waitFor(() => expect(hub.handlers.length).toBe(0));
  });

  it("says when no AI tool has worked on it, and when it has too many tasks to count", async () => {
    api.getTaskTreeCost.mockResolvedValue({ ...tree({ runs: 0 }), parts: [] });
    const { unmount } = render(<TaskCostSummary taskId="t1" />);
    expect(await screen.findByText("No AI tool has worked on it yet.")).toBeInTheDocument();
    unmount();

    api.getTaskTreeCost.mockResolvedValue(tree({}, true));
    render(<TaskCostSummary taskId="t1" />);
    expect(
      await screen.findByText("It has more than 200 tasks: only the first 200 are counted."),
    ).toBeInTheDocument();
  });

  it("is one line on a result card, and nothing before any AI tool works on it", async () => {
    api.getTaskTreeCost.mockResolvedValue(tree());
    const { unmount } = render(<TaskCostSummary taskId="t1" compact />);
    const box = await screen.findByLabelText("Tokens and cost");
    expect(box.textContent).toBe("Used 24K tokens · across 2 workers · $0.04 on paid keys");
    unmount();

    api.getTaskTreeCost.mockResolvedValue({ ...tree({ runs: 0 }), parts: [] });
    render(<TaskCostSummary taskId="t1" compact />);
    await waitFor(() => expect(api.getTaskTreeCost).toHaveBeenCalledTimes(2));
    expect(screen.queryByLabelText("Tokens and cost")).not.toBeInTheDocument();
  });
});
