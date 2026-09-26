import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, OrgSnapshot, WorkView } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { initialCamera, worldToScreen } from "../org/camera";
import { layoutOrganization } from "../org/layout";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { OrganizationView } from "./OrganizationView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getWork: vi.fn(),
    renameOrganization: vi.fn(),
    createRole: vi.fn(),
    createDepartment: vi.fn(),
    updateDepartment: vi.fn(),
    removeDepartment: vi.fn(),
    createProject: vi.fn(),
    updateProject: vi.fn(),
    archiveProject: vi.fn(),
    hirePosition: vi.fn(),
    fillPosition: vi.fn(),
    vacatePosition: vi.fn(),
    updatePosition: vi.fn(),
    movePosition: vi.fn(),
    archivePosition: vi.fn(),
    assignOversight: vi.fn(),
    endOversight: vi.fn(),
    giveObjective: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
let emitLedger: (event: LedgerEvent) => void = () => undefined;
const openSession = vi.fn();
const openTask = vi.fn();

const noWork = (positionId: string | null): WorkView => ({
  positionId,
  running: [],
  waiting: [],
  queued: [],
  recent: [],
  team: [],
});

function show(snapshot: OrgSnapshot = sampleOrganization()) {
  api.getOrganization.mockResolvedValue(snapshot);
  return render(<OrganizationView onOpenSession={openSession} onOpenTask={openTask} />);
}

/** Where a node's center is on screen (the canvas is 960×640 in tests; nothing measures it). */
function screenPoint(snapshot: OrgSnapshot, id: string): { clientX: number; clientY: number } {
  const layout = layoutOrganization(snapshot);
  const node = layout.byId.get(id);
  if (!node) throw new Error(`no node ${id}`);
  const view = { w: 960, h: 640 };
  const [x, y] = worldToScreen(
    initialCamera(layout.bounds, view),
    view,
    node.x + node.w / 2,
    node.y + node.h / 2,
  );
  return { clientX: x, clientY: y };
}

/** Drag from `from` (an element) to a screen point, the way a mouse does. */
function drag(
  from: Element,
  start: { clientX: number; clientY: number },
  to: { clientX: number; clientY: number },
) {
  fireEvent.pointerDown(from, { pointerId: 1, button: 0, ...start });
  fireEvent.pointerMove(window, {
    pointerId: 1,
    clientX: start.clientX + 20,
    clientY: start.clientY + 20,
  });
  fireEvent.pointerMove(window, { pointerId: 1, ...to });
}

function release(to: { clientX: number; clientY: number }) {
  fireEvent.pointerUp(window, { pointerId: 1, button: 0, ...to });
}

