// Samples for keeping Plenipo dependable (Phase 13): recovery, Start and close, backups, and
// updates.

import type { LedgerBackups, RecoveryStatus, StartAndClose, UpdateStatus } from "@plenipo/types";

export const NO_RECOVERY: RecoveryStatus = { recovery: null, window: null, settingsProblems: [] };

/** Plenipo closed unexpectedly while two tasks ran. */
export function crashRecovery(): RecoveryStatus {
  return {
    recovery: {
      id: "11111111-2222-3333-4444-555555555555",
      cause: "crash",
      lastSeenAt: new Date(2026, 8, 27, 15, 14).getTime(),
      foundAt: new Date(2026, 8, 27, 15, 20).getTime(),
      previousVersion: "1.9.0",
      stoppedTasks: [
        {
          taskId: "0f8fad5b-d9cb-469f-a165-70867728950e",
          objective: "Fix the login page",
          who: "Web Supervisor",
          canRunAgain: true,
          runAgainAs: null,
        },
        {
          taskId: "7c9e6679-7425-40de-944b-e07fc1f90ae7",
          objective: "Synthetic diagnostic task #1",
          who: null,
          canRunAgain: false,
          runAgainAs: null,
        },
      ],
      stoppedPrograms: 1,
    },
    window: null,
    settingsProblems: [],
  };
}

export function startAndClose(): StartAndClose {
  return { startWithWindows: false, canStartWithWindows: true, closeWindow: "keepWhileWorking" };
}

export function upToDate(): UpdateStatus {
  return {
    version: "1.9.0",
    state: "upToDate",
    canInstall: true,
    lastCheckedAt: new Date(2026, 8, 27, 9, 5).getTime(),
    available: null,
    message: null,
    releasesPage: "https://github.com/Seckcey/plenipo/releases",
    how: "installer",
  };
}

export function updateReady(): UpdateStatus {
  return {
    ...upToDate(),
    state: "available",
    available: { version: "1.10.0", notes: "Fixes and safety.", published: "2026-10-01" },
  };
}

export function sampleBackups(): LedgerBackups {
  return {
    backups: [
      {
        name: "daily-backup-1790000000000.db",
        kind: "daily",
        createdAt: new Date(2026, 8, 27, 10, 0).getTime(),
        sizeBytes: 2_400_000,
        version: null,
        schemaVersion: 8,
        restorable: true,
        problem: null,
      },
      {
        name: "pre-upgrade-1.8.0-1789990000000.db",
        kind: "beforeUpgrade",
        createdAt: new Date(2026, 8, 26, 18, 30).getTime(),
        sizeBytes: 2_300_000,
        version: "1.8.0",
        schemaVersion: 8,
        restorable: true,
        problem: null,
      },
      {
        name: "daily-backup-1780000000000.db",
        kind: "daily",
        createdAt: new Date(2026, 8, 20, 10, 0).getTime(),
        sizeBytes: 12,
        version: null,
        schemaVersion: null,
        restorable: false,
        problem: "It cannot be read (file is not a database).",
      },
    ],
    pendingRestore: null,
    folder: "C:\\Users\\you\\AppData\\Local\\com.eightwest.plenipo\\ledger\\backups",
  };
}
