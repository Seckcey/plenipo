import { useState } from "react";
import type { ApprovalQueue, ApprovalView } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { useRead, useSession } from "../session-context";
import { ago, timeLeft } from "../words";

/** One organization's approvals, as the PC sends them. */
interface OrgApprovals {
  org: string;
  name: string;
  queue: ApprovalQueue;
  /** Waiting approvals you keep on your PC only. */
  keptOnPc: string[];
}

/** One waiting approval: what, exactly, why, and Approve or Refuse (or "Approve on your PC"). */
function Card({
  org,
  approval: a,
  keptOnPc,
  onAnswered,
}: {
  org: string;
  approval: ApprovalView;
  keptOnPc: boolean;
  onAnswered: () => void;
}) {
  const { ask } = useSession();
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const answer = (approve: boolean) => {
    setBusy(true);
    setSaid(null);
    ask({ kind: approve ? "approve" : "refuse", org, approval: a.id })
      .then((r) => {
        if (r.ok) {
          setSaid(approve ? "Approved." : "Refused.");
          onAnswered();
        } else {
          setSaid(r.refused?.message ?? r.failed ?? "Your PC did not take that answer.");
        }
      })
      .catch(() => setSaid("We don't know if your PC got this. Check again when it's back."))
      .finally(() => setBusy(false));
  };
  const left = timeLeft(a.expiresAt === null ? null : Number(a.expiresAt));
  return (
    <li className="card approval">
      <div className="card__head">
        <strong>{a.summary}</strong>
        {left && <StatusPill status="warn" label={left} />}
      </div>
      <p className="muted">
        {a.worker} · {a.capabilityLabel}
        {a.project ? ` · ${a.project}` : ""}
      </p>
      {a.sensitiveLabel && <StatusPill status="error" label={a.sensitiveLabel} />}
      {a.detail && <pre className="approval__detail">{a.detail}</pre>}
      {a.reason && <p>{a.reason}</p>}
      {a.server && (
        <p className="muted">
          Server: {a.server}
          {a.environment === "production" ? " (Production)" : ""}
        </p>
      )}
      {keptOnPc ? (
        <p className="notice-box" role="note">
          <strong>Approve on your PC.</strong> You keep this kind of approval on your PC.
        </p>
      ) : (
        <div className="actions">
          <Button variant="primary" icon="check" disabled={busy} onClick={() => answer(true)}>
            Approve
          </Button>
          <Button disabled={busy} onClick={() => answer(false)}>
            Refuse
          </Button>
        </div>
      )}
      {said && (
        <p role="status" className="approval__said">
          {said}
        </p>
      )}
    </li>
  );
}

/**
 * Approvals: every organization's, waiting first. Approving and refusing go to your PC, where
 * Guard decides, the first answer counts (on the PC or here), and Activity records which phone
 * answered.
 */
export function ApprovalsPage() {
  const { data, error, reload } = useRead<OrgApprovals[]>({ kind: "readApprovals" }, ["approvals"]);
  if (error) {
    return (
      <section className="page">
        <h1>Approvals</h1>
        <p className="form-error" role="alert">
          {error}
        </p>
        <Button onClick={reload}>Try again</Button>
      </section>
    );
  }
  if (!data) return <p role="status">Loading Approvals…</p>;
  const several = data.length > 1;
  const waiting = data.reduce((n, o) => n + o.queue.pending.length, 0);
  return (
    <section className="page" aria-labelledby="approvals-title">
      <h1 id="approvals-title">Approvals</h1>
      {waiting === 0 && <p className="muted">Nothing is waiting for you.</p>}
      {data.map((o) =>
        o.queue.pending.length === 0 ? null : (
          <div key={o.org}>
            {several && <h2>{o.name}</h2>}
            <ul className="list">
              {o.queue.pending.map((a) => (
                <Card
                  key={a.id}
                  org={o.org}
                  approval={a}
                  keptOnPc={o.keptOnPc.includes(a.id)}
                  onAnswered={reload}
                />
              ))}
            </ul>
          </div>
        ),
      )}
      <h2>Answered lately</h2>
      <ul className="list">
        {data
          .flatMap((o) => o.queue.recent.map((a) => ({ org: o, a })))
          .slice(0, 20)
          .map(({ org, a }) => (
            <li key={`${org.org}:${a.id}`} className="card card--quiet">
              <strong>{a.summary}</strong>
              <p className="muted">
                {a.note ?? a.status} {a.resolvedAt ? `· ${ago(Number(a.resolvedAt))}` : ""}
                {several ? ` · ${org.name}` : ""}
              </p>
            </li>
          ))}
      </ul>
    </section>
  );
}