beforeEach(() => {
  sessionStorage.clear();
  api.getWork.mockImplementation((id) => Promise.resolve(noWork(id ?? null)));
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    emitLedger = handler;
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Organization view", () => {
  it("maps the organization: leads, teams, live workers, status, and totals", async () => {
    show();
    const map = await screen.findByRole("region", { name: "Organization topology" });
    for (const name of [
      "You, owner",
      "Northwind Studio, organization",
      "Superintendent, Working",
      "Engineering Manager, Idle",
      "Website Coordinator, Waiting on team",
      "Marketing Manager, Vacant",
      "Designer, Unavailable",
      "Worker for Senior Developer: Build the new pricing page, Working",
      "Worker for Senior Developer: Migrate the blog to the new theme, Queued",
    ]) {
      expect(within(map).getByRole("button", { name })).toBeInTheDocument();
    }
    // Links are labelled with the department, the project, and each worker's runtime.
    expect(within(map).getByText("Engineering")).toBeInTheDocument();
    expect(within(map).getByText("Website Relaunch")).toBeInTheDocument();
    expect(within(map).getByText("Codex")).toBeInTheDocument();
    // Oversight shows on the nodes as well as the links.
    expect(within(map).getAllByText(/Security → Website Coordinator/).length).toBeGreaterThan(0);
    const totals = screen.getByLabelText("At a glance");
    expect(within(totals).getByText("Positions").nextSibling).toHaveTextContent("111 vacant");
    expect(within(totals).getByText("Live workers").nextSibling).toHaveTextContent("3");
  });

  it("gives an objective to a staffed lead from its details", async () => {
    show();
    api.giveObjective.mockResolvedValue({} as never);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Engineering Manager, Idle" }));
    const details = screen.getByRole("complementary", { name: "Details: Engineering Manager" });
    const form = within(details).getByRole("form", { name: "Give an objective" });
    await user.type(within(form).getByRole("textbox"), "Plan the Q4 roadmap");
    await user.click(within(form).getByRole("button", { name: "Give objective" }));
    expect(api.giveObjective).toHaveBeenCalledWith("p-eng", "Plan the Q4 roadmap");
    expect(await within(form).findByRole("status")).toHaveTextContent("Objective given");
    // The snapshot is refreshed to show it working.
    await waitFor(() => expect(api.getOrganization).toHaveBeenCalledTimes(2));
    // A busy lead cannot take another objective yet; a vacant one needs an agent first.
    await user.click(screen.getByRole("button", { name: "Superintendent, Working" }));
    expect(screen.getByRole("button", { name: "Give objective" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Marketing Manager, Vacant" }));
    expect(screen.getByText(/is vacant\. Hire an agent/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Give objective" })).not.toBeInTheDocument();
  });

  it("hires by dragging a role from the palette onto a lead", async () => {
    const org = sampleOrganization();
    show(org);
    api.hirePosition.mockResolvedValue(org);
    const card = await screen.findByRole("button", { name: "Hire Researcher" });
    const target = screenPoint(org, "p-web");
    drag(card, { clientX: 10, clientY: 10 }, target);
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "Release to hire into this team",
    );
    release(target);

    const dialog = await screen.findByRole("dialog", { name: "Hire" });
    expect(within(dialog).getByRole("combobox", { name: "Reports to" })).toHaveDisplayValue(
      "Website Coordinator — Project Coordinator",
    );
    await userEvent.setup().click(within(dialog).getByRole("button", { name: "Hire" }));
    expect(api.hirePosition).toHaveBeenCalledWith({
      roleId: "r-research",
      title: "Researcher",
      reportsTo: "p-web",
      runtimeId: "claude-code",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("Hired Researcher.")).toBeInTheDocument();
  });

  it("drags a position onto a lead to reassign it or make it that team's auditor", async () => {
    const org = sampleOrganization();
    show(org);
    api.assignOversight.mockResolvedValue(org);
    api.movePosition.mockResolvedValue(org);
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    const target = screenPoint(org, "p-camp");
    drag(auditor, screenPoint(org, "p-sec"), target);
    expect(screen.getByRole("button", { name: "Campaign Coordinator, Idle" })).toHaveClass(
      "is-drop-target",
    );
    release(target);

    const menu = await screen.findByRole("menu", {
      name: "Security Auditor → Campaign Coordinator",
    });
    const items = within(menu)
      .getAllByRole("menuitem")
      .map((i) => i.textContent);
    // Its specialty comes first after reporting.
    expect(items[0]).toMatch(/^Report to Campaign Coordinator/);
    expect(items[1]).toMatch(/^Security auditor for Campaign Coordinator's team/);
    const user = userEvent.setup();
    await user.click(within(menu).getByRole("menuitem", { name: /^Security auditor/ }));
    expect(api.assignOversight).toHaveBeenCalledWith("p-sec", "p-camp", "security");
    expect(
      await screen.findByText(/is now Campaign Coordinator's security auditor/),
    ).toBeInTheDocument();

    drag(auditor, screenPoint(org, "p-sec"), target);
    release(target);
    await user.click(
      await screen.findByRole("menuitem", { name: /^Report to Campaign Coordinator/ }),
    );
    expect(api.movePosition).toHaveBeenCalledWith("p-sec", "p-camp");
  });

  it("explains a drop the rules refuse and shows the Ledger's own refusals", async () => {
    const org = sampleOrganization();
    show(org);
    const developer = await screen.findByRole("button", { name: "Senior Developer, Working" });
    const reviewer = screenPoint(org, "p-review");
    drag(developer, screenPoint(org, "p-dev"), reviewer);
    expect(screen.getByRole("button", { name: "Code Reviewer, Idle" })).toHaveClass(
      "is-drop-refused",
    );
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "Code Reviewer is an on-demand position; only persistent positions lead a team.",
    );
    release(reviewer);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    // Escape cancels a drag.
    drag(developer, screenPoint(org, "p-dev"), screenPoint(org, "p-camp"));
    fireEvent.keyDown(window, { key: "Escape" });
    release(screenPoint(org, "p-camp"));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    api.movePosition.mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "Campaign Coordinator's project does not allow the codex runtime",
      ),
    );
    drag(developer, screenPoint(org, "p-dev"), screenPoint(org, "p-camp"));
    release(screenPoint(org, "p-camp"));
    await userEvent.setup().click(await screen.findByRole("menuitem", { name: /^Report to/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("does not allow the codex runtime");
  });

  it("collapses and expands a team", async () => {
    show();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Collapse Website Coordinator's team (3 below)" }),
    );
    expect(
      screen.queryByRole("button", { name: "Senior Developer, Working" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Expand Website Coordinator's team (6 hidden)" }),
    );
    expect(screen.getByRole("button", { name: "Senior Developer, Working" })).toBeInTheDocument();
  });

  it("finds positions and lists the organization as tables", async () => {
    show();
    const user = userEvent.setup();
    const search = await screen.findByRole("searchbox", { name: "Find in the organization" });
    await user.type(search, "security{Enter}");
    expect(screen.getByText("1 match")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Superintendent, Working" })).toHaveClass(
      "is-dimmed",
    );
    expect(
      screen.getByRole("complementary", { name: "Details: Security Auditor" }),
    ).toBeInTheDocument();

    await user.clear(search);
    await user.click(screen.getByRole("button", { name: "List" }));
    const table = screen.getByRole("table", { name: "Positions" });
    expect(within(table).getAllByRole("row")).toHaveLength(12);
    await user.selectOptions(screen.getByRole("combobox", { name: "Status" }), "vacant");
    expect(
      within(screen.getByRole("table", { name: "Positions" })).getAllByRole("row"),
    ).toHaveLength(2);
    await user.click(screen.getByRole("tab", { name: "Projects (2)" }));
    const projects = screen.getByRole("table", { name: "Projects" });
    const campaign = within(projects)
      .getByRole("rowheader", { name: /Q4 Campaign/ })
      .closest("tr");
    expect(campaign).toHaveTextContent(/MarketingCampaign CoordinatorClaude CodeActive$/);
  });

  it("creates a department with its head, and starts empty with guidance", async () => {
    const empty = emptyOrganization();
    show(empty);
    api.createDepartment.mockResolvedValue(sampleOrganization());
    expect(await screen.findByText("Build your organization")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Create a department" }));
    const dialog = screen.getByRole("dialog", { name: "New department" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Research");
    expect(within(dialog).getByRole("textbox", { name: "Title" })).toHaveValue("Research Manager");
    await user.click(within(dialog).getByRole("button", { name: "Create department" }));
    expect(api.createDepartment).toHaveBeenCalledWith({
      name: "Research",
      description: "",
      head: { roleId: "r-manager", title: "Research Manager", runtimeId: "claude-code" },
    });
    // The snapshot the command returns is shown straight away.
    expect(
      await screen.findByRole("button", { name: "Engineering Manager, Idle" }),
    ).toBeInTheDocument();
  });

  it("keeps a refused dialog open with the Ledger's reason", async () => {
    show();
    api.hirePosition.mockRejectedValue(
      new commands.PlenipoCommandError(
        "invalidInput",
        "that team already has a member with this title",
      ),
    );
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Hire Code Reviewer" }));
    const dialog = screen.getByRole("dialog", { name: "Hire" });
    await user.click(within(dialog).getByRole("button", { name: "Hire" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "already has a member with this title",
    );
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("follows the Ledger: organization and task events reload it", async () => {
    show();
    await screen.findByRole("region", { name: "Organization topology" });
    expect(api.getOrganization).toHaveBeenCalledTimes(1);
    act(() =>
      emitLedger({
        seq: 9,
        id: "e9",
        taskId: null,
        executionId: null,
        source: "owner",
        destination: null,
        eventType: "org.position_created",
        payload: { title: "Researcher" },
        createdAt: 1,
      }),
    );
    await waitFor(() => expect(api.getOrganization).toHaveBeenCalledTimes(2));
  });

  it("zooms and fits from the canvas controls", async () => {
    show();
    const user = userEvent.setup();
    const zoom = await screen.findByLabelText("Zoom level");
    const before = zoom.textContent;
    await user.click(screen.getByRole("button", { name: "Zoom in" }));
    await waitFor(() => expect(zoom.textContent).not.toBe(before));
    await user.click(screen.getByRole("button", { name: "Show oversight links" }));
    expect(screen.getByRole("button", { name: "Show oversight links" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });
});
