import { useState } from "react";

import { toCommandError } from "../api/commands";
import { AUTH_LABEL, INSTALL_LABEL, notReadyHint, runtimeStatus } from "../agents/format";
import { useAgents } from "../agents/useAgents";
import { formatTime } from "../runtime/format";

/** Provider diagnostics: installation, sign-in, and capabilities of each agent runtime. */
export function AgentRuntimeCards() {
  const { state, refresh } = useAgents();
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function recheck() {
    setChecking(true);
    setError(null);
    try {
      await refresh();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setChecking(false);
    }
  }

  return (
    <>
      <div className="section-header">
        <h2>Agent runtimes</h2>
        <button
          type="button"
          className="button button--small"
          disabled={checking}
          onClick={() => void recheck()}
        >
          {checking ? "Checking…" : "Re-check"}
        </button>
      </div>
      <p className="muted">
        Plenipo uses the Claude Code and Codex command-line tools already signed in on this
        computer. It never asks for passwords and never falls back to paid API billing.
      </p>
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <ul className="profiles" aria-label="Agent runtimes">
        {state.runtimes.map((r) => {
          const status = runtimeStatus(r);
          const hint = notReadyHint(r);
          return (
            <li key={r.id} className="card card--stack" aria-label={`${r.label} runtime`}>
              <div className="card__row">
                <div>
                  <div className="card__title">{r.label}</div>
                  <div className="card__meta">{r.providerLabel}</div>
                </div>
                <span className={`pill pill--${status.tone}`}>{status.text}</span>
              </div>
              <dl className="kv">
                <dt>Installation</dt>
                <dd>
                  {INSTALL_LABEL[r.installation.state]}
                  {r.installation.version && ` · v${r.installation.version}`}
                </dd>
                {r.installation.executable && (
                  <>
                    <dt>Executable</dt>
                    <dd className="path">{r.installation.executable}</dd>
                  </>
                )}
                <dt>Sign-in</dt>
                <dd>
                  {AUTH_LABEL[r.auth.state]}
                  {r.auth.method && ` · ${r.auth.method}`}
                </dd>
                <dt>Capabilities</dt>
                <dd>
                  {[
                    r.capabilities.streamingText && "streaming text",
                    r.capabilities.resume && "resume",
                    r.capabilities.cancel && "cancel",
                    r.capabilities.structuredResults && "structured results",
                    r.capabilities.billingCheckedPerTurn && "billing checked every turn",
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </dd>
                <dt>Permissions</dt>
                <dd>{r.capabilities.toolPosture}</dd>
                {r.checkedAt !== null && (
                  <>
                    <dt>Checked</dt>
                    <dd>{formatTime(r.checkedAt)}</dd>
                  </>
                )}
              </dl>
              {hint && <p className="hint">{hint}</p>}
            </li>
          );
        })}
        {state.runtimes.length === 0 && <li className="card card--empty">Loading…</li>}
      </ul>
    </>
  );
}
