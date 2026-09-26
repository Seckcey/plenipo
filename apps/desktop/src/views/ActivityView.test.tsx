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
