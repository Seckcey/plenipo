import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { WatchChange, WatchFileView, WatchUpdate, WatchView } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AgentsContext, type AgentsContextValue } from "../agents/context";
import { initialAgentState } from "../agents/store";
import * as commands from "../api/commands";
import * as events from "../api/events";
import { TerminalPanel } from "./TerminalPanel";
import { TerminalProvider } from "./TerminalProvider";
import { useOpenWatch } from "./useTerminal";

// The panel can hold terminals too; xterm.js draws on a real screen, so a stand-in is used.
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    loadAddon() {}
    open() {}
    dispose() {}
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getControlStatus: vi.fn(),
    getTaskTimeline: vi.fn(),
    getServers: vi.fn(),
    getTerminalSettings: vi.fn(),
    getWatch: vi.fn(),
    getWatchChange: vi.fn(),
    getLiveView: vi.fn(),
    getOrganization: vi.fn(),
    cancelAgentTurn: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeWatch: vi.fn(),
}));

const api = vi.mocked(commands);
const listening = new Set<(u: WatchUpdate) => void>();
/** Plenipo tells the main window about a change. */
const tell = (update: WatchUpdate) =>
  act(() => {
    for (const handler of [...listening]) handler(update);
  });

let n = 0;
function change(over: Partial<WatchChange> = {}): WatchChange {
  n += 1;
  return {
    id: `c${n}`,
    taskId: "t1",
    sessionId: "s-dev",
    positionId: "p-dev",
    worker: "Senior Developer",
    objectiveTaskId: "root",
    path: "src/app.rs",
    state: "saved",
    kind: "changed",
    added: 3,
    removed: 2,
    at: 1_000 + n,
    ...over,
  };
}

function view(changes: WatchChange[], over: Partial<WatchView> = {}): WatchView {
  return {
    positionId: "p-dev",
    objectiveTaskId: "root",
    changes,
    fromTheRecord: false,
    teamTaskIds: [],
    ...over,
  };
}

/** The file after a change: "TWO" changed, a line removed before "five", two new lines. */
function marked(c: WatchChange): WatchFileView {
  return {
    change: c,
    lines: [
      { text: "one", removedBefore: 0 },
      { text: "TWO", mark: "changed", removedBefore: 0 },
      { text: "three", removedBefore: 0 },
      { text: "five", removedBefore: 1 },
      { text: "six", mark: "new", removedBefore: 0 },
      { text: "seven", mark: "new", removedBefore: 0 },
    ],
    removedAtEnd: 2,
  };
}

/** A Watch button, as the properties panel and the canvas have. */
function WatchButton({ id = "p-dev", title = "Senior Developer" }) {
  const open = useOpenWatch();
  return (
    <button type="button" onClick={() => open?.(id, title)}>
      Watch {title}
    </button>
  );
}

function Harness() {
  return (
    <TerminalProvider>
      <WatchButton />
      <TerminalPanel theme="dark" />
    </TerminalProvider>
  );
}

/** Opens the Senior Developer's Watch tab; resolves with its body. */
async function openTab(user: ReturnType<typeof userEvent.setup>): Promise<HTMLElement> {
  await user.click(screen.getByRole("button", { name: "Watch Senior Developer" }));
  const tab = await screen.findByRole("tab", { name: /Watch · Senior Developer/ });
  expect(tab).toHaveAttribute("aria-selected", "true");
  await waitFor(() => expect(api.getWatch).toHaveBeenCalledWith("p-dev"));
  return screen.getByRole("tabpanel");
}

