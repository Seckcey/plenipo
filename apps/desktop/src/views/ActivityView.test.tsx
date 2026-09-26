import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, Task, TaskTimeline } from "@plenipo/types";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { LedgerPanel } from "../components/LedgerPanel";
import { ActivityView } from "./ActivityView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    listTasks: vi.fn(),
    listRecentEvents: vi.fn(),
    getTaskTimeline: vi.fn(),
    advanceSyntheticTask: vi.fn(),
    getLedgerStatus: vi.fn(),
    runIntegrityCheck: vi.fn(),
    createLedgerBackup: vi.fn(),
    exportLedger: vi.fn(),
    createSyntheticTask: vi.fn(),
    getTaskTree: vi.fn(),
  };
});
vi.mock("../api/events", () => ({ subscribeLedgerEvents: vi.fn() }));

const api = vi.mocked(commands);
let emit: (e: LedgerEvent) => void = () => undefined;

const task = (patch: Partial<Task> = {}): Task => ({
  id: "t1",
  parentTaskId: null,
  requestedBy: "owner",
  assignedTo: null,
  projectId: null,
  objective: "Synthetic diagnostic task #1",
  acceptanceCriteria: "",
  priority: 3,
  state: "queued",
  metadata: { synthetic: true },
  createdAt: 1_000,
  updatedAt: 1_000,
  startedAt: null,
  completedAt: null,
  ...patch,
});

let seq = 0;
const ev = (eventType: string, payload: Record<string, unknown>, taskId = "t1"): LedgerEvent => ({
  seq: ++seq,
  id: `e${seq}`,
  taskId,
  executionId: null,
  source: "owner",
  destination: null,
  eventType,
  payload,
  createdAt: 1_000 + seq,
});

function Harness() {
  const [selected, setSelected] = useState<string | null>(null);
  return <ActivityView selectedTaskId={selected} onSelectTask={setSelected} />;
}

beforeEach(() => {
  seq = 0;
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
  api.listTasks.mockResolvedValue([task()]);
  api.listRecentEvents.mockResolvedValue([]);
});

describe("Activity timeline", () => {
  it("shows a task's complete ordered trail, including rejected transitions", async () => {
    const trail: TaskTimeline = {
      task: task({ state: "running", startedAt: 2_000 }),
      children: [],
      events: [
        ev("task.created", { objective: "Synthetic diagnostic task #1" }),
        ev("task.transition_rejected", { from: "queued", to: "succeeded", reason: "diagnostics" }),
        ev("task.state_changed", { from: "queued", to: "running", reason: "diagnostics" }),
      ],
    };
    api.getTaskTimeline.mockResolvedValue(trail);
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: /Synthetic diagnostic task #1 — Queued/ }),
    );

    const list = await screen.findByRole("list", { name: "Activity trail" });
    const items = within(list).getAllByRole("listitem");
    expect(items.map((i) => i.getAttribute("data-event-type"))).toEqual([
      "task.created",
      "task.transition_rejected",
      "task.state_changed",
    ]);
    expect(items[1]).toHaveTextContent("Rejected: Queued → Succeeded is not allowed");
    expect(items[2]).toHaveTextContent("Queued → Running (diagnostics)");
    expect(items.map((i) => i.querySelector(".trail__seq")?.textContent)).toEqual([
      "1.",
      "2.",
      "3.",
    ]);
    expect(items.map((i) => i.getAttribute("data-seq"))).toEqual(["1", "2", "3"]);
  });

  it("offers only valid actions and surfaces a rejection", async () => {
    api.getTaskTimeline.mockResolvedValue({ task: task(), children: [], events: [] });
    api.advanceSyntheticTask.mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "invalid task transition for t1: queued -> running",
      ),
    );
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /Synthetic diagnostic task #1/ }));
    const actions = await screen.findByLabelText("Synthetic task actions");
    const labels = within(actions)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(labels).toEqual(["Start", "Fail", "Cancel", "Add step"]);
    await user.click(within(actions).getByRole("button", { name: "Start" }));
    expect(api.advanceSyntheticTask).toHaveBeenCalledWith("t1", "start");
    expect(await screen.findByRole("alert")).toHaveTextContent("queued -> running");
  });

  it("stays on the current task after adding a step", async () => {
    const parent = task({ state: "running" });
    api.listTasks.mockResolvedValue([parent]);
    api.getTaskTimeline.mockResolvedValue({ task: parent, children: [], events: [] });
    api.advanceSyntheticTask.mockResolvedValue(task({ id: "child", parentTaskId: "t1" }));
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /Synthetic diagnostic task #1/ }));
    const actions = await screen.findByLabelText("Synthetic task actions");
    await user.click(within(actions).getByRole("button", { name: "Add step" }));
    expect(api.advanceSyntheticTask).toHaveBeenCalledWith("t1", "addChild");
    // Still on the parent: its running-state actions (including Complete) remain.
    expect(within(actions).getByRole("button", { name: "Complete" })).toBeInTheDocument();
    expect(api.getTaskTimeline).not.toHaveBeenCalledWith("child");
  });

  it("hides actions for non-synthetic tasks", async () => {
    const real = task({ metadata: {}, objective: "Real work" });
    api.listTasks.mockResolvedValue([real]);
    api.getTaskTimeline.mockResolvedValue({ task: real, children: [], events: [] });
    render(<Harness />);
    await userEvent.setup().click(await screen.findByRole("button", { name: /Real work/ }));
    await screen.findByRole("list", { name: "Activity trail" });
    expect(screen.queryByLabelText("Synthetic task actions")).not.toBeInTheDocument();
  });

  it("updates live when the ledger commits an event", async () => {
    render(<Harness />);
    await userEvent.setup().click(await screen.findByRole("tab", { name: "All events" }));
    act(() => emit(ev("execution.succeeded", { label: "Echo test", exitCode: 0 }, null as never)));
    const all = await screen.findByRole("list", { name: "All events" });
    expect(within(all).getByText("Echo test: succeeded · exit 0")).toBeInTheDocument();
    await waitFor(() => expect(api.listTasks).toHaveBeenCalledTimes(2)); // debounced refresh
  });
});

