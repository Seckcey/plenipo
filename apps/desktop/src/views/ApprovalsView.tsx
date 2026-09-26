import { useState } from "react";
import type { ApprovalQueue, PermissionsSnapshot } from "@plenipo/types";

import { resolveApproval, revokeGrant } from "../api/commands";
import { ApprovalCard, ApprovalOutcome } from "../components/permissions/ApprovalCard";
import { Refusal } from "../components/models/shared";
import { LEVEL_LABEL } from "../guard/format";
import { useApprovals, usePermissions } from "../guard/usePermissions";
import { useRun } from "../guard/useRun";
import { ago } from "../org/format";
import { useNow } from "../runtime/useNow";

/**
 * Approvals: requests waiting for you (approval cards), workers using permissions now (with
 * Revoke), requests Guard blocked, and your recent answers.
 */
export function ApprovalsView({ onOpenTask }: { onOpenTask?: (taskId: string) => void }) {
  const approvals = useApprovals();
  const permissions = usePermissions();
  const now = useNow(1000);
  return (
    <section className="view" aria-labelledby="approvals-title">
      <h1 id="approvals-title">Approvals</h1>
      <p className="view__lead">
        Workers stop here before anything sensitive, or anything not on your approved list, and wait
        for your answer. Nothing happens until you approve it. Settings → Permissions decides what
        needs asking.
      </p>
      <Queue queue={approvals.queue} error={approvals.error} now={now} onApply={approvals.apply} />
      <Grants
        snapshot={permissions.snapshot}
        now={now}
        onApply={permissions.apply}
        onOpenTask={onOpenTask}
      />
      <Blocked snapshot={permissions.snapshot} now={now} onOpenTask={onOpenTask} />
      {approvals.queue && approvals.queue.recent.length > 0 && (
        <section aria-labelledby="answered-title">
          <h2 id="answered-title">Recent answers</h2>
          <ul className="approval-outcomes">
            {approvals.queue.recent.map((a) => (
              <ApprovalOutcome key={a.id} approval={a} now={now} />
            ))}
          </ul>
        </section>
      )}
    </section>
  );
}

function Queue({
  queue,
  error,
  now,
  onApply,
}: {
  queue: ApprovalQueue | null;
  error: string | null;
  now: number;
  onApply: (q: ApprovalQueue) => void;
}) {
  const { pending, error: refusal, run } = useRun(onApply);
  if (!queue) {
    return (
      <p className={error ? "form-error" : "muted"} role={error ? "alert" : undefined}>
        {error ?? "Loading the approval requests…"}
      </p>
    );
  }
  return (
    <section aria-labelledby="waiting-title">
      <h2 id="waiting-title">Waiting for you</h2>
      <Refusal error={refusal} />
      {queue.pending.length === 0 ? (
        <p className="empty">Nothing is waiting for your approval.</p>
      ) : (
        <div className="approvals">
          {queue.pending.map((a) => (
            <ApprovalCard
              key={a.id}
              approval={a}
              now={now}
              pending={pending}
              onAnswer={(approve) => void run(() => resolveApproval(a.id, approve))}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function Grants({
  snapshot,
  now,
  onApply,
  onOpenTask,
}: {
  snapshot: PermissionsSnapshot | null;
  now: number;
  onApply: (s: PermissionsSnapshot) => void;
  onOpenTask?: ((taskId: string) => void) | undefined;
}) {
  const { pending, error, run } = useRun(onApply);
  const [confirming, setConfirming] = useState<string | null>(null);
  if (!snapshot) return null;
  return (
    <section aria-labelledby="grants-title">
      <h2 id="grants-title">Workers using permissions now</h2>
      {!snapshot.tools.running && <p className="form-error">{snapshot.tools.detail}</p>}
      <Refusal error={error} />
      {snapshot.grants.length === 0 ? (
        <p className="empty">No worker is using permissions right now.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th scope="col">Worker</th>
              <th scope="col">Folder</th>
              <th scope="col">Permissions</th>
              <th scope="col">So far</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {snapshot.grants.map((g) => (
              <tr key={g.grantId}>
                <th scope="row">
                  {g.worker}
                  <span className="table__sub">
                    {g.role}
                    {g.project ? ` · ${g.project}` : ""} · since {ago(g.openedAt, now)}
                  </span>
                </th>
                <td>
                  <span className="path">{g.folder ?? "—"}</span>
                </td>
                <td>
                  {g.permissions.map((p) => (
                    <span key={p.capability} className="pill" title={LEVEL_LABEL[p.level]}>
                      {p.label}
                      {p.level === "ask" ? " (asks)" : ""}
                    </span>
                  ))}
                </td>
                <td>
                  {g.used} done · {g.blocked} blocked · {g.asked} asked
                  {g.revoked && <span className="pill pill--bad">Revoked</span>}
                </td>
                <td>
                  {onOpenTask && (
                    <button
                      type="button"
                      className="button button--small button--quiet"
                      onClick={() => onOpenTask(g.taskId)}
                    >
                      Open task
                    </button>
                  )}{" "}
                  {!g.revoked &&
                    (confirming === g.grantId ? (
                      <>
                        <button
                          type="button"
                          className="button button--small button--danger"
                          disabled={pending}
                          onClick={() =>
                            void run(() => revokeGrant(g.grantId)).then(() => setConfirming(null))
                          }
                        >
                          Revoke now
                        </button>{" "}
                        <button
                          type="button"
                          className="button button--small button--quiet"
                          onClick={() => setConfirming(null)}
                        >
                          Keep
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        className="button button--small button--danger"
                        aria-label={`Revoke ${g.worker}'s permissions`}
                        onClick={() => setConfirming(g.grantId)}
                      >
                        Revoke
                      </button>
                    ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="muted">
        Revoke stops the worker&apos;s running programs, refuses its waiting requests, and blocks
        every later request in this step. The worker can still answer in words.
      </p>
    </section>
  );
}

function Blocked({
  snapshot,
  now,
  onOpenTask,
}: {
  snapshot: PermissionsSnapshot | null;
  now: number;
  onOpenTask?: ((taskId: string) => void) | undefined;
}) {
  if (!snapshot) return null;
  return (
    <section aria-labelledby="blocked-title">
      <h2 id="blocked-title">Recently blocked</h2>
      {snapshot.blocked.length === 0 ? (
        <p className="empty">Guard has not blocked anything recently.</p>
      ) : (
        <ul className="blocked">
          {snapshot.blocked.map((b, i) => (
            <li key={`${b.at}-${i}`} className="blocked__item">
              <span className="pill pill--bad">Blocked</span> <strong>{b.worker}</strong> tried to{" "}
              {b.summary}
              <span className="table__sub">
                {b.reason} · {ago(b.at, now)}
              </span>
              {onOpenTask && b.taskId && (
                <button
                  type="button"
                  className="link"
                  onClick={() => onOpenTask(b.taskId as string)}
                >
                  Open task
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
