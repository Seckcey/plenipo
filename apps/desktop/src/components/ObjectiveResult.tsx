import type {
  ObjectiveReport,
  ReportApprovalState,
  ReportTask,
  ReviewVerdict,
  Severity,
} from "@plenipo/types";

import { WORKER_STATE_LABEL, ago, plural } from "../org/format";

const VERDICT_LABEL: Record<ReviewVerdict, string> = {
  approve: "Approved",
  "request-changes": "Changes requested",
};

const SEVERITY_LABEL: Record<Severity, string> = {
  blocker: "Blocker",
  major: "Major",
  minor: "Minor",
};

const APPROVAL_LABEL: Record<ReportApprovalState, string> = {
  waiting: "Waiting for you",
  approved: "Approved",
  denied: "Not approved",
  expired: "Expired",
};

function firstLine(text: string, max = 160): string {
  const line = text.trim().split("\n")[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/** "Codex · gpt-5.5", or "Codex · its default model" when the AI tool named none. */
function toolAndModel(runtime: string | null, model: string | null): string {
  if (!runtime) return "—";
  return `${runtime} · ${model ?? "its default model"}`;
}

function lines(added: number | null, removed: number | null): string {
  if (added === null && removed === null) return "—";
  return `+${added ?? 0} −${removed ?? 0}`;
}

function TaskRow({
  task,
  onOpenTask,
}: {
  task: ReportTask;
  onOpenTask: ((id: string) => void) | undefined;
}) {
  return (
    <li className="result__task" style={{ paddingLeft: `${Math.min(task.depth, 6) * 18}px` }}>
      <div className="card__row">
        <div>
          <strong>{task.who}</strong>
          {task.role && task.role !== task.who && <span className="muted"> · {task.role}</span>}
          <div>{firstLine(task.objective) || "(no objective)"}</div>
          {task.summary && <div className="muted">{firstLine(task.summary, 220)}</div>}
        </div>
        <div className="result__task-side">
          <span className={`badge badge--task-${task.state}`}>
            {WORKER_STATE_LABEL[task.state]}
          </span>
          {onOpenTask && (
            <button type="button" className="link" onClick={() => onOpenTask(task.taskId)}>
              Details
            </button>
          )}
        </div>
      </div>
    </li>
  );
}

/**
 * Plenipo's result for an objective, built from what the Ledger recorded (not from what the AI
 * said): who worked on it and on which models, the files changed, the tests run, reviews and
 * their findings, the branch and pull request, and approvals still needed.
 */
export function ObjectiveResult({
  report,
  onOpenTask,
  onOpenApprovals,
  openLabel = "Open in Activity",
}: {
  report: ObjectiveReport;
  onOpenTask?: (taskId: string) => void;
  onOpenApprovals?: () => void;
  /** The words of the link that opens the objective's own task; `null` leaves it out (its page). */
  openLabel?: string | null;
}) {
  const tests = report.checks.filter((c) => c.test);
  const passed = tests.filter((c) => c.ok).length;
  const failed = tests.length - passed;
  const waiting = report.approvals.filter((a) => a.state === "waiting");
  const blocking = report.findings.filter((f) => f.blocking).length;
  const committed = report.files.filter((f) => f.committed).length;

  return (
    <article className="result" aria-label="Result">
      <header className="detail__header">
        <div>
          <h2>{firstLine(report.objective)}</h2>
          <p className="muted">
            {report.positionTitle ? `Given to ${report.positionTitle}` : "Objective"}
            {report.projectName ? ` · ${report.projectName}` : ""} · {ago(report.createdAt)}
            {report.completedAt !== null ? ` · finished ${ago(report.completedAt)}` : ""}
          </p>
        </div>
        <div className="result__task-side">
          <span className={`badge badge--task-${report.state}`}>
            {WORKER_STATE_LABEL[report.state]}
          </span>
          {onOpenTask && openLabel && (
            <button type="button" className="link" onClick={() => onOpenTask(report.rootTaskId)}>
              {openLabel}
            </button>
          )}
        </div>
      </header>

      <dl className="facts facts--compact" aria-label="Summary">
        <div>
          <dt>Tasks</dt>
          <dd>{report.tasks.length}</dd>
        </div>
        <div>
          <dt>Workers</dt>
          <dd>{report.workers.length}</dd>
        </div>
        <div>
          <dt>Files changed</dt>
          <dd>
            {report.files.length}
            {report.files.length > committed && (
              <span className="muted"> ({report.files.length - committed} not committed)</span>
            )}
          </dd>
        </div>
        <div>
          <dt>Tests</dt>
          <dd className={tests.length > 0 && failed === 0 ? "ok" : undefined}>
            {tests.length === 0
              ? "None run"
              : failed === 0
                ? `${passed} passed`
                : `${passed} passed, ${failed} failed`}
          </dd>
        </div>
        <div>
          <dt>Open findings</dt>
          <dd>
            {report.findings.length}
            {blocking > 0 && <span className="muted"> ({blocking} must be fixed)</span>}
          </dd>
        </div>
        <div>
          <dt>Approvals waiting</dt>
          <dd>{waiting.length}</dd>
        </div>
      </dl>

      {report.problems.length > 0 && (
        <div className="hint" role="note" aria-label="Needs your attention">
          <strong>Needs your attention</strong>
          <ul className="result__list">
            {report.problems.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </div>
      )}

      {report.approvals.length > 0 && (
        <section aria-label="Approvals">
          <div className="section-header">
            <h3>Approvals</h3>
            {waiting.length > 0 && onOpenApprovals && (
              <button type="button" className="button button--small" onClick={onOpenApprovals}>
                Review in Approvals
              </button>
            )}
          </div>
          <ul className="result__list">
            {report.approvals.map((a) => (
              <li key={a.approvalId}>
                <strong>{a.who}</strong>: {a.summary}{" "}
                <span
                  className={`pill${a.state === "waiting" ? " pill--warn" : a.state === "approved" ? " pill--ok" : ""}`}
                >
                  {APPROVAL_LABEL[a.state]}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section aria-label="Answer">
        <h3>Answer</h3>
        {report.answer ? (
          <p className="result__answer">{report.answer}</p>
        ) : (
          <p className="muted">
            {report.state === "succeeded" ? "No answer was given." : "Not finished yet."}
          </p>
        )}
      </section>

      {report.workers.length > 0 && (
        <section aria-label="Who worked on it">
          <h3>Who worked on it</h3>
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Who</th>
                <th scope="col">AI tool and model</th>
                <th scope="col">Tasks</th>
              </tr>
            </thead>
            <tbody>
              {report.workers.map((w) => (
                <tr key={`${w.who}-${w.runtimeLabel ?? ""}-${w.model ?? ""}`}>
                  <th scope="row">
                    {w.who}
                    {w.role && w.role !== w.who && <span className="table__sub">{w.role}</span>}
                  </th>
                  <td>{toolAndModel(w.runtimeLabel, w.model)}</td>
                  <td>{w.tasks}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      <section aria-label="Tasks">
        <h3>Tasks</h3>
        <ul className="result__tasks">
          {report.tasks.map((t) => (
            <TaskRow key={t.taskId} task={t} onOpenTask={onOpenTask} />
          ))}
        </ul>
      </section>

      {report.files.length > 0 && (
        <section aria-label="Files changed">
          <h3>Files changed</h3>
          <table className="table">
            <thead>
              <tr>
                <th scope="col">File</th>
                <th scope="col">Lines</th>
                <th scope="col">Saved</th>
                <th scope="col">Branch</th>
              </tr>
            </thead>
            <tbody>
              {report.files.map((f) => (
                <tr key={`${f.branch}:${f.path}`}>
                  <th scope="row">
                    <code>{f.path}</code>
                  </th>
                  <td>{lines(f.added, f.removed)}</td>
                  <td>
                    {f.committed ? (
                      <span className="pill pill--ok">Committed</span>
                    ) : (
                      <span className="pill pill--warn">Not committed</span>
                    )}
                  </td>
                  <td>
                    <code>{f.branch}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {report.checks.length > 0 && (
        <section aria-label="Tests and programs">
          <h3>Tests and programs</h3>
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Program</th>
                <th scope="col">Run by</th>
                <th scope="col">Result</th>
              </tr>
            </thead>
            <tbody>
              {report.checks.map((c, i) => (
                <tr key={`${c.taskId}-${c.at}-${i}`}>
                  <th scope="row">
                    <code>{c.command}</code>
                    {c.test && <span className="table__sub">Test</span>}
                  </th>
                  <td>{c.who}</td>
                  <td>
                    {c.ok ? (
                      <span className="pill pill--ok">Passed</span>
                    ) : (
                      <span className="pill pill--bad">Failed</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {report.reviews.length > 0 && (
        <section aria-label="Reviews">
          <h3>Reviews</h3>
          <ul className="result__list">
            {report.reviews.map((r) => (
              <li key={`${r.taskId}-${r.at}`}>
                <strong>{r.who}</strong>:{" "}
                <span className={`pill ${r.verdict === "approve" ? "pill--ok" : "pill--warn"}`}>
                  {VERDICT_LABEL[r.verdict]}
                </span>{" "}
                <span className="muted">
                  {plural(r.findings, "finding")} · {ago(r.at)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {report.findings.length > 0 && (
        <section aria-label="Open findings">
          <h3>Open findings</h3>
          <table className="table">
            <thead>
              <tr>
                <th scope="col">Severity</th>
                <th scope="col">Finding</th>
                <th scope="col">From</th>
              </tr>
            </thead>
            <tbody>
              {report.findings.map((f, i) => (
                <tr key={`${f.taskId}-${i}`}>
                  <td>
                    <span
                      className={`pill ${f.severity === "minor" ? "" : f.blocking ? "pill--bad" : "pill--warn"}`}
                    >
                      {SEVERITY_LABEL[f.severity]}
                    </span>
                    {f.blocking && <span className="table__sub">Must be fixed</span>}
                  </td>
                  <th scope="row">
                    {f.summary}
                    {f.file && (
                      <span className="table__sub">
                        <code>{f.file}</code>
                      </span>
                    )}
                  </th>
                  <td>{f.who}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {report.branches.length > 0 && (
        <section aria-label="Branches">
          <h3>Branches</h3>
          <ul className="result__list">
            {report.branches.map((b) => (
              <li key={b.workspaceId}>
                <code>{b.branch}</code>
                {b.baseRef && <span className="muted"> from {b.baseRef}</span>}{" "}
                <span className={`pill${b.pushed ? " pill--ok" : ""}`}>
                  {b.pushed ? "Pushed" : "Not pushed"}
                </span>
                <div className="muted">
                  {plural(b.commits.length, "commit")}
                  {b.uncommitted > 0 && ` · ${plural(b.uncommitted, "file")} not committed`}
                  {b.removed ? " · working copy removed" : ` · working copy in ${b.path}`}
                </div>
                {b.mergeInto && (
                  <div className="muted">
                    Made while another worker held the working copy: merge it into{" "}
                    <code>{b.mergeInto}</code>.
                  </div>
                )}
                {b.commits.length > 0 && (
                  <ul className="result__commits">
                    {b.commits.map((c) => (
                      <li key={c.hash}>
                        <code>{c.hash}</code> {c.subject}
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      {report.pullRequests.length > 0 && (
        <section aria-label="Pull requests">
          <h3>Pull requests</h3>
          <ul className="result__list">
            {report.pullRequests.map((pr) => (
              <li key={pr.url}>
                Pull request #{pr.number}, opened by {pr.who}: <code>{pr.url}</code>
              </li>
            ))}
          </ul>
        </section>
      )}

      {report.blocked.length > 0 && (
        <section aria-label="Refused">
          <h3>Refused</h3>
          <ul className="result__list">
            {report.blocked.map((b, i) => (
              <li key={`${b.taskId}-${b.at}-${i}`}>
                <strong>{b.who}</strong>: {b.summary} <span className="muted">— {b.reason}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </article>
  );
}
