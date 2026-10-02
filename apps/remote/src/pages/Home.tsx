import type { HomeView, ObjectiveBrief } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

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

/** Home: what needs you, what is going, what is stuck, and what just finished. */
export function HomePage({ go }: { go: (tab: Tab) => void }) {
  const { org } = useSession();
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
