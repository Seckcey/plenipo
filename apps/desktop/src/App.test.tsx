import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ControlStatus, ExecutionRecord, RuntimeEvent, RuntimeOverview } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as commands from "./api/commands";
import * as events from "./api/events";
import { runtime } from "./test/agentFixtures";
import { emptyOrganization, sampleOrganization } from "./test/orgFixtures";
import { samplePermissions, sampleQueue } from "./test/permissionFixtures";
import { sampleWork } from "./test/projectFixtures";
import { NO_RECOVERY, crashRecovery, updateReady, upToDate } from "./test/upkeepFixtures";

vi.mock("./api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getAppInfo: vi.fn(),
    frontendReady: vi.fn(),
    getRuntimeOverview: vi.fn(),
    startExecution: vi.fn(),
    cancelExecution: vi.fn(),
    getExecutionOutput: vi.fn(),
    getLedgerStatus: vi.fn(),
    listTasks: vi.fn(),
    listRecentEvents: vi.fn(),
    getTaskTimeline: vi.fn(),
    getAgentOverview: vi.fn(),
    getAgentSession: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    startAgentSession: vi.fn(),
    resumeAgentSession: vi.fn(),
    cancelAgentTurn: vi.fn(),
    closeAgentSession: vi.fn(),
    getLiaisonOverview: vi.fn(),
    getOrganization: vi.fn(),
    getWork: vi.fn(),
    setOrganizationTitles: vi.fn(),
    getApprovals: vi.fn(),
    getPermissions: vi.fn(),
    resolveApproval: vi.fn(),
    getControlStatus: vi.fn(),
    stopAllControl: vi.fn(),
    takeOverControl: vi.fn(),
    allowControl: vi.fn(),
    getActivity: vi.fn(),
    getProjectWork: vi.fn(),
    getHome: vi.fn(),
    getScopeEvents: vi.fn(),
    getTaskEvents: vi.fn(),
    getProjectRecord: vi.fn(),
    getTaskRecord: vi.fn(),
    getTaskTree: vi.fn(),
    getObjectiveReport: vi.fn(),
    getLearning: vi.fn(),
    getRecoveryStatus: vi.fn(),
    runAgain: vi.fn(),
    dismissRecovery: vi.fn(),
    windowAlive: vi.fn(),
    getUpdateStatus: vi.fn(),
    getOwnerProfile: vi.fn(),
    getAiTools: vi.fn(),
  };
});
vi.mock("./api/events", () => ({
  subscribeRuntimeEvents: vi.fn(),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeControl: vi.fn((handler: (s: ControlStatus) => void) => {
    emitControl = handler;
    return Promise.resolve(() => undefined);
  }),
  subscribePopOuts: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeDrops: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeOrganizations: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeWatch: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
let emitControl: (s: ControlStatus) => void = () => undefined;
const subscribe = vi.mocked(events.subscribeRuntimeEvents);
let emit: (event: RuntimeEvent) => void = () => undefined;

const record = (id: string, patch: Partial<ExecutionRecord> = {}): ExecutionRecord => ({
  id,
  profileId: "diagnostic.echo",
  label: "Echo test",
  executable: "/x",
  args: [],
  workingDir: "/",
  pid: 4242,
  state: "running",
  exitCode: null,
  detail: null,
  startedAt: Date.now(),
  endedAt: null,
  agent: null,
  ...patch,
});

const profiles: RuntimeOverview["profiles"] = [
  { id: "diagnostic.echo", label: "Echo test", description: "Prints lines", maxRuntimeSecs: 60 },
  {
    id: "diagnostic.long-running",
    label: "Long-running process",
    description: "Heartbeat",
    maxRuntimeSecs: 600,
  },
];

function overview(executions: ExecutionRecord[] = []): RuntimeOverview {
  return {
    profiles,
    executions,
    activeCount: executions.filter((e) => e.state === "running").length,
    notices: [],
  };
}

function ledgerStatus(notices: string[]) {
  return {
    path: "/data/ledger/plenipo.db",
    schemaVersion: 1,
    sizeBytes: 4096,
    taskCount: 0,
    eventCount: 0,
    executionCount: 0,
    notices,
    lastIntegrityCheck: null,
    lastBackup: null,
    persistent: true,
  };
}

function send(event: RuntimeEvent) {
  act(() => emit(event));
}

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
  delete document.documentElement.dataset.theme;
  api.getRecoveryStatus.mockResolvedValue(NO_RECOVERY);
  api.windowAlive.mockResolvedValue(undefined);
  api.getUpdateStatus.mockResolvedValue(upToDate());
  api.getOwnerProfile.mockResolvedValue({
    status: "busy",
    mood: "focused",
    message: "Heads down",
    picture: null,
  });
  api.getActivity.mockImplementation((scopes, from, to, buckets = 96) =>
    Promise.resolve(
      scopes.map(() => ({
        from,
        to,
        bucketMs: Math.ceil((to - from) / buckets),
        buckets: Array.from({ length: buckets }, (_, i) => ({
          events: i % 7 === 0 ? 3 : 0,
          problems: i === 90 ? 1 : 0,
          waiting: 0,
        })),
      })),
    ),
  );
  api.getProjectWork.mockResolvedValue(sampleWork());
  api.getHome.mockResolvedValue({ current: [], finished: [], stuck: [], going: 0, finishedDay: 0 });
  api.getScopeEvents.mockResolvedValue([]);
  api.getTaskEvents.mockResolvedValue([]);
  api.getProjectRecord.mockResolvedValue({ pullRequests: [], artifacts: [], decisions: [] });
  api.getLearning.mockResolvedValue({
    enabled: true,
    autoRoles: [],
    offRoles: [],
    agents: {},
    waiting: [],
    kept: [],
  });
  api.getAppInfo.mockResolvedValue({
    name: "Plenipo",
    version: "0.1.0",
    buildProfile: "release",
    os: "windows",
    arch: "x86_64",
  });
  api.frontendReady.mockResolvedValue(undefined);
  api.getAiTools.mockResolvedValue({
    tools: [],
    autoUpdate: false,
    lastLookedAt: null,
    looking: false,
  });
  api.getRuntimeOverview.mockResolvedValue(overview());
  api.getExecutionOutput.mockImplementation((executionId) =>
    Promise.resolve({ executionId, lines: [], dropped: 0, available: true }),
  );
  api.getLedgerStatus.mockResolvedValue(ledgerStatus([]));
  api.listTasks.mockResolvedValue([]);
  api.listRecentEvents.mockResolvedValue([]);
  api.getAgentOverview.mockResolvedValue({ runtimes: [], sessions: [], notices: [] });
  api.getOrganization.mockResolvedValue(emptyOrganization());
  api.getApprovals.mockResolvedValue({ pending: [], recent: [] });
  api.getPermissions.mockResolvedValue(samplePermissions());
  api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [], revision: 0 });
  api.getLiaisonOverview.mockResolvedValue({
    protocol: "plenipo-liaison/1",
    contextFormat: "plenipo-context/1",
    limits: { maxDepth: 3, maxRequestsPerAnswer: 3, maxRounds: 5, maxWorkflowHandoffs: 12 },
    destinations: [
      { address: "claude-code", runtimeId: "claude-code", label: "Claude Code", ready: true },
      { address: "codex", runtimeId: "codex", label: "Codex", ready: false },
    ],
    openHandoffs: 1,
    notices: [],
  });
  subscribe.mockImplementation((handler) => {
    emit = handler;
    return Promise.resolve(() => undefined);
  });
});

