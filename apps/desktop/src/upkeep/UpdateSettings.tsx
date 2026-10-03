import { useEffect, useState } from "react";
import type { UpdateStatus } from "@plenipo/types";
import { Button, ErrorState, LoadingState, PropertyList, StatusPill } from "@plenipo/ui";

import {
  checkForUpdates,
  getUpdateStatus,
  installUpdate,
  openReleasesPage,
  toCommandError,
} from "../api/commands";
import type { Go } from "../components/views";
import { useLive } from "../pages/useLive";
import { when } from "../pages/words";
import { systemWords } from "../system/words";
import { updateLine } from "./words";
import { useShown } from "./useShown";

/** How often the update state is read again (it lives in Plenipo, not only in the Ledger). */
const LOOK_AGAIN_MS = 60_000;

function useUpdates() {
  const live = useLive<UpdateStatus>(
    "updates",
    () => getUpdateStatus(),
    (e) => e.eventType.startsWith("plenipo.update"),
  );
  // A check that finds a version already announced writes nothing new to the Ledger (after a
  // restart, say): look again now and then, and when the window comes to the front.
  const { reload } = live;
  useEffect(() => {
    const timer = setInterval(reload, LOOK_AGAIN_MS);
    window.addEventListener("focus", reload);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", reload);
    };
  }, [reload]);
  return live;
}

/**
 * The top bar's "Update ready" mark (ADR-038): shown only when a newer version is ready. It
 * opens Settings → Updates; nothing installs until you say so there.
 */
export function UpdateMark({ go }: { go: Go }) {
  const live = useUpdates();
  const s = live.value;
  if (!s || s.state !== "available" || !s.available) return null;
  return (
    <button
      type="button"
      className="shell__update"
      aria-label={`Update ready: Plenipo ${s.available.version}. Open Settings → Updates.`}
      title={`Plenipo ${s.available.version} is ready to ${s.how === "byHand" ? "download" : "install"}`}
      onClick={() => go({ view: "settings", id: "updates" })}
    >
      <StatusPill status="pending" label="Update ready" />
    </button>
  );
}

/**
 * Settings → Updates (ADR-038): this version, when Plenipo last checked, Check now, and a
 * newer version's notes with Install now. Checking is always on (once a day); installing only
 * when you say so, and only a version 8 West signed. A copy your computer's own installer put
 * there (Linux's `.deb`) is updated the same way: Download the new version (Phase 23, ADR-152).
 */
export function UpdateSettings() {
  const live = useUpdates();
  const [current, setShown] = useShown(live);
  const [busy, setBusy] = useState<"check" | "install" | "download" | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** Installing would stop running work: ask first. */
  const [confirm, setConfirm] = useState<string | null>(null);

  if (live.status === "loading") return <LoadingState label="Loading updates" />;
  const s = current;
  if (!s) {
    return <ErrorState title="Couldn't load updates" message={live.error} onRetry={live.reload} />;
  }

  const check = async () => {
    setBusy("check");
    setError(null);
    try {
      setShown(await checkForUpdates());
      live.reload();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setBusy(null);
    }
  };

  const install = async (stopWork: boolean) => {
    setBusy("install");
    setError(null);
    setConfirm(null);
    try {
      setShown(await installUpdate(stopWork));
    } catch (reason) {
      const message = toCommandError(reason).message;
      if (!stopWork && message.startsWith("Work is running")) setConfirm(message);
      else setError(message);
      live.reload();
    } finally {
      setBusy(null);
    }
  };

  const download = async () => {
    setBusy("download");
    setError(null);
    try {
      await openReleasesPage();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setBusy(null);
    }
  };

  const byHand = s.how === "byHand";
  const replacesItself = s.how === "replacesItself";

  return (
    <div className="settings-section__body settings-updates">
      <PropertyList
        items={[
          { label: "This version", value: s.version },
          { label: "Newest version", value: updateLine(s) },
          {
            label: "Last checked",
            value: s.lastCheckedAt ? when(s.lastCheckedAt) : "Not yet",
          },
        ]}
      />
      <div className="settings-section__actions">
        <Button size="sm" icon="refresh" disabled={busy !== null} onClick={() => void check()}>
          {busy === "check" ? "Checking…" : "Check now"}
        </Button>
      </div>
      {s.available && (
        <section aria-labelledby="updates-new" className="settings-updates__new">
          <h3 id="updates-new">What&apos;s new in Plenipo {s.available.version}</h3>
          {s.available.notes ? (
            <pre className="settings-updates__notes">{s.available.notes}</pre>
          ) : (
            <p className="muted">No notes came with it.</p>
          )}
          {s.canInstall && !byHand && (
            <div className="settings-section__actions">
              <Button
                size="sm"
                variant="primary"
                disabled={busy !== null}
                onClick={() => void install(false)}
              >
                {busy === "install" ? "Installing…" : "Install now"}
              </Button>
            </div>
          )}
          {s.canInstall && byHand && (
            <>
              <div className="settings-section__actions">
                <Button
                  size="sm"
                  variant="primary"
                  disabled={busy !== null}
                  onClick={() => void download()}
                >
                  {busy === "download" ? "Opening…" : "Download the new version"}
                </Button>
              </div>
              <p className="muted">
                It opens GitHub in your browser. Download the new version there, then install it the
                way you installed this one.
              </p>
            </>
          )}
          {confirm && (
            <div className="notice-box" role="note">
              <p>
                <strong>Work is running.</strong> Installing stops it, the same way Quit does, and
                records how it ended. Plenipo then installs the update and opens again.
              </p>
              <div className="settings-section__actions">
                <Button size="sm" variant="primary" onClick={() => void install(true)}>
                  Stop the work and install
                </Button>
                <Button size="sm" onClick={() => setConfirm(null)}>
                  Not now
                </Button>
              </div>
            </div>
          )}
        </section>
      )}
      {s.message && s.state !== "failed" && <p className="muted">{s.message}</p>}
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <h3>How updates work</h3>
      <ul className="settings">
        <li>
          Plenipo checks GitHub for a new version a few minutes after it starts, then once a day,
          for Free and Pro alike. The check sends nothing about you or your work.
        </li>
        {byHand && systemWords().system === "mac" ? (
          <li>
            Move Plenipo to your Applications folder, and it updates itself. Until then, Plenipo
            tells you when a new version is ready, and you download it and install it by hand. Your
            Ledger and settings are kept.
          </li>
        ) : byHand ? (
          <li>
            Plenipo tells you when a new version is ready, and never changes itself: you download it
            and install it the way you installed this one. Your Ledger and settings are kept.
          </li>
        ) : (
          <li>
            Nothing is downloaded or installed until you choose <strong>Install now</strong>.
            Plenipo {replacesItself ? "puts in place" : "installs"} only a version signed by 8 West,
            and backs up the Ledger first.
          </li>
        )}
        <li>
          To go back to an older version, {s.how === "installer" ? "run its installer" : "get it"}{" "}
          from {s.releasesPage}. Your Ledger and settings are kept; Diagnostics can restore the
          backup made before an update.
        </li>
      </ul>
    </div>
  );
}