describe("Ledger panel", () => {
  const status = {
    path: "C:/Users/me/AppData/Local/com.eightwest.plenipo/ledger/plenipo.db",
    schemaVersion: 1,
    sizeBytes: 2048,
    taskCount: 3,
    eventCount: 12,
    executionCount: 2,
    notices: [],
    lastIntegrityCheck: null,
    lastBackup: null,
    persistent: true,
  };

  it("shows ledger health and runs maintenance", async () => {
    api.getLedgerStatus.mockResolvedValue(status);
    api.runIntegrityCheck.mockResolvedValue({ ok: true, messages: ["ok"], checkedAt: 5 });
    api.createLedgerBackup.mockResolvedValue({
      path: "C:/backups/plenipo-backup-1.db",
      sizeBytes: 2048,
      createdAt: 1,
      verified: true,
    });
    const onTaskCreated = vi.fn();
    api.createSyntheticTask.mockResolvedValue(task({ id: "new" }));
    render(<LedgerPanel onTaskCreated={onTaskCreated} />);
    const facts = await screen.findByLabelText("Ledger details");
    expect(within(facts).getByText("12")).toBeInTheDocument();
    expect(screen.getByText(/Location: C:\/Users/)).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Run integrity check" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Integrity check passed.");
    await user.click(screen.getByRole("button", { name: "Create backup" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Backup saved and verified");
    await user.click(screen.getByRole("button", { name: "Create synthetic task" }));
    await waitFor(() => expect(onTaskCreated).toHaveBeenCalledWith("new"));
  });

  it("warns loudly when running on a temporary ledger", async () => {
    api.getLedgerStatus.mockResolvedValue({ ...status, persistent: false, path: null });
    render(<LedgerPanel onTaskCreated={vi.fn()} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("temporary ledger");
    expect(screen.getByRole("button", { name: "Create backup" })).toBeDisabled();
  });
});

describe("Delegation tree (Phase 4)", () => {
  const liaison = (depth: number) => ({
    liaison: { correlationId: "c0ffee00-1111", depth, protocol: "plenipo-liaison/1" },
  });
  const root = task({
    objective: "Write a parser",
    requestedBy: "owner",
    state: "succeeded",
    metadata: { ...liaison(0), sessionId: "s1", runtimeId: "codex" },
  });
  const review = task({
    id: "t2",
    parentTaskId: "t1",
    objective: "Review the parser",
    requestedBy: "agent:codex",
    state: "succeeded",
    metadata: { ...liaison(1), sessionId: "w1", runtimeId: "claude-code" },
  });

  it("shows a workflow's tasks across workers and selects one", async () => {
    api.listTasks.mockResolvedValue([review, root]);
    api.getTaskTimeline.mockImplementation((id) =>
      Promise.resolve({
        task: id === "t2" ? review : root,
        children: id === "t2" ? [] : [review],
        events: [
          ev("liaison.handoff_requested", {
            destination: "runtime:claude-code",
            objective: "Review the parser",
          }),
        ],
      }),
    );
    api.getTaskTree.mockImplementation((id) =>
      Promise.resolve({
        rootId: "t1",
        focusId: id,
        correlationId: "c0ffee00-1111",
        nodes: [
          { task: root, depth: 0, runtimeId: "codex", sessionId: "s1", handoff: null },
          {
            task: review,
            depth: 1,
            runtimeId: "claude-code",
            sessionId: "w1",
            handoff: {
              messageId: "m1",
              state: "answered",
              destinationLabel: "Claude Code",
              replyOutcome: "completed",
            },
          },
        ],
      }),
    );
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /Write a parser — Succeeded/ }));

    const tree = await screen.findByRole("list", { name: "Delegation tree" });
    const nodes = within(tree).getAllByRole("listitem");
    expect(nodes).toHaveLength(2);
    expect(nodes[0]).toHaveAttribute("aria-current", "true");
    expect(nodes[1]).toHaveTextContent("Review the parser");
    expect(nodes[1]).toHaveTextContent("Claude Code · reply: Completed");
    expect(screen.getByText(/One workflow \(c0ffee00\)/)).toBeInTheDocument();
    expect(
      within(screen.getByRole("list", { name: "Activity trail" })).getByText(
        "Handoff requested → claude-code: Review the parser",
      ),
    ).toBeInTheDocument();

    await user.click(within(nodes[1]!).getByRole("button", { name: "Review the parser" }));
    await waitFor(() => expect(api.getTaskTree).toHaveBeenLastCalledWith("t2"));
    const again = await screen.findByRole("list", { name: "Delegation tree" });
    await waitFor(() =>
      expect(within(again).getAllByRole("listitem")[1]).toHaveAttribute("aria-current", "true"),
    );
  });

  it("is not fetched for tasks outside a workflow", async () => {
    api.getTaskTimeline.mockResolvedValue({ task: task(), children: [], events: [] });
    render(<Harness />);
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: /Synthetic diagnostic task #1/ }));
    await screen.findByRole("list", { name: "Activity trail" });
    expect(api.getTaskTree).not.toHaveBeenCalled();
    expect(screen.queryByRole("list", { name: "Delegation tree" })).toBeNull();
  });
});
