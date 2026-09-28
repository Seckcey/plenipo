import { useState } from "react";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { NoticeSettings, TerminalSettings as Terminal } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { AgentsProvider } from "../agents/AgentsProvider";
import { RuntimeProvider } from "../runtime/RuntimeProvider";
import { runtime } from "../test/agentFixtures";
import { aiPage, aiTool } from "../test/aiToolFixtures";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { sampleRouting } from "../test/routingFixtures";
import { a11yProblems } from "../test/a11y";
import { samplePermissions } from "../test/permissionFixtures";
import { sampleServers } from "../test/serverFixtures";
import { SettingsView } from "../views/SettingsView";
import { SETTINGS_SECTION_KEY } from "./sections";
import { sampleBackups, startAndClose, upToDate } from "../test/upkeepFixtures";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRuntimeOverview: vi.fn(),
    getAgentOverview: vi.fn(),
    refreshAgentRuntimes: vi.fn(),
    getOrganization: vi.fn(),
    getPermissions: vi.fn(),
    getLearning: vi.fn(),
    getRouting: vi.fn(),
    getServers: vi.fn(),
    getNoticeSettings: vi.fn(),
    setNoticeSettings: vi.fn(),
    sendTestNotice: vi.fn(),
    getTerminalSettings: vi.fn(),
    setTerminalShell: vi.fn(),
    getLocalPaths: vi.fn(),
    getLedgerStatus: vi.fn(),
    getStartAndClose: vi.fn(),
    setStartAndClose: vi.fn(),
    getUpdateStatus: vi.fn(),
    checkForUpdates: vi.fn(),
    installUpdate: vi.fn(),
    listLedgerBackups: vi.fn(),
    saveDiagnosticsFile: vi.fn(),
    getAiTools: vi.fn(),
    setAiToolsAutoUpdate: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeRuntimeEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();

const notices: NoticeSettings = {
  approvals: true,
  checks: true,
  problems: true,
  finished: true,
  lessons: true,
  plenipo: true,
  onlyWhenAway: true,
};

const terminal: Terminal = {
  shell: "windowsPowerShell",
  shells: [
    { shell: "windowsPowerShell", label: "Windows PowerShell", installed: true },
    { shell: "powerShell7", label: "PowerShell 7", installed: false },
    { shell: "commandPrompt", label: "Command Prompt", installed: true },
  ],
  serversSwitchedOn: false,
  open: [],
};

/** Settings, with the place's section kept as the app keeps it (a choice becomes the place's). */
function Placed({ asked }: { asked: string | null }) {
  const [section, setSection] = useState(asked);
  return <SettingsView go={go} info={null} section={section} onSection={setSection} />;
}

function show(section: string | null = null) {
  return render(
    <RuntimeProvider>
      <AgentsProvider>
        <Placed asked={section} />
      </AgentsProvider>
    </RuntimeProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
  go.mockReset();
  api.getRuntimeOverview.mockResolvedValue({
    profiles: [],
    executions: [],
    activeCount: 0,
    notices: [],
  });
  api.getAgentOverview.mockResolvedValue({ runtimes: [], sessions: [], notices: [] });
  api.getAiTools.mockResolvedValue(aiPage([aiTool("codex")]));
  api.getOrganization.mockResolvedValue(sampleOrganization());
  api.getPermissions.mockResolvedValue(samplePermissions());
  api.getLearning.mockResolvedValue({
    enabled: true,
    autoRoles: [],
    offRoles: [],
    agents: {},
    waiting: [],
    kept: [],
  });
  // The choices are kept, as the Ledger keeps them.
  let kept = notices;
  api.getNoticeSettings.mockImplementation(() => Promise.resolve(kept));
  api.setNoticeSettings.mockImplementation((s) => {
    kept = s;
    return Promise.resolve(s);
  });
  api.sendTestNotice.mockResolvedValue(undefined);
  let start = startAndClose();
  api.getStartAndClose.mockImplementation(() => Promise.resolve(start));
  api.setStartAndClose.mockImplementation((input) => {
    start = { ...start, ...input };
    return Promise.resolve(start);
  });
  api.getUpdateStatus.mockResolvedValue(upToDate());
  api.checkForUpdates.mockResolvedValue(upToDate());
  api.listLedgerBackups.mockResolvedValue(sampleBackups());
  let shell = terminal;
  api.getTerminalSettings.mockImplementation(() => Promise.resolve(shell));
  api.setTerminalShell.mockImplementation((choice) => {
    shell = { ...terminal, shell: choice };
    return Promise.resolve(shell);
  });
  api.getLocalPaths.mockResolvedValue([
    {
      label: "Everything that happened (the Ledger)",
      path: "C:\\Plenipo\\ledger\\plenipo.db",
      kept: true,
    },
  ]);
});

