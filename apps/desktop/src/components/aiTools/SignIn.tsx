import { useEffect, useRef } from "react";
import type { AccountAction, AgentRuntimeInfo, AuthState } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { AUTH_LABEL, INSTALL_LABEL } from "../../agents/format";
import { useAgents } from "../../agents/useAgents";
import { useTerminalIfAny } from "../../terminal/useTerminal";
import { firstSentence, tasksUsing, usingWords } from "./words";

/** Signed in, with anything (a subscription, an API key, a cloud): Reconnect and Sign out. */
const SIGNED_IN: ReadonlySet<AuthState> = new Set([
  "subscription",
  "unverified",
  "apiKey",
  "thirdPartyCloud",
]);

const ACTION_WORDS: Record<AccountAction, string> = { signIn: "sign-in", signOut: "sign-out" };

/**
 * Sign in, Reconnect, and Sign out (ADR-058): each opens a tab in the terminal panel that runs the
 * AI tool's own command. You sign in there yourself; Plenipo never types into it and never sees
 * it. While a task is using the tool, the tab waits until it is free (or you press Cancel). The
 * wait is kept with the terminal panel, so the tab still opens after you leave the card or page.
 */
export function SignIn({ info, checking }: { info: AgentRuntimeInfo; checking: boolean }) {
  const terminal = useTerminalIfAny();
  const { state } = useAgents();
  const busy = tasksUsing(state.sessions, info.id);
  const wait = terminal?.aiToolWaits[info.id] ?? null;
  const waiting = wait !== null && !wait.stopped;
  const stopped = wait?.stopped === true ? wait : null;
  const shown = waiting ? "waiting" : stopped ? "stopped" : null;

  // Pressed while the tool is busy: the keyboard goes to Cancel. When that line goes (the tab
  // opened, or Cancel, or Plenipo stopped trying), the keyboard that was in it goes to the card's
  // first button (or to Try again), not to the top of the window.
  const block = useRef<HTMLDivElement>(null);
  const focusCancel = useRef(false);
  const inLine = useRef(false);
  const shownBefore = useRef(shown);
  useEffect(() => {
    const before = shownBefore.current;
    shownBefore.current = shown;
    const el = block.current;
    if (!el || shown === before) return;
    if (shown === "waiting" && focusCancel.current) {
      focusCancel.current = false;
      el.querySelector<HTMLButtonElement>(".ai-tool__waiting button")?.focus();
      return;
    }
    const lost = document.activeElement === null || document.activeElement === document.body;
    if (!inLine.current || !lost) return;
    inLine.current = false;
    el.querySelector<HTMLButtonElement>(
      shown === "stopped"
        ? ".ai-tool__waiting button:not(:disabled)"
        : ".ai-tool__buttons button:not(:disabled)",
    )?.focus();
  }, [shown]);
  const lineFocus = {
    onFocus: () => {
      inLine.current = true;
    },
    onBlur: () => {
      inLine.current = false;
    },
  };

  const press = (action: AccountAction) => {
    if (!terminal) return;
    if (busy > 0) {
      focusCancel.current = true;
      terminal.waitForAiTool(info.id, info.label, action);
    } else {
      terminal.openAiTool(info.id, info.label, action);
    }
  };

  const install = info.installation.state;
  const installed = install === "installed";
  const signedIn = SIGNED_IN.has(info.auth.state);
  const { signIn, signOut } = info.account;
  const canSignOut = signOut !== null && (signedIn || info.auth.state === "unknown");
  const off = !installed || checking || waiting;
  const where = terminal?.panel.side === "right" ? "on the right" : "at the bottom";

  return (
    <div className="ai-tool__block" ref={block}>
      <div>
        {checking
          ? "Checking…"
          : `${AUTH_LABEL[info.auth.state]}${info.auth.method ? ` · ${info.auth.method}` : ""}`}
      </div>
      {/* Why the check will not let a task run, with what to do (GitHub Copilot's paid extra
          use, ADR-083; a key where a subscription is needed). */}
      {!checking &&
        !info.ready &&
        info.installation.state === "installed" &&
        !info.installation.detail &&
        info.auth.detail && <p className="muted ai-tool__why">{info.auth.detail}</p>}
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
          {wait && waiting && (
            <p className="ai-tool__waiting" role="status" {...lineFocus}>
              Waiting:{" "}
              {busy > 0
                ? `${usingWords(busy, info.label)}.`
                : (wait.refused ?? `the task using ${info.label} is finishing.`)}{" "}
              The {ACTION_WORDS[wait.action]} tab opens when {info.label} is free.{" "}
              <Button
                size="sm"
                variant="quiet"
                aria-label={`Cancel ${info.label}'s ${ACTION_WORDS[wait.action]}`}
                onClick={() => terminal?.cancelAiToolWait(info.id)}
              >
                Cancel
              </Button>
            </p>
          )}
          {stopped && (
            <p className="ai-tool__waiting" role="status" {...lineFocus}>
              The {ACTION_WORDS[stopped.action]} tab didn&apos;t open:{" "}
              {firstSentence(stopped.refused ?? "")}. Plenipo stopped trying by itself.{" "}
              <Button
                size="sm"
                disabled={off}
                aria-label={`Try ${info.label}'s ${ACTION_WORDS[stopped.action]} again`}
                onClick={() => press(stopped.action)}
              >
                Try again
              </Button>{" "}
              <Button
                size="sm"
                variant="quiet"
                aria-label={`Cancel ${info.label}'s ${ACTION_WORDS[stopped.action]}`}
                onClick={() => terminal?.cancelAiToolWait(info.id)}
              >
                Cancel
              </Button>
            </p>
          )}
          {install === "checking" ? null : !installed ? (
            <p className="muted">
              {install === "notInstalled"
                ? `${info.label} is not installed, so you can't sign in to it here.`
                : `${INSTALL_LABEL[install]}: you can't sign in to ${info.label} here now.`}{" "}
              {[install === "notInstalled" ? null : info.installation.detail, info.installHint]
                .filter(Boolean)
                .join(" ")}
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
