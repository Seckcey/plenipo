import type { ExecutionRecord, ExecutionState } from "@plenipo/types";

export const STATE_LABEL: Record<ExecutionState, string> = {
  starting: "Starting",
  running: "Running",
  succeeded: "Succeeded",
  failed: "Failed",
  cancelled: "Cancelled",
  timedOut: "Timed out",
  interrupted: "Interrupted",
};

export function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatDuration(record: ExecutionRecord, now: number): string {
  const end = record.endedAt ?? (record.state === "interrupted" ? null : now);
  if (end === null) return "—";
  const secs = Math.max(0, Math.round((end - record.startedAt) / 1000));
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  return `${m}m ${secs % 60}s`;
}

export function outcomeText(record: ExecutionRecord): string {
  const label = STATE_LABEL[record.state];
  return record.exitCode !== null && record.state === "failed"
    ? `${label} · exit ${record.exitCode}`
    : label;
}
