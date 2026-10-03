import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { TaskBrief } from "@plenipo/types";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { position, worker } from "../../test/orgFixtures";
import { StopButton } from "./StopWork";
import { stopQuestion, workToStop } from "./whatToStop";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, cancelAgentTurn: vi.fn() };
});

const api = vi.mocked(commands);

const task = (patch: Partial<TaskBrief> = {}): TaskBrief => ({
  id: "t-1",
  objective: "Build the home page\nwith a hero",
  state: "running",
  positionId: "sup",
  positionTitle: "Website Supervisor",
  projectId: null,
  parentTaskId: null,
  sessionId: "session-sup",
  createdAt: 1,
  startedAt: 2,
  completedAt: null,
  ...patch,
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Stop on every worker (Phase 25, item 3.3)", () => {
  it("stops only a full-time agent's current task, and nothing while it's idle or queued", () => {
    const busy = position("sup", "Website Supervisor", "r-coord", null, {
      status: "working",
      currentTask: task(),
    });
    expect(workToStop(busy)).toEqual([
      { sessionId: "session-sup", objective: "Build the home page" },
    ]);
    // Waiting on its team: still stoppable.
    expect(workToStop({ ...busy, currentTask: task({ state: "blocked" }) })).toHaveLength(1);
    expect(workToStop({ ...busy, currentTask: task({ state: "queued" }) })).toEqual([]);
    expect(workToStop({ ...busy, currentTask: null })).toEqual([]);
  });

  it("stops each of an on-call position's workers that has started", () => {
    const dev = position("dev", "Senior Developer", "r-dev", "sup", {
      workers: [
        worker("a"),
        worker("b", { state: "blocked" }),
        worker("c", { state: "queued", sessionId: null }),
      ],
    });
    expect(workToStop(dev).map((w) => w.sessionId)).toEqual(["session-a", "session-b"]);
    expect(stopQuestion("Senior Developer", 2)).toBe("Stop Senior Developer's 2 tasks?");
    expect(stopQuestion("Alex", 1)).toBe("Stop Alex's task?");
  });

  it("asks first, keeps working on Keep working, and stops on Stop", async () => {
    api.cancelAgentTurn.mockResolvedValue({} as never);
    const user = userEvent.setup();
    const work = [{ sessionId: "session-sup", objective: "Build the home page" }];
    const { rerender } = render(<StopButton who="Alex" work={work} fullTime />);
    await user.click(screen.getByRole("button", { name: "Stop Alex" }));
    let dialog = screen.getByRole("dialog", { name: "Stop Alex's task?" });
    expect(within(dialog).getByRole("list", { name: "What stops" })).toHaveTextContent(
      "Build the home page",
    );
    expect(dialog).toHaveTextContent("Its conversation stays");
    await user.click(within(dialog).getByRole("button", { name: "Keep working" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.cancelAgentTurn).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Stop Alex" }));
    dialog = screen.getByRole("dialog", { name: "Stop Alex's task?" });
    await user.click(within(dialog).getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("session-sup");

    // Nothing to stop: no button.
    rerender(<StopButton who="Alex" work={[]} />);
    expect(screen.queryByRole("button", { name: "Stop Alex" })).not.toBeInTheDocument();
  });

  it("says what went wrong and stays open when a task can't be stopped", async () => {
    api.cancelAgentTurn.mockRejectedValue(new Error("That task has ended."));
    const user = userEvent.setup();
    render(
      <StopButton
        who="Senior Developer"
        work={[
          { sessionId: "s-1", objective: "One" },
          { sessionId: "s-2", objective: "Two" },
        ]}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Stop Senior Developer" }));
    const dialog = screen.getByRole("dialog", { name: "Stop Senior Developer's 2 tasks?" });
    await user.click(within(dialog).getByRole("button", { name: "Stop" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("That task has ended.");
    expect(api.cancelAgentTurn).toHaveBeenCalledTimes(2);
  });
});
