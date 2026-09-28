import { useEffect, useState } from "react";
import type { AccountAction, AgentRuntimeInfo, AgentSession, AuthState } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { AUTH_LABEL } from "../../agents/format";
import { useAgents } from "../../agents/useAgents";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { tasksUsing, usingWords } from "./words";

/** A moment after the AI tool looks free: Plenipo may still be finishing the task's step. */
const FREE_MS = 500;
/** A tab Plenipo refused tries again this often while nothing changes (or when a task does). */
const RETRY_MS = 5000;

/** Signed in, with anything (a subscription, an API key, a cloud): Reconnect and Sign out. */
const SIGNED_IN: ReadonlySet<AuthState> = new Set([
  "subscription",
  "unverified",
  "apiKey",
  "thirdPartyCloud",
]);

/** A sign-in or sign-out tab waiting for the AI tool to be free (ADR-058 §5). */
interface Waiting {
  action: AccountAction;
  /** Plenipo would not open it (a task was using the tool): its words. */
  refused: string | null;
  /** The conversations when it was refused: it tries again soon after they change. */
  sessions: Readonly<Record<string, AgentSession>> | null;
}

const ACTION_WORDS: Record<AccountAction, string> = { signIn: "sign-in", signOut: "sign-out" };

/**
 * Sign in, Reconnect, and Sign out (ADR-058): each opens a tab in the terminal panel that runs the
 * AI tool's own command. You sign in there yourself; Plenipo never types into it and never sees
 * it. While a task is using the tool, the tab waits until it is free (or you press Cancel).
 */
export function SignIn({ info, checking }: { info: AgentRuntimeInfo; checking: boolean }) {
  const terminal = useTerminalIfAny();
  const { state } = useAgents();
  const sessions = state.sessions;
  const busy = tasksUsing(sessions, info.id);
  const [waiting, setWaiting] = useState<Waiting | null>(null);
  // A tab refused after this card was shown makes it wait, then try again.
  const refusal = terminal?.signInRefused[info.id];
  const [seen, setSeen] = useState(() => refusal?.at ?? 0);
  if (refusal && refusal.at > seen) {
    setSeen(refusal.at);
    setWaiting({ action: refusal.action, refused: refusal.message, sessions });
  }

  const openAiTool = terminal?.openAiTool;
  useEffect(() => {
    if (!waiting || busy > 0 || !openAiTool) return;
    const changed = waiting.sessions !== null && waiting.sessions !== sessions;
    const timer = setTimeout(
      () => {
        setWaiting(null);
        openAiTool(info.id, info.label, waiting.action);
      },
      waiting.refused === null || changed ? FREE_MS : RETRY_MS,
    );
    return () => clearTimeout(timer);
  }, [waiting, busy, sessions, openAiTool, info.id, info.label]);

  const press = (action: AccountAction) => {
    if (!openAiTool) return;
    if (busy > 0) setWaiting({ action, refused: null, sessions: null });
    else openAiTool(info.id, info.label, action);
  };

  const installed = info.installation.state === "installed";
  const signedIn = SIGNED_IN.has(info.auth.state);
  const { signIn, signOut } = info.account;
  const canSignOut = signOut !== null && (signedIn || info.auth.state === "unknown");
  const off = !installed || checking || waiting !== null;
  const where = terminal?.panel.side === "right" ? "on the right" : "at the bottom";

  return (
    <div className="ai-tool__block">
      <div>
        {checking
          ? "Checking…"
          : `${AUTH_LABEL[info.auth.state]}${info.auth.method ? ` · ${info.auth.method}` : ""}`}
      </div>
      {terminal ? (
        <>
          <div className="ai-tool__buttons">
            {signIn !== null &&
              (signedIn ? (
                <Button
                  size="sm"
                  disabled={off}
                  aria-label={`Reconnect ${info.label}`}
                  onClick={() => press("signIn")}
                >
                  Reconnect
                </Button>
              ) : (
                <Button
                  size="sm"
                  variant="primary"
                  disabled={off}
                  aria-label={`Sign in to ${info.label}`}
                  onClick={() => press("signIn")}
                >
                  Sign in
                </Button>
              ))}
            {canSignOut && (
              <Button
                size="sm"
                disabled={off}
                aria-label={`Sign out of ${info.label}`}
                onClick={() => press("signOut")}
              >
                Sign out
              </Button>
            )}
          </div>
          {waiting && (
            <p className="ai-tool__waiting" role="status">
              Waiting:{" "}
              {busy > 0
                ? `${usingWords(busy, info.label)}.`
                : (waiting.refused ?? `the task using ${info.label} is finishing.`)}{" "}
              The {ACTION_WORDS[waiting.action]} tab opens when {info.label} is free.{" "}
              <Button
                size="sm"
                variant="quiet"
                aria-label={`Cancel ${info.label}'s ${ACTION_WORDS[waiting.action]}`}
                onClick={() => setWaiting(null)}
              >
                Cancel
              </Button>
            </p>
          )}
          {!installed ? (
            <p className="muted">
              {info.label} is not installed, so you can&apos;t sign in to it here.{" "}
              {info.installHint}
            </p>
          ) : (
            signIn !== null && (
              <p className="muted">
                {signedIn && signOut !== null ? (
                  <>
                    Reconnect opens a tab {where} that runs <code>{signIn}</code>, and Sign out one
                    that runs <code>{signOut}</code>.
                  </>
                ) : (
                  <>
                    Opens a tab {where} that runs <code>{signIn}</code>.
                  </>
                )}{" "}
                You sign in there, and in your browser; Plenipo never sees it. When it ends, Plenipo
                checks {info.label} again by itself.
              </p>
            )
          )}
        </>
      ) : (
        <p className="muted">{info.loginHint}</p>
      )}
      {signOut === null && <p className="muted">{info.label} has no sign-out command.</p>}
    </div>
  );
}
