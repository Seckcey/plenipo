import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { AgentSessionDetail, LedgerEvent, Lesson } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import type { useApprovals } from "../guard/usePermissions";
import type { Learning } from "../learning/useLearning";
import { emptyOrganization, largeOrganization, sampleOrganization } from "../test/orgFixtures";
import {
  NOW,
  event,
  sampleHome,
  sampleTaskRecord,
  sampleTimeline,
  sampleTree,
  sampleWorkView,
} from "../test/pageFixtures";
import { a11yProblems } from "../test/a11y";
import { approval, samplePermissions } from "../test/permissionFixtures";
import { sampleReport, sampleWork } from "../test/projectFixtures";
import { DepartmentPage } from "./DepartmentPage";
import { EventHistory, HISTORY_PAGE } from "./EventHistory";
import { HomePage } from "./HomePage";
import { ProjectPage } from "./ProjectPage";
import { TaskPage } from "./TaskPage";
import { WorkerPage } from "./WorkerPage";
import { homeLine, homePip, type HomeMood } from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getOrganization: vi.fn(),
    getHome: vi.fn(),
    getActivity: vi.fn(),
    getWork: vi.fn(),
    getScopeEvents: vi.fn(),
    getTaskEvents: vi.fn(),
    getProjectWork: vi.fn(),
    getProjectRecord: vi.fn(),
    getTaskRecord: vi.fn(),
    getTaskTree: vi.fn(),
    getTaskTimeline: vi.fn(),
    getObjectiveReport: vi.fn(),
    getPermissions: vi.fn(),
    getAgentSession: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();

type Approvals = ReturnType<typeof useApprovals>;

function approvals(pending = [approval()]): Approvals {
  return {
    queue: { pending, recent: [] },
    error: null,
    reload: vi.fn(() => Promise.resolve()),
    apply: vi.fn(),
  };
}

function learning(waiting: Lesson[] = []): Learning {
  return {
    snapshot: { enabled: true, autoRoles: [], offRoles: [], agents: {}, waiting, kept: [] },
    error: null,
    reload: vi.fn(() => Promise.resolve()),
    apply: vi.fn(),
  };
}

const lesson: Lesson = {
  id: "lesson-1",
  roleId: "r-dev",
  taskId: "task-web",
  positionId: "p-dev",
  worker: "Senior Developer",
  text: "Run the tests before pushing.\nThey take a minute.",
  state: "waiting",
  fromWeb: false,
  createdAt: NOW - 5 * 60_000,
  decidedAt: null,
  decidedBy: null,
};

beforeEach(() => {
  go.mockReset();
  localStorage.clear();
  api.getOrganization.mockResolvedValue(sampleOrganization());
  api.getHome.mockResolvedValue(sampleHome());
  api.getActivity.mockImplementation((scopes, from, to, buckets = 96) =>
    Promise.resolve(
      scopes.map(() => ({
        from,
        to,
        bucketMs: Math.ceil((to - from) / buckets),
        buckets: Array.from({ length: buckets }, (_, i) => ({
          events: i % 5 === 0 ? 2 : 0,
          problems: 0,
          waiting: 0,
        })),
      })),
    ),
  );
  api.getWork.mockResolvedValue(sampleWorkView());
  api.getScopeEvents.mockResolvedValue([]);
  api.getTaskEvents.mockResolvedValue([]);
  api.getProjectWork.mockResolvedValue(sampleWork());
  api.getProjectRecord.mockResolvedValue(sampleTaskRecord().record);
  api.getTaskRecord.mockResolvedValue(sampleTaskRecord());
  api.getTaskTree.mockResolvedValue(sampleTree());
  api.getTaskTimeline.mockImplementation((id) => Promise.resolve(sampleTimeline(id)));
  api.getObjectiveReport.mockResolvedValue(sampleReport());
  api.getPermissions.mockResolvedValue(samplePermissions());
  api.getAgentSession.mockRejectedValue(new commands.PlenipoCommandError("internal", "none"));
});

