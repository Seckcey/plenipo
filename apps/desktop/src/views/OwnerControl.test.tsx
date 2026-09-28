import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OrgSnapshot, SavedAgentInfo, WorkView } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { position, sampleOrganization } from "../test/orgFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { OrganizationView } from "./OrganizationView";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getWork: vi.fn(),
    getRouting: vi.fn(),
    bringBack: vi.fn(),
    previewDeleteForGood: vi.fn(),
    deleteForGood: vi.fn(),
    saveToWorkforce: vi.fn(),
    hireFromWorkforce: vi.fn(),
    deleteSavedAgent: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

const noWork = (positionId: string | null): WorkView => ({
  positionId,
  running: [],
  waiting: [],
  queued: [],
  recent: [],
  team: [],
});

const saved: SavedAgentInfo = {
  id: "w-1",
  title: "Database Developer",
  roleId: "r-dev",
  roleName: "Senior Developer",
  specialtyId: null,
  specialty: "Database",
  experience: { score: 57, keptLessons: 4, tasksDone: 17, experienced: true },
  places: ["Website Relaunch", "Engineering"],
  firstWorked: Date.now() - 86_400_000 * 20,
  lastWorked: Date.now() - 86_400_000,
  lessons: ["Run the migrations on a copy first."],
  runtimeId: null,
  model: null,
  savedAt: Date.now() - 3_600_000,
};

/** The sample organization with an archived project (and its team) and a saved agent. */
function organization(): OrgSnapshot {
  const org = sampleOrganization();
  const camp = org.projects.find((p) => p.id === "pr-camp")!;
  Object.assign(camp, { active: false, archivedAt: Date.now() - 7_200_000 });
  for (const p of org.positions.filter((x) => x.projectId === "pr-camp")) {
    Object.assign(p, {
      active: false,
      status: "archived",
      archivedAt: Date.now() - 7_200_000,
      archivedWith: { kind: "project", id: "pr-camp", name: "Q4 Campaign" },
    });
  }
  org.positions.push(
    position("p-scout", "Scout", "r-research", "p-web", {
      active: false,
      status: "archived",
      archivedAt: Date.now() - 600_000,
      departmentId: "d-eng",
      projectId: "pr-web",
    }),
    position("p-gone", "Old Writer", "r-docs", "p-web", {
      active: false,
      status: "archived",
      deleted: true,
    }),
  );
  return { ...org, workforce: [saved], averageExperience: 22 };
}

function show(org = organization()) {
  api.getOrganization.mockResolvedValue(org);
  render(<OrganizationView onOpenSession={vi.fn()} onOpenTask={vi.fn()} />);
  return org;
}

