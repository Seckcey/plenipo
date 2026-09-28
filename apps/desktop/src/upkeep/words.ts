// Plain words for keeping Plenipo dependable (Phase 13): how the last run ended, backups, and
// updates. docs/design/vocabulary.md lists the pairs.

import type {
  BackupKind,
  LedgerBackup,
  Recovery,
  RecoveryCause,
  UpdateStatus,
} from "@plenipo/types";

import { when } from "../pages/words";

/** What happened to the last run, as a title with its time. */
export function recoveryTitle(recovery: Recovery, now: number = Date.now()): string {
  const at = recovery.lastSeenAt ? ` at ${when(recovery.lastSeenAt, now)}` : "";
  const titles: Record<RecoveryCause, string> = {
    crash: `Plenipo closed unexpectedly${at}`,
    windowsRestart: `Windows closed Plenipo${at} (a restart, a shutdown, or signing out)`,
    layoutChange: `Plenipo was stopped${at} while updating the Ledger`,
    unknown: "Plenipo did not close normally last time",
  };
  return titles[recovery.cause];
}

/** A line under the title: what it means now. */
export function recoveryLead(recovery: Recovery): string {
  const n = recovery.stoppedTasks.length;
  const layout =
    recovery.cause === "layoutChange"
      ? "Nothing was lost: the unfinished step was undone and done again, with a backup from before it. "
      : "";
  if (n === 0) {
    const programs = recovery.stoppedPrograms;
    if (programs === 1) return `${layout}1 program that was running was stopped.`;
    if (programs > 1) return `${layout}${programs} programs that were running were stopped.`;
    return `${layout}Nothing was running. Plenipo is running again.`;
  }
  return `${layout}${n === 1 ? "This task was stopped" : `These ${n} tasks were stopped`}. Nothing runs again until you choose Run again.`;
}

const KIND_WORDS: Record<BackupKind, string> = {
  manual: "Made by you",
  daily: "Daily",
  beforeUpgrade: "Before a new version",
  beforeUpdate: "Before an update",
  beforeMigration: "Before a layout change",
  beforeRestore: "Before a restore",
};

/** What made a backup, in plain words. */
export function backupKind(backup: LedgerBackup): string {
  const words = KIND_WORDS[backup.kind];
  if (backup.kind === "beforeUpgrade" && backup.version) {
    return backup.version === "earlier"
      ? "Before a new version (from an earlier one)"
      : `Before a new version (from ${backup.version})`;
  }
  if (backup.kind === "beforeUpdate" && backup.version) {
    return `Before updating to ${backup.version}`;
  }
  return words;
}

/** Settings → Updates: where checking got to, in a line. */
export function updateLine(status: UpdateStatus, now: number = Date.now()): string {
  const checked = status.lastCheckedAt ? ` (checked ${when(status.lastCheckedAt, now)})` : "";
  switch (status.state) {
    case "notChecked":
      return "Not checked yet since Plenipo started.";
    case "checking":
      return "Checking GitHub for a new version…";
    case "upToDate":
      return `You have the newest version${checked}.`;
    case "available":
      return `Plenipo ${status.available?.version ?? ""} is ready to install${checked}.`;
    case "installing":
      return "Downloading and checking the update…";
    case "failed":
      return status.message ?? "The last check did not work.";
  }
}
