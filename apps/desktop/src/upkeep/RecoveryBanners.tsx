import { useState } from "react";
import type { RecoveryStatus } from "@plenipo/types";
import { Banner, Button } from "@plenipo/ui";

import {
  dismissRecovery,
  dismissWindowRecovery,
  getRecoveryStatus,
  resetSettings,
  runAgain,
  toCommandError,
} from "../api/commands";
import type { Go } from "../components/views";
import { useLive } from "../pages/useLive";
import { when } from "../pages/words";
import { recoveryLead, recoveryTitle } from "./words";
import { useShown } from "./useShown";

/** Ledger events after which recovery has something new to say. */
function relevant(eventType: string): boolean {
  return (
    eventType.startsWith("plenipo.") ||
    eventType === "guard.settings_reset" ||
    eventType === "routing.settings_reset" ||
    eventType === "org.settings_changed"
  );
}

/**
 * What recovery has to tell you (Phase 13, ADR-037): how the last run ended and what it
 * stopped, with Run again and Leave stopped; a window that was brought back; and settings
 * Plenipo could not read. Notices, never alerts: the work is safe, and nothing runs again
 * until you say so.
 */
export function RecoveryBanners({ go }: { go: Go }) {
  const live = useLive<RecoveryStatus>(
    "recovery",
    () => getRecoveryStatus(),
    (e) => relevant(e.eventType),
  );
  const [current, setShown] = useShown(live);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const status = current;
  if (!status) return null;

  const act = async (key: string, work: () => Promise<RecoveryStatus>) => {
    setBusy(key);
    setError(null);
    try {
      setShown(await work());
      live.reload();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setBusy(null);
    }
  };

  const recovery = status.recovery;
  return (
    <>
      {recovery && (
        <Banner
          tone="warn"
          role="status"
          className="banner--recovery"
          label="How Plenipo last stopped"
          title={recoveryTitle(recovery)}
          action={
            <Button
              size="sm"
              disabled={busy !== null}
              onClick={() => void act("dismiss", () => dismissRecovery(recovery.id))}
            >
              {recovery.stoppedTasks.length > 0 ? "Leave stopped" : "OK"}
            </Button>
          }
        >
          <div className="muted">{recoveryLead(recovery)}</div>
          {recovery.stoppedTasks.length > 0 && (
            <ul className="banner__list recovery__tasks">
              {recovery.stoppedTasks.map((t) => (
                <li key={t.taskId} className="recovery__task">
                  <button
                    type="button"
                    className="ui-link"
                    onClick={() => go({ view: "task", id: t.taskId })}
                  >
                    {t.objective || "A task"}
                  </button>
                  {t.who && <span className="muted"> · {t.who}</span>}{" "}
                  {t.runAgainAs !== null ? (
                    <span className="muted">· started again</span>
                  ) : t.canRunAgain ? (
                    <Button
                      size="sm"
                      icon="play"
                      disabled={busy !== null}
                      onClick={() => void act(t.taskId, () => runAgain(t.taskId))}
                    >
                      {busy === t.taskId ? "Starting…" : "Run again"}
                    </Button>
                  ) : (
                    <span className="muted">· give it again from its page</span>
                  )}
                </li>
              ))}
            </ul>
          )}
          {error && (
            <p className="status status--error" role="alert">
              {error}
            </p>
          )}
        </Banner>
      )}
      {status.window && (
        <Banner
          tone="info"
          role="status"
          className="banner--notice"
          label="The window was brought back"
          title={`The window stopped unexpectedly at ${when(status.window.at)} and was ${
            status.window.reopened ? "opened again" : "reloaded"
          }`}
          onDismiss={() => void act("window", () => dismissWindowRecovery())}
        >
          <div className="muted">Your work kept running. Terminals open in it were closed.</div>
        </Banner>
      )}
      {status.settingsProblems.map((p) => (
        <Banner
          key={p.key}
          tone="warn"
          role="status"
          className="banner--recovery"
          title={`${p.label}: Plenipo could not read these settings`}
          action={
            <>
              <Button size="sm" onClick={() => go({ view: "diagnostics", id: null })}>
                Restore a backup
              </Button>
              <Button
                size="sm"
                disabled={busy !== null}
                onClick={() => void act(p.key, () => resetSettings(p.key))}
              >
                {busy === p.key ? "Resetting…" : "Reset to starting settings"}
              </Button>
            </>
          }
        >
          <div className="muted">{p.message}</div>
        </Banner>
      ))}
    </>
  );
}