beforeEach(() => {
  sessionStorage.clear();
  sessionStorage.setItem("plenipo.orgMode", "list");
  api.getWork.mockImplementation((id) => Promise.resolve(noWork(id ?? null)));
  api.getRouting.mockResolvedValue(sampleRouting());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Archive, bring back, delete for good, and the Workforce", () => {
  it("lists what is archived, and brings it back", async () => {
    const org = show();
    api.bringBack.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("tab", { name: "Archived (5)" }));
    const projects = screen.getByRole("table", { name: "Archived projects" });
    expect(within(projects).getByRole("rowheader", { name: /Q4 Campaign/ })).toBeInTheDocument();
    expect(within(projects).getByText("3 agents")).toBeInTheDocument();
    const agents = screen.getByRole("table", { name: "Archived agents" });
    // Deleted for good: gone from every list.
    expect(within(agents).queryByText("Old Writer")).not.toBeInTheDocument();
    // Archived with the project: it comes back with it.
    const designer = within(agents)
      .getByRole("rowheader", { name: /Designer/ })
      .closest("tr")!;
    expect(designer).toHaveTextContent("with the Q4 Campaign project");
    expect(within(designer).queryByRole("button", { name: "Bring back Designer" })).toBeNull();
    await user.click(within(projects).getByRole("button", { name: "Bring back Q4 Campaign" }));
    expect(api.bringBack).toHaveBeenCalledWith("project", "pr-camp");
    expect(await screen.findByText("Brought back Q4 Campaign.")).toBeInTheDocument();
    await user.click(within(agents).getByRole("button", { name: "Save Scout to my Workforce" }));
    expect(api.saveToWorkforce).toHaveBeenCalledWith("p-scout");
  });

  it("asks before deleting for good, and saves the experienced agents unless told otherwise", async () => {
    const org = show();
    api.previewDeleteForGood.mockResolvedValue({
      kind: "project",
      id: "pr-camp",
      name: "Q4 Campaign",
      agents: [
        {
          positionId: "p-camp",
          title: "Campaign Supervisor",
          roleName: "Supervisor",
          experience: { score: 40, keptLessons: 3, tasksDone: 10, experienced: true },
        },
        {
          positionId: "p-design",
          title: "Designer",
          roleName: "Designer",
          experience: { score: 5, keptLessons: 0, tasksDone: 5, experienced: false },
        },
        {
          positionId: "p-docs",
          title: "Documentation Writer",
          roleName: "Documentation Writer",
          experience: { score: 31, keptLessons: 2, tasksDone: 11, experienced: true },
        },
      ],
      projects: ["Q4 Campaign"],
      departments: [],
      averageExperience: 22,
    });
    api.deleteForGood.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("tab", { name: "Archived (5)" }));
    await user.click(screen.getByRole("button", { name: "Delete Q4 Campaign for good" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete Q4 Campaign for good?" });
    expect(dialog).toHaveTextContent("This cannot be undone");
    expect(dialog).toHaveTextContent("a short record stays in the Ledger");
    // The experienced ones start out checked.
    const supervisor = within(dialog).getByRole("checkbox", { name: /Campaign Supervisor/ });
    const designer = within(dialog).getByRole("checkbox", { name: /^Designer/ });
    const writer = within(dialog).getByRole("checkbox", { name: /Documentation Writer/ });
    expect(supervisor).toBeChecked();
    expect(designer).not.toBeChecked();
    expect(writer).toBeChecked();
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      "2 agents saved to your Workforce; 1 agent deleted for good.",
    );
    await user.click(writer);
    await user.click(within(dialog).getByRole("button", { name: "Delete for good" }));
    expect(api.previewDeleteForGood).toHaveBeenCalledWith("project", "pr-camp");
    expect(api.deleteForGood).toHaveBeenCalledWith("project", "pr-camp", ["p-camp"]);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByText("Deleted for good; 1 saved to your Workforce.")).toBeInTheDocument();
  });

  it("keeps saved agents in the Workforce and hires them into a team", async () => {
    const org = show();
    api.hireFromWorkforce.mockResolvedValue(org);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("tab", { name: "Workforce (1)" }));
    const table = screen.getByRole("table", { name: "Workforce" });
    const row = within(table)
      .getByRole("rowheader", { name: /Database Developer/ })
      .closest("tr")!;
    expect(row).toHaveTextContent("Senior Developer (Database)");
    expect(row).toHaveTextContent("57 — 4 lessons you kept, 17 tasks done · experienced");
    expect(row).toHaveTextContent("Website Relaunch, Engineering");
    await user.click(
      within(row).getByRole("button", { name: "Hire Database Developer into a team" }),
    );
    const dialog = screen.getByRole("dialog", { name: "Hire Database Developer" });
    expect(within(dialog).getByRole("combobox", { name: /Reports to/ })).toHaveDisplayValue(
      /Engineering Manager|VP|Website Supervisor/,
    );
    await user.selectOptions(within(dialog).getByRole("combobox", { name: /Reports to/ }), "p-web");
    await user.click(within(dialog).getByRole("button", { name: "Hire" }));
    expect(api.hireFromWorkforce).toHaveBeenCalledWith("w-1", "p-web", "Database Developer");
  });

  it("deletes an agent in the Workforce for good only after asking", async () => {
    const org = show();
    api.deleteSavedAgent.mockResolvedValue({ ...org, workforce: [] });
    const user = userEvent.setup();
    await user.click(await screen.findByRole("tab", { name: "Workforce (1)" }));
    await user.click(screen.getByRole("button", { name: "Delete Database Developer for good" }));
    const dialog = screen.getByRole("dialog", { name: "Delete Database Developer for good?" });
    expect(api.deleteSavedAgent).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Delete for good" }));
    expect(api.deleteSavedAgent).toHaveBeenCalledWith("w-1");
  });
});
