import { useEffect, useState } from "react";

import { toCommandError } from "../api/commands";
import { OutputPanel } from "../components/OutputPanel";
import { StateBadge } from "../components/StateBadge";
import { formatDuration, formatTime, outcomeText } from "../runtime/format";
import { isActive } from "../runtime/store";
import { useNow } from "../runtime/useNow";
import { useRuntime } from "../runtime/useRuntime";

export function RuntimesView({
  selectedId,
  onSelect,
}: {
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  const { state, start, cancel, loadOutput } = useRuntime();
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const now = useNow(1000);

  const selected = selectedId ? state.executions[selectedId] : undefined;
  const hasOutput = selectedId ? selectedId in state.outputs : true;

  useEffect(() => {
    if (selectedId && !hasOutput) void loadOutput(selectedId);
  }, [selectedId, hasOutput, loadOutput]);

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

  return (
    <section className="view" aria-labelledby="runtimes-title">
      <h1 id="runtimes-title">Runtimes</h1>
      <p className="view__lead">
        Plenipo launches only approved profiles, isolates each process tree, and streams its output
        here. Phase 1 includes built-in diagnostic profiles only.
      </p>

      {error && (
        <p className="status status--error" role="alert">
          {error}
        </p>
      )}
      {state.status === "error" && (
        <p className="status status--error" role="alert">
          Could not load runtimes: {state.error}
        </p>
      )}

      <h2>Launch profiles</h2>
      <ul className="profiles">
        {state.profiles.map((profile) => (
          <li key={profile.id} className="card">
            <div>
              <div className="card__title">{profile.label}</div>
              <div className="card__meta">{profile.description}</div>
              <div className="card__meta">Limit: {profile.maxRuntimeSecs}s</div>
            </div>
            <button
              type="button"
              className="button"
              disabled={pending !== null}
              aria-label={`Start ${profile.label}`}
              onClick={() =>
                void run(`start:${profile.id}`, async () => onSelect(await start(profile.id)))
              }
            >
              Start
            </button>
          </li>
        ))}
        {state.status === "ready" && state.profiles.length === 0 && (
          <li className="card card--empty">No launch profiles are available.</li>
        )}
      </ul>

      <div className="split">
        <div className="split__list">
          <h2>Executions</h2>
          {state.order.length === 0 ? (
            <p className="muted">Nothing has run yet.</p>
          ) : (
            <ul className="executions" aria-label="Executions">
              {state.order.map((id) => {
                const record = state.executions[id];
                if (!record) return null;
                return (
                  <li key={id}>
                    <button
                      type="button"
                      className="execution"
                      aria-current={id === selectedId ? "true" : undefined}
                      aria-label={`${record.label} — ${outcomeText(record)}, started ${formatTime(record.startedAt)}`}
                      onClick={() => onSelect(id)}
                    >
                      <span className="execution__label">{record.label}</span>
                      <StateBadge record={record} />
                      <span className="execution__meta">
                        {formatTime(record.startedAt)} · {formatDuration(record, now)}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <div className="split__detail">
          {selected ? (
            <>
              <div className="detail__header">
                <div>
                  <h2>{selected.label}</h2>
                  <div className="card__meta">
                    <StateBadge record={selected} />
                    {selected.pid !== null && <span> · PID {selected.pid}</span>}
                    <span> · {formatDuration(selected, now)}</span>
                  </div>
                  {selected.detail && <div className="card__meta">{selected.detail}</div>}
                </div>
                {isActive(selected) && (
                  <button
                    type="button"
                    className="button button--danger"
                    disabled={pending !== null}
                    onClick={() => void run(`cancel:${selected.id}`, () => cancel(selected.id))}
                  >
                    {pending === `cancel:${selected.id}` ? "Cancelling…" : "Cancel"}
                  </button>
                )}
              </div>
              <OutputPanel record={selected} output={state.outputs[selected.id]} />
            </>
          ) : (
            <p className="muted">Select an execution to see its output.</p>
          )}
        </div>
      </div>
    </section>
  );
}
