/**
 * When a plan runs out (Phase 25, item 4.2; ADR-203): a notice for each AI tool at its usage
 * limit, with the work that waits for it and your choices, each a button. Waiting is the
 * default: Plenipo picks the work back up by itself once the limit is over. **Use a reset**
 * opens the company's own page; Plenipo never uses a reset or buys anything for you.
 */
import { useState } from "react";
import type { LimitWait } from "@plenipo/types";
import { Banner, Button } from "@plenipo/ui";

import {
  getLimitWaits,
  leaveWorkStopped,
  openResetPage,
  pickUpWorkNow,
  toCommandError,
} from "../api/commands";
import type { Go } from "../components/views";
import { useLive } from "../pages/useLive";
import { useNow } from "../runtime/useNow";
import { useShown } from "../upkeep/useShown";
import { limitTitle, otherToolWords, resetWords, waitWords, waitingLead } from "./words";

/** Ledger events after which the waiting work may have changed. */
function relevant(eventType: string): boolean {
  return (
    eventType === "agent.result" ||
    eventType.startsWith("router.") ||
    eventType.startsWith("work.") ||
    eventType === "plenipo.run_again"
  );
}

/** One notice: this limit, until a new one. */
const keyOf = (w: LimitWait) => `${w.runtimeId}:${w.since}`;

export function LimitBanners({ go }: { go: Go }) {
  const live = useLive<LimitWait[]>(
    "limits",
    () => getLimitWaits(),
    (e) => relevant(e.eventType),
  );
  const [current, setShown] = useShown(live);
  // The notices you chose to wait on (shown again for a new limit).
  const [waited, setWaited] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<{ runtimeId: string; message: string } | null>(null);
  const now = useNow(60_000);
  const waits = (current ?? []).filter((w) => !waited.includes(keyOf(w)));
  if (waits.length === 0) return null;

  const act = async (w: LimitWait, key: string, work: () => Promise<LimitWait[] | void>) => {
    setBusy(`${w.runtimeId}:${key}`);
    setError(null);
    try {
      const answer = await work();
      if (answer) setShown(answer);
      live.reload();
    } catch (reason) {
      setError({ runtimeId: w.runtimeId, message: toCommandError(reason).message });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      {waits.map((w) => {
        const reset = resetWords(w);
        const doing = (key: string) => busy === `${w.runtimeId}:${key}`;
        return (
          <Banner
            key={w.runtimeId}
            tone="warn"
            role="status"
            className="banner--limit"
            label={`${w.label}'s usage limit`}
            title={limitTitle(w, now)}
          >
            <div className="muted">{waitingLead(w)}</div>
            <ul className="banner__list limit__work">
              {w.work.map((t) => (
                <li key={t.taskId}>
                  <button
                    type="button"
                    className="ui-link"
                    onClick={() => go({ view: "task", id: t.taskId })}
                  >
                    {t.objective}
                  </button>
                  {t.who && <span className="muted"> · {t.who}</span>}
                </li>
              ))}
            </ul>
            <p className="limit__lead">You can:</p>
            <ul className="limit__choices" aria-label={`What you can do about ${w.label}`}>
              <li className="limit__choice">
                <Button size="sm" onClick={() => setWaited((k) => [...k, keyOf(w)])}>
                  Wait
                </Button>
                <span>{waitWords(w, now)}</span>
              </li>
              {reset && (
                <li className="limit__choice">
                  <Button
                    size="sm"
                    disabled={busy !== null}
                    onClick={() => void act(w, "reset", () => openResetPage(w.runtimeId))}
                  >
                    Use a reset
                  </Button>
                  <span>{reset}</span>
                  <Button
                    size="sm"
                    icon="play"
                    disabled={busy !== null}
                    onClick={() => void act(w, "now", () => pickUpWorkNow(w.runtimeId))}
                  >
                    {doing("now") ? "Picking it up…" : "Pick it up now"}
                  </Button>
                </li>
              )}
              <li className="limit__choice">
                <Button size="sm" onClick={() => go({ view: "settings", id: "aiModels" })}>
                  Use another AI tool
                </Button>
                <span>{otherToolWords(w)}</span>
              </li>
              <li className="limit__choice">
                <Button
                  size="sm"
                  disabled={busy !== null}
                  onClick={() =>
                    void act(w, "leave", () => leaveWorkStopped(w.work.map((t) => t.taskId)))
                  }
                >
                  Leave stopped
                </Button>
                <span>Plenipo won't pick this work back up. You can give it again yourself.</span>
              </li>
            </ul>
            {error?.runtimeId === w.runtimeId && (
              <p className="status status--error" role="alert">
                {error.message}
              </p>
            )}
          </Banner>
        );
      })}
    </>
  );
}
