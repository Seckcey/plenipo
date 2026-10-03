import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail, LedgerEvent } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { T0, sampleReport, sampleWork, workingCopy } from "../test/projectFixtures";
import { ProjectsView } from "./ProjectsView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getPermissions: vi.fn(),
    getProjectWork: vi.fn(),
    getObjectiveReport: vi.fn(),
    giveObjective: vi.fn(),
    removeWorkspace: vi.fn(),
    setUpDevelopment: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
let handlers: ((event: LedgerEvent) => void)[] = [];
const emit = (eventType: string) =>
  handlers.forEach((h) =>
    h({
      id: "e",
      sequence: 1,
      eventType,
      aggregateType: "workspace",
      aggregateId: "ws-1",
      actor: "system",
      correlationId: null,
      causationId: null,
      payload: {},
      createdAt: T0,
    } as unknown as LedgerEvent),
  );
const openTask = vi.fn();
const openApprovals = vi.fn();

function show() {
  return render(<ProjectsView onOpenTask={openTask} onOpenApprovals={openApprovals} />);
}

const user = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.setSystemTime(T0);
  api.getOrganization.mockResolvedValue(sampleOrganization());
  api.getPermissions.mockRejectedValue(new Error("not needed"));
  api.getProjectWork.mockResolvedValue(sampleWork());
  api.getObjectiveReport.mockResolvedValue(sampleReport());
  handlers = [];
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    handlers.push(handler);
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("Projects", () => {
  it("lists projects and shows the newest objective's result", async () => {
    show();
    const list = await screen.findByRole("list", { name: "Projects" });
    expect(within(list).getByRole("button", { name: /Website Relaunch/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(within(list).getByRole("button", { name: /Q4 Campaign/ })).toBeInTheDocument();
    const objectives = await screen.findByRole("list", { name: "Objectives" });
    expect(api.getProjectWork).toHaveBeenCalledWith("pr-web");
    expect(
      within(objectives).getByRole("button", { name: /Add a contact page/ }),
    ).toHaveTextContent("1 approval waiting");
    const result = await screen.findByRole("article", { name: "Result" });
    expect(api.getObjectiveReport).toHaveBeenCalledWith("task-obj-1");
    expect(within(result).getByRole("heading", { name: "Add a contact page" })).toBeInTheDocument();

    const summary = within(result).getByLabelText("Summary");
    expect(summary).toHaveTextContent("Tasks5");
    expect(summary).toHaveTextContent("1 passed, 1 failed");
    expect(summary).toHaveTextContent("(1 must be fixed)");
    expect(summary).toHaveTextContent("(1 not committed)");

    const workers = within(result).getByRole("region", { name: "Who worked on it" });
    expect(within(workers).getByText("Codex · gpt-5.5")).toBeInTheDocument();
    const files = within(result).getByRole("region", { name: "Files changed" });
    expect(within(files).getByText("src/styles.css")).toBeInTheDocument();
    expect(within(files).getByText("Not committed")).toBeInTheDocument();
    const checks = within(result).getByRole("region", { name: "Tests and programs" });
    expect(within(checks).getByText("Failed")).toBeInTheDocument();
    expect(within(result).getByText("Changes requested")).toBeInTheDocument();
    expect(within(result).getByText("The email field has no label")).toBeInTheDocument();
    expect(
      within(result).getByText("https://github.com/northwind/website/pull/42"),
    ).toBeInTheDocument();
    expect(within(result).getByLabelText("Needs your attention")).toHaveTextContent(
      "1 file changed but not committed",
    );

    await user().click(within(result).getByRole("button", { name: "Review in Approvals" }));
    expect(openApprovals).toHaveBeenCalled();
    await user().click(within(result).getByRole("button", { name: "Open in Activity" }));
    expect(openTask).toHaveBeenCalledWith("task-obj-1");
  });

  it("gives an objective for the project and follows its result", async () => {
    const detail = {
      session: {},
      turns: [
        { taskId: "task-old", number: 1 },
        { taskId: "task-new", number: 2 },
      ],
      activity: [],
    } as unknown as AgentSessionDetail;
    api.giveObjective.mockResolvedValue(detail);
    show();
    const form = await screen.findByRole("form", { name: "Give an objective" });
    const to = within(form).getByRole("combobox", { name: "Give it to" });
    expect(
      within(to)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Engineering Manager (Manager)", "Website Supervisor (Supervisor)"]);
    expect(to).toHaveValue("p-eng");
    await user().type(
      within(form).getByRole("textbox", { name: "Objective for Website Relaunch" }),
      "Add a pricing page",
    );
    await user().click(within(form).getByRole("button", { name: "Give objective" }));
    expect(api.giveObjective).toHaveBeenCalledWith("p-eng", "Add a pricing page", "pr-web");
    expect(await within(form).findByRole("status")).toHaveTextContent(
      "Objective given to Engineering Manager",
    );
    await waitFor(() => expect(api.getObjectiveReport).toHaveBeenCalledWith("task-new"));
  });

  it("does not send to a busy lead, and shows a refusal", async () => {
    api.giveObjective.mockRejectedValue({ kind: "invalidInput", message: "no such project" });
    show();
    const form = await screen.findByRole("form", { name: "Give an objective" });
    await user().selectOptions(within(form).getByRole("combobox", { name: "Give it to" }), "p-web");
    await user().type(within(form).getByRole("textbox"), "Ship it");
    expect(within(form).getByRole("button", { name: "Give objective" })).toBeDisabled();
    expect(within(form).getByText(/busy with its current objective/)).toBeInTheDocument();

    await user().selectOptions(within(form).getByRole("combobox", { name: "Give it to" }), "p-eng");
    await user().click(within(form).getByRole("button", { name: "Give objective" }));
    expect(await within(form).findByRole("alert")).toHaveTextContent("no such project");
  });

  it("removes a working copy after asking, keeping its branch", async () => {
    api.removeWorkspace.mockResolvedValue(
      sampleWork({ workingCopies: [workingCopy({ state: "removed", removedAt: T0 })] }),
    );
    show();
    const copies = await screen.findByRole("table", { name: "Working copies" });
    expect(within(copies).getByText("1 file not committed")).toBeInTheDocument();
    await user().click(
      within(copies).getByRole("button", {
        name: "Remove the working copy of plenipo/add-a-contact-page-corr0001",
      }),
    );
    const dialog = screen.getByRole("dialog", { name: "Remove working copy" });
    expect(dialog).toHaveTextContent("stays in the repository");
    expect(dialog).toHaveTextContent("1 file changed but not committed will be lost");
    await user().click(within(dialog).getByRole("button", { name: "Remove" }));
    expect(api.removeWorkspace).toHaveBeenCalledWith("ws-1");
    expect(await within(copies).findByText("Removed; the branch stays")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("reloads the project's work when its team's work changes", async () => {
    show();
    await screen.findByRole("list", { name: "Objectives" });
    const calls = api.getProjectWork.mock.calls.length;
    api.getProjectWork.mockResolvedValue(
      sampleWork({ objectives: [{ ...sampleWork().objectives[0]!, state: "succeeded" }] }),
    );
    act(() => emit("workspace.updated"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(api.getProjectWork.mock.calls.length).toBeGreaterThan(calls);
    const objectives = screen.getByRole("list", { name: "Objectives" });
    expect(await within(objectives).findByText("Done")).toBeInTheDocument();
  });

  it("sets up a Development project from nothing", async () => {
    api.getOrganization.mockResolvedValue(emptyOrganization());
    const after = sampleOrganization();
    after.projects = [
      ...after.projects,
      { ...after.projects[0]!, id: "pr-shop", name: "Shop", departmentId: "d-eng" },
    ];
    api.setUpDevelopment.mockResolvedValue(after);
    api.getProjectWork.mockResolvedValue(
      sampleWork({ projectId: "pr-shop", objectives: [], workingCopies: [] }),
    );
    show();
    expect(await screen.findByRole("heading", { name: "No projects yet" })).toBeInTheDocument();
    await user().click(screen.getByRole("button", { name: "Set up a Development project" }));
    const dialog = screen.getByRole("dialog", { name: "Set up a Development project" });
    expect(dialog).toHaveTextContent("creates the Development department with its VP");
    expect(dialog).toHaveTextContent(
      "Senior Developer, Code Reviewer, QA Engineer, Documentation Writer",
    );
    await user().type(within(dialog).getByRole("textbox", { name: "Name" }), "Shop");
    await user().type(within(dialog).getByRole("textbox", { name: /^Project folder/ }), "D:\\shop");
    await user().selectOptions(
      within(dialog).getByRole("combobox", { name: /^AI tool of the VP and the Supervisor/ }),
      "claude-code",
    );
    await user().click(within(dialog).getByRole("button", { name: "Set up" }));
    expect(api.setUpDevelopment).toHaveBeenCalledWith({
      project: {
        name: "Shop",
        description: "",
        allowedRuntimes: ["claude-code"],
        branchPerObjective: true,
        localPath: "D:\\shop",
      },
      runtimeId: "claude-code",
      hireNew: [],
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const list = screen.getByRole("list", { name: "Projects" });
    expect(within(list).getByRole("button", { name: /Shop/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(await screen.findByText("No objectives yet.")).toBeInTheDocument();
  });
});
