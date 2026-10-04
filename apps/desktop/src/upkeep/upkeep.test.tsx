import type { ReactNode } from "react";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SYSTEM_WORDS } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { setSystemWords } from "../system/words";
import { a11yProblems } from "../test/a11y";
import {
  NO_RECOVERY,
  crashRecovery,
  sampleBackups,
  startAndClose,
  updateReady,
  upToDate,
} from "../test/upkeepFixtures";
import { BackupsPanel, DiagnosticsFileButton } from "./BackupsPanel";
import { RecoveryBanners } from "./RecoveryBanners";
import { StartAndCloseSettings } from "./StartAndCloseSettings";
import { UpdateMark, UpdateSettings } from "./UpdateSettings";
import { HEARTBEAT_MS, useWindowHeartbeat } from "./useWindowHeartbeat";
import { backupKind, recoveryLead, recoveryTitle, updateLine } from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRecoveryStatus: vi.fn(),
    runAgain: vi.fn(),
    dismissRecovery: vi.fn(),
    dismissWindowRecovery: vi.fn(),
    resetSettings: vi.fn(),
    windowAlive: vi.fn(),
    getStartAndClose: vi.fn(),
    setStartAndClose: vi.fn(),
    getUpdateStatus: vi.fn(),
    checkForUpdates: vi.fn(),
    installUpdate: vi.fn(),
    openReleasesPage: vi.fn(),
    listLedgerBackups: vi.fn(),
    restoreLedgerBackup: vi.fn(),
    cancelLedgerRestore: vi.fn(),
    saveDiagnosticsFile: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();

/** A page around a part: its main heading, and the section heading a Settings panel has. */
function inPage(part: ReactNode, section = true) {
  return render(
    <main>
      <h1>Page</h1>
      {section && <h2>Section</h2>}
      {part}
    </main>,
  );
}

beforeEach(() => {
  go.mockReset();
  api.getRecoveryStatus.mockResolvedValue(NO_RECOVERY);
  api.windowAlive.mockResolvedValue(undefined);
  api.getStartAndClose.mockResolvedValue(startAndClose());
  api.getUpdateStatus.mockResolvedValue(upToDate());
  api.listLedgerBackups.mockResolvedValue(sampleBackups());
});

afterEach(() => {
  vi.useRealTimers();
  setSystemWords(SYSTEM_WORDS.windows);
});

describe("words", () => {
  it("says how the last run ended, with its time, and what it means", () => {
    const r = crashRecovery().recovery!;
    const now = new Date(2026, 8, 27, 16, 0).getTime();
    expect(recoveryTitle(r, now)).toBe("Plenipo closed unexpectedly at 3:14 PM");
    expect(recoveryTitle({ ...r, cause: "windowsRestart" }, now)).toBe(
      "Windows closed Plenipo at 3:14 PM (a restart, a shutdown, or signing out)",
    );
    // Each system names itself (ADR-155).
    setSystemWords(SYSTEM_WORDS.mac);
    expect(recoveryTitle({ ...r, cause: "windowsRestart" }, now)).toBe(
      "Your Mac closed Plenipo at 3:14 PM (a restart, a shutdown, or logging out)",
    );
    setSystemWords(SYSTEM_WORDS.linux);
    expect(recoveryTitle({ ...r, cause: "windowsRestart" }, now)).toBe(
      "Your computer closed Plenipo at 3:14 PM (a restart, a shutdown, or signing out)",
    );
    expect(recoveryTitle({ ...r, cause: "unknown", lastSeenAt: null }, now)).toBe(
      "Plenipo did not close normally last time",
    );
    expect(recoveryLead(r)).toBe(
      "These 2 tasks were stopped. Nothing runs again until you choose Run again.",
    );
    expect(recoveryLead({ ...r, stoppedTasks: [] })).toBe(
      "1 program that was running was stopped.",
    );
    expect(recoveryLead({ ...r, stoppedTasks: [], stoppedPrograms: 0 })).toBe(
      "Nothing was running. Plenipo is running again.",
    );
    expect(recoveryLead({ ...r, cause: "layoutChange", stoppedTasks: [] })).toMatch(
      /^Nothing was lost: the unfinished step was undone and done again/,
    );
  });

  it("names backups and update states in plain words", () => {
    const [daily, upgrade, broken] = sampleBackups().backups;
    expect(backupKind(daily!)).toBe("Daily");
    expect(backupKind(upgrade!)).toBe("Before a new version (from 1.8.0)");
    expect(backupKind({ ...broken!, kind: "beforeUpdate", version: "1.10.0" })).toBe(
      "Before updating to 1.10.0",
    );
    expect(backupKind({ ...broken!, kind: "beforeUpgrade", version: "earlier" })).toBe(
      "Before a new version (from an earlier one)",
    );
    const now = new Date(2026, 8, 27, 12, 0).getTime();
    expect(updateLine(upToDate(), now)).toBe("You have the newest version (checked 9:05 AM).");
    expect(updateLine(updateReady(), now)).toBe(
      "Plenipo 1.10.0 is ready to install (checked 9:05 AM).",
    );
    expect(updateLine({ ...upToDate(), state: "failed", message: "No internet." }, now)).toBe(
      "No internet.",
    );
  });
});

