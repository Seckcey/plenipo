import { useId, useState, type FormEvent } from "react";
import { Button, StatusPill } from "@plenipo/ui";

import { toCommandError } from "../api/commands";
import { heldNote, notReadyHint, runtimeStatus } from "../agents/format";
import { useAgents } from "../agents/useAgents";
import { ENTER_SENDS, enterSends } from "../components/enterSends";
import { ModelPicker } from "../components/models/ModelPicker";
import { PILL_TONE } from "../components/tones";
import { useRoutingOnce } from "../routing/useRouting";

const MAX_OBJECTIVE = 10_000;

/**
 * A conversation outside your organization (the Workers page's "Other conversations"): any AI
 * tool you have, directly, without a position or a team. It cannot change files or use the
 * internet, and with handoffs allowed it may ask a worker on another AI tool for help through
 * Plenipo Liaison.
 */
export function StartConversation({
  onStarted,
  onOpenRuntimes,
}: {
  /** The new conversation, to show its chat. */
  onStarted: (sessionId: string) => void;
  /** Opens the AI tools page, at that AI tool's card. */
  onOpenRuntimes: (runtimeId?: string) => void;
}) {
  const { state, start, refresh } = useAgents();
  const [runtimeId, setRuntimeId] = useState<string | null>(null);
  const [objective, setObjective] = useState("");
  const [model, setModel] = useState("");
  const [handoffs, setHandoffs] = useState(false);
  const routing = useRoutingOnce();
  const keys = useId();
  const [pending, setPending] = useState<"start" | "refresh" | null>(null);
  const [error, setError] = useState<string | null>(null);

  // The first ready AI tool, until you pick one.
  const chosen =
    state.runtimes.find((r) => r.id === runtimeId) ??
    state.runtimes.find((r) => r.ready) ??
    state.runtimes[0];
  const hint = chosen ? notReadyHint(chosen) : null;

  async function run(key: "start" | "refresh", action: () => Promise<void>) {
    setPending(key);
    setError(null);
    try {
      await action();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(null);
    }
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    if (!chosen) return;
    void run("start", async () => {
      const id = await start(chosen.id, objective, model, handoffs);
      setObjective("");
      onStarted(id);
    });
  }

  return (
    <form className="panel workers-start" aria-label="Start a conversation" onSubmit={submit}>
      <h2>Start a conversation outside your organization</h2>
      <p className="muted">
        Talk to one of your AI tools directly, without a position or a team. It runs on your own
        signed-in AI tool, watched over by Plenipo, and every step is recorded in the Ledger. It
        cannot change files or use the internet: Claude Code and Grok get none of their own tools,
        Kimi runs in its read-only mode, Codex runs read-only, and Ollama, Antigravity, and GitHub
        Copilot only answer in text.
      </p>
      <fieldset className="choices">
        <legend>AI tool</legend>
        {state.runtimes.map((r) => {
          const status = runtimeStatus(r);
          return (
            <label key={r.id} className="choice">
              <input
                type="radio"
                name="runtime"
                value={r.id}
                checked={chosen?.id === r.id}
                onChange={() => {
                  setRuntimeId(r.id);
                  setModel("");
                }}
              />
              <span className="choice__label">{r.label}</span>
              <StatusPill status={PILL_TONE[status.tone]} label={status.text} />
            </label>
          );
        })}
        {state.runtimes.length === 0 && <p className="muted">Loading AI tools…</p>}
      </fieldset>

      <label className="field">
        <span>What should it do?</span>
        <textarea
          value={objective}
          maxLength={MAX_OBJECTIVE}
          rows={3}
          aria-describedby={keys}
          onChange={(e) => setObjective(e.target.value)}
          onKeyDown={enterSends}
        />
      </label>
      <span id={keys} className="visually-hidden">
        {ENTER_SENDS}
      </span>
      <label className="check">
        <input type="checkbox" checked={handoffs} onChange={(e) => setHandoffs(e.target.checked)} />
        <span>
          Allow handoffs to other workers
          <span className="check__hint">
            It may ask a worker on another AI tool (for example Codex asking Claude Code for a
            review) through Plenipo Liaison. Handoffs are limited in depth and number, use only your
            signed-in AI tools, and are all recorded in the Ledger.
          </span>
        </span>
      </label>
      <details className="advanced">
        <summary>Advanced</summary>
        {chosen && (
          <ModelPicker routing={routing} runtimeId={chosen.id} value={model} onChange={setModel} />
        )}
      </details>

      {chosen && heldNote(chosen) && (
        <p className="hint" role="status">
          {heldNote(chosen)}
        </p>
      )}
      {hint && chosen && (
        <p className="hint" role="note">
          <strong>{chosen.label} is not ready.</strong> {hint}{" "}
          <button type="button" className="link" onClick={() => onOpenRuntimes(chosen.id)}>
            Open AI tools
          </button>{" "}
          <button
            type="button"
            className="link"
            disabled={pending !== null}
            onClick={() => void run("refresh", refresh)}
          >
            Re-check
          </button>
        </p>
      )}
      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      <div className="actions">
        <Button
          type="submit"
          variant="primary"
          disabled={pending !== null || !chosen?.ready || objective.trim() === ""}
        >
          {pending === "start" ? "Starting…" : "Start"}
        </Button>
      </div>
    </form>
  );
}
