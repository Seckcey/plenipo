import { useState } from "react";
import type { NoticeSettings } from "@plenipo/types";
import { Button, ErrorState, LoadingState } from "@plenipo/ui";

import {
  getNoticeSettings,
  sendTestNotice,
  setNoticeSettings,
  toCommandError,
} from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { useLive } from "../pages/useLive";

type Kind = Exclude<keyof NoticeSettings, "onlyWhenAway">;

/** Each kind of notice, in plain words. */
const KINDS: readonly { key: Kind; label: string; hint: string }[] = [
  {
    key: "approvals",
    label: "Waiting for your OK",
    hint: "A worker asks before doing something that needs you.",
  },
  {
    key: "checks",
    label: "A check for you to solve",
    hint: "A website asks whether a person is using it, and a worker hands the check to you.",
  },
  {
    key: "problems",
    label: "Problems",
    hint: "An objective failed, a server's ID changed, or an AI tool signed out or reached its usage limit.",
  },
  {
    key: "finished",
    label: "Finished work",
    hint: "An objective's result is ready.",
  },
  {
    key: "lessons",
    label: "Lessons",
    hint: "A worker learned something for you to keep or discard.",
  },
  {
    key: "plenipo",
    label: "Plenipo itself",
    hint: "Plenipo closed unexpectedly or Windows closed it, or a new version is ready.",
  },
  {
    key: "spending",
    label: "Paid AI spending",
    hint: "80% of a spending cap is used, or a cap stopped paid AI work.",
  },
];

/**
 * Settings → Notifications (Phase 12): which pop-up notices Windows shows, and whether only
 * while Plenipo's window is not in front. Several that arrive together become one notice, and
 * the same one is not repeated within a minute.
 */
export function NotificationSettings() {
  const live = useLive<NoticeSettings>(
    "notices",
    () => getNoticeSettings(),
    (e) => e.eventType === "org.settings_changed",
  );
  // What the owner just chose, until the Ledger's answer arrives; then what the Ledger kept,
  // until the choices are read again (so a quick second change builds on the first).
  const [pending, setPending] = useState<NoticeSettings | null>(null);
  const [kept, setKept] = useState<{ value: NoticeSettings; over: NoticeSettings | null } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [test, setTest] = useState<{
    state: "idle" | "sending" | "sent" | "failed";
    words: string;
  }>({ state: "idle", words: "" });

  if (live.status === "loading") return <LoadingState label="Loading your notice choices" />;
  if (live.status === "error" || !live.value) {
    return (
      <ErrorState
        title="Couldn't load your notice choices"
        message={live.error}
        onRetry={live.reload}
      />
    );
  }
  const shown = pending ?? (kept && kept.over === live.value ? kept.value : live.value);

  const choose = async (next: NoticeSettings) => {
    setPending(next);
    setError(null);
    const over = live.value;
    try {
      setKept({ value: await setNoticeSettings(next), over });
      live.reload();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(null);
    }
  };

  const sendTest = async () => {
    setTest({ state: "sending", words: "" });
    try {
      await sendTestNotice();
      setTest({
        state: "sent",
        words:
          "Sent. If no notice appeared, check that Windows allows Plenipo's notifications (Windows Settings → System → Notifications).",
      });
    } catch (reason) {
      setTest({ state: "failed", words: toCommandError(reason).message });
    }
  };

  return (
    <div className="settings-notices">
      <section aria-labelledby="notices-kinds">
        <h3 id="notices-kinds">Tell me when</h3>
        {KINDS.map((k) => (
          <Toggle
            key={k.key}
            label={k.label}
            hint={k.hint}
            checked={shown[k.key]}
            disabled={pending !== null}
            onChange={(on) => void choose({ ...shown, [k.key]: on })}
          />
        ))}
      </section>
      <section aria-labelledby="notices-when">
        <h3 id="notices-when">When Plenipo is open</h3>
        <Toggle
          label="Only while Plenipo's window is not in front"
          hint="On: no pop-up while you are looking at Plenipo (Home and the bell show what needs you). Off: a notice pops up either way."
          checked={shown.onlyWhenAway}
          disabled={pending !== null}
          onChange={(on) => void choose({ ...shown, onlyWhenAway: on })}
        />
      </section>
      {error && (
        <p className="status status--error" role="alert">
          Couldn't keep your choice: {error}
        </p>
      )}
      <div className="settings-notices__test">
        <Button icon="bell" onClick={() => void sendTest()} disabled={test.state === "sending"}>
          {test.state === "sending" ? "Sending…" : "Send a test notice"}
        </Button>
        {test.words && (
          <p
            className={test.state === "failed" ? "status status--error" : "muted"}
            role={test.state === "failed" ? "alert" : "status"}
          >
            {test.words}
          </p>
        )}
      </div>
    </div>
  );
}
