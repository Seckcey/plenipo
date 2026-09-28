import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OrgSnapshot, PositionInfo, WorkView } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { samplePermissions } from "../../test/permissionFixtures";
import { position, sampleOrganization } from "../../test/orgFixtures";
import { sampleRouting } from "../../test/routingFixtures";
import { Inspector, type InspectorActions } from "./Inspector";
import { PANEL_TABS } from "./inspector/panel";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getWork: vi.fn(),
    getRouting: vi.fn(),
    getLearning: vi.fn(),
    getPermissions: vi.fn(),
    setModelRule: vi.fn(),
    setAgentLearning: vi.fn(),
    setRoleLearns: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
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

/** The sample organization, with one agent archived on its own and a kept lesson. */
function organization(): OrgSnapshot {
  const org = sampleOrganization();
  const dev = org.roles.find((r) => r.id === "r-dev")!;
  dev.specialties = [
    {
      id: "s-db",
      roleId: "r-dev",
      name: "Database",
      title: "Database Developer",
      builtIn: true,
      job: { duties: ["Design tables and queries."], returns: [], limits: [], askLead: [] },
      suggest: { needs: [], minContextTokens: 200_000, models: [], permissions: ["git.read"] },
    },
  ];
  const archived: PositionInfo = position("p-old", "Old Researcher", "r-research", "p-web", {
    active: false,
    status: "archived",
    archivedAt: Date.now() - 3_600_000,
    departmentId: "d-eng",
    projectId: "pr-web",
    experience: { score: 31, keptLessons: 2, tasksDone: 11, experienced: true },
  });
  org.positions.push(archived);
  const developer = org.positions.find((p) => p.id === "p-dev")!;
  developer.specialtyId = "s-db";
  developer.specialty = "Database";
  developer.route = {
    choice: {
      modelId: "m-opus",
      runtimeId: "claude-code",
      runtimeLabel: "Claude Code",
      company: "anthropic",
      model: "opus",
      effort: "high",
      label: "Opus (Claude Code)",
    },
    reason:
      "Opus (Claude Code) is Senior Developer's first choice and is ready. It runs at high effort, from Senior Developer's rule.",
    rank: 1,
    candidates: [],
    fixed: false,
    modelFrom: { layer: "role", name: "Senior Developer", id: "r-dev" },
    effortFrom: { layer: "role", name: "Senior Developer", id: "r-dev" },
  };
  developer.runtimeId = "claude-code";
  developer.model = "opus";
  developer.automatic = true;
  return { ...org, averageExperience: 12 };
}

function actions(): InspectorActions {
  return {
    run: vi.fn(async (work: () => Promise<OrgSnapshot>) => {
      await work();
      return null;
    }),
    change: vi.fn(async (work: () => Promise<unknown>) => {
      await work();
      return null;
    }),
    giveObjective: vi.fn(() => Promise.resolve(null)),
    hire: vi.fn(),
    newDepartment: vi.fn(),
    newProject: vi.fn(),
    newRole: vi.fn(),
    editDepartment: vi.fn(),
    editProject: vi.fn(),
    editRole: vi.fn(),
    newSpecialty: vi.fn(),
    editSpecialty: vi.fn(),
    rename: vi.fn(),
    confirm: vi.fn(),
    deleteForGood: vi.fn(),
    openSession: vi.fn(),
    openTask: vi.fn(),
    openPage: vi.fn(),
    api: {
      fill: vi.fn(),
      vacate: vi.fn(),
      update: vi.fn(),
      move: vi.fn(),
      archive: vi.fn(),
      assign: vi.fn(),
      endOversight: vi.fn(),
      archiveDepartment: vi.fn(),
      archiveProject: vi.fn(),
      bringBack: vi.fn(),
      saveToWorkforce: vi.fn(),
      sendHome: vi.fn(),
    },
  };
}

function show(selectedId: string, org = organization(), a = actions(), onWidth = vi.fn()) {
  render(
    <Inspector
      snapshot={org}
      selectedId={selectedId}
      revision={0}
      actions={a}
      onSelect={vi.fn()}
      onClose={vi.fn()}
      width={400}
      onWidth={onWidth}
    />,
  );
  return { org, actions: a, onWidth };
}

