import { useCallback, useEffect, useState } from "react";
import { Button, LoadingState } from "@plenipo/ui";

import type { Keep, Kept } from "./keep";
import type { SocketMaker } from "./line/relay";
import { PasskeyProblem } from "./lock/passkey";
import { PairPage } from "./pages/Pair";
import { Shell } from "./pages/Shell";
import { SessionProvider } from "./session";
import { useSession } from "./session-context";

/** Sign in with the phone's passkey: its face, fingerprint, or passcode check (ADR-142). */
function SignIn() {
  const { kept, signIn, status } = useSession();
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const said = status.kind === "signIn" ? status.problem : undefined;
  return (
    <section className="page sign-in" aria-labelledby="sign-in-title">
      <h1 id="sign-in-title">Sign in</h1>
      <p className="lead">
        Sign in to <strong>{kept.paired.pcName}</strong> with your face, fingerprint, or phone
        passcode. You stay signed in for 30 minutes after your last tap.
      </p>
      <Button
        variant="primary"
        icon="lock"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setProblem(null);
          signIn()
            .catch((e: unknown) =>
              setProblem(
                e instanceof PasskeyProblem ? e.message : "Your PC could not check that it is you.",
              ),
            )
            .finally(() => setBusy(false));
        }}
      >
        Check it&rsquo;s you
      </Button>
      {(problem ?? said) && (
        <p className="form-error" role="alert">
          {problem ?? said}
        </p>
      )}
    </section>
  );
}

function Connected({ onForget }: { onForget: () => void }) {
  const { status, connect, kept } = useSession();
  switch (status.kind) {
    case "connecting":
      return <LoadingState label={`Connecting to ${kept.paired.pcName}`} />;
    case "notListed":
      return (
        <section className="page" aria-labelledby="gone-title">
          <h1 id="gone-title">This phone is no longer on your PC&rsquo;s list</h1>
          <p>
            It was removed on your PC, or it was not used for 90 days. Add it again: on your PC,
            Settings → Devices → Add a phone.
          </p>
          <div className="actions">
            <Button variant="primary" onClick={onForget}>
              Pair this phone again
            </Button>
            <Button onClick={connect}>Try again</Button>
          </div>
        </section>
      );
    case "signIn":
      return <SignIn />;
    case "offline":
    case "ready":
      return <Shell />;
  }
}

/**
 * Plenipo on your phone (Phase 14): pair this phone with your PC once, then sign in and use it.
 * The page is built from Plenipo's repository with each release, and loads nothing from anywhere
 * else (ADR-146).
 */
export function App({ keep, make }: { keep: Keep; make?: SocketMaker }) {
  const [kept, setKept] = useState<Kept | null | undefined>(undefined);
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    keep
      .load()
      .then(setKept)
      .catch(() => {
        setProblem(
          "This browser would not let Plenipo keep anything. Use Safari or Chrome, not a private window.",
        );
        setKept(null);
      });
  }, [keep]);
  const forget = useCallback(() => {
    void keep.forget().finally(() => setKept(null));
  }, [keep]);
  if (kept === undefined) return <LoadingState label="Starting" />;
  if (kept === null) {
    return (
      <>
        {problem && (
          <p className="form-error" role="alert">
            {problem}
          </p>
        )}
        <PairPage keep={keep} onPaired={setKept} {...(make ? { make } : {})} />
      </>
    );
  }
  return (
    <SessionProvider
      kept={kept}
      keep={keep}
      onForgotten={() => setKept(null)}
      {...(make ? { make } : {})}
    >
      <Connected onForget={forget} />
    </SessionProvider>
  );
}
