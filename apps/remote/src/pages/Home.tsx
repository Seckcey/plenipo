import { useState } from "react";
import type { HomeView, ObjectiveBrief, PhoneAsk, Recovery } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { recoveryLead, recoveryTitle, type ControlRead } from "../control";
import { useRead, useSession } from "../session-context";
import { STATE_WORDS, ago, describe } from "../words";
import type { Tab } from "./Shell";

function pill(o: ObjectiveBrief) {
  const status =
    o.state === "failed" || o.state === "blocked"
      ? "error"
      : o.state === "awaitingApproval"
        ? "pending"
        : o.state === "succeeded"
          ? "ok"
          : o.state === "running"
            ? "warn"
            : "offline";
  return <StatusPill status={status} label={STATE_WORDS[o.state]} />;
}

/**
 * Work that stopped because Plenipo did (it closed unexpectedly on your PC): **Run again**, or
 * **Leave stopped**. Nothing runs again until you choose.
 */
function Stopped({ recovery, onDone }: { recovery: Recovery; onDone: () => void }) {
  const { ask, org } = useSession();
  const [busy, setBusy] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const act = (key: string, request: PhoneAsk) => {
    setBusy(key);
    setSaid(null);
    ask(request)
      .then((r) => {
        if (r.ok === undefined) setSaid(r.refused?.message ?? r.failed ?? "Your PC did not do it.");
        onDone();
      })
      .catch(() => setSaid("We don't know if your PC got this. Check again when it's back."))
      .finally(() => setBusy(null));
  };
  return (
    <section className="card card--problem" aria-labelledby="stopped-title">
      <h2 id="stopped-title">{recoveryTitle(recovery)}</h2>
      <p className="muted">{recoveryLead(recovery)}</p>
      {recovery.stoppedTasks.length > 0 && (
        <ul className="list">
          {recovery.stoppedTasks.map((t) => (
            <li key={t.taskId} className="stopped__task">
              <strong>{t.objective || "A task"}</strong>
              {t.who && <span className="muted"> · {t.who}</span>}
              <div className="actions">
                {t.runAgainAs !== null ? (
                  <span className="muted">Started again</span>
                ) : t.canRunAgain ? (
                  <Button
                    size="sm"
                    icon="play"
                    disabled={busy !== null}
                    onClick={() => act(t.taskId, { kind: "runAgain", org, task: t.taskId })}
                  >
                    {busy === t.taskId ? "Starting…" : "Run again"}
                  </Button>
                ) : (
                  <span className="muted">Give it again from its page on your PC</span>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
      <div className="actions">
        <Button
          size="sm"
          disabled={busy !== null}
          onClick={() => act("leave", { kind: "leaveStopped", org, notice: recovery.id })}
        >
          {recovery.stoppedTasks.length > 0 ? "Leave stopped" : "OK"}
        </Button>
      </div>
      {said && (
        <p className="form-error" role="alert">
          {said}
        </p>
      )}
    </section>
  );
}

/** Home: what needs you, what is going, what is stuck, and what just finished. */
export function HomePage({
  go,
  control,
  reloadControl,
}: {
  go: (tab: Tab) => void;
  control: ControlRead | null;
  reloadControl: () => void;
}) {
  const { org } = useSession();
  // A PC on 1.19.0 answers with its first organization's notice alone, in another shape: the
  // page (always the newest) then shows none, and offers nothing that PC cannot do.
  const recovery = Array.isArray(control?.recovery)
    ? (control.recovery.find((r) => r.org === org)?.recovery ?? null)
    : null;
  const { data, error, reload } = useRead<HomeView>({ kind: "readHome", org }, [
    "approvals",
    "tasks",
    "home",
  ]);
  if (error) {
    return (
      <section className="page">
        <h1>Home</h1>
        <p className="form-error" role="alert">
          {error}
        </p>
        <Button onClick={reload}>Try again</Button>
      </section>
    );
  }
  if (!data) return <p role="status">Loading Home…</p>;
  const waiting = data.current.reduce((n, o) => n + o.waitingApprovals, 0);
  return (
    <section className="page" aria-labelledby="home-title">
      <h1 id="home-title">Home</h1>
      {recovery && <Stopped recovery={recovery} onDone={reloadControl} />}
      {waiting > 0 && (
        <button type="button" className="card card--attention" onClick={() => go("approvals")}>
          <strong>
            Waiting for you: {waiting} {waiting === 1 ? "approval" : "approvals"}
          </strong>
          <span>Open Approvals</span>
        </button>
      )}
      <h2>Current objectives</h2>
      {data.current.length === 0 ? (
        <p className="muted">Nothing is going right now.</p>
      ) : (
        <ul className="list">
          {data.current.map((o) => (
            <li key={o.rootTaskId} className="card">
              <div className="card__head">
                <strong>{o.objective}</strong>
                {pill(o)}
              </div>
              <p className="muted">
                {o.positionTitle ?? "Your organization"} · started {ago(o.createdAt)} · {o.tasks}{" "}
                {o.tasks === 1 ? "task" : "tasks"}
                {o.failed > 0 ? ` · ${o.failed} didn't finish` : ""}
              </p>
            </li>
          ))}
        </ul>
      )}
      {data.stuck.length > 0 && (
        <>
          <h2>What&rsquo;s stuck</h2>
          <ul className="list">
            {data.stuck.map((s) => (
              <li key={s.event.id} className="card card--problem">
                <strong>{s.task?.objective ?? describe(s.event)}</strong>
                <p className="muted">
                  {describe(s.event)} · {ago(s.event.createdAt)}
                </p>
              </li>
            ))}
          </ul>
        </>
      )}
      <h2>Just finished</h2>
      {data.finished.length === 0 ? (
        <p className="muted">Nothing finished in the last week.</p>
      ) : (
        <ul className="list">
          {data.finished.map((o) => (
            <li key={o.rootTaskId} className="card">
              <div className="card__head">
                <strong>{o.objective}</strong>
                {pill(o)}
              </div>
              {o.answer && <p className="card__answer">{o.answer}</p>}
              <p className="muted">{o.completedAt ? ago(o.completedAt) : ""}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
