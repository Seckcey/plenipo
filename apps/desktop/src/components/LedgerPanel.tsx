import { useCallback, useEffect, useState } from "react";
import type { LedgerStatus } from "@plenipo/types";

import {
  createLedgerBackup,
  createSyntheticTask,
  exportLedger,
  getLedgerStatus,
  runIntegrityCheck,
  toCommandError,
} from "../api/commands";
import { formatBytes } from "../ledger/format";
import { formatTime } from "../runtime/format";

/** Ledger health and maintenance tools for the Diagnostics view. */
export function LedgerPanel({ onTaskCreated }: { onTaskCreated: (taskId: string) => void }) {
  const [status, setStatus] = useState<LedgerStatus | null>(null);
  const [message, setMessage] = useState<{ tone: "ok" | "error"; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await getLedgerStatus());
    } catch (reason) {
      setMessage({ tone: "error", text: toCommandError(reason).message });
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getLedgerStatus()
      .then((s) => !cancelled && setStatus(s))
      .catch((reason) => {
        if (!cancelled) setMessage({ tone: "error", text: toCommandError(reason).message });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function run(action: () => Promise<string>) {
    setBusy(true);
    setMessage(null);
    try {
      setMessage({ tone: "ok", text: await action() });
    } catch (reason) {
      setMessage({ tone: "error", text: toCommandError(reason).message });
    } finally {
      setBusy(false);
      void refresh();
    }
  }

  return (
    <>
      <h2>Ledger</h2>
      {status && !status.persistent && (
        <p className="status status--error" role="alert">
          Using a temporary ledger — nothing will be saved this session.
        </p>
      )}
      {status && (
        <dl className="facts" aria-label="Ledger details">
          <div>
            <dt>Schema</dt>
            <dd>v{status.schemaVersion}</dd>
          </div>
          <div>
            <dt>Size</dt>
            <dd>{formatBytes(status.sizeBytes)}</dd>
          </div>
          <div>
            <dt>Tasks</dt>
            <dd>{status.taskCount}</dd>
          </div>
          <div>
            <dt>Events</dt>
            <dd>{status.eventCount}</dd>
          </div>
          <div>
            <dt>Runs</dt>
            <dd>{status.executionCount}</dd>
          </div>
          <div>
            <dt>Integrity</dt>
            <dd className={status.lastIntegrityCheck?.ok ? "ok" : undefined}>
              {status.lastIntegrityCheck
                ? status.lastIntegrityCheck.ok
                  ? `OK · ${formatTime(status.lastIntegrityCheck.checkedAt)}`
                  : "Problems found"
                : "Checked at startup"}
            </dd>
          </div>
        </dl>
      )}
      {status?.path && <p className="muted path">Location: {status.path}</p>}
      {status?.lastBackup && (
        <p className="muted path">
          Last backup: {status.lastBackup.path} ({formatBytes(status.lastBackup.sizeBytes)}
          {status.lastBackup.verified ? ", verified" : ", NOT verified"})
        </p>
      )}

      <div className="actions">
        <button
          type="button"
          className="button button--small"
          disabled={busy}
          onClick={() =>
            void run(async () => {
              const r = await runIntegrityCheck();
              return r.ok
                ? "Integrity check passed."
                : `Integrity check found problems: ${r.messages.join("; ")}`;
            })
          }
        >
          Run integrity check
        </button>
        <button
          type="button"
          className="button button--small"
          disabled={busy || !status?.persistent}
          onClick={() =>
            void run(async () => {
              const b = await createLedgerBackup();
              return `Backup saved${b.verified ? " and verified" : ""}: ${b.path}`;
            })
          }
        >
          Create backup
        </button>
        <button
          type="button"
          className="button button--small"
          disabled={busy || !status?.persistent}
          onClick={() => void run(async () => `Exported to ${(await exportLedger()).path}`)}
        >
          Export JSON
        </button>
        <button
          type="button"
          className="button button--small"
          disabled={busy}
          onClick={() =>
            void run(async () => {
              const task = await createSyntheticTask();
              onTaskCreated(task.id);
              return `Created "${task.objective}". It is selected in Activity.`;
            })
          }
        >
          Create synthetic task
        </button>
      </div>
      {message && (
        <p
          className={`status ${message.tone === "error" ? "status--error" : "status--ok"}`}
          role={message.tone === "error" ? "alert" : "status"}
        >
          {message.text}
        </p>
      )}
    </>
  );
}
