import { useId } from "react";
import type { GettingStarted } from "@plenipo/types";
import { Button, Icon, IconButton } from "@plenipo/ui";

import { GETTING_STARTED } from "./rewardsWords";
import { gettingStartedShows } from "./useGettingStarted";

/**
 * **Getting started**, at the top of the People tab and for you only (Phase 24, ADR-169 §5,
 * ADR-172 §4): **Fill in your profile**, **Find someone**, and **Send a message**. A step that is
 * done has a check mark and the word "Done" (for a screen reader); a step that is not has a button
 * that takes you to it. It is gone when all three are done, or when you close it.
 */
export function GettingStartedPanel({
  state,
  error,
  onProfile,
  onFind,
  onMessage,
  onClose,
}: {
  state: GettingStarted | null;
  /** What went wrong when closing it. */
  error: string | null;
  /** Go to Settings → Community, where your profile is. */
  onProfile: () => void;
  /** Put the cursor in the **Find someone** box. */
  onFind: () => void;
  /** Open Messages. */
  onMessage: () => void;
  onClose: () => void;
}) {
  const titleId = useId();
  if (!gettingStartedShows(state)) return null;
  const steps = [
    { label: "Fill in your profile", done: state.profile, go: onProfile },
    { label: "Find someone", done: state.foundSomeone, go: onFind },
    { label: "Send a message", done: state.sentAMessage, go: onMessage },
  ];
  return (
    <section className="getting-started" aria-labelledby={titleId}>
      <div className="getting-started__head">
        <h2 id={titleId}>{GETTING_STARTED}</h2>
        <IconButton icon="close" label={`Close ${GETTING_STARTED}`} onClick={onClose} />
      </div>
      <p className="muted">Three things to try. Each one is ticked when you have done it.</p>
      <ol className="getting-started__steps">
        {steps.map((step) => (
          <li
            key={step.label}
            className={`getting-started__step${step.done ? " getting-started__step--done" : ""}`}
          >
            <span className="getting-started__mark" aria-hidden="true">
              {step.done && <Icon name="check" size={14} />}
            </span>
            {step.done ? (
              <>
                <span className="visually-hidden">Done</span> <span>{step.label}</span>
              </>
            ) : (
              <>
                <span className="visually-hidden">Not done yet</span>{" "}
                <Button size="sm" onClick={step.go}>
                  {step.label}
                </Button>
              </>
            )}
          </li>
        ))}
      </ol>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