describe("recovery notices", () => {
  it("say what stopped, as a notice and not an alarm, with Run again and Leave stopped", async () => {
    const status = crashRecovery();
    api.getRecoveryStatus.mockResolvedValue(status);
    const again = crashRecovery();
    again.recovery!.stoppedTasks[0]!.runAgainAs = "new-task";
    api.runAgain.mockResolvedValue(again);
    api.dismissRecovery.mockResolvedValue(NO_RECOVERY);
    const { container } = inPage(<RecoveryBanners go={go} />);
    const banner = await screen.findByRole("status", { name: "How Plenipo last stopped" });
    expect(within(banner).getByText(/^Plenipo closed unexpectedly at/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(within(banner).getByText("Fix the login page")).toBeInTheDocument();
    expect(within(banner).getByText(/Web Supervisor/)).toBeInTheDocument();
    // A task nobody can be given again says so.
    expect(within(banner).getByText(/give it again from its page/)).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
    const user = userEvent.setup();
    // Its page opens from the notice.
    await user.click(within(banner).getByRole("button", { name: "Fix the login page" }));
    expect(go).toHaveBeenCalledWith({ view: "task", id: status.recovery!.stoppedTasks[0]!.taskId });
    await user.click(within(banner).getByRole("button", { name: "Run again" }));
    expect(api.runAgain).toHaveBeenCalledWith(status.recovery!.stoppedTasks[0]!.taskId);
    expect(await within(banner).findByText(/started again/)).toBeInTheDocument();
    await user.click(within(banner).getByRole("button", { name: "Leave stopped" }));
    expect(api.dismissRecovery).toHaveBeenCalledWith(status.recovery!.id);
    await waitFor(() =>
      expect(screen.queryByRole("status", { name: "How Plenipo last stopped" })).toBeNull(),
    );
  });

  it("say when the window was brought back, and when settings could not be read", async () => {
    api.getRecoveryStatus.mockResolvedValue({
      recovery: null,
      window: { at: new Date(2026, 8, 27, 15, 14).getTime(), reopened: true },
      settingsProblems: [
        {
          key: "guard",
          label: "Permissions",
          message: "Plenipo could not read your permission settings.",
        },
      ],
    });
    api.dismissWindowRecovery.mockResolvedValue(NO_RECOVERY);
    api.resetSettings.mockResolvedValue(NO_RECOVERY);
    render(<RecoveryBanners go={go} />);
    const window = await screen.findByRole("status", { name: "The window was brought back" });
    expect(window).toHaveTextContent(/and was opened again/);
    expect(window).toHaveTextContent("Your work kept running.");
    expect(
      screen.getByText("Permissions: Plenipo could not read these settings"),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Restore a backup" }));
    expect(go).toHaveBeenCalledWith({ view: "diagnostics", id: null });
    await user.click(screen.getByRole("button", { name: "Reset to starting settings" }));
    expect(api.resetSettings).toHaveBeenCalledWith("guard");
    await waitFor(() => expect(screen.queryByText(/could not read these settings/)).toBeNull());
  });

  it("show nothing when there is nothing to say", async () => {
    const { container } = render(<RecoveryBanners go={go} />);
    await waitFor(() => expect(api.getRecoveryStatus).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});

describe("the window's heartbeat", () => {
  function Beating() {
    useWindowHeartbeat();
    return null;
  }

  it("tells Plenipo the page is alive every few seconds, and whether it is seen", () => {
    vi.useFakeTimers();
    const { unmount } = render(<Beating />);
    expect(api.windowAlive).toHaveBeenCalledTimes(1);
    expect(api.windowAlive).toHaveBeenLastCalledWith(true);
    act(() => {
      vi.advanceTimersByTime(HEARTBEAT_MS * 3);
    });
    expect(api.windowAlive).toHaveBeenCalledTimes(4);
    unmount();
    act(() => {
      vi.advanceTimersByTime(HEARTBEAT_MS * 3);
    });
    expect(api.windowAlive).toHaveBeenCalledTimes(4);
  });
});

describe("Settings → Start and close", () => {
  it("turns Start with Windows on and chooses what closing the window does", async () => {
    api.setStartAndClose.mockImplementation((input) =>
      Promise.resolve({ ...startAndClose(), ...input }),
    );
    const { container } = inPage(<StartAndCloseSettings />);
    const start = await screen.findByRole("switch", { name: "Start Plenipo with Windows" });
    expect(start).toHaveAttribute("aria-checked", "false");
    const user = userEvent.setup();
    await user.click(start);
    expect(api.setStartAndClose).toHaveBeenCalledWith({
      startWithWindows: true,
      closeWindow: "keepWhileWorking",
    });
    await waitFor(() => expect(start).toHaveAttribute("aria-checked", "true"));
    expect(
      screen.getByRole("radio", { name: /Keep Plenipo in the tray while work is going/ }),
    ).toBeChecked();
    await user.click(screen.getByRole("radio", { name: /Quit Plenipo and stop its work/ }));
    expect(api.setStartAndClose).toHaveBeenLastCalledWith({
      startWithWindows: true,
      closeWindow: "quit",
    });
    expect(a11yProblems(container)).toEqual([]);
  });

  it("says when this computer cannot start Plenipo with Windows", async () => {
    api.getStartAndClose.mockResolvedValue({ ...startAndClose(), canStartWithWindows: false });
    render(<StartAndCloseSettings />);
    expect(
      await screen.findByRole("switch", { name: "Start Plenipo with Windows" }),
    ).toBeDisabled();
    expect(
      screen.getByText("Starting with Windows is not available on this computer."),
    ).toBeInTheDocument();
  });

  it("says it in a Mac's words on a Mac, and Linux's on Linux (ADR-155)", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    const { unmount } = render(<StartAndCloseSettings />);
    expect(await screen.findByRole("heading", { name: "When you log in" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Open Plenipo when you log in" })).toBeEnabled();
    expect(screen.getByText(/System Settings → General → Login Items/)).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /Keep Plenipo in the menu bar while work is going/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Quit Plenipo from its menu in the menu bar/)).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/Windows|tray/);
    unmount();
    setSystemWords(SYSTEM_WORDS.linux);
    render(<StartAndCloseSettings />);
    expect(
      await screen.findByRole("switch", { name: "Start Plenipo when you sign in" }),
    ).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/Windows/);
  });
});

describe("Settings → Updates", () => {
  it("checks now, and installs only when you say so, asking first if work is running", async () => {
    api.getUpdateStatus.mockResolvedValue(upToDate());
    api.checkForUpdates.mockResolvedValue(updateReady());
    api.installUpdate
      .mockRejectedValueOnce(
        new commands.PlenipoCommandError(
          "invalidInput",
          "Work is running. Installing the update stops it; say so to go ahead.",
        ),
      )
      .mockResolvedValueOnce({ ...updateReady(), state: "installing" });
    const { container } = inPage(<UpdateSettings />);
    expect(await screen.findByText(/You have the newest version/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Install now" })).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(api.checkForUpdates).toHaveBeenCalled();
    expect(await screen.findByText("What's new in Plenipo 1.10.0")).toBeInTheDocument();
    expect(screen.getByText("Fixes and safety.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Install now" }));
    expect(api.installUpdate).toHaveBeenCalledWith(false);
    // Work is running: nothing happens until you agree it stops.
    expect(await screen.findByText("Work is running.")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Stop the work and install" }));
    expect(api.installUpdate).toHaveBeenLastCalledWith(true);
    expect(a11yProblems(container)).toEqual([]);
  });

  it("a copy your computer's installer put there downloads the new version (Phase 23)", async () => {
    api.getUpdateStatus.mockResolvedValue({ ...updateReady(), how: "byHand" });
    api.openReleasesPage.mockResolvedValue(undefined);
    const { container } = inPage(<UpdateSettings />);
    expect(await screen.findByText(/Plenipo 1.10.0 is ready to download/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Install now" })).toBeNull();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Download the new version" }));
    expect(api.openReleasesPage).toHaveBeenCalled();
    expect(api.installUpdate).not.toHaveBeenCalled();
    expect(screen.getByText(/It opens GitHub in your browser/)).toBeInTheDocument();
    expect(screen.getByText(/never changes itself/)).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("a Mac copy that cannot update itself says to move it to Applications (Phase 23)", async () => {
    setSystemWords(SYSTEM_WORDS.mac);
    api.getUpdateStatus.mockResolvedValue({ ...updateReady(), how: "byHand" });
    inPage(<UpdateSettings />);
    expect(await screen.findByText(/Plenipo 1.10.0 is ready to download/)).toBeInTheDocument();
    expect(
      screen.getByText(/Move Plenipo to the Applications folder on your Mac's own disk/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/never changes itself/)).toBeNull();
  });

  it("says when this copy cannot install updates", async () => {
    api.getUpdateStatus.mockResolvedValue({
      ...updateReady(),
      canInstall: false,
      message:
        "This copy of Plenipo was not built by 8 West's Release workflow, so it cannot install updates.",
    });
    render(<UpdateSettings />);
    expect(await screen.findByText(/cannot install updates/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Install now" })).toBeNull();
  });

  it("shows Update ready in the top bar only when a new version is ready", async () => {
    const { unmount } = render(<UpdateMark go={go} />);
    await waitFor(() => expect(api.getUpdateStatus).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /Update ready/ })).toBeNull();
    unmount();
    api.getUpdateStatus.mockResolvedValue(updateReady());
    render(<UpdateMark go={go} />);
    await userEvent.setup().click(await screen.findByRole("button", { name: /Update ready/ }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "updates" });
  });

  it("looks again when the window comes to the front, with no new Ledger event", async () => {
    api.getUpdateStatus.mockResolvedValue(upToDate());
    render(<UpdateMark go={go} />);
    await waitFor(() => expect(api.getUpdateStatus).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /Update ready/ })).toBeNull();
    // The daily check found a version it had announced before (after a restart, say).
    api.getUpdateStatus.mockResolvedValue(updateReady());
    window.dispatchEvent(new Event("focus"));
    expect(await screen.findByRole("button", { name: /Update ready/ })).toBeInTheDocument();
  });
});

describe("Diagnostics → backups and the diagnostics file", () => {
  it("lists the backups by why they were made, and restores one after asking", async () => {
    api.restoreLedgerBackup.mockResolvedValue({
      ...sampleBackups(),
      pendingRestore: "daily-backup-1790000000000.db",
    });
    const { container } = inPage(<BackupsPanel />, false);
    const table = await screen.findByRole("table");
    expect(within(table).getAllByText("Daily")).toHaveLength(2);
    expect(within(table).getByText("Before a new version (from 1.8.0)")).toBeInTheDocument();
    // A backup that cannot be restored says why, and has no Restore button.
    expect(within(table).getByText(/cannot be read/)).toBeInTheDocument();
    expect(within(table).getAllByRole("button", { name: /^Restore the backup from/ })).toHaveLength(
      2,
    );
    expect(a11yProblems(container)).toEqual([]);
    const user = userEvent.setup();
    await user.click(
      within(table).getAllByRole("button", { name: /^Restore the backup from/ })[0]!,
    );
    // Nothing happens before you confirm.
    expect(api.restoreLedgerBackup).not.toHaveBeenCalled();
    const confirm = screen.getByRole("note", { name: "Restore the Ledger" });
    expect(confirm).toHaveTextContent(/keeps the Ledger as it is now as a backup/);
    await user.click(within(confirm).getByRole("button", { name: "Restore and restart" }));
    expect(api.restoreLedgerBackup).toHaveBeenCalledWith("daily-backup-1790000000000.db");
    expect(
      await screen.findByText(/Plenipo is restarting to restore the Ledger/),
    ).toBeInTheDocument();
    expect(screen.getByText(/will be restored from/)).toBeInTheDocument();
  });

  it("saves a diagnostics file and says where it is", async () => {
    api.saveDiagnosticsFile.mockResolvedValue({
      path: "C:\\Users\\you\\AppData\\Local\\com.eightwest.plenipo\\diagnostics\\plenipo-diagnostics-20260927-151400.zip",
      sizeBytes: 48_000,
      createdAt: 1,
      contents: ["README.txt", "about.json", "recent-events.json", "logs/plenipo.log"],
    });
    render(<DiagnosticsFileButton />);
    expect(
      screen.getByText(/No tasks, no answers, nothing you typed in the terminal/),
    ).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Save a diagnostics file" }));
    expect(await screen.findByText(/plenipo-diagnostics-20260927-151400\.zip/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy its location" })).toBeInTheDocument();
  });
});
