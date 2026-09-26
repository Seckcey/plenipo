import { useEffect, useMemo, useState, type FormEvent } from "react";
import type {
  AgentActivity,
  AgentRuntimeInfo,
  AgentSession,
  AgentTurn,
  TurnStep,
} from "@plenipo/types";

import { toCommandError } from "../api/commands";
import {
  describeActivity,
  describeUsage,
  notReadyHint,
  OUTCOME_LABEL,
  outcomeTone,
  runtimeStatus,
} from "../agents/format";
import {
  activityItems,
  isRunning,
  isWaiting,
  liaisonInfo,
  stepOf,
  type LiaisonSessionInfo,
} from "../agents/store";
import { useAgents } from "../agents/useAgents";
import { openHandoffs, useLiaisonRevision, useTaskHandoffs } from "../agents/useTaskHandoffs";
import { HandoffCard, ReceivedHandoff } from "../components/Handoffs";
import { formatTime } from "../runtime/format";

const MAX_OBJECTIVE = 10_000;

function runtimeLabel(runtimes: AgentRuntimeInfo[], id: string): string {
  return runtimes.find((r) => r.id === id)?.label ?? id;
}

function SessionBadge({ session }: { session: AgentSession }) {
  if (isRunning(session)) return <span className="badge badge--task-running">Running</span>;
  if (isWaiting(session)) return <span className="badge badge--task-blocked">Waiting</span>;
  if (session.state === "closed") return <span className="badge">Closed</span>;
  return <span className="badge badge--task-succeeded">Open</span>;
}

/** Selection helpers shared by the turn cards. */
interface Navigation {
  runtimes: AgentRuntimeInfo[];
  canOpen: (sessionId: string) => boolean;
  onOpenSession: (sessionId: string) => void;
  onShowExecution: (executionId: string) => void;
  onOpenPosition?: ((positionId: string) => void) | undefined;
}

