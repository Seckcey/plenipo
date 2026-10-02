import type { ControlStatus, Recovery, RecoveryCause } from "@plenipo/types";

/** One organization's notice that Plenipo stopped unexpectedly, and what stopped with it. */
export interface OrgRecovery {
  org: string;
  name: string;
  recovery: Recovery;
}

/** What `readControl` says: Stop all, and the work that stopped when Plenipo did. */
export interface ControlRead {
  control: ControlStatus;
  recovery: OrgRecovery[];
}

const TITLES: Record<RecoveryCause, string> = {
  crash: "Plenipo closed unexpectedly on your PC",
  windowsRestart: "Windows closed Plenipo on your PC (a restart, a shutdown, or signing out)",
  layoutChange: "Plenipo was stopped while updating the Ledger on your PC",
  unknown: "Plenipo did not close normally last time",
};

/** The notice's title, in plain words (as the PC says it). */
export function recoveryTitle(recovery: Recovery): string {
  return TITLES[recovery.cause];
}

/** A line under the title: what it means now. */
export function recoveryLead(recovery: Recovery): string {
  const n = recovery.stoppedTasks.length;
  if (n === 0) return "Nothing was running. Plenipo is running again.";
  return `${n === 1 ? "This task was stopped" : `These ${n} tasks were stopped`}. Nothing runs again until you choose Run again.`;
}
