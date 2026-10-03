import { useState, type ReactNode } from "react";
import type { CommunityView } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import {
  cancelCommunitySignIn,
  checkCommunityAgain,
  setCommunitySwitch,
  toCommandError,
} from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { systemWords } from "../system/words";
import { oneLine } from "./safeText";
import { LeaveCommunityDialog } from "./LeaveCommunity";
import { useCommunity } from "./useCommunity";

const NOT_REACHED = "Community can't be reached right now. Nothing was changed.";

/** What the switch says under its name, for where Community stands (ADR-170 §3). */
function hintFor(view: CommunityView, checkAgain: ReactNode): ReactNode {
  switch (view.stage) {
    case "off":
      return "Off to start with. On: find other Plenipo owners, message them, and work together, through your 8 West account. Plenipo asks 8 West whether Community is open only when you press this.";
    case "comingSoon":
      return (
        <>
          {"Coming soon: 8 West hasn't opened Community yet."} {checkAgain}
        </>
      );
    case "updateNeeded":
      return "Update Plenipo to use Community (Settings → Updates).";
    case "unreachable":
      return view.problem ?? NOT_REACHED;
    case "signingIn":
      return "Signing in: finish in Settings → Community.";
    case "joining":
      return "Almost there: finish joining in Settings → Community.";
    case "signedIn":
      return `On. Signed in as ${oneLine(view.accountName ?? "")}.`;
    case "signedOut":
      return `On, but ${systemWords().thisComputer} is signed out. Sign in from Settings → Community.`;
    case "closed":
      return `Community is closed for now. Everything on ${systemWords().thisComputer} is kept.`;
  }
}

/**
 * Settings → Switches → **Community** (Phase 24, ADR-162, ADR-170): off to begin with, and
 * "Coming soon" until 8 West opens it. Turning it on asks 8 West whether Community is open, and
 * shows a code to sign in with; nothing is asked before you press it. Turning it off after you
 * joined is Leave Community, and asks first.
 */
export function CommunitySwitch() {
  const { view, error: loadError, show } = useCommunity();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [leaving, setLeaving] = useState(false);
  if (!view) {
    return (
      <p className={loadError ? "form-error" : "muted"}>{loadError ?? "Loading Community…"}</p>
    );
  }
  const run = (work: () => Promise<CommunityView>) => {
    setPending(true);
    setError(null);
    work()
      .then(show)
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setPending(false));
  };
  const checked = view.switchedOn || view.stage === "signingIn" || view.stage === "joining";
  // What went wrong, said once: the last thing you pressed, or what Community remembers.
  const problem = error ?? (view.stage === "unreachable" ? null : view.problem);
  return (
    <>
      <Toggle
        label="Community"
        hint={hintFor(
          view,
          <Button size="sm" disabled={pending} onClick={() => run(checkCommunityAgain)}>
            Check again
          </Button>,
        )}
        checked={checked}
        disabled={pending}
        onChange={(on) => {
          if (on) run(() => setCommunitySwitch(true));
          else if (view.stage === "signingIn") run(cancelCommunitySignIn);
          else if (view.switchedOn) setLeaving(true);
          else run(() => setCommunitySwitch(false));
        }}
      />
      {problem && (
        <p className="form-error" role="alert">
          {problem}
        </p>
      )}
      {leaving && (
        <LeaveCommunityDialog
          onCancel={() => setLeaving(false)}
          onLeft={(next) => {
            setLeaving(false);
            setError(null);
            show(next);
          }}
        />
      )}
    </>
  );
}