describe("Settings in one place", () => {
  it("lists every section on the left, shows one at a time, and keeps the last one", async () => {
    show();
    const list = screen.getByRole("tablist", { name: "Settings sections" });
    expect(
      within(list)
        .getAllByRole("tab")
        .map((t) => t.textContent),
    ).toEqual([
      "AI tools",
      "AI models",
      "Permissions",
      "Organization",
      "Servers",
      "Switches",
      "Notifications",
      "Terminal",
      "Start and close",
      "Personalization",
      "Local paths",
      "Diagnostics",
      "Updates",
      "About Plenipo",
    ]);
    expect(screen.getByRole("tab", { name: "AI tools" })).toHaveAttribute("aria-selected", "true");
    const user = userEvent.setup();
    // The keyboard moves down the list.
    screen.getByRole("tab", { name: "AI tools" }).focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("tab", { name: "AI models" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("tab", { name: "Local paths" }));
    expect(await screen.findByText("C:\\Plenipo\\ledger\\plenipo.db")).toBeInTheDocument();
    expect(screen.getByRole("tabpanel")).toHaveAccessibleName("Local paths");
    cleanup();
    show();
    expect(screen.getByRole("tab", { name: "Local paths" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(localStorage.getItem(SETTINGS_SECTION_KEY)).toBe('"localPaths"');
  });

  it("opens the section another page asked for", async () => {
    api.getServers.mockResolvedValue(sampleServers());
    show("servers");
    expect(screen.getByRole("tab", { name: "Servers" })).toHaveAttribute("aria-selected", "true");
    // The owner can move on from there.
    await userEvent.setup().click(screen.getByRole("tab", { name: "About Plenipo" }));
    expect(screen.getByRole("tab", { name: "About Plenipo" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("img", { name: /Pip, Plenipo's helper/ })).toBeInTheDocument();
  });

  it("chooses which notices Windows shows, and sends a test notice", async () => {
    show("notifications");
    const user = userEvent.setup();
    const finished = await screen.findByRole("switch", { name: "Finished work" });
    expect(finished).toHaveAttribute("aria-checked", "true");
    await user.click(finished);
    expect(api.setNoticeSettings).toHaveBeenCalledWith({ ...notices, finished: false });
    await user.click(
      screen.getByRole("switch", { name: "Only while Plenipo's window is not in front" }),
    );
    // The second choice builds on the first (it never undoes it).
    await waitFor(() =>
      expect(api.setNoticeSettings).toHaveBeenLastCalledWith({
        ...notices,
        finished: false,
        onlyWhenAway: false,
      }),
    );
    expect(finished).toHaveAttribute("aria-checked", "false");
    await user.click(screen.getByRole("button", { name: "Send a test notice" }));
    expect(api.sendTestNotice).toHaveBeenCalled();
    expect(await screen.findByText(/^Sent\./)).toBeInTheDocument();
    api.sendTestNotice.mockRejectedValueOnce(
      new commands.PlenipoCommandError("internal", "The system did not show the notice: off"),
    );
    await user.click(screen.getByRole("button", { name: "Send a test notice" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("did not show the notice");
  });

  it("picks the terminal's shell from those on this PC", async () => {
    show("terminal");
    const user = userEvent.setup();
    const cmd = await screen.findByRole("radio", { name: "Command Prompt" });
    expect(screen.getByRole("radio", { name: /PowerShell 7/ })).toBeDisabled();
    expect(screen.getByRole("radio", { name: "Windows PowerShell" })).toBeChecked();
    await user.click(cmd);
    expect(api.setTerminalShell).toHaveBeenCalledWith("commandPrompt");
    expect(screen.getByText(/Turn on Settings → Switches → Remote computers/)).toBeInTheDocument();
  });

  it("lists departments and projects, each opening its page", async () => {
    show("organization");
    const user = userEvent.setup();
    const departments = await screen.findByRole("list", { name: "Departments" });
    await user.click(within(departments).getByRole("button", { name: /Engineering/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "department", id: "d-eng" });
    const projects = screen.getByRole("list", { name: "Projects" });
    await user.click(within(projects).getByRole("button", { name: /Q4 Campaign/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "project", id: "pr-camp" });
  });

  it("opens each AI tool's card, and AI models links to the AI tools page (Phase 19)", async () => {
    api.getAgentOverview.mockResolvedValue({
      runtimes: [runtime("claude-code"), runtime("codex", false)],
      sessions: [],
      notices: [],
    });
    api.getRouting.mockResolvedValue(sampleRouting());
    show("aiTools");
    const user = userEvent.setup();
    const tools = await screen.findByRole("list", { name: "AI tools" });
    await user.click(within(tools).getByRole("button", { name: /^Codex/ }));
    expect(go).toHaveBeenLastCalledWith({ view: "runtimes", id: "codex" });
    await user.click(screen.getByRole("tab", { name: "AI models" }));
    expect(
      await screen.findByText("Usage limits, sign-in, and updates are on the AI tools page."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Open the AI tools page" }));
    expect(go).toHaveBeenLastCalledWith({ view: "runtimes", id: null });
  });

  it("says so when there is nothing to list yet", async () => {
    api.getOrganization.mockResolvedValue(emptyOrganization());
    show("organization");
    expect(await screen.findByText("No departments yet")).toBeInTheDocument();
    expect(screen.getByText("No projects yet")).toBeInTheDocument();
  });
});

describe("Settings: accessibility smoke", () => {
  it("names every control in every section, with one main heading and no skipped level", async () => {
    api.getServers.mockResolvedValue(sampleServers());
    api.getLedgerStatus.mockResolvedValue({
      path: "/data/ledger/plenipo.db",
      schemaVersion: 8,
      sizeBytes: 4096,
      taskCount: 3,
      eventCount: 40,
      executionCount: 0,
      notices: [],
      lastIntegrityCheck: null,
      lastBackup: null,
      persistent: true,
    });
    const { container } = show();
    const user = userEvent.setup();
    for (const tab of screen.getAllByRole("tab")) {
      await user.click(tab);
      await waitFor(() => expect(tab).toHaveAttribute("aria-selected", "true"));
      // Let the section load before checking it.
      await waitFor(() => expect(container.querySelector('[role="status"][aria-busy]')).toBeNull());
      expect(a11yProblems(container), tab.textContent ?? "").toEqual([]);
    }
  });
});