beforeEach(() => {
  cleanup();
  localStorage.clear();
  listening.clear();
  vi.clearAllMocks();
  n = 0;
  api.getLiveView.mockResolvedValue({ workers: [], handoffs: [], at: 0 });
  vi.mocked(events.subscribeWatch).mockImplementation((handler) => {
    listening.add(handler);
    return Promise.resolve(() => {
      listening.delete(handler);
    });
  });
  api.getControlStatus.mockResolvedValue({ stopped: false, sessions: [], revision: 1 });
  api.getServers.mockResolvedValue({ switchedOn: false, servers: [] } as never);
  api.getTerminalSettings.mockResolvedValue({
    shell: "windowsPowerShell",
    shells: [],
    runsAs: "as your own Windows user — never as administrator",
    serversSwitchedOn: false,
    open: [],
  });
  api.getWatch.mockResolvedValue(view([]));
  api.getWatchChange.mockResolvedValue(null);
  api.cancelAgentTurn.mockResolvedValue({} as never);
});

describe("the Watch tab for code", () => {
  it("opens one tab per agent, shows the panel, and closes like the others", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const section = document.querySelector<HTMLElement>('section[aria-label="Terminal"]')!;
    expect(section).not.toBeVisible();
    await openTab(user);
    expect(section).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Watch Senior Developer" }));
    expect(screen.getAllByRole("tab", { name: /Watch · Senior Developer/ })).toHaveLength(1);
    expect(await screen.findByText("No file changes yet in this objective")).toBeInTheDocument();
    expect(
      screen.getByText(/Changes show here as the worker, or the team it hands work to, makes them/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Read-only: you see only what Guard lets this worker change."),
    ).toBeInTheDocument();
    // Nothing is being worked on yet: Stop waits.
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Close Watch for Senior Developer" }));
    expect(screen.queryByRole("tab")).toBeNull();
    // It stopped listening.
    expect(listening.size).toBe(0);
  });

  it("shows a saved change's file with its new and changed lines marked, and where lines went", async () => {
    const user = userEvent.setup();
    const saved = change();
    api.getWatch.mockResolvedValue(
      view([saved, change({ path: "README.md", kind: "created", at: 500 })]),
    );
    api.getWatchChange.mockImplementation((id) =>
      Promise.resolve(id === saved.id ? marked(saved) : null),
    );
    render(<Harness />);
    const body = await openTab(user);
    const files = within(body).getByRole("navigation", { name: "Files touched in this objective" });
    const row = within(files).getByRole("button", { name: /app\.rs/ });
    expect(row).toHaveAttribute("aria-current", "true");
    expect(row).toHaveTextContent("saved");
    expect(row).toHaveTextContent("changed");
    expect(row).toHaveTextContent("3 lines added, 2 removed");
    expect(within(files).getByRole("button", { name: /README\.md/ })).toHaveTextContent("created");
    const file = await within(body).findByRole("region", { name: "src/app.rs, after the change" });
    expect(api.getWatchChange).toHaveBeenCalledWith(saved.id);
    const line = (text: string) => within(file).getByText(text).closest(".code-watch__line")!;
    // Each mark is a sign and a word as well as a color.
    expect(line("TWO")).toHaveAttribute("data-mark", "changed");
    expect(line("TWO")).toHaveTextContent("~changed line: TWO");
    expect(line("six")).toHaveAttribute("data-mark", "new");
    expect(line("six")).toHaveTextContent("+new line: six");
    expect(line("one")).not.toHaveAttribute("data-mark");
    expect(within(file).getByText("1 line removed here")).toBeInTheDocument();
    expect(within(file).getByText("2 lines removed here")).toBeInTheDocument();
    // Picking another file shows it and pins it.
    await user.click(within(files).getByRole("button", { name: /README\.md/ }));
    expect(await within(body).findByText(/Plenipo no longer has the lines/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pin this file" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "Follow along" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("says which files a command made or changed (Phase 25, item 3.2)", async () => {
    const user = userEvent.setup();
    const made = change({ path: "src/page.txt", kind: "created", byCommand: true });
    api.getWatch.mockResolvedValue(view([made, change({ path: "README.md", at: 500 })]));
    api.getWatchChange.mockResolvedValue(null);
    render(<Harness />);
    const body = await openTab(user);
    const files = within(body).getByRole("navigation", { name: "Files touched in this objective" });
    expect(within(files).getByRole("button", { name: /page\.txt/ })).toHaveTextContent(
      "made by a command",
    );
    expect(within(files).getByRole("button", { name: /README\.md/ })).not.toHaveTextContent(
      "made by a command",
    );
  });

  it("shows a change as it is written, then saved, following along; a pinned file stays", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const body = await openTab(user);
    await within(body).findByText("No file changes yet in this objective");
    expect(screen.getByRole("button", { name: "Follow along" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    const writing = change({ path: "src/main.rs", state: "writing", added: 0, removed: 0 });
    delete writing.kind;
    tell({ change: writing, writing: "fn main() {" });
    const tab = screen.getByRole("tab", { name: /Watch · Senior Developer/ });
    expect(tab).toHaveTextContent("being written");
    expect(within(body).getAllByText("being written — not saved yet").length).toBeGreaterThan(0);
    const text = within(body).getByRole("region", { name: "src/main.rs, not saved yet" });
    expect(text).toHaveTextContent("fn main() {");
    tell({ change: { ...writing, at: writing.at + 1 }, writing: "fn main() {\n    run();\n}\n" });
    expect(text).toHaveTextContent("run();");
    // The preview became the saved change.
    const saved: WatchChange = { ...writing, state: "saved", kind: "created", added: 3, at: 5_000 };
    api.getWatchChange.mockResolvedValue({
      change: saved,
      lines: ["fn main() {", "    run();", "}"].map((t) => ({
        text: t,
        mark: "new" as const,
        removedBefore: 0,
      })),
      removedAtEnd: 0,
    });
    tell({ change: saved });
    const file = await within(body).findByRole("region", { name: "src/main.rs, after the change" });
    expect(file).toHaveTextContent("run();");
    expect(within(body).queryByText("being written — not saved yet")).toBeNull();
    expect(within(body).getAllByText("saved").length).toBeGreaterThan(0);
    expect(tab).not.toHaveTextContent("being written");
    // Pinned: another file's change arrives, and this file stays in view.
    await user.click(screen.getByRole("button", { name: "Pin this file" }));
    tell({
      change: change({ path: "src/other.rs", state: "writing", at: 6_000 }),
      writing: "mod other;",
    });
    expect(within(body).getByRole("region", { name: "src/main.rs, after the change" })).toBe(file);
    expect(within(body).queryByText("mod other;")).toBeNull();
    // Follow along again: the newest shows.
    await user.click(screen.getByRole("button", { name: "Follow along" }));
    expect(within(body).getByText("mod other;")).toBeInTheDocument();
  });

  it("shows a refused change's reason, never its text", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const body = await openTab(user);
    const c = change({ path: "src/b.rs", state: "writing", added: 0, removed: 0 });
    delete c.kind;
    tell({ change: c, writing: "the text Guard will refuse" });
    expect(within(body).getByText("the text Guard will refuse")).toBeInTheDocument();
    tell({
      change: { ...c, state: "refused", reason: "Blocked: src/b.rs is a blocked file", at: 9_000 },
    });
    expect(within(body).getAllByText("refused").length).toBeGreaterThan(0);
    expect(within(body).getByText("Blocked: src/b.rs is a blocked file")).toBeInTheDocument();
    expect(within(body).queryByText("the text Guard will refuse")).toBeNull();
    expect(within(body).queryByRole("region")).toBeNull();
    expect(api.getWatchChange).not.toHaveBeenCalled();
    // A change that could not be made says why too.
    tell({
      change: change({
        path: "src/c.rs",
        state: "notSaved",
        reason: "the text to replace was not found",
        added: 0,
        removed: 0,
        at: 10_000,
      }),
    });
    expect(within(body).getAllByText("not saved").length).toBeGreaterThan(0);
    expect(within(body).getByText("the text to replace was not found")).toBeInTheDocument();
  });

  it("shows a summary instead of the lines of a large or non-text file", async () => {
    const user = userEvent.setup();
    api.getWatch.mockResolvedValue(
      view([change({ path: "logo.png", summary: "Not a text file (34 KB)" })]),
    );
    render(<Harness />);
    const body = await openTab(user);
    expect(await within(body).findAllByText("Not a text file (34 KB)")).not.toHaveLength(0);
    expect(within(body).queryByRole("region")).toBeNull();
    expect(api.getWatchChange).not.toHaveBeenCalled();
  });

  it("says when the lines are from before Plenipo started again", async () => {
    const user = userEvent.setup();
    api.getWatch.mockResolvedValue(
      view(
        [
          change({
            sessionId: "",
            summary:
              "The lines are shown only while Plenipo runs; the file is in the working copy.",
          }),
        ],
        { fromTheRecord: true },
      ),
    );
    render(<Harness />);
    const body = await openTab(user);
    expect(
      await within(body).findByText(/lines from before it started again are not kept/),
    ).toBeInTheDocument();
    // No conversation to stop.
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });

  it("Stop stops the worker's task, and says when it could not", async () => {
    const user = userEvent.setup();
    api.getWatch.mockResolvedValue(view([change({ sessionId: "s-dev" })]));
    render(<Harness />);
    await openTab(user);
    const stop = screen.getByRole("button", { name: "Stop" });
    await waitFor(() => expect(stop).toBeEnabled());
    await user.click(stop);
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("s-dev");
    api.cancelAgentTurn.mockRejectedValueOnce({
      kind: "invalidInput",
      message: "No task is running in this conversation",
    });
    await user.click(stop);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "No task is running in this conversation",
    );
  });

  it("is read-only: there is nowhere to type", async () => {
    const user = userEvent.setup();
    const saved = change();
    api.getWatch.mockResolvedValue(view([saved]));
    api.getWatchChange.mockResolvedValue(marked(saved));
    render(<Harness />);
    const body = await openTab(user);
    await within(body).findByRole("region", { name: "src/app.rs, after the change" });
    tell({
      change: change({ path: "src/x.rs", state: "writing", at: 9_000 }),
      writing: "let x = 1;",
    });
    await within(body).findByText("let x = 1;");
    expect(within(body).queryByRole("textbox")).toBeNull();
    expect(body.querySelector("input, textarea, select, [contenteditable]")).toBeNull();
  });
  it("lists the agents working now under New terminal, as Watch a worker (ADR-055 §1)", async () => {
    const user = userEvent.setup();
    api.getWatch.mockResolvedValue(view([]));
    api.getLiveView.mockResolvedValue({
      at: 1,
      handoffs: [],
      workers: [
        {
          grantId: "g1",
          taskId: "t1",
          positionId: "p-dev",
          worker: "Senior Developer",
          runtimeId: "claude-code",
        },
      ],
    });
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Watch Senior Developer" }));
    await user.click(screen.getByRole("button", { name: /Close Watch for Senior Developer/ }));
    await user.click(await screen.findByRole("button", { name: "New terminal" }));
    await user.click(await screen.findByRole("menuitem", { name: /^Watch Senior Developer/ }));
    expect(
      await screen.findByRole("tab", { name: /Watch · Senior Developer/ }),
    ).toBeInTheDocument();
  });
  it("says so when Plenipo no longer keeps a saved change's lines, not that the file is empty", async () => {
    const user = userEvent.setup();
    const saved = change();
    api.getWatch.mockResolvedValue(view([saved]));
    // Plenipo let go of the lines (to keep its memory small) without telling the tab.
    const note =
      "3 lines added, 2 removed. The lines are no longer kept; the file is in the working copy.";
    api.getWatchChange.mockResolvedValue({
      change: { ...saved, summary: note },
      lines: [],
      removedAtEnd: 0,
    });
    render(<Harness />);
    const body = await openTab(user);
    expect(await within(body).findByText(note)).toBeInTheDocument();
    expect(within(body).queryByText("The file is empty now.")).toBeNull();
  });

  it("shows what Plenipo said when the file was read, when that is newer", async () => {
    const user = userEvent.setup();
    // The tab has it as being written (without its text); Plenipo has saved it since.
    const writing = change({ state: "writing", added: 0, removed: 0 });
    api.getWatch.mockResolvedValue(view([writing]));
    api.getWatchChange.mockResolvedValue(
      marked({ ...writing, state: "saved", at: writing.at + 5 }),
    );
    render(<Harness />);
    const body = await openTab(user);
    const file = await within(body).findByRole("region", { name: "src/app.rs, after the change" });
    expect(file).toHaveTextContent("TWO");
    expect(within(body).queryByText("Being written — not saved yet")).toBeNull();
  });

  it("Stop stops the task of the file shown, and names it when the list holds several", async () => {
    const user = userEvent.setup();
    api.getWatch.mockResolvedValue(
      view([
        change({ path: "src/b.rs", taskId: "t2", sessionId: "s-2", at: 2_000 }),
        change({ path: "src/a.rs", taskId: "t1", sessionId: "s-1", at: 1_000 }),
      ]),
    );
    render(<Harness />);
    const body = await openTab(user);
    // Following along: the newest file shows, made by Task 2.
    const stop2 = await screen.findByRole("button", { name: "Stop Task 2" });
    await waitFor(() => expect(stop2).toBeEnabled());
    // The file on screen decides which task Stop stops.
    await user.click(within(body).getByRole("button", { name: /a\.rs/ }));
    await user.click(screen.getByRole("button", { name: "Stop Task 1" }));
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("s-1");
    expect(api.cancelAgentTurn).not.toHaveBeenCalledWith("s-2");
  });

  it("Stop cancels through the Workers page when it is there, and says when a task is done", async () => {
    const user = userEvent.setup();
    const cancel = vi.fn(() => Promise.resolve());
    const agents = {
      state: {
        ...initialAgentState,
        sessions: {
          "s-busy": { id: "s-busy", activeTaskId: "t2", waitingTaskId: null },
          "s-done": { id: "s-done", activeTaskId: null, waitingTaskId: null },
        },
      },
      cancel,
    } as unknown as AgentsContextValue;
    api.getWatch.mockResolvedValue(
      view([
        change({ path: "src/b.rs", taskId: "t2", sessionId: "s-busy", at: 2_000 }),
        change({ path: "src/a.rs", taskId: "t1", sessionId: "s-done", at: 1_000 }),
      ]),
    );
    render(
      <AgentsContext.Provider value={agents}>
        <Harness />
      </AgentsContext.Provider>,
    );
    const body = await openTab(user);
    const stop = await screen.findByRole("button", { name: "Stop Task 2" });
    expect(stop).toHaveAttribute("title", "Stop Task 2, the task that changed this file");
    await user.click(stop);
    expect(cancel).toHaveBeenCalledWith("s-busy");
    expect(api.cancelAgentTurn).not.toHaveBeenCalled();
    // Task 1 is done: nothing to stop in its file's task (while Task 2 still works).
    await user.click(within(body).getByRole("button", { name: /a\.rs/ }));
    const stop1 = screen.getByRole("button", { name: "Stop Task 1" });
    expect(stop1).toBeDisabled();
    expect(stop1).toHaveAttribute("title", "Nothing to stop in this file's task");
  });

  it("says when Plenipo could not read the changes, and Try again reads them again", async () => {
    const user = userEvent.setup();
    api.getWatch.mockRejectedValueOnce({ kind: "internal", message: "The list is not ready" });
    render(<Harness />);
    const body = await openTab(user);
    const banner = await within(body).findByRole("alert");
    expect(banner).toHaveTextContent("Plenipo could not read the changes");
    expect(banner).toHaveTextContent("The list is not ready");
    api.getWatch.mockResolvedValue(view([change({ path: "README.md", summary: "Kept short" })]));
    await user.click(within(banner).getByRole("button", { name: "Try again" }));
    expect(await within(body).findAllByText("Kept short")).not.toHaveLength(0);
    expect(within(body).queryByRole("alert")).toBeNull();
    expect(api.getWatch).toHaveBeenCalledTimes(2);
  });

  it("clears the could-not-read banner when news of this agent's changes arrives", async () => {
    const user = userEvent.setup();
    api.getWatch.mockRejectedValueOnce({ kind: "internal", message: "The list is not ready" });
    render(<Harness />);
    const body = await openTab(user);
    await within(body).findByRole("alert");
    // Another agent's news is not this tab's.
    tell({
      change: change({ positionId: "p-other", path: "x.rs", state: "writing" }),
      writing: "x",
    });
    expect(within(body).getByRole("alert")).toHaveTextContent("Plenipo could not read the changes");
    tell({ change: change({ path: "src/x.rs", state: "writing" }), writing: "let x = 1;" });
    expect(within(body).queryByText("Plenipo could not read the changes")).toBeNull();
    expect(within(body).getByText("let x = 1;")).toBeInTheDocument();
  });

  it("reads a file again when it is picked after reading it failed", async () => {
    const user = userEvent.setup();
    const saved = change();
    api.getWatch.mockResolvedValue(view([saved]));
    api.getWatchChange.mockRejectedValueOnce({ kind: "internal", message: "Plenipo is busy" });
    render(<Harness />);
    const body = await openTab(user);
    expect(await within(body).findByText("Plenipo could not read this file")).toBeInTheDocument();
    expect(within(body).getByText("Plenipo is busy")).toBeInTheDocument();
    expect(within(body).getByText("Pick the file in the list to try again.")).toBeInTheDocument();
    expect(api.getWatchChange).toHaveBeenCalledTimes(1);
    api.getWatchChange.mockResolvedValue(marked(saved));
    await user.click(within(body).getByRole("button", { name: /app\.rs/ }));
    expect(
      await within(body).findByRole("region", { name: "src/app.rs, after the change" }),
    ).toBeInTheDocument();
    expect(within(body).queryByText("Plenipo could not read this file")).toBeNull();
    expect(api.getWatchChange).toHaveBeenCalledTimes(2);
  });

  it("names agents that share a title by their team (a title is unique only within a team)", async () => {
    const user = userEvent.setup();
    api.getLiveView.mockResolvedValue({
      at: 1,
      handoffs: [],
      workers: [
        {
          grantId: "g1",
          taskId: "t1",
          positionId: "p-web-dev",
          worker: "Senior Developer",
          runtimeId: "claude-code",
        },
        {
          grantId: "g2",
          taskId: "t2",
          positionId: "p-app-dev",
          worker: "Senior Developer",
          runtimeId: "claude-code",
        },
      ],
    });
    api.getOrganization.mockResolvedValue({
      positions: [
        { id: "p-web", title: "Website Supervisor", reportsTo: null },
        { id: "p-app", title: "App Supervisor", reportsTo: null },
        { id: "p-web-dev", title: "Senior Developer", reportsTo: "p-web" },
        { id: "p-app-dev", title: "Senior Developer", reportsTo: "p-app" },
      ],
    } as never);
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Watch Senior Developer" }));
    await user.click(screen.getByRole("button", { name: /Close Watch for Senior Developer/ }));
    await user.click(await screen.findByRole("button", { name: "New terminal" }));
    expect(
      await screen.findByRole("menuitem", {
        name: /^Watch Senior Developer \(Website Supervisor's team\)/,
      }),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("menuitem", { name: /^Watch Senior Developer \(App Supervisor's team\)/ }),
    );
    expect(
      await screen.findByRole("tab", {
        name: /Watch · Senior Developer \(App Supervisor's team\)/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "Close Watch for Senior Developer (App Supervisor's team)",
      }),
    ).toBeInTheDocument();
  });
});