beforeEach(() => {
  api.getWork.mockImplementation((id) => Promise.resolve(noWork(id ?? null)));
  api.getRouting.mockResolvedValue(sampleRouting());
  api.getLearning.mockResolvedValue({
    enabled: true,
    autoRoles: [],
    offRoles: [],
    agents: {},
    waiting: [],
    kept: [
      {
        id: "l-1",
        roleId: "r-dev",
        positionId: "p-dev",
        worker: "Senior Developer",
        taskId: "t-1",
        text: "Run the tests before handing back.",
        state: "kept",
        fromWeb: false,
        createdAt: 0,
        decidedAt: 0,
        decidedBy: "owner",
        projectId: null,
        heldReason: null,
      },
    ],
  });
  const permissions = samplePermissions();
  permissions.settings.roles = [
    { roleId: "r-dev", roleName: "Senior Developer", fullTime: false, setId: "developer" },
  ];
  permissions.settings.projects = [{ id: "pr-web", name: "Website Relaunch", setId: "read-only" }];
  api.getPermissions.mockResolvedValue(permissions);
  api.setModelRule.mockResolvedValue(sampleRouting());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

/** The panel's own tabs. */
const details = () => within(screen.getByRole("tablist", { name: "Details" }));

/** The open tab of the panel (the Work tab has a list of its own inside). */
function tabPanel(): HTMLElement {
  const panel = document.querySelector<HTMLElement>('[id^="details-panel-"]');
  if (!panel) throw new Error("no open tab");
  return panel;
}

/** Waits for what each tab loads (the model settings, learning, permissions, work). */
async function settle(tab: string, active: boolean) {
  const panel = tabPanel();
  if (tab === "job") await within(panel).findByText("What it has learned");
  if (tab === "model" && active) await within(panel).findByText("Its own rule");
  if (tab === "work") {
    await within(panel).findByText("Its permissions");
    await within(panel).findAllByText("Nothing here.");
  }
}

/** A control's name: its label, or its own words. */
function nameOf(el: Element): string {
  const aria = el.getAttribute("aria-label");
  if (aria) return aria;
  if (el instanceof HTMLButtonElement) return (el.textContent ?? "").trim();
  const id = el.getAttribute("id");
  const byFor = id ? document.querySelector(`label[for="${id}"]`) : null;
  // A label around the control: its own words (not the menu's choices).
  const around = el.closest("label")?.querySelector(":scope > span");
  return ((byFor ?? around)?.textContent ?? "").trim();
}

/** Every option in the open tab: each control (not the links to other items, nor the tabs of
 * the Work list), its name, and the line that says what it does. */
function options(): { name: string; hint: string }[] {
  const panel = tabPanel();
  const controls = [...panel.querySelectorAll("button, select, input, textarea")].filter(
    (el) => !el.hasAttribute("data-nav") && el.getAttribute("role") !== "tab",
  );
  return controls.map((el) => {
    const ids = (el.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean);
    const hint = ids
      .map((id) => document.getElementById(id)?.textContent?.trim() ?? "")
      .join(" ")
      .trim();
    return { name: nameOf(el), hint };
  });
}

/** The words the owner should not see, from the "Not" column of the word list. */
function forbiddenWords(): string[] {
  // Tests run in apps/desktop.
  const doc = readFileSync(resolve(process.cwd(), "../../docs/design/vocabulary.md"), "utf8");
  const table = doc.slice(doc.indexOf("## Say this, not that"), doc.indexOf("## Where technical"));
  const words = new Set<string>();
  for (const line of table.split("\n")) {
    const cells = line.split("|").map((c) => c.trim());
    if (cells.length < 4 || cells[1] === "Say" || cells[1]?.startsWith("---")) continue;
    const say = (cells[1] ?? "").toLowerCase();
    for (const raw of (cells[2] ?? "").split(",")) {
      if (raw.includes("fine as is")) continue;
      const word = raw
        .replace(/\(.*?\)/g, "")
        .trim()
        .toLowerCase();
      // "Ultra" is said as "Ultra": its lower-case twin is the same word.
      if (word.length > 1 && !say.includes(word)) words.add(word);
    }
  }
  return [...words];
}

const PEOPLE: [string, string, boolean][] = [
  ["a full-time agent", "p-super", true],
  ["an on-call agent", "p-dev", true],
  ["a manager", "p-eng", true],
  ["a supervisor", "p-web", true],
  ["an archived agent", "p-old", false],
];

describe("The properties panel", () => {
  it("has six tabs, and every option on every tab says what it does", async () => {
    const user = userEvent.setup();
    const lines: string[] = [];
    const forbidden = forbiddenWords();
    expect(forbidden).toContain("runtime");
    for (const [who, id, active] of PEOPLE) {
      show(id);
      const tabs = screen.getAllByRole("tab").filter((t) => t.id.startsWith("details-tab-"));
      expect(tabs.map((t) => t.textContent)).toEqual(PANEL_TABS.map((t) => t.label));
      for (const tab of PANEL_TABS) {
        await user.click(details().getByRole("tab", { name: tab.label }));
        await settle(tab.value, active);
        for (const { name, hint } of options()) {
          expect(name, `${who} › ${tab.label}: a control without a name`).not.toBe("");
          expect(hint, `${who} › ${tab.label} › ${name}: no line saying what it does`).not.toBe("");
          for (const word of forbidden) {
            const re = new RegExp(`\\b${word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`, "i");
            expect(re.test(name) || re.test(hint), `${who} › ${name}: "${word}"`).toBe(false);
          }
          lines.push(`${who} › ${tab.label} › ${name} — ${hint}`);
        }
        if (tab.value === "model" && id === "p-dev") {
          // Its own rule, opened: every control of the rule editor says what it does too.
          await user.click(screen.getByRole("button", { name: "Give it its own rule" }));
          await user.selectOptions(
            screen.getByRole("combobox", { name: "Add a model to the list" }),
            "Opus (Claude Code)",
          );
          for (const { name, hint } of options()) {
            expect(hint, `${who} › its own rule › ${name}: no line saying what it does`).not.toBe(
              "",
            );
            lines.push(`${who} › ${tab.label} (its own rule) › ${name} — ${hint}`);
          }
        }
      }
      cleanup();
    }
    expect(lines.join("\n")).toMatchSnapshot();
  });

  it("changes only an agent's effort, which never hires a new agent", async () => {
    const user = userEvent.setup();
    const { actions: a } = show("p-dev");
    await user.click(details().getByRole("tab", { name: "AI model" }));
    expect(screen.getByTestId("route-reason")).toHaveTextContent(
      "It runs at high effort, from Senior Developer's rule.",
    );
    const effort = await screen.findByRole("combobox", { name: "Effort" });
    expect(effort).toHaveAccessibleDescription(
      "How hard the model thinks before it answers. Changing it never hires a new agent.",
    );
    await user.selectOptions(effort, "Max effort");
    expect(api.setModelRule).toHaveBeenCalledWith(
      { layer: "agent", id: "p-dev" },
      { models: [], efforts: {}, effort: "max", neverCompanies: [] },
    );
    expect(a.api.update).not.toHaveBeenCalled();
  });

  it("shows the effort set for the model it runs, and waits while its own rule is open", async () => {
    const user = userEvent.setup();
    const org = organization();
    const dev = org.positions.find((p) => p.id === "p-dev")!;
    dev.ownRule = { models: [], efforts: { "m-opus": "low" }, effort: "high", neverCompanies: [] };
    dev.route = { ...dev.route!, effortFrom: { layer: "agent", name: "this agent", id: "p-dev" } };
    show("p-dev", org);
    await user.click(details().getByRole("tab", { name: "AI model" }));
    const effort = await screen.findByRole("combobox", { name: "Effort" });
    // Its effort for Opus comes before its effort for any model.
    expect(effort).toHaveDisplayValue("Low effort");
    // Its own setting decides now, so following the rules would change it: no "now".
    expect(within(effort).getByRole("option", { name: "Follow the rules" })).toBeInTheDocument();
    // The choice here is for any model, so the one for Opus goes.
    await user.selectOptions(effort, "Max effort");
    expect(api.setModelRule).toHaveBeenCalledWith(
      { layer: "agent", id: "p-dev" },
      { models: [], efforts: {}, effort: "max", neverCompanies: [] },
    );
    // While its own rule is open, the effort waits: saving the rule would undo it.
    await user.click(screen.getByRole("button", { name: "Change its own rule" }));
    expect(screen.getByRole("combobox", { name: "Effort" })).toBeDisabled();
    expect(screen.getByText("Save or cancel its own rule below first.")).toBeInTheDocument();
  });

  it("says so when the model settings cannot be read", async () => {
    api.getRouting.mockRejectedValue({ kind: "internal", message: "the Ledger is busy" });
    const user = userEvent.setup();
    show("p-dev");
    await user.click(details().getByRole("tab", { name: "AI model" }));
    expect(
      await screen.findByText(/the model settings could not be read \(the Ledger is busy\)/),
    ).toBeInTheDocument();
  });

  it("names the rule that chose its model", () => {
    const org = organization();
    const dev = org.positions.find((p) => p.id === "p-dev")!;
    dev.route = {
      ...dev.route!,
      modelFrom: { layer: "department", name: "the Engineering department", id: "d-eng" },
    };
    show("p-dev", org);
    expect(screen.getByText("Automatic: the Engineering department's rule")).toBeInTheDocument();
  });

  it("offers People only to a position that leads a team", async () => {
    const user = userEvent.setup();
    show("p-dev");
    await user.click(details().getByRole("tab", { name: "Manage" }));
    expect(screen.queryByText("People")).not.toBeInTheDocument();
    cleanup();
    show("p-super");
    await user.click(details().getByRole("tab", { name: "Manage" }));
    expect(screen.getByText("People")).toBeInTheDocument();
  });

  it("brings back the department of a supervisor whose project went with it", async () => {
    const user = userEvent.setup();
    const org = organization();
    const project = org.projects.find((x) => x.id === "pr-camp")!;
    Object.assign(project, {
      active: false,
      archivedAt: Date.now() - 60_000,
      archivedWith: { kind: "department", id: "d-mkt", name: "Marketing" },
    });
    const supervisor = org.positions.find((p) => p.id === "p-camp")!;
    Object.assign(supervisor, { active: false, status: "archived", archivedAt: Date.now() });
    const { actions: a } = show("p-camp", org);
    await user.click(details().getByRole("tab", { name: "Manage" }));
    expect(screen.queryByRole("button", { name: /Bring back the Q4 Campaign project/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Bring back the Marketing department" }));
    expect(a.api.bringBack).toHaveBeenCalledWith("department", "d-mkt");
  });

  it("chooses a specialty and learning for one agent", async () => {
    const user = userEvent.setup();
    const { actions: a } = show("p-dev");
    await user.click(details().getByRole("tab", { name: "Job" }));
    const specialty = screen.getByRole("combobox", { name: "Specialty" });
    expect(specialty).toHaveDisplayValue("Database");
    expect(screen.getByText("Design tables and queries.")).toBeInTheDocument();
    await user.selectOptions(specialty, "");
    expect(a.api.update).toHaveBeenCalledWith("p-dev", { specialtyId: "" });
    api.setAgentLearning.mockResolvedValue({} as never);
    await user.selectOptions(screen.getByRole("combobox", { name: "This agent learns" }), "off");
    expect(api.setAgentLearning).toHaveBeenCalledWith("p-dev", false);
  });

  it("offers Bring back, Save to my Workforce, and Delete for good for an archived agent", async () => {
    const user = userEvent.setup();
    const { actions: a } = show("p-old");
    await user.click(details().getByRole("tab", { name: "Manage" }));
    await user.click(screen.getByRole("button", { name: "Bring back" }));
    expect(a.api.bringBack).toHaveBeenCalledWith("position", "p-old");
    await user.click(screen.getByRole("button", { name: "Save to my Workforce" }));
    expect(a.api.saveToWorkforce).toHaveBeenCalledWith("p-old");
    await user.click(screen.getByRole("button", { name: "Delete for good…" }));
    expect(a.deleteForGood).toHaveBeenCalledWith("position", "p-old");
  });

  it("archives a department from its manager's Team tab", async () => {
    const user = userEvent.setup();
    const { actions: a } = show("p-eng");
    await user.click(details().getByRole("tab", { name: "Team" }));
    await user.click(screen.getByRole("button", { name: "Archive department" }));
    expect(a.confirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Archive Engineering?",
        confirmLabel: "Archive department",
      }),
    );
  });

  it("can be widened from its edge, with the arrow keys too", async () => {
    const { onWidth } = show("p-dev");
    const edge = screen.getByRole("separator", { name: "Widen or narrow the details" });
    expect(edge).toHaveAttribute("aria-valuemin", "320");
    expect(edge).toHaveAttribute("aria-valuemax", "720");
    fireEvent.keyDown(edge, { key: "ArrowLeft" });
    await waitFor(() => expect(onWidth).toHaveBeenCalledWith(416));
  });
});
