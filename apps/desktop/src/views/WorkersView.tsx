import { useEffect, useMemo, useState, type FormEvent } from "react";
import type { AgentRuntimeInfo, AgentSession, AgentTurn } from "@plenipo/types";

import { toCommandError } from "../api/commands";
import {
  describeActivity,
  describeUsage,
  notReadyHint,
  OUTCOME_LABEL,
  outcomeTone,
  runtimeStatus,
} from "../agents/format";
import { activityItems, isRunning } from "../agents/store";
import { useAgents } from "../agents/useAgents";
import { formatTime } from "../runtime/format";

const MAX_OBJECTIVE = 10_000;

function runtimeLabel(runtimes: AgentRuntimeInfo[], id: string): string {
  return runtimes.find((r) => r.id === id)?.label ?? id;
}

function SessionBadge({ session }: { session: AgentSession }) {
  if (isRunning(session)) return <span className="badge badge--task-running">Running</span>;
  if (session.state === "closed") return <span className="badge">Closed</span>;
  return <span className="badge badge--task-succeeded">Open</span>;
}

export function WorkersView({
  selectedSessionId,
  onSelectSession,
  onShowExecution,
  onOpenRuntimes,
}: {
  selectedSessionId: string | null;
  onSelectSession: (id: string | null) => void;
  onShowExecution: (executionId: string) => void;
  onOpenRuntimes: () => void;
}) {
  const { state, start, resume, cancel, close, loadSession, refresh } = useAgents();
  const [runtimeId, setRuntimeId] = useState<string | null>(null);
  const [objective, setObjective] = useState("");
  const [model, setModel] = useState("");
  const [followUp, setFollowUp] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Default to the first ready runtime once runtimes are known.
  const chosen =
    state.runtimes.find((r) => r.id === runtimeId) ??
    state.runtimes.find((r) => r.ready) ??
    state.runtimes[0];
  const hint = chosen ? notReadyHint(chosen) : null;

  const session = selectedSessionId ? state.sessions[selectedSessionId] : undefined;
  const turns = useMemo(
    () => (selectedSessionId ? (state.turns[selectedSessionId] ?? []) : []),
    [selectedSessionId, state.turns],
  );
  // Fetch the full history whenever a session is selected: live updates alone only carry the
  // turns that changed while this view was open.
  useEffect(() => {
    if (selectedSessionId) loadSession(selectedSessionId).catch(() => undefined);
  }, [selectedSessionId, loadSession]);

  async function run(key: string, action: () => Promise<void>) {
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

  function submitNew(e: FormEvent) {
    e.preventDefault();
    if (!chosen) return;
    void run("start", async () => {
      const id = await start(chosen.id, objective, model);
      setObjective("");
      onSelectSession(id);
    });
  }

  function submitFollowUp(e: FormEvent) {
    e.preventDefault();
    if (!session) return;
    void run("resume", async () => {
      await resume(session.id, followUp);
      setFollowUp("");
    });
  }

  const running = isRunning(session);

  return (
    <section className="view" aria-labelledby="workers-title">
      <h1 id="workers-title">Workers</h1>
      <p className="view__lead">
        Give an objective to an AI worker. It runs on your own signed-in Claude Code or Codex
        command-line tool, supervised by Plenipo, and every turn is recorded in the Ledger. In this
        phase workers cannot change files or use the network: Claude Code has no tools, and Codex
        runs in its read-only sandbox.
      </p>

      {state.status === "error" && (
        <p className="status status--error" role="alert">
          Could not load workers: {state.error}
        </p>
      )}
      {state.notices.length > 0 && (
        <ul className="notices" aria-label="Worker notices">
          {state.notices.map((n) => (
            <li key={n} className="muted">
              {n}
            </li>
          ))}
        </ul>
      )}

      <form className="panel" aria-label="New task" onSubmit={submitNew}>
        <h2>New task</h2>
        <fieldset className="choices">
          <legend>Runtime</legend>
          {state.runtimes.map((r) => {
            const status = runtimeStatus(r);
            return (
              <label key={r.id} className="choice">
                <input
                  type="radio"
                  name="runtime"
                  value={r.id}
                  checked={chosen?.id === r.id}
                  onChange={() => setRuntimeId(r.id)}
                />
                <span className="choice__label">{r.label}</span>
                <span className={`pill pill--${status.tone}`}>{status.text}</span>
              </label>
            );
          })}
          {state.runtimes.length === 0 && <p className="muted">Loading runtimes…</p>}
        </fieldset>

        <label className="field">
          <span>Objective</span>
          <textarea
            value={objective}
            maxLength={MAX_OBJECTIVE}
            rows={3}
            placeholder="What should the worker do?"
            onChange={(e) => setObjective(e.target.value)}
          />
        </label>
        <details className="advanced">
          <summary>Advanced</summary>
          <label className="field">
            <span>Model (optional)</span>
            <input
              value={model}
              maxLength={64}
              placeholder="Runtime default"
              onChange={(e) => setModel(e.target.value)}
            />
          </label>
        </details>

        {hint && chosen && (
          <p className="hint" role="note">
            <strong>{chosen.label} is not ready.</strong> {hint}{" "}
            <button type="button" className="link" onClick={onOpenRuntimes}>
              Open Runtimes
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
        <div className="actions">
          <button
            type="submit"
            className="button"
            disabled={pending !== null || !chosen?.ready || objective.trim() === ""}
          >
            {pending === "start" ? "Starting…" : "Start task"}
          </button>
        </div>
      </form>

      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}

      <div className="split">
        <div className="split__list">
          <h2>Sessions</h2>
          {state.order.length === 0 ? (
            <p className="muted">No sessions yet. Start a task above.</p>
          ) : (
            <ul className="executions" aria-label="Sessions">
              {state.order.map((id) => {
                const s = state.sessions[id];
                if (!s) return null;
                return (
                  <li key={id}>
                    <button
                      type="button"
                      className="execution"
                      aria-current={id === selectedSessionId ? "true" : undefined}
                      onClick={() => onSelectSession(id)}
                    >
                      <span className="execution__label">{s.title}</span>
                      <SessionBadge session={s} />
                      <span className="execution__meta">
                        {runtimeLabel(state.runtimes, s.runtimeId)} · {s.turnCount} turn
                        {s.turnCount === 1 ? "" : "s"} · {formatTime(s.updatedAt)}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <div className="split__detail">
          {session ? (
            <>
              <div className="detail__header">
                <div>
                  <h2>{session.title}</h2>
                  <div className="card__meta">
                    <SessionBadge session={session} /> ·{" "}
                    {runtimeLabel(state.runtimes, session.runtimeId)}
                    {session.model && <> · model {session.model}</>}
                  </div>
                  <div className="card__meta">
                    {session.providerSessionConfirmed && session.providerSessionId
                      ? `Provider session ${session.providerSessionId}`
                      : "Provider session not started yet"}
                  </div>
                </div>
                <div className="actions">
                  {running && (
                    <button
                      type="button"
                      className="button button--danger"
                      disabled={pending !== null}
                      onClick={() => void run("cancel", () => cancel(session.id))}
                    >
                      {pending === "cancel" ? "Cancelling…" : "Cancel turn"}
                    </button>
                  )}
                  {!running && session.state === "open" && (
                    <button
                      type="button"
                      className="button button--small button--quiet"
                      disabled={pending !== null}
                      onClick={() => void run("close", () => close(session.id))}
                    >
                      Close session
                    </button>
                  )}
                </div>
              </div>

              <ol className="turns" aria-label="Turns">
                {turns.map((t) => (
                  <TurnCard
                    key={t.taskId}
                    turn={t}
                    activity={state.activity[t.taskId] ?? []}
                    onShowExecution={onShowExecution}
                  />
                ))}
              </ol>

              {session.state === "open" ? (
                <form className="followup" aria-label="Continue session" onSubmit={submitFollowUp}>
                  <label className="field">
                    <span>Continue this session</span>
                    <textarea
                      value={followUp}
                      maxLength={MAX_OBJECTIVE}
                      rows={2}
                      placeholder="Follow-up objective"
                      onChange={(e) => setFollowUp(e.target.value)}
                    />
                  </label>
                  <button
                    type="submit"
                    className="button"
                    disabled={pending !== null || running || followUp.trim() === ""}
                  >
                    {pending === "resume" ? "Sending…" : "Send"}
                  </button>
                </form>
              ) : (
                <p className="muted">This session is closed.</p>
              )}
            </>
          ) : (
            <p className="muted">Select a session to see its turns and live activity.</p>
          )}
        </div>
      </div>
    </section>
  );
}

function TurnCard({
  turn,
  activity,
  onShowExecution,
}: {
  turn: AgentTurn;
  activity: Parameters<typeof activityItems>[0];
  onShowExecution: (executionId: string) => void;
}) {
  const items = activityItems(activity);
  const result = turn.result;
  return (
    <li
      className="turn"
      data-running={turn.running ? "true" : undefined}
      data-outcome={result?.outcome}
    >
      <div className="turn__header">
        <span className="turn__number">Turn {turn.number}</span>
        <span className="turn__objective">{turn.objective}</span>
        {turn.running ? (
          <span className="badge badge--task-running">Working…</span>
        ) : (
          result && (
            <span className={`badge badge--task-${outcomeTone(result.outcome)}`}>
              {OUTCOME_LABEL[result.outcome]}
            </span>
          )
        )}
      </div>

      {(turn.running || items.length > 0) && (
        <details className="turn__activity" open={turn.running}>
          <summary>Live activity ({items.length})</summary>
          <ol className="agent-log" aria-label={`Turn ${turn.number} activity`}>
            {items.map((item) => {
              if (item.kind === "streaming") {
                return (
                  <li key={item.key} className="agent-log__item agent-log__item--streaming">
                    <span className="agent-log__label">Agent</span>
                    <span className="agent-log__text">{item.text}</span>
                  </li>
                );
              }
              const d = describeActivity(item.activity.event);
              return (
                <li
                  key={item.key}
                  className={`agent-log__item${d.tone ? ` agent-log__item--${d.tone}` : ""}`}
                  data-type={item.activity.event.type}
                >
                  <span className="agent-log__label">{d.label}</span>
                  <span className="agent-log__text">{d.text}</span>
                </li>
              );
            })}
            {turn.running && items.length === 0 && (
              <li className="muted">Waiting for the worker…</li>
            )}
          </ol>
        </details>
      )}

      {result && (
        <div className="turn__result" aria-label={`Turn ${turn.number} result`}>
          {result.outcome === "completed" && result.text ? (
            <div className="turn__text">{result.text}</div>
          ) : (
            <p className={result.outcome === "cancelled" ? "muted" : "status status--error"}>
              {result.summary}
            </p>
          )}
          {result.error && result.outcome !== "completed" && result.error !== result.summary && (
            <pre className="turn__error">{result.error}</pre>
          )}
          <div className="card__meta">
            {result.usage && <>{describeUsage(result.usage)} · </>}
            {result.durationMs !== null && <>{(result.durationMs / 1000).toFixed(1)}s · </>}
            {turn.endedAt !== null && <>finished {formatTime(turn.endedAt)}</>}
            {result.ignoredLines > 0 && <> · {result.ignoredLines} unrecognized line(s) ignored</>}
          </div>
        </div>
      )}
      {turn.executionId && (
        <button
          type="button"
          className="link"
          onClick={() => onShowExecution(turn.executionId as string)}
        >
          Raw output
        </button>
      )}
    </li>
  );
}
