import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, OrgSnapshot, WorkView } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { TOOLBAR_ROOM, forInset, initialCamera, uncovered, worldToScreen } from "../org/camera";
import { layoutOrganization } from "../org/layout";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { OrganizationView } from "./OrganizationView";
import { afterChange } from "../test/core";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getWork: vi.fn(),
    getRouting: vi.fn(),
    renameOrganization: vi.fn(),
    createRole: vi.fn(),
    updateRole: vi.fn(),
    createDepartment: vi.fn(),
    updateDepartment: vi.fn(),
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
    getLiveView: vi.fn(() => Promise.resolve({ workers: [], handoffs: [], at: 0 })),
    placeTiles: vi.fn(() => Promise.resolve()),
    tidyUp: vi.fn(() => Promise.resolve([])),
    lendAgent: vi.fn(),
    sendHome: vi.fn(),
    retargetOversight: vi.fn(),
    bringBack: vi.fn(),
    archiveDepartment: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLicense: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeControl: vi.fn(() => Promise.resolve(() => undefined)),
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
  // The first view leaves the toolbar's strip along the top clear.
  const [x, y] = worldToScreen(
    forInset(initialCamera(layout.bounds, uncovered(view, 0, TOOLBAR_ROOM)), 0, TOOLBAR_ROOM),
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
  // The first-time tour has been seen (it has its own test).
  localStorage.setItem("plenipo.canvasTour", "seen");
  api.getWork.mockImplementation((id) => Promise.resolve(noWork(id ?? null)));
  api.getRouting.mockResolvedValue(sampleRouting());
  // Every listener hears each event (the organization and the live view both listen).
  const listeners = new Set<(event: LedgerEvent) => void>();
  emitLedger = (event) => listeners.forEach((l) => l(event));
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    listeners.add(handler);
    return Promise.resolve(() => listeners.delete(handler));
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
      "You, President",
      "Northwind Studio, organization",
      "VP, Working",
      "Engineering Manager, Idle",
      "Website Supervisor, Waiting on team",
      "Marketing Manager, Vacant",
      "Designer, Unavailable",
      "Worker for Senior Developer: Build the new pricing page, Working",
      "Worker for Senior Developer: Migrate the blog to the new theme, Queued",
    ]) {
      expect(within(map).getByRole("button", { name })).toBeInTheDocument();
    }
    // Links are labelled with the department, the project, and each worker's AI tool.
    expect(within(map).getByText("Engineering")).toBeInTheDocument();
    expect(within(map).getByText("Website Relaunch")).toBeInTheDocument();
    const codex = within(map).getAllByText("Codex");
    expect(codex.some((el) => el.classList.contains("topo-chip"))).toBe(true);
    // Each position also names its AI tool.
    expect(codex.some((el) => el.classList.contains("topo-node__runtime"))).toBe(true);
    // Oversight shows on the nodes as well as the links.
    expect(within(map).getAllByText(/Security → Website Supervisor/).length).toBeGreaterThan(0);
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
    await user.click(screen.getByRole("button", { name: "VP, Working" }));
    expect(screen.getByRole("button", { name: "Give objective" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Marketing Manager, Vacant" }));
    expect(screen.getByText(/is vacant\. Hire an agent/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Give objective" })).not.toBeInTheDocument();
  });

  it("gives an objective with Enter from its details; Shift+Enter starts a new line", async () => {
    show();
    api.giveObjective.mockResolvedValue({} as never);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Engineering Manager, Idle" }));
    const details = screen.getByRole("complementary", { name: "Details: Engineering Manager" });
    const form = within(details).getByRole("form", { name: "Give an objective" });
    const box = within(form).getByRole("textbox");
    expect(box).toHaveAccessibleDescription(/Enter sends\. Shift and Enter start a new line\.$/);
    // Nothing written yet: Enter gives nothing.
    await user.type(box, "{Enter}");
    await user.type(box, "Plan the roadmap{Shift>}{Enter}{/Shift}for Q4");
    // A word still being put together (an IME) is only finished.
    fireEvent.keyDown(box, { key: "Enter", isComposing: true });
    expect(api.giveObjective).not.toHaveBeenCalled();
    await user.keyboard("{Enter}");
    expect(api.giveObjective).toHaveBeenCalledWith("p-eng", "Plan the roadmap\nfor Q4");
    expect(await within(form).findByRole("status")).toHaveTextContent("Objective given");
    // A busy lead: its button cannot be pressed, so Enter gives nothing (and adds no line).
    await user.click(screen.getByRole("button", { name: "VP, Working" }));
    const busy = within(screen.getByRole("form", { name: "Give an objective" })).getByRole(
      "textbox",
    );
    await user.type(busy, "More{Enter}");
    expect(busy).toHaveValue("More");
    expect(api.giveObjective).toHaveBeenCalledTimes(1);
  });

  it("hires by dragging a role from the palette onto a lead", async () => {
    const org = sampleOrganization();
    show(org);
    afterChange(api.hirePosition, org, api.getOrganization);
    const card = await screen.findByRole("button", { name: "Hire Researcher" });
    const target = screenPoint(org, "p-web");
    drag(card, { clientX: 10, clientY: 10 }, target);
    expect(screen.getByRole("status", { name: "" })).toHaveTextContent(
      "Release to hire into this team",
    );
    release(target);

    const dialog = await screen.findByRole("dialog", { name: "Hire" });
    expect(within(dialog).getByRole("combobox", { name: "Reports to" })).toHaveDisplayValue(
      "Website Supervisor",
    );
    // New positions follow their role's model choices unless you fix an AI tool.
    expect(within(dialog).getByRole("combobox", { name: /AI tool/ })).toHaveDisplayValue(
      "Automatic (the role's model choices)",
    );
    expect(within(dialog).queryByRole("textbox", { name: /Model/ })).not.toBeInTheDocument();
    await userEvent.setup().click(within(dialog).getByRole("button", { name: "Hire" }));
    expect(api.hirePosition).toHaveBeenCalledWith({
      roleId: "r-research",
      title: "Researcher",
      reportsTo: "p-web",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("Hired Researcher.")).toBeInTheDocument();
  });

  it("explains an automatic position's model and lets you fix an AI tool instead", async () => {
    const org = sampleOrganization();
    const dev = org.positions.find((x) => x.id === "p-dev")!;
    Object.assign(dev, {
      automatic: true,
      runtimeId: "codex",
      model: null,
      route: {
        choice: {
          modelId: "m-codex",
          runtimeId: "codex",
          runtimeLabel: "Codex",
          company: "openai",
          model: null,
          label: "Codex: its own choice",
        },
        reason:
          "Codex: its own choice is Senior Developer's second choice: Opus (Claude Code) was skipped because Claude Code is not signed in.",
        rank: 2,
        candidates: [
          {
            modelId: "m-opus",
            label: "Opus (Claude Code)",
            verdict: "skipped",
            note: "Claude Code is not signed in",
          },
          { modelId: "m-codex", label: "Codex: its own choice", verdict: "chosen", note: "" },
        ],
        fixed: false,
      },
    });
    show(org);
    afterChange(api.updatePosition, org, api.getOrganization);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /^Senior Developer, / }));
    const details = screen.getByRole("complementary", { name: "Details: Senior Developer" });
    expect(
      within(details).getByText("Automatic: Senior Developer model choices"),
    ).toBeInTheDocument();
    // Why this model, on the AI model tab.
    await user.click(within(details).getByRole("tab", { name: "AI model" }));
    expect(within(details).getByTestId("route-reason")).toHaveTextContent(
      "Opus (Claude Code) was skipped because Claude Code is not signed in",
    );
    expect(within(details).getByText(/Claude Code is not signed in$/)).toBeInTheDocument();
    // Fix it to Codex with a named model.
    const form = within(details).getByRole("form", { name: "AI tool and model" });
    const tool = within(form).getByRole("combobox", { name: "AI tool" });
    expect(tool).toHaveDisplayValue("Automatic (the rules pick)");
    await user.selectOptions(tool, "codex");
    // A name Codex does not list: type it.
    await user.selectOptions(
      within(form).getByRole("combobox", { name: "Model" }),
      "Type another name…",
    );
    await user.type(
      within(form).getByRole("textbox", { name: /Model name the AI tool accepts/ }),
      "gpt-x",
    );
    await user.click(within(form).getByRole("button", { name: "Save AI tool and model" }));
    expect(api.updatePosition).toHaveBeenCalledWith("p-dev", {
      runtimeId: "codex",
      model: "gpt-x",
    });
  });

  it("lists your subscriptions first, and paid AI tools only once their key works (Phase 25, item 1.5)", async () => {
    const org = sampleOrganization();
    org.runtimes = [
      ...org.runtimes,
      { id: "grok", label: "Grok", ready: true, company: "xAI", paid: false },
      { id: "xai-key", label: "xAI", ready: false, company: "xAI", paid: true },
      { id: "anthropic-key", label: "Anthropic", ready: true, company: "Anthropic", paid: true },
    ];
    show(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /^Website Supervisor, / }));
    const details = screen.getByRole("complementary", { name: "Details: Website Supervisor" });
    await user.click(within(details).getByRole("tab", { name: "Manage" }));
    await user.click(within(details).getByRole("button", { name: "Hire into team" }));
    const dialog = screen.getByRole("dialog", { name: "Hire" });
    const tool = within(dialog).getByRole("combobox", { name: /AI tool/ });
    const groups = within(tool).getAllByRole("group");
    expect(groups.map((g) => g.getAttribute("label"))).toEqual([
      "Your subscriptions",
      "Paid per use with your key",
    ]);
    expect(
      within(groups[0]!)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Claude Code", "Codex (not ready)", "Grok"]);
    // xAI's key isn't saved, so only Anthropic's is offered.
    expect(
      within(groups[1]!)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Anthropic"]);
  });

  it("points a position fixed to a paid AI tool with no key to the same company's subscription", async () => {
    const org = sampleOrganization();
    org.runtimes = [
      ...org.runtimes,
      { id: "grok", label: "Grok", ready: true, company: "xAI", paid: false },
      { id: "xai-key", label: "xAI", ready: false, company: "xAI", paid: true },
    ];
    org.positions = org.positions.map((p) =>
      p.id === "p-dev" ? { ...p, automatic: false, runtimeId: "xai-key", model: null } : p,
    );
    show(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /^Senior Developer, / }));
    const details = screen.getByRole("complementary", { name: "Details: Senior Developer" });
    await user.click(within(details).getByRole("tab", { name: "AI model" }));
    const form = within(details).getByRole("form", { name: "AI tool and model" });
    const tool = within(form).getByRole("combobox", { name: "AI tool" });
    // Still listed, as it is the saved choice.
    expect(tool).toHaveDisplayValue("xAI (not ready)");
    expect(tool).toHaveAccessibleDescription(
      "xAI's key isn't set up. To use your Grok subscription, choose Grok.",
    );
  });

  it("hires a position fixed to an AI tool and model", async () => {
    const org = sampleOrganization();
    show(org);
    afterChange(api.hirePosition, org, api.getOrganization);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /^Website Supervisor, / }));
    const details = screen.getByRole("complementary", { name: "Details: Website Supervisor" });
    await user.click(within(details).getByRole("tab", { name: "Manage" }));
    await user.click(within(details).getByRole("button", { name: "Hire into team" }));
    const dialog = screen.getByRole("dialog", { name: "Hire" });
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: /AI tool/ }),
      "claude-code",
    );
    // Claude Code's own models, your models, and the models seen in use.
    const model = within(dialog).getByRole("combobox", { name: "Model" });
    await within(model).findByRole("option", { name: "sonnet — made by Anthropic" });
    expect(
      within(model)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "The AI tool's default",
      "fable — made by Anthropic",
      "opus — made by Anthropic",
      "sonnet — made by Anthropic",
      "haiku — made by Anthropic",
      "claude-opus-5-5 — made by Anthropic",
      "Type another name…",
    ]);
    await user.selectOptions(model, "sonnet");
    await user.click(within(dialog).getByRole("button", { name: "Hire" }));
    expect(api.hirePosition).toHaveBeenCalledWith(
      expect.objectContaining({ reportsTo: "p-web", runtimeId: "claude-code", model: "sonnet" }),
    );
  });

  it("drags a position onto a lead to reassign it or make it that team's auditor", async () => {
    const org = sampleOrganization();
    show(org);
    afterChange(api.assignOversight, org, api.getOrganization);
    afterChange(api.movePosition, org, api.getOrganization);
    const auditor = await screen.findByRole("button", { name: "Security Auditor, Idle" });
    const target = screenPoint(org, "p-camp");
    drag(auditor, screenPoint(org, "p-sec"), target);
    expect(screen.getByRole("button", { name: "Campaign Supervisor, Idle" })).toHaveClass(
      "is-drop-target",
    );
    release(target);

    const menu = await screen.findByRole("menu", {
      name: "Security Auditor → Campaign Supervisor",
    });
    const items = within(menu)
      .getAllByRole("menuitem")
      .map((i) => i.textContent);
    // Move here first, then lending it (on call), then its specialty first among oversight.
    expect(items[0]).toMatch(/^Move hereReports to Campaign Supervisor for good/);
    expect(items[1]).toMatch(/^Lend for one objective/);
    expect(items[2]).toMatch(/^Lend until I send it home/);
    expect(items[3]).toMatch(/^Security auditor for Campaign Supervisor's team/);
    const user = userEvent.setup();
    await user.click(within(menu).getByRole("menuitem", { name: /^Security auditor/ }));
    expect(api.assignOversight).toHaveBeenCalledWith("p-sec", "p-camp", "security");
    expect(
      await screen.findByText(/is now Campaign Supervisor's security auditor/),
    ).toBeInTheDocument();

    drag(auditor, screenPoint(org, "p-sec"), target);
    release(target);
    await user.click(await screen.findByRole("menuitem", { name: /^Move here/ }));
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
      "Code Reviewer is an on-call position; only full-time positions lead a team.",
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
        "Campaign Supervisor's project does not allow the codex AI tool",
      ),
    );
    drag(developer, screenPoint(org, "p-dev"), screenPoint(org, "p-camp"));
    release(screenPoint(org, "p-camp"));
    await userEvent.setup().click(await screen.findByRole("menuitem", { name: /^Move here/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("does not allow the codex AI tool");
  });

  it("collapses and expands a team", async () => {
    show();
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Collapse Website Supervisor's team (3 below)" }),
    );
    expect(
      screen.queryByRole("button", { name: "Senior Developer, Working" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Expand Website Supervisor's team (6 hidden)" }),
    );
    expect(screen.getByRole("button", { name: "Senior Developer, Working" })).toBeInTheDocument();
  });

  it("finds positions and lists the organization as tables", async () => {
    show();
    const user = userEvent.setup();
    const search = await screen.findByRole("searchbox", { name: "Find in the organization" });
    await user.type(search, "security{Enter}");
    expect(screen.getByText("1 match")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "VP, Working" })).toHaveClass("is-dimmed");
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
    expect(campaign).toHaveTextContent(/MarketingCampaign SupervisorClaude CodeActive$/);
  });

  it("creates a department with its head, and starts empty with guidance", async () => {
    const empty = emptyOrganization();
    show(empty);
    afterChange(api.createDepartment, sampleOrganization(), api.getOrganization);
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
      head: { roleId: "r-manager", title: "Research Manager" },
    });
    // The snapshot the command returns is shown straight away.
    expect(
      await screen.findByRole("button", { name: "Engineering Manager, Idle" }),
    ).toBeInTheDocument();
  });

  it("hires a VP with the VP role already chosen", async () => {
    show(emptyOrganization());
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Hire a VP" }));
    const dialog = screen.getByRole("dialog", { name: /^Hire/ });
    expect(within(dialog).getByRole("combobox", { name: /^Role/ })).toHaveValue("r-super");
  });

  it("writes what a role does in plain words, and edits a role you created", async () => {
    const org = sampleOrganization();
    const dev = org.roles.find((r) => r.id === "r-dev")!;
    dev.job = {
      duties: ["Write the code for the task"],
      returns: ["What changed"],
      limits: ["Stay in the project folder"],
      askLead: ["The task is unclear"],
    };
    org.roles.push({
      ...dev,
      id: "r-scout",
      name: "Scout",
      description: "Finds suppliers.",
      template: false,
      job: { duties: ["Find three suppliers"], returns: [], limits: [], askLead: [] },
    });
    show(org);
    afterChange(api.createRole, org, api.getOrganization);
    afterChange(api.updateRole, org, api.getOrganization);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "+ Role" }));
    const dialog = screen.getByRole("dialog", { name: "New role" });
    await user.type(within(dialog).getByRole("textbox", { name: "Name" }), "Buyer");
    await user.type(
      within(dialog).getByRole("textbox", { name: /^What this role does/ }),
      "Compares offers.",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Its job/ }),
      "Compare prices\n- Note each source",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /^What it must not do/ }),
      "Buy anything",
    );
    await user.click(within(dialog).getByRole("button", { name: "Create role" }));
    expect(api.createRole).toHaveBeenCalledWith({
      name: "Buyer",
      description: "Compares offers.",
      kind: "worker",
      staffing: "onDemand",
      job: {
        duties: ["Compare prices", "Note each source"],
        returns: [],
        limits: ["Buy anything"],
        askLead: [],
      },
    });
    // A built-in role shows its instructions and cannot be edited.
    await user.click(screen.getByRole("button", { name: /^Senior Developer, / }));
    const details = screen.getByRole("complementary", { name: "Details: Senior Developer" });
    await user.click(within(details).getByRole("tab", { name: "Job" }));
    expect(within(details).getByText("What the Senior Developer role does")).toBeInTheDocument();
    expect(within(details).getByText("Stay in the project folder")).toBeInTheDocument();
    expect(within(details).queryByRole("button", { name: "Edit role" })).toBeNull();
    // Your own roles are listed on the organization, and editing keeps rank and staffing.
    await user.click(screen.getByRole("button", { name: `${org.name}, organization` }));
    await user.click(screen.getByRole("button", { name: "Edit Scout" }));
    const edit = screen.getByRole("dialog", { name: "Edit role: Scout" });
    expect(within(edit).queryByRole("combobox", { name: "Rank" })).toBeNull();
    const returns = within(edit).getByRole("textbox", { name: /^What it hands back/ });
    await user.type(returns, "A table of suppliers");
    await user.click(within(edit).getByRole("button", { name: "Save role" }));
    expect(api.updateRole).toHaveBeenCalledWith("r-scout", {
      name: "Scout",
      description: "Finds suppliers.",
      job: {
        duties: ["Find three suppliers"],
        returns: ["A table of suppliers"],
        limits: [],
        askLead: [],
      },
    });
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

  it("names the ranks with the organization's chosen titles", async () => {
    show({ ...sampleOrganization(), titles: "army" });
    const user = userEvent.setup();
    const map = await screen.findByRole("region", { name: "Organization topology" });
    // Job titles stay as written; each node shows its rank in the chosen set.
    const you = within(map).getByRole("button", { name: "You, General" });
    expect(you).toHaveTextContent("General");
    expect(within(map).getByRole("button", { name: "VP, Working" })).toHaveTextContent("Colonel");
    expect(
      within(map).getByRole("button", { name: "Engineering Manager, Idle" }),
    ).toHaveTextContent("Captain");
    expect(
      within(map).getByRole("button", { name: "Website Supervisor, Waiting on team" }),
    ).toHaveTextContent("Sergeant");
    expect(within(map).getByRole("button", { name: "Code Reviewer, Idle" })).toHaveTextContent(
      "Private",
    );
    // The palette hires a VP-class leader by its rank.
    expect(screen.getByRole("button", { name: "Hire Colonel" })).toBeInTheDocument();
    await user.click(you);
    const details = screen.getByRole("complementary", { name: "Details: You" });
    expect(details).toHaveTextContent("You run this organization as its General.");
    expect(within(details).getByRole("button", { name: "Hire a Colonel" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Website Supervisor, Waiting on team" }));
    const lead = screen.getByRole("complementary", { name: "Details: Website Supervisor" });
    expect(within(lead).getByText("Rank").nextSibling).toHaveTextContent("Sergeant");
  });

  it("zooms and fits from the canvas controls", async () => {
    show();
    const user = userEvent.setup();
    const zoom = await screen.findByLabelText("Zoom level");
    const before = zoom.textContent;
    await user.click(screen.getByRole("button", { name: "Zoom in" }));
    await waitFor(() => expect(zoom.textContent).not.toBe(before));
    await user.click(screen.getByRole("button", { name: "Oversight lines" }));
    expect(screen.getByRole("button", { name: "Oversight lines" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });
});
