import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ExecutionRecord, RuntimeEvent, RuntimeOverview } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import * as commands from "./api/commands";
import * as events from "./api/events";
import { emptyOrganization } from "./test/orgFixtures";
import { samplePermissions, sampleQueue } from "./test/permissionFixtures";

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
  };
});
vi.mock("./api/events", () => ({
  subscribeRuntimeEvents: vi.fn(),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
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
  api.getAppInfo.mockResolvedValue({
    name: "Plenipo",
    version: "0.1.0",
    buildProfile: "release",
    os: "windows",
    arch: "x86_64",
  });
  api.frontendReady.mockResolvedValue(undefined);
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
    expect(screen.getByText("Plenipo")).toBeInTheDocument();
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
    expect(await screen.findByText("Build your organization")).toBeInTheDocument();
    expect(api.getOrganization).toHaveBeenCalled();
    const map = screen.getByRole("region", { name: "Organization topology" });
    expect(
      within(map).getAllByRole("button", { name: /^You, President$|, organization$/ }),
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
    expect(await screen.findByRole("status")).toHaveTextContent("Imported 3 execution(s)");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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
    expect(screen.getByText("git push origin")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByLabelText("1 waiting for you")).toHaveTextContent("1");
    const user = userEvent.setup();
    await user.click(
      within(banner.closest(".banner") as HTMLElement).getByRole("button", { name: "Review" }),
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
});