export function WorkersView({
  selectedSessionId,
  onSelectSession,
  onShowExecution,
  onOpenRuntimes,
  onOpenPosition,
}: {
  selectedSessionId: string | null;
  onSelectSession: (id: string | null) => void;
  onShowExecution: (executionId: string) => void;
  onOpenRuntimes: () => void;
  /** Show an organization position (for sessions that work for one). */
  onOpenPosition?: (positionId: string) => void;
}) {
  const { state, start, resume, cancel, close, loadSession, refresh } = useAgents();
  const [runtimeId, setRuntimeId] = useState<string | null>(null);
  const [objective, setObjective] = useState("");
  const [model, setModel] = useState("");
  const [handoffs, setHandoffs] = useState(false);
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
  const info = liaisonInfo(session);
  const liaisonRevision = useLiaisonRevision();
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
      const id = await start(chosen.id, objective, model, handoffs);
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
  const waiting = isWaiting(session);
  const nav: Navigation = {
    runtimes: state.runtimes,
    canOpen: (id) => id in state.sessions,
    onOpenSession: onSelectSession,
    onShowExecution,
    onOpenPosition,
  };

  return (
    <section className="view" aria-labelledby="workers-title">
      <h1 id="workers-title">Workers</h1>
      <p className="view__lead">
        Give an objective to an AI worker. It runs on your own signed-in Claude Code or Codex,
        watched over by Plenipo, and every step is recorded in the Ledger. For now workers cannot
        change files or use the internet: Claude Code gets no tools, and Codex runs read-only. With
        handoffs allowed, a worker can ask a worker on another AI tool for help through Plenipo
        Liaison.
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
                  onChange={() => setRuntimeId(r.id)}
                />
                <span className="choice__label">{r.label}</span>
                <span className={`pill pill--${status.tone}`}>{status.text}</span>
              </label>
            );
          })}
          {state.runtimes.length === 0 && <p className="muted">Loading AI tools…</p>}
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
        <label className="check">
          <input
            type="checkbox"
            checked={handoffs}
            onChange={(e) => setHandoffs(e.target.checked)}
          />
          <span>
            Allow handoffs to other workers
            <span className="check__hint">
              The worker may ask a worker on another AI tool — for example Codex asking Claude Code
              for a review — through Plenipo Liaison. Handoffs are limited in depth and number, use
              only your signed-in AI tools, and are all recorded in the Ledger.
            </span>
          </span>
        </label>
        <details className="advanced">
          <summary>Advanced</summary>
          <label className="field">
            <span>Model (optional)</span>
            <input
              value={model}
              maxLength={64}
              placeholder="The AI tool's default"
              onChange={(e) => setModel(e.target.value)}
            />
          </label>
        </details>

        {hint && chosen && (
          <p className="hint" role="note">
            <strong>{chosen.label} is not ready.</strong> {hint}{" "}
            <button type="button" className="link" onClick={onOpenRuntimes}>
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
                      <span className="execution__label">
                        {liaisonInfo(s).origin === "handoff" && <span aria-hidden="true">↳ </span>}
                        {s.title}
                      </span>
                      <SessionBadge session={s} />
                      <span className="execution__meta">
                        {runtimeLabel(state.runtimes, s.runtimeId)} · {s.turnCount} turn
                        {s.turnCount === 1 ? "" : "s"} · {formatTime(s.updatedAt)}
                        {liaisonInfo(s).origin === "handoff" && " · handoff worker"}
                        {liaisonInfo(s).origin === "member" && " · organization member"}
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
                  <LiaisonLine info={info} nav={nav} />
                </div>
                <div className="actions">
                  {(running || waiting) && (
                    <button
                      type="button"
                      className="button button--danger"
                      disabled={pending !== null}
                      onClick={() => void run("cancel", () => cancel(session.id))}
                    >
                      {pending === "cancel" ? "Cancelling…" : "Cancel turn"}
                    </button>
                  )}
                  {!running && !waiting && session.state === "open" && (
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

              {waiting && (
                <p className="hint" role="status">
                  This turn is waiting for replies to its handoffs and continues by itself when they
                  are in. Cancelling it also stops the handoffs it is waiting for.
                </p>
              )}

              <ol className="turns" aria-label="Turns">
                {turns.map((t) => (
                  <TurnCard
                    key={t.taskId}
                    turn={t}
                    activity={state.activity[t.taskId] ?? []}
                    liaison={info.enabled}
                    liaisonRevision={liaisonRevision}
                    nav={nav}
                  />
                ))}
              </ol>

              {info.origin === "handoff" ? (
                <p className="muted">
                  This worker was started by Plenipo Liaison for another worker&apos;s request; it
                  takes work only through Liaison.
                </p>
              ) : info.origin === "member" ? (
                <p className="muted">
                  This agent holds a position in the organization. Give it objectives from the
                  Organization view, where its team and oversight are set.
                </p>
              ) : session.state === "open" ? (
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
                    disabled={pending !== null || running || waiting || followUp.trim() === ""}
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

/** Where a session stands with Liaison, for its header. */
function LiaisonLine({ info, nav }: { info: LiaisonSessionInfo; nav: Navigation }) {
  const position = info.positionId;
  const openPosition = position && nav.onOpenPosition && (
    <>
      {" "}
      ·{" "}
      <button type="button" className="link" onClick={() => nav.onOpenPosition?.(position)}>
        Open in Organization
      </button>
    </>
  );
  if (info.origin === "handoff") {
    const parent = info.parentSessionId;
    return (
      <div className="card__meta">
        <span className="pill">Handoff worker</span> Started by Plenipo Liaison
        {info.depth !== null && <> · depth {info.depth}</>}
        {parent && nav.canOpen(parent) && (
          <>
            {" "}
            ·{" "}
            <button type="button" className="link" onClick={() => nav.onOpenSession(parent)}>
              Open requester session
            </button>
          </>
        )}
        {openPosition}
      </div>
    );
  }
  if (info.origin === "member") {
    return (
      <div className="card__meta">
        <span className="pill pill--ok">Organization member</span> Hands work to its team through
        Liaison
        {openPosition}
      </div>
    );
  }
  if (info.enabled) {
    return (
      <div className="card__meta">
        <span className="pill pill--ok">Handoffs allowed</span> Each objective starts a new workflow
      </div>
    );
  }
  return null;
}

function ActivityLog({
  label,
  items,
  running,
}: {
  label: string;
  items: ReturnType<typeof activityItems>;
  running: boolean;
}) {
  return (
    <ol className="agent-log" aria-label={label}>
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
      {running && items.length === 0 && <li className="muted">Waiting for the worker…</li>}
    </ol>
  );
}

/** The steps a turn ran, from its record and its live activity. */
function stepsOf(turn: AgentTurn, activity: AgentActivity[]): TurnStep[] {
  const steps = [...turn.steps];
  for (const a of activity) {
    const n = stepOf(a.seq);
    if (!steps.some((s) => s.number === n)) {
      steps.push({
        number: n,
        executionId: null,
        running: turn.running,
        result: null,
        startedAt: null,
        endedAt: null,
      });
    }
  }
  return steps.sort((a, b) => a.number - b.number);
}

function TurnCard({
  turn,
  activity,
  liaison,
  liaisonRevision,
  nav,
}: {
  turn: AgentTurn;
  activity: AgentActivity[];
  /** The session uses Liaison: show the turn's handoffs. */
  liaison: boolean;
  liaisonRevision: number;
  nav: Navigation;
}) {
  const handoffs = useTaskHandoffs(
    liaison ? turn.taskId : null,
    `${turn.steps.length}:${turn.waiting}:${turn.endedAt}`,
    liaisonRevision,
    turn.result !== null,
  );

  const result = turn.result;
  const sent = handoffs?.sent ?? [];
  const stepped = turn.steps.length > 1 || turn.waiting || sent.length > 0;
  const open = openHandoffs(handoffs);
  const received = handoffs?.received;
  return (
    <li
      className="turn"
      data-running={turn.running ? "true" : undefined}
      data-waiting={turn.waiting ? "true" : undefined}
      data-outcome={result?.outcome}
    >
      <div className="turn__header">
        <span className="turn__number">Turn {turn.number}</span>
        <span className="turn__objective">{turn.objective}</span>
        {turn.running ? (
          <span className="badge badge--task-running">Working…</span>
        ) : turn.waiting ? (
          <span className="badge badge--task-blocked">Waiting for replies</span>
        ) : (
          result && (
            <span className={`badge badge--task-${outcomeTone(result.outcome)}`}>
              {OUTCOME_LABEL[result.outcome]}
            </span>
          )
        )}
      </div>

      {received && (
        <ReceivedHandoff
          view={received}
          requesterLabel={runtimeLabel(nav.runtimes, received.requesterRuntimeId ?? "")}
        />
      )}

      {stepped ? (
        <ol className="steps" aria-label={`Turn ${turn.number} steps`}>
          {stepsOf(turn, activity).map((step) => {
            const items = activityItems(activity.filter((a) => stepOf(a.seq) === step.number));
            const asked = sent.filter((h) => h.step === step.number);
            const status = step.running
              ? "working…"
              : step.result
                ? OUTCOME_LABEL[step.result.outcome]
                : "";
            return (
              <li key={step.number} className="step">
                <details className="turn__activity" open={step.running}>
                  <summary>
                    Step {step.number}
                    {step.number > 1 && " · continued with handoff replies"}
                    {status && <> · {status}</>} ({items.length} event
                    {items.length === 1 ? "" : "s"})
                  </summary>
                  <ActivityLog
                    label={`Turn ${turn.number} step ${step.number} activity`}
                    items={items}
                    running={step.running}
                  />
                  {step.result && asked.length > 0 && (
                    <div className="turn__text">{step.result.text ?? step.result.summary}</div>
                  )}
                </details>
                {asked.length > 0 && (
                  <ul className="handoffs" aria-label={`Step ${step.number} handoffs`}>
                    {asked.map((h) => (
                      <HandoffCard
                        key={h.messageId}
                        view={h}
                        canOpen={nav.canOpen}
                        onOpenSession={nav.onOpenSession}
                      />
                    ))}
                  </ul>
                )}
              </li>
            );
          })}
          {turn.waiting && (
            <li className="muted">
              Waiting for {open} handoff repl{open === 1 ? "y" : "ies"}…
            </li>
          )}
        </ol>
      ) : (
        (turn.running || activity.length > 0) && (
          <details className="turn__activity" open={turn.running}>
            <summary>Live activity ({activityItems(activity).length})</summary>
            <ActivityLog
              label={`Turn ${turn.number} activity`}
              items={activityItems(activity)}
              running={turn.running}
            />
          </details>
        )
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
          onClick={() => nav.onShowExecution(turn.executionId as string)}
        >
          Raw output
        </button>
      )}
    </li>
  );
}