describe("Home", () => {
  it("greets you, says how things are, and lists what waits for you and what's stuck", async () => {
    render(<HomePage go={go} approvals={approvals()} learning={learning([lesson])} />);
    const hero = await screen.findByRole("region", {
      name: /Good (morning|afternoon|evening)|Working late/,
    });
    await waitFor(() =>
      expect(hero).toHaveTextContent(
        "2 things are waiting for you, 2 things are stuck, and 4 workers are working.",
      ),
    );
    // Something is stuck, so Pip is here to help.
    expect(hero.querySelector("[data-pip]")).toHaveAttribute("data-pip", "support");

    const waiting = screen.getByRole("list", { name: "Waiting for you" });
    expect(within(waiting).getByText("git push origin")).toBeInTheDocument();
    expect(within(waiting).getByText("Senior Developer learned something")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(within(waiting).getByRole("button", { name: /git push origin/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "approvals", id: null });

    const stuck = await screen.findByRole("list", { name: "What's stuck" });
    const failed = within(stuck).getByRole("button", { name: /Ship the newsletter/ });
    expect(failed).toHaveTextContent("Campaign Supervisor");
    expect(failed).toHaveTextContent("Failed");
    await user.click(failed);
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-fail" });
    // A position that cannot work is stuck too.
    await user.click(within(stuck).getByRole("button", { name: /Designer can't work/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "worker", id: "p-design" });
  });

  it("shows each department's health, who's working, the objectives going, and what finished", async () => {
    render(<HomePage go={go} approvals={approvals([])} learning={learning()} />);
    const departments = await screen.findByRole("list", { name: "Departments" });
    // Marketing has a Designer who cannot work and a stuck objective.
    const marketing = await within(departments).findByRole("article", {
      name: "Marketing, 1 thing stuck",
    });
    expect(marketing).toHaveTextContent("Manager: Marketing Manager · Vacant");
    const engineering = within(departments).getByRole("article", { name: /^Engineering, / });
    expect(engineering).toHaveTextContent("Manager: Engineering Manager");
    const user = userEvent.setup();
    await user.click(within(engineering).getByRole("button", { name: "Engineering" }));
    expect(go).toHaveBeenLastCalledWith({ view: "department", id: "d-eng" });

    const working = screen.getByRole("list", { name: "Who's working" });
    expect(within(working).getByText("Build the new pricing page")).toBeInTheDocument();
    expect(within(working).getByText("Verify checkout on mobile")).toBeInTheDocument();
    await user.click(within(working).getByRole("button", { name: /Verify checkout on mobile/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "worker", id: "p-qa" });

    const current = screen.getByRole("list", { name: "Current objectives" });
    expect(within(current).getByRole("button", { name: /Relaunch the website/ })).toHaveTextContent(
      "Website Relaunch",
    );
    const finished = screen.getByRole("list", { name: "Just finished" });
    const done = within(finished).getByRole("button", { name: /Fix the typo on the pricing page/ });
    expect(done).toHaveTextContent("Fixed the typo and checked the page.");
    expect(done).not.toHaveTextContent("More detail follows");
    await user.click(done);
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-obj-0" });
  });

  it("welcomes a new company and offers to set it up", async () => {
    api.getOrganization.mockResolvedValue(emptyOrganization());
    api.getHome.mockResolvedValue({
      current: [],
      finished: [],
      stuck: [],
      going: 0,
      finishedDay: 0,
    });
    render(<HomePage go={go} approvals={approvals([])} learning={learning()} />);
    const hero = await screen.findByRole("region", {
      name: /Good (morning|afternoon|evening)|Working late/,
    });
    await waitFor(() =>
      expect(hero.querySelector("[data-pip]")).toHaveAttribute("data-pip", "welcome"),
    );
    await userEvent.setup().click(screen.getByRole("button", { name: "Set up your company" }));
    expect(go).toHaveBeenLastCalledWith({ view: "organization", id: null });
    expect(screen.getByText("No departments yet")).toBeInTheDocument();
    expect(screen.getByText("Nothing is stuck")).toBeInTheDocument();
  });

  it("says when it couldn't read what's stuck, and tries again", async () => {
    api.getHome.mockRejectedValueOnce(new commands.PlenipoCommandError("internal", "Ledger busy"));
    render(<HomePage go={go} approvals={approvals([])} learning={learning()} />);
    expect(await screen.findByText("Couldn't load What's stuck")).toBeInTheDocument();
    // Not "nothing stuck": it couldn't check.
    const tiles = screen.getByLabelText("How things are");
    expect(within(tiles).getByText("Couldn't check")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /Good|Working late/ })).toHaveTextContent(
      "Plenipo couldn't read how things are",
    );
    await userEvent.setup().click(screen.getAllByRole("button", { name: "Try again" })[0]!);
    expect(await screen.findByRole("list", { name: "What's stuck" })).toBeInTheDocument();
  });

  it("never says nothing is waiting when it couldn't read the requests", async () => {
    const failed = { ...approvals([]), queue: null, error: "Guard is busy" };
    render(<HomePage go={go} approvals={failed} learning={learning()} />);
    const waiting = await screen.findByRole("region", { name: "Waiting for you" });
    expect(within(waiting).getByText("Guard is busy")).toBeInTheDocument();
    expect(within(waiting).queryByText("Nothing is waiting for you")).toBeNull();
    const tiles = screen.getByLabelText("How things are");
    expect(within(tiles).getByText("Couldn't check")).toBeInTheDocument();
    await userEvent.setup().click(within(waiting).getByRole("button", { name: "Try again" }));
    expect(failed.reload).toHaveBeenCalled();
  });

  it("stays usable with a large organization: a card per department, everyone working listed", async () => {
    api.getOrganization.mockResolvedValue(largeOrganization());
    api.getHome.mockResolvedValue({
      current: [],
      finished: [],
      stuck: [],
      going: 0,
      finishedDay: 0,
    });
    const started = performance.now();
    render(<HomePage go={go} approvals={approvals([])} learning={learning()} />);
    const departments = await screen.findByRole("list", { name: "Departments" });
    await waitFor(() => expect(within(departments).getAllByRole("article")).toHaveLength(20));
    expect(
      within(screen.getByRole("list", { name: "Who's working" })).getAllByRole("listitem"),
    ).toHaveLength(100);
    expect(performance.now() - started).toBeLessThan(5_000);
    // One activity query for all the departments' strips.
    expect(api.getActivity).toHaveBeenCalledTimes(1);
    expect(api.getActivity.mock.calls[0]![0]).toHaveLength(20);
  });

  it("picks Pip's pose and the line from what matters most", () => {
    const mood: HomeMood = {
      loading: false,
      failed: false,
      empty: false,
      waiting: 0,
      stuck: 0,
      working: 0,
      finishedDay: 0,
    };
    expect(homePip(mood)).toBe("recharging");
    expect(homeLine(mood)).toBe("All quiet. Nobody is working right now.");
    expect(homePip({ ...mood, finishedDay: 2 })).toBe("celebrating");
    expect(homeLine({ ...mood, finishedDay: 2 })).toBe(
      "All quiet. 2 objectives finished in the last day.",
    );
    expect(homePip({ ...mood, working: 1 })).toBe("coding");
    expect(homePip({ ...mood, working: 1, waiting: 1 })).toBe("presenting");
    expect(homeLine({ ...mood, working: 1, waiting: 1 })).toBe(
      "1 thing is waiting for you, and 1 worker is working.",
    );
    expect(homePip({ ...mood, waiting: 1, stuck: 1 })).toBe("support");
    expect(homePip({ ...mood, loading: true })).toBe("launch");
  });
});

describe("A department's page", () => {
  it("shows its Manager, projects, workers, queue, and history, a page at a time", async () => {
    const page = Array.from({ length: HISTORY_PAGE }, (_, i) =>
      event("task.created", { objective: `Step ${i}` }, { taskId: "task-web", seq: 5000 - i }),
    );
    api.getScopeEvents.mockImplementation((_scope, _limit, before) =>
      Promise.resolve(
        before === undefined
          ? page
          : [event("task.created", { objective: "The oldest one" }, { taskId: "task-q", seq: 10 })],
      ),
    );
    const back = vi.fn();
    render(<DepartmentPage id="d-eng" go={go} onBack={back} />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Engineering" }),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(back).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Engineering Manager" }));
    expect(go).toHaveBeenLastCalledWith({ view: "worker", id: "p-eng" });

    const projects = screen.getByRole("list", { name: "Projects" });
    await user.click(within(projects).getByRole("button", { name: /Website Relaunch/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "project", id: "pr-web" });
    expect(screen.getByRole("list", { name: "Working now" })).toHaveTextContent(
      "Build the new pricing page",
    );
    const queue = await screen.findByRole("list", { name: "Queue" });
    expect(within(queue).getByText("Write the release notes")).toBeInTheDocument();
    expect(api.getWork).toHaveBeenCalledWith("p-eng");
    expect(
      await screen.findByRole("img", { name: /Engineering activity, last 7 days/ }),
    ).toBeInTheDocument();

    const history = await screen.findByRole("list", { name: "Engineering history" });
    expect(within(history).getAllByRole("listitem")).toHaveLength(HISTORY_PAGE);
    await user.click(screen.getByRole("button", { name: "Show older" }));
    expect(await within(history).findByText(/The oldest one/)).toBeInTheDocument();
    expect(api.getScopeEvents).toHaveBeenLastCalledWith(
      { kind: "department", id: "d-eng" },
      HISTORY_PAGE,
      5000 - HISTORY_PAGE + 1,
    );
    // Fewer than a page came back: that was the oldest.
    expect(screen.queryByRole("button", { name: "Show older" })).toBeNull();
  });

  it("says so when the department is gone", async () => {
    render(<DepartmentPage id="d-gone" go={go} />);
    expect(await screen.findByText("Plenipo can't find this department")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Go to Home" }));
    expect(go).toHaveBeenLastCalledWith({ view: "home", id: null });
  });
});

describe("A project's page", () => {
  it("shows its Supervisor, objectives, task tree, pull requests, and decisions", async () => {
    render(<ProjectPage id="pr-web" go={go} />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Website Relaunch" }),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Website Supervisor" }));
    expect(go).toHaveBeenLastCalledWith({ view: "worker", id: "p-web" });
    const objectives = await screen.findByRole("list", { name: "Objectives" });
    expect(within(objectives).getByText("Add a contact page")).toBeInTheDocument();
    const tree = await screen.findByRole("group", { name: "Task tree" });
    await user.click(await within(tree).findByRole("button", { name: /Website Supervisor/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-web" });
    const branches = await screen.findByRole("list", { name: "Branches and pull requests" });
    expect(within(branches).getByText("Pull request #12")).toBeInTheDocument();
    const decisions = screen.getByRole("list", { name: "Recent decisions" });
    expect(within(decisions).getByText("Approved: Run npm publish")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Give an objective" }));
    expect(go).toHaveBeenLastCalledWith({ view: "projects", id: "pr-web" });
  });
});

describe("A worker's page", () => {
  it("shows its role, AI tool and why, its work, permissions in use, and conversation", async () => {
    const org = sampleOrganization();
    const web = org.positions.find((p) => p.id === "p-web")!;
    web.route = {
      choice: null,
      reason: "Claude Code is its role's first choice.",
      rank: 1,
      candidates: [],
      fixed: false,
      modelFrom: null,
      effortFrom: null,
    };
    web.agent = { ...web.agent!, sessionId: "session-web" };
    api.getOrganization.mockResolvedValue(org);
    api.getPermissions.mockResolvedValue(
      samplePermissions({
        grants: [
          { ...samplePermissions().grants[0]!, positionId: "p-web", grantId: "grant-web" },
          { ...samplePermissions().grants[0]!, positionId: "p-other", grantId: "grant-other" },
        ],
      }),
    );
    const detail: AgentSessionDetail = {
      session: {} as AgentSessionDetail["session"],
      turns: [
        {
          taskId: "task-web",
          sessionId: "session-web",
          number: 1,
          objective: "Build the pricing page",
          executionId: null,
          running: false,
          waiting: false,
          result: {
            outcome: "completed",
            summary: "Done",
            text: "Built the pricing page.",
            error: null,
            providerSessionId: null,
            model: null,
            usage: null,
            durationMs: 1000,
            ignoredLines: 0,
          },
          steps: [],
          startedAt: NOW - 600_000,
          endedAt: NOW - 300_000,
        },
      ],
      activity: [],
    };
    api.getAgentSession.mockResolvedValue(detail);
    const openSession = vi.fn();
    render(<WorkerPage id="p-web" go={go} onOpenSession={openSession} />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Website Supervisor" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Claude Code is its role's first choice.")).toBeInTheDocument();
    const permissions = await screen.findByRole("list", { name: "Permissions in use" });
    expect(within(permissions).getAllByRole("listitem")).toHaveLength(1);
    expect(permissions).toHaveTextContent("Read files, Run PowerShell scripts");
    const conversation = await screen.findByRole("list", { name: "Conversation" });
    expect(await within(conversation).findByText("Built the pricing page.")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Open the conversation" }));
    expect(openSession).toHaveBeenCalledWith("session-web");
    const now = await screen.findByRole("list", { name: "Working on" });
    await user.click(within(now).getByRole("button", { name: /Build the pricing page/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-web" });
  });
});

describe("A task's page", () => {
  it("shows its objective, what counts as done, the tree, approvals, result, and activity", async () => {
    api.getTaskRecord.mockResolvedValue(
      sampleTaskRecord({ approvals: [approval({ taskId: "task-web" })] }),
    );
    const activity: LedgerEvent[] = [
      event("task.state_changed", { from: "queued", to: "running" }, { taskId: "task-web" }),
    ];
    api.getTaskEvents.mockResolvedValue(activity);
    render(<TaskPage id="task-obj-1" go={go} onOpenSession={vi.fn()} />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Relaunch the website" }),
    ).toBeInTheDocument();
    expect(screen.getByText("The new pages are live and pass QA.")).toBeInTheDocument();
    // The objective's own page (not a task handed on).
    expect(document.querySelector(".ui-page-head__kicker")).toHaveTextContent("Objective");
    const tree = await screen.findByRole("group", { name: "Delegation tree" });
    expect(within(tree).getByRole("button", { name: /Engineering Manager/ })).toBeInTheDocument();
    const approvals = await screen.findByRole("list", { name: "Approvals" });
    const user = userEvent.setup();
    await user.click(within(approvals).getByRole("button", { name: /git push origin/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "approvals", id: null });
    expect(await screen.findByRole("article", { name: "Result" })).toBeInTheDocument();
    const stream = await screen.findByRole("list", {
      name: "Activity of this task and the tasks under it",
    });
    await user.click(within(stream).getByRole("button", { name: /^Queued → / }));
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-web" });
  });

  it("links a task handed on to the objective it is part of", async () => {
    render(<TaskPage id="task-web" go={go} onOpenSession={vi.fn()} />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Build the pricing page" }),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Relaunch the website" }));
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-obj-1" });
    // Its result is its objective's: the page opens it, and builds no objective report here.
    const result = screen.getByRole("region", { name: "Result" });
    expect(within(result).getByText("Finished")).toBeInTheDocument();
    await user.click(within(result).getByRole("button", { name: "Open the objective's result" }));
    expect(go).toHaveBeenLastCalledWith({ view: "task", id: "task-obj-1" });
    expect(api.getObjectiveReport).not.toHaveBeenCalled();
    expect(api.getTaskTimeline).not.toHaveBeenCalled();
  });

  it("says when the task can't be read, and tries again", async () => {
    api.getTaskTree.mockRejectedValueOnce(
      new commands.PlenipoCommandError("internal", "database error: the Ledger is busy"),
    );
    render(<TaskPage id="task-obj-1" go={go} onOpenSession={vi.fn()} />);
    expect(await screen.findByText("Plenipo couldn't read this task")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Try again" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Relaunch the website" }),
    ).toBeInTheDocument();
  });

  it("says it can't find a task that isn't in the Ledger, and offers Home", async () => {
    api.getTaskTree.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "not found: task task-gone"),
    );
    render(<TaskPage id="task-gone" go={go} onOpenSession={vi.fn()} />);
    expect(await screen.findByText("Plenipo can't find this task")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    await userEvent.setup().click(screen.getByRole("button", { name: "Go to Home" }));
    expect(go).toHaveBeenLastCalledWith({ view: "home", id: null });
  });
});

describe("A page's history", () => {
  it("keeps every event after Show older, even when many new ones arrive at once", async () => {
    const user = userEvent.setup();
    const listening: ((e: LedgerEvent) => void)[] = [];
    vi.mocked(events.subscribeLedgerEvents).mockImplementation((h) => {
      listening.push(h);
      return Promise.resolve(() => undefined);
    });
    // A Ledger whose newest event is `top`: a page is the HISTORY_PAGE events before `before`.
    let top = 100;
    const load = (before?: number) => {
      const page: LedgerEvent[] = [];
      for (let s = Math.min((before ?? top + 1) - 1, top); s > 0; s--) {
        if (page.length === HISTORY_PAGE) break;
        page.push(event("agent.message", { text: `Step ${s}` }, { seq: s }));
      }
      return Promise.resolve(page);
    };
    render(<EventHistory label="History" historyKey="k" load={load} now={NOW} />);
    const list = await screen.findByRole("list", { name: "History" });
    await within(list).findByText("Agent: Step 100");
    await user.click(screen.getByRole("button", { name: "Show older" }));
    await within(list).findByText("Agent: Step 1");
    // 80 events at once: the newest page is now 180 to 131, well above the older page.
    top = 180;
    for (const h of listening) h(event("agent.message", { text: "Step 180" }, { seq: 180 }));
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(180), {
      timeout: 4_000,
    });
    expect(within(list).getByText("Agent: Step 120")).toBeInTheDocument();
    expect(within(list).getByText("Agent: Step 60")).toBeInTheDocument();
  });
});

describe("accessibility smoke", () => {
  it("names every control, has one main heading, and skips no heading level, on every page", async () => {
    const pages = [
      {
        name: "Home",
        page: <HomePage go={go} approvals={approvals()} learning={learning([lesson])} />,
        ready: "What's stuck",
      },
      {
        name: "Department",
        page: <DepartmentPage id="d-eng" go={go} onBack={vi.fn()} />,
        ready: "Queue",
      },
      {
        name: "Project",
        page: <ProjectPage id="pr-web" go={go} onBack={vi.fn()} />,
        ready: "Objectives",
      },
      {
        name: "Worker",
        page: <WorkerPage id="p-web" go={go} onBack={vi.fn()} onOpenSession={vi.fn()} />,
        ready: "Working on",
      },
      {
        name: "Task",
        page: <TaskPage id="task-obj-1" go={go} onBack={vi.fn()} onOpenSession={vi.fn()} />,
        ready: "Decisions",
      },
    ];
    for (const { name, page, ready } of pages) {
      const { container, unmount } = render(page);
      await screen.findByRole("list", { name: ready });
      expect(a11yProblems(container), name).toEqual([]);
      unmount();
    }
  });
});
