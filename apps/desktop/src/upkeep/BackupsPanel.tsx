import { useState } from "react";
import type { DiagnosticsFile, LedgerBackup, LedgerBackups } from "@plenipo/types";
import { Button, EmptyState, ErrorState, LoadingState } from "@plenipo/ui";

import {
  cancelLedgerRestore,
  listLedgerBackups,
  restoreLedgerBackup,
  saveDiagnosticsFile,
  toCommandError,
} from "../api/commands";
import { formatBytes } from "../ledger/format";
import { useLive } from "../pages/useLive";
import { when } from "../pages/words";
import { backupKind } from "./words";
import { useShown } from "./useShown";

/**
 * The Ledger's backups and Restore (Phase 13): made once a day, before a new version first
 * runs, before an update, before a layout change, and when you ask. Restoring keeps the Ledger
 * as it is now as a backup too, then restarts Plenipo to do it.
 */
export function BackupsPanel() {
  const live = useLive<LedgerBackups>(
    "backups",
    () => listLedgerBackups(),
    (e) => e.eventType.startsWith("ledger."),
  );
  const [current, setShown] = useShown(live);
  const [confirm, setConfirm] = useState<LedgerBackup | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  if (live.status === "loading") return <LoadingState label="Loading the backups" />;
  const b = current;
  if (!b) {
    return (
      <ErrorState title="Couldn't load the backups" message={live.error} onRetry={live.reload} />
    );
  }

  const restore = async (backup: LedgerBackup) => {
    setBusy(true);
    setMessage(null);
    try {
      setShown(await restoreLedgerBackup(backup.name));
      setMessage({
        ok: true,
        text: "Plenipo is restarting to restore the Ledger. It opens again in a moment.",
      });
    } catch (reason) {
      setMessage({ ok: false, text: toCommandError(reason).message });
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  };

  const cancel = async () => {
    setBusy(true);
    try {
      setShown(await cancelLedgerRestore());
      setMessage(null);
    } catch (reason) {
      setMessage({ ok: false, text: toCommandError(reason).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="backups" aria-labelledby="backups-title">
      <h2 id="backups-title">Backups of the Ledger</h2>
      <p className="muted">
        Plenipo backs up the Ledger once a day while it runs (keeping a week), before a new version
        first runs, before installing an update, and when you choose Create backup.
        {b.folder && (
          <>
            {" "}
            They are kept in <code className="page__path">{b.folder}</code>.
          </>
        )}
      </p>
      {b.pendingRestore && (
        <div className="notice-box" role="note">
          <p>
            The Ledger will be restored from <strong>{b.pendingRestore}</strong> when Plenipo
            starts.
          </p>
          <Button size="sm" disabled={busy} onClick={() => void cancel()}>
            Cancel the restore
          </Button>
        </div>
      )}
      {b.backups.length === 0 ? (
        <EmptyState compact title="No backups yet">
          Plenipo makes the first one about ten minutes after it starts. Or choose Create backup.
        </EmptyState>
      ) : (
        <table className="table backups__table">
          <thead>
            <tr>
              <th scope="col">Made</th>
              <th scope="col">Why</th>
              <th scope="col">Size</th>
              <th scope="col">
                <span className="visually-hidden">Restore</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {b.backups.map((backup) => (
              <tr key={backup.name}>
                <td>{when(backup.createdAt)}</td>
                <td>
                  {backupKind(backup)}
                  {backup.problem && <div className="muted">{backup.problem}</div>}
                </td>
                <td className="ui-num">{formatBytes(backup.sizeBytes)}</td>
                <td>
                  {backup.restorable && (
                    <Button
                      size="sm"
                      disabled={busy}
                      aria-label={`Restore the backup from ${when(backup.createdAt)}`}
                      onClick={() => setConfirm(backup)}
                    >
                      Restore
                    </Button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {confirm && (
        <div className="notice-box" role="note" aria-label="Restore the Ledger">
          <p>
            <strong>Restore the Ledger from {when(confirm.createdAt)}?</strong> Everything recorded
            after it is set aside: Plenipo keeps the Ledger as it is now as a backup, so you can go
            back to it. Plenipo then restarts to restore it; work that is running stops first.
          </p>
          <div className="settings-section__actions">
            <Button
              size="sm"
              variant="primary"
              disabled={busy}
              onClick={() => void restore(confirm)}
            >
              Restore and restart
            </Button>
            <Button size="sm" onClick={() => setConfirm(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}
      {message && (
        <p
          className={message.ok ? "status status--ok" : "status status--error"}
          role={message.ok ? "status" : "alert"}
        >
          {message.text}
        </p>
      )}
    </section>
  );
}

/**
 * Save a diagnostics file to send when something went wrong (Phase 13). It holds no task
 * text, no answers, nothing typed in the terminal, and no secrets.
 */
export function DiagnosticsFileButton() {
  const [busy, setBusy] = useState(false);
  const [file, setFile] = useState<DiagnosticsFile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const save = async () => {
    setBusy(true);
    setError(null);
    setCopied(false);
    try {
      setFile(await saveDiagnosticsFile());
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setBusy(false);
    }
  };

  const copy = async () => {
    if (!file) return;
    try {
      await navigator.clipboard.writeText(file.path);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="diagnostics-file">
      <div className="settings-section__actions">
        <Button size="sm" icon="file" disabled={busy} onClick={() => void save()}>
          {busy ? "Saving…" : "Save a diagnostics file"}
        </Button>
      </div>
      <p className="muted">
        One file to send when something went wrong: this version, Windows, the Ledger&apos;s health,
        how Plenipo last stopped, the AI tools it found, and its log files. No tasks, no answers,
        nothing you typed in the terminal, and no secrets.
      </p>
      {file && (
        <div className="status status--ok" role="status">
          Saved ({formatBytes(file.sizeBytes)}): <code className="page__path">{file.path}</code>{" "}
          <Button size="sm" onClick={() => void copy()}>
            {copied ? "Copied" : "Copy its location"}
          </Button>
        </div>
      )}
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
