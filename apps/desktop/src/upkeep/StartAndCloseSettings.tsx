import { useState } from "react";
import type { CloseWindow, StartAndClose } from "@plenipo/types";
import { ErrorState, LoadingState } from "@plenipo/ui";

import { getStartAndClose, setStartAndClose, toCommandError } from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { useLive } from "../pages/useLive";
import { systemWords } from "../system/words";
import { useShown } from "./useShown";

/** The choices for closing the window, in plain words ("the tray": the system's own, ADR-155). */
const closeChoices = (): readonly { value: CloseWindow; label: string; hint: string }[] => [
  {
    value: "keepWhileWorking",
    label: `Keep Plenipo in ${systemWords().waitsIn} while work is going`,
    hint: "With work going, the window hides and the work goes on. With nothing going, Plenipo quits.",
  },
  {
    value: "alwaysKeep",
    label: `Always keep Plenipo in ${systemWords().waitsIn}`,
    hint: `Closing only hides the window. Quit Plenipo from ${systemWords().waitsInMenu}.`,
  },
  {
    value: "quit",
    label: "Quit Plenipo and stop its work",
    hint: "Closing the window is the same as Quit: work stops cleanly and is recorded as stopped.",
  },
];

/**
 * Settings → Start and close (ADR-037, background work): Start with Windows (off until you
 * turn it on), and what closing the window does. Each system says it its own way (ADR-155).
 */
export function StartAndCloseSettings() {
  const live = useLive<StartAndClose>(
    "startAndClose",
    () => getStartAndClose(),
    (e) => e.eventType === "org.settings_changed",
  );
  const [current, setShown] = useShown(live);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (live.status === "loading") return <LoadingState label="Loading Start and close" />;
  const s = current;
  if (!s) {
    return (
      <ErrorState
        title="Couldn't load Start and close"
        message={live.error}
        onRetry={live.reload}
      />
    );
  }

  const change = async (next: { startWithWindows: boolean; closeWindow: CloseWindow }) => {
    setPending(true);
    setError(null);
    try {
      setShown(await setStartAndClose(next));
      live.reload();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  };

  const words = systemWords();
  return (
    <div className="settings-section__body settings-start">
      <section aria-labelledby="start-sign-in">
        <h3 id="start-sign-in">{words.whenYouSignIn}</h3>
        <Toggle
          label={words.startAtSignIn}
          hint={
            s.canStartWithWindows
              ? words.startAtSignInHint
              : `${words.startingAtSignIn} is not available on this computer.`
          }
          checked={s.startWithWindows}
          disabled={pending || !s.canStartWithWindows}
          onChange={(on) => void change({ startWithWindows: on, closeWindow: s.closeWindow })}
        />
      </section>
      <section aria-labelledby="start-close">
        <h3 id="start-close">When you close the window</h3>
        <fieldset className="fieldset" disabled={pending}>
          <legend className="visually-hidden">What closing the window does</legend>
          {closeChoices().map((c) => (
            <label key={c.value} className="check">
              <input
                type="radio"
                name="close-window"
                checked={s.closeWindow === c.value}
                onChange={() =>
                  void change({ startWithWindows: s.startWithWindows, closeWindow: c.value })
                }
              />
              <span>
                {c.label}
                <span className="muted"> — {c.hint}</span>
              </span>
            </label>
          ))}
        </fieldset>
        <p className="muted">
          Opening Plenipo again shows the one that is running: there is only ever one. If the window
          stops responding, Plenipo reopens it; your work keeps running.
        </p>
      </section>
      {error && (
        <p className="status status--error" role="alert">
          Couldn&apos;t keep your choice: {error}
        </p>
      )}
    </div>
  );
}