async function openRuntimes() {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: /^AI tools/ }));
  await screen.findByRole("button", { name: "Start Echo test" });
  return user;
}

describe("App shell", () => {
  it("renders the branded shell with navigation and reports ready", async () => {
    render(<App />);
    expect(screen.getByRole("img", { name: "Plenipo" })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Main" });
    for (const label of [
      "Organization",
      "Workers",
      "Approvals",
      "AI tools",
      "Activity",
      "Settings",
      "Diagnostics",
    ]) {
      expect(within(nav).getByRole("button", { name: new RegExp(label) })).toBeInTheDocument();
    }
    expect(await screen.findByLabelText("Application version")).toHaveTextContent("v0.1.0");
    await waitFor(() => expect(api.frontendReady).toHaveBeenCalledTimes(1));
  });

  it("shows your picture and status in the top bar (ADR-056)", async () => {
    render(<App />);
    const you = await screen.findByRole("button", {
      name: "You: Busy, feeling Focused — change your picture, status, mood, and message",
    });
    await userEvent.setup().click(you);
    expect(screen.getByRole("dialog", { name: "You" })).toBeInTheDocument();
  });

  it("opens each view at its top", async () => {
    render(<App />);
    const user = userEvent.setup();
    const main = screen.getByRole("main");
    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    main.scrollTop = 480;
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(main.scrollTop).toBe(0);
  });

  it("shows diagnostics from Core", async () => {
    render(<App />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Diagnostics" }));
    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("windows / x86_64")).toBeInTheDocument();
    const liaison = await screen.findByLabelText("Liaison details");
    expect(within(liaison).getByText("plenipo-liaison/1")).toBeInTheDocument();
    expect(
      within(liaison).getByText(/depth 3 · 3 per answer · 5 reply rounds · 12 per workflow/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Claude Code \(claude-code, ready\) · Codex \(codex, not ready\)/),
    ).toBeInTheDocument();
  });

  it("shows an error and does not report ready when Core is unavailable", async () => {
    api.getAppInfo.mockRejectedValue(new commands.PlenipoCommandError("internal", "IPC down"));
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("IPC down");
    expect(api.frontendReady).not.toHaveBeenCalled();
  });

  it("requires no credentials to render", async () => {
    render(<App />);
    await screen.findByLabelText("Application version");
    expect(screen.queryByLabelText(/api key|password|token/i)).not.toBeInTheDocument();
  });

  it("lets the owner choose what the ranks are called, in Settings", async () => {
    api.setOrganizationTitles.mockImplementation((titles) =>
      Promise.resolve({ ...emptyOrganization(), titles }),
    );
    render(<App />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("tab", { name: "Personalization" }));
    const pick = await screen.findByRole("combobox", { name: "Titles" });
    await waitFor(() => expect(pick).toBeEnabled());
    const chain = screen.getByRole("list", { name: "Chain of command" });
    expect(chain).toHaveTextContent(/^President — youVPManagerSupervisorWorker$/);
    await user.selectOptions(pick, "navy");
    expect(api.setOrganizationTitles).toHaveBeenCalledWith("navy");
    expect(await screen.findByRole("status")).toHaveTextContent("U.S. Navy titles");
    expect(chain).toHaveTextContent("Admiral — President · you");
    expect(chain).toHaveTextContent("Chief Petty Officer — Supervisor");
  });

  it("does not hard-code departments: the organization comes from Core", async () => {
    render(<App />);
    // Home, the first page, invites the owner to set the company up.
    expect(await screen.findByText(/ready for its first department/)).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Organization" }));
    expect(await screen.findByText("Build your organization")).toBeInTheDocument();
    expect(api.getOrganization).toHaveBeenCalled();
    const map = screen.getByRole("region", { name: "Organization topology" });
    expect(
      within(map).getAllByRole("button", { name: /^You, President(: .+)?$|, organization$/ }),
    ).toHaveLength(2);
    for (const name of ["Development", "Sales", "Marketing", "Westy"]) {
      expect(screen.queryByText(new RegExp(name))).not.toBeInTheDocument();
    }
  });
});

describe("Ledger notices", () => {
  it("shows severe ledger notices as an alert on every view", async () => {
    api.getLedgerStatus.mockResolvedValue(
      ledgerStatus([
        "The ledger database failed its integrity check (malformed). It was moved to /x.corrupt-1",
      ]),
    );
    render(<App />);
    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("failed its integrity check");
    await userEvent.setup().click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows informational notices without alarm", async () => {
    api.getLedgerStatus.mockResolvedValue(
      ledgerStatus(["Imported 3 execution(s) from the previous history file into the ledger."]),
    );
    render(<App />);
    const notice = await screen.findByText(/Imported 3 execution\(s\)/);
    expect(notice.closest('[role="status"]')).not.toBeNull();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Keeping Plenipo dependable (Phase 13)", () => {
  it("says after a crash what stopped, as a notice and not an alarm, on every page", async () => {
    api.getRecoveryStatus.mockResolvedValue(crashRecovery());
    render(<App />);
    const notice = await screen.findByRole("status", { name: "How Plenipo last stopped" });
    expect(notice).toHaveTextContent(/Plenipo closed unexpectedly at/);
    expect(notice).toHaveTextContent("Fix the login page");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    // The page tells Plenipo it is alive, so a frozen window can be brought back.
    await waitFor(() => expect(api.windowAlive).toHaveBeenCalledWith(true));
  });

  it("marks a ready update next to the version, and opens Settings → Updates from it", async () => {
    api.getUpdateStatus.mockResolvedValue(updateReady());
    render(<App />);
    const mark = await screen.findByRole("button", { name: /Update ready: Plenipo 1\.10\.0/ });
    expect(await screen.findByLabelText("Application version")).toBeInTheDocument();
    await userEvent.setup().click(mark);
    expect(await screen.findByRole("tab", { name: "Updates" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });
});

describe("AI tools page", () => {
  it("starts a profile and shows live stdout, stderr, and the final state", async () => {
    render(<App />);
    const user = await openRuntimes();
    api.startExecution.mockResolvedValue(record("e1"));

    await user.click(screen.getByRole("button", { name: "Start Echo test" }));
    expect(api.startExecution).toHaveBeenCalledWith("diagnostic.echo");

    const log = await screen.findByRole("log", { name: "Output of Echo test" });
    send({
      kind: "output",
      executionId: "e1",
      lines: [
        { seq: 1, stream: "stdout", text: "stdout line 1 of 10", truncated: false, ts: 1 },
        { seq: 2, stream: "stderr", text: "stderr notice 3", truncated: false, ts: 2 },
      ],
    });
    expect(within(log).getByText("stdout line 1 of 10")).toHaveAttribute("data-stream", "stdout");
    expect(within(log).getByText("stderr notice 3")).toHaveAttribute("data-stream", "stderr");

    send({
      kind: "output",
      executionId: "e1",
      lines: [{ seq: 3, stream: "stdout", text: "echo complete", truncated: false, ts: 3 }],
    });
    expect(within(log).getByText("echo complete")).toBeInTheDocument();

    send({
      kind: "lifecycle",
      record: record("e1", { state: "succeeded", exitCode: 0, endedAt: Date.now() }),
    });
    expect((await screen.findAllByText("Succeeded")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
  });

  it("goes straight to an AI tool's card when the place names it (Phase 19)", async () => {
    localStorage.setItem("plenipo.place", JSON.stringify({ view: "runtimes", id: "codex" }));
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex")],
      sessions: [],
      notices: [],
    });
    render(<App />);
    const codex = await screen.findByRole("listitem", { name: "Codex AI tool" });
    await waitFor(() => expect(codex).toHaveFocus());
    expect(within(codex).getByRole("heading", { name: "Codex" })).toBeInTheDocument();
  });

  it("shows abnormal exits with their exit code", async () => {
    api.getRuntimeOverview.mockResolvedValue(
      overview([record("f1", { label: "Failing process", state: "failed", exitCode: 3 })]),
    );
    render(<App />);
    await openRuntimes();
    expect(screen.getByText("Failed · exit 3")).toBeInTheDocument();
  });

  it("cancels a running execution", async () => {
    api.getRuntimeOverview.mockResolvedValue(
      overview([record("l1", { label: "Long-running process" })]),
    );
    api.cancelExecution.mockResolvedValue(
      record("l1", {
        label: "Long-running process",
        state: "cancelled",
        detail: "Cancelled by user",
        endedAt: Date.now(),
      }),
    );
    render(<App />);
    const user = await openRuntimes();
    await user.click(screen.getByRole("button", { name: /Long-running process — Running/ }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.cancelExecution).toHaveBeenCalledWith("l1");
    expect((await screen.findAllByText("Cancelled")).length).toBeGreaterThan(0);
    expect(screen.getByText("Cancelled by user")).toBeInTheDocument();
  });

  it("surfaces start errors", async () => {
    api.startExecution.mockRejectedValue(
      new commands.PlenipoCommandError("invalidInput", "unknown launch profile"),
    );
    render(<App />);
    const user = await openRuntimes();
    await user.click(screen.getByRole("button", { name: "Start Echo test" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("unknown launch profile");
  });

  it("keeps runtime state while navigating between views", async () => {
    render(<App />);
    const user = await openRuntimes();
    api.startExecution.mockResolvedValue(record("e1"));
    await user.click(screen.getByRole("button", { name: "Start Echo test" }));
    send({
      kind: "output",
      executionId: "e1",
      lines: [{ seq: 1, stream: "stdout", text: "kept across views", truncated: false, ts: 1 }],
    });

    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(await screen.findByRole("heading", { name: "Activity" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^AI tools/ }));
    expect(screen.getByText("kept across views")).toBeInTheDocument();
    expect(api.getRuntimeOverview).toHaveBeenCalledTimes(1);
  });

  it("rebuilds from Core after the webview reloads", async () => {
    const running = record("l1", { label: "Long-running process" });
    api.getRuntimeOverview.mockResolvedValue(overview([running]));
    const first = render(<App />);
    const user = await openRuntimes();
    await user.click(screen.getByRole("button", { name: /Long-running process — Running/ }));
    first.unmount();
    cleanup();

    // Simulated reload: fresh React tree, same backend.
    api.getExecutionOutput.mockResolvedValue({
      executionId: "l1",
      lines: [{ seq: 7, stream: "stdout", text: "heartbeat 7", truncated: false, ts: 7 }],
      dropped: 0,
      available: true,
    });
    render(<App />);
    // View and selection are restored, output is fetched for the running execution.
    const log = await screen.findByRole("log", { name: "Output of Long-running process" });
    expect(await within(log).findByText("heartbeat 7")).toBeInTheDocument();
    expect(api.getExecutionOutput).toHaveBeenCalledWith("l1");
    // Live events continue on the new subscription.
    send({
      kind: "output",
      executionId: "l1",
      lines: [{ seq: 8, stream: "stdout", text: "heartbeat 8", truncated: false, ts: 8 }],
    });
    expect(within(log).getByText("heartbeat 8")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });

  it("shows requests waiting for approval on every page, with a count", async () => {
    api.getApprovals.mockResolvedValue(sampleQueue());
    api.resolveApproval.mockResolvedValue({ pending: [], recent: [] });
    render(<App />);
    const banner = await screen.findByText("Backend Developer is waiting for your approval");
    const notice = banner.closest(".banner--approval") as HTMLElement;
    expect(within(notice).getByText("git push origin")).toBeInTheDocument();
    // Home lists it under Waiting for you, too.
    const home = await screen.findByRole("list", { name: "Waiting for you" });
    expect(within(home).getByText("git push origin")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByLabelText("1 waiting for you")).toHaveTextContent("1");
    const user = userEvent.setup();
    await user.click(
      within(banner.closest(".banner--approval") as HTMLElement).getByRole("button", {
        name: "Review",
      }),
    );
    const card = await screen.findByRole("article", {
      name: "Backend Developer wants to git push origin",
    });
    expect(screen.queryByText("Backend Developer is waiting for your approval")).toBeNull();
    await user.click(within(card).getByRole("button", { name: "Approve" }));
    expect(api.resolveApproval).toHaveBeenCalledWith("approval-1", true);
    // The answer updates the sidebar count at once.
    await waitFor(() => expect(within(nav).queryByLabelText("1 waiting for you")).toBeNull());
  });

  it("shows who controls the browser on every page, with Take over and Stop all", async () => {
    const session = {
      id: "browser:g1",
      grantId: "g1",
      taskId: "t1",
      worker: "Web Assistant",
      kind: "browser" as const,
      state: "active" as const,
      detail: "http://shop.test/form",
      lastAction: 'clicked "Send message"',
      since: 0,
      production: false,
    };
    api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [session], revision: 1 });
    api.takeOverControl.mockResolvedValue({
      stopped: false,
      sessions: [{ ...session, state: "takenOver" }],
      revision: 2,
    });
    api.stopAllControl.mockResolvedValue({ stopped: true, sessions: [], revision: 3 });
    api.allowControl.mockResolvedValue({ stopped: false, sessions: [], revision: 4 });
    render(<App />);
    const banner = await screen.findByRole("alert", { name: "Browser, desktop, and server work" });
    expect(banner).toHaveTextContent("Web Assistant is using Plenipo's browser");
    expect(banner).toHaveTextContent('http://shop.test/form · clicked "Send message"');
    expect(document.querySelector(".shell__footer")).toHaveTextContent(
      "Web Assistant is using Plenipo's browser",
    );
    const user = userEvent.setup();
    await user.click(within(banner).getByRole("button", { name: "Take over" }));
    expect(api.takeOverControl).toHaveBeenCalledWith("browser:g1");
    await waitFor(() =>
      expect(banner).toHaveTextContent("You have control of the browser. Web Assistant stopped."),
    );
    // It stays until dismissed, even after the worker's step ends.
    act(() => emitControl({ stopped: false, sessions: [], revision: 5 }));
    expect(banner).toHaveTextContent("You have control of the browser.");
    // An older update arriving late changes nothing.
    act(() => emitControl({ stopped: false, sessions: [session], revision: 1 }));
    expect(banner).not.toHaveTextContent("Web Assistant is using Plenipo's browser");
    await user.click(within(banner).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert", { name: "Browser, desktop, and server work" })).toBeNull();
    // Stop all appears only while a worker is active; stop from a fresh active state.
    api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [session], revision: 1 });
    cleanup();
    render(<App />);
    const again = await screen.findByRole("alert", { name: "Browser, desktop, and server work" });
    await user.click(within(again).getByRole("button", { name: "Stop all" }));
    expect(api.stopAllControl).toHaveBeenCalled();
    await waitFor(() =>
      expect(again).toHaveTextContent("Browser, desktop, and server work is stopped."),
    );
    await user.click(within(again).getByRole("button", { name: "Allow again" }));
    expect(api.allowControl).toHaveBeenCalled();
    await waitFor(() =>
      expect(screen.queryByRole("alert", { name: "Browser, desktop, and server work" })).toBeNull(),
    );
  });

  it("shows a worker connected to a production server in red, with Disconnect", async () => {
    const session = {
      id: "server:g2",
      grantId: "g2",
      taskId: "t2",
      worker: "Operations Engineer",
      kind: "server" as const,
      state: "active" as const,
      detail: "Shop (production)",
      lastAction: "Shop: load average: 0.00",
      since: 0,
      production: true,
    };
    api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [session], revision: 1 });
    api.takeOverControl.mockResolvedValue({
      stopped: false,
      sessions: [{ ...session, state: "takenOver" }],
      revision: 2,
    });
    render(<App />);
    const banner = await screen.findByRole("alert", { name: "Browser, desktop, and server work" });
    expect(banner).toHaveTextContent("Operations Engineer is connected to Shop (production)");
    expect(banner).toHaveTextContent("PRODUCTION");
    expect(banner).toHaveTextContent("Shop: load average: 0.00");
    expect(within(banner).queryByRole("button", { name: "Take over" })).toBeNull();
    const user = userEvent.setup();
    await user.click(within(banner).getByRole("button", { name: "Disconnect" }));
    expect(api.takeOverControl).toHaveBeenCalledWith("server:g2");
    await waitFor(() =>
      expect(banner).toHaveTextContent(
        "You disconnected Operations Engineer from Shop (production). It stopped.",
      ),
    );
  });
});

describe("The frame (Phase 12A)", () => {
  it("switches light and dark from the top bar, and remembers the choice", async () => {
    render(<App />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Switch to the light theme" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("plenipo.theme")).toBe("light");
    await user.click(screen.getByRole("button", { name: "Switch to the dark theme" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("names each section under its icon, and marks the current one", async () => {
    render(<App />);
    const nav = screen.getByRole("navigation", { name: "Main" });
    const home = within(nav).getByRole("button", { name: "Home" });
    expect(home).toHaveAttribute("aria-current", "page");
    expect(within(nav).getAllByRole("button")[0]).toBe(home);
    expect(within(home).getByText("Home")).toBeVisible();
    const org = within(nav).getByRole("button", { name: "Organization" });
    expect(org).not.toHaveAttribute("aria-current");
    expect(org).toHaveAttribute("title", expect.stringContaining("departments"));
    await userEvent.setup().click(within(nav).getByRole("button", { name: "Settings" }));
    expect(within(nav).getByRole("button", { name: "Settings" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(document.querySelector(".ui-topbar__title")).toHaveTextContent("Settings");
  });

  it("rings the bell for requests waiting for you, and opens Approvals", async () => {
    api.getApprovals.mockResolvedValue(sampleQueue());
    render(<App />);
    const bell = await screen.findByRole("button", {
      name: "Notifications: 1 waiting for your approval",
    });
    await userEvent.setup().click(bell);
    expect(
      await screen.findByRole("article", { name: "Backend Developer wants to git push origin" }),
    ).toBeInTheDocument();
    expect(document.querySelector(".ui-topbar__title")).toHaveTextContent("Approvals");
  });

  it("opens a project's or a department's page from the Showing picker", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    render(<App />);
    const user = userEvent.setup();
    const picker = await screen.findByRole("combobox", { name: "Showing" });
    await waitFor(() =>
      expect(within(picker).getByRole("option", { name: "Q4 Campaign" })).toBeInTheDocument(),
    );
    expect(within(picker).getByRole("group", { name: "Departments" })).toBeInTheDocument();
    await user.selectOptions(picker, "project:pr-camp");
    expect(
      await screen.findByRole("heading", { level: 1, name: "Q4 Campaign" }),
    ).toBeInTheDocument();
    expect(document.querySelector(".ui-topbar__title")).toHaveTextContent("Project · Q4 Campaign");
    const nav = screen.getByRole("navigation", { name: "Main" });
    // The page belongs to the Projects section, which the strip marks.
    expect(within(nav).getByRole("button", { name: "Projects" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(picker).toHaveValue("project:pr-camp");
    await user.selectOptions(picker, "department:d-eng");
    expect(
      await screen.findByRole("heading", { level: 1, name: "Engineering" }),
    ).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Organization" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    // Back returns to the project's page; everything opens Home.
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Q4 Campaign" }),
    ).toBeInTheDocument();
    await user.selectOptions(picker, "all");
    expect(
      await screen.findByRole("region", { name: /Good (morning|afternoon|evening)|Working late/ }),
    ).toBeInTheDocument();
    // Leaving by the strip shows everything again.
    await user.selectOptions(picker, "project:pr-camp");
    await user.click(within(nav).getByRole("button", { name: "Settings" }));
    expect(picker).toHaveValue("all");
  });

  it("comes back to the same page after a restart, and Back then goes to its section", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    const first = render(<App />);
    const user = userEvent.setup();
    const picker = await screen.findByRole("combobox", { name: "Showing" });
    await waitFor(() =>
      expect(within(picker).getByRole("option", { name: "Engineering" })).toBeInTheDocument(),
    );
    await user.selectOptions(picker, "department:d-eng");
    await screen.findByRole("heading", { level: 1, name: "Engineering" });
    expect(JSON.parse(localStorage.getItem("plenipo.place") ?? "null")).toEqual({
      view: "department",
      id: "d-eng",
    });
    first.unmount();
    cleanup();

    render(<App />);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Engineering" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(
      await screen.findByRole("region", { name: "Organization topology" }),
    ).toBeInTheDocument();
    // Something unreadable in storage opens Home.
    cleanup();
    localStorage.setItem("plenipo.place", JSON.stringify({ view: "task", id: "" }));
    render(<App />);
    expect(
      await screen.findByRole("region", { name: /Good (morning|afternoon|evening)|Working late/ }),
    ).toBeInTheDocument();
  });

  it("opens the Gallery from Diagnostics, with real departments and projects first", async () => {
    api.getOrganization.mockResolvedValue(sampleOrganization());
    render(<App />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    await user.click(await screen.findByRole("button", { name: "Open the gallery" }));
    expect(screen.getByRole("heading", { level: 1, name: "Gallery" })).toBeInTheDocument();
    const live = await screen.findByRole("region", { name: "From your organization" });
    const card = await within(live).findByRole("article", {
      name: "Website Relaunch, Waiting on team",
    });
    expect(
      await within(card).findByRole("img", {
        name: /Website Relaunch activity\. Last 24 hours: 42 events, 1 problem/,
      }),
    ).toBeInTheDocument();
    expect(api.getActivity).toHaveBeenCalledWith(
      expect.arrayContaining([
        { kind: "department", id: "d-eng" },
        { kind: "project", id: "pr-web" },
      ]),
      expect.any(Number),
      expect.any(Number),
      96,
    );
    // The page shows every building block too.
    expect(screen.getByRole("table", { name: "Workers" })).toHaveAttribute("aria-rowcount", "5001");
  });
});
