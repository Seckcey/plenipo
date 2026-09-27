import { useState } from "react";
import { Button, LoadingState, Panel, StatusPill } from "@plenipo/ui";

import { toCommandError } from "../api/commands";
import { AUTH_LABEL, INSTALL_LABEL, notReadyHint, runtimeStatus } from "../agents/format";
import { useAgents } from "../agents/useAgents";
import { formatTime } from "../runtime/format";
import { PILL_TONE } from "./tones";

/** Each AI tool (Claude Code, Codex, Grok, Kimi, Ollama): whether it is installed and signed in, and what it can do. */
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
        <h2>Your AI tools</h2>
        <Button variant="primary" size="sm" disabled={checking} onClick={() => void recheck()}>
          {checking ? "Checking…" : "Re-check"}
        </Button>
      </div>
      <p className="muted">
        Plenipo uses the AI tools already signed in on this computer: Claude Code, Codex, Grok,
        Kimi, and Ollama&apos;s cloud models. It never asks for passwords or API keys and never
        falls back to pay-per-use API billing.
      </p>
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <ul className="profiles" aria-label="AI tools">
        {state.runtimes.map((r) => {
          const status = runtimeStatus(r);
          const hint = notReadyHint(r);
          return (
            <li key={r.id} aria-label={`${r.label} AI tool`}>
              <Panel
                id={`ai-tool-${r.id}`}
                title={r.label}
                actions={<StatusPill status={PILL_TONE[status.tone]} label={status.text} />}
              >
                <div className="card__meta">{r.providerLabel}</div>
                <dl className="kv">
                  <dt>Installation</dt>
                  <dd>
                    {INSTALL_LABEL[r.installation.state]}
                    {r.installation.version && ` · v${r.installation.version}`}
                  </dd>
                  {r.installation.executable && (
                    <>
                      <dt>Program file</dt>
                      <dd className="path">{r.installation.executable}</dd>
                    </>
                  )}
                  <dt>Sign-in</dt>
                  <dd>
                    {AUTH_LABEL[r.auth.state]}
                    {r.auth.method && ` · ${r.auth.method}`}
                  </dd>
                  <dt>What it can do</dt>
                  <dd>
                    {[
                      r.capabilities.streamingText && "live text",
                      r.capabilities.resume && "continue conversations",
                      r.capabilities.cancel && "cancel",
                      r.capabilities.structuredResults && "clear results",
                      r.capabilities.billingCheckedPerTurn && "billing checked on every task",
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
              </Panel>
            </li>
          );
        })}
        {state.runtimes.length === 0 && (
          <li>
            <LoadingState />
          </li>
        )}
      </ul>
    </>
  );
}
