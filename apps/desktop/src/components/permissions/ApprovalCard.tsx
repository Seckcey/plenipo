import type { ApprovalView } from "@plenipo/types";

import { ago } from "../../org/format";
import { ScreenshotView } from "../ScreenshotView";
import { EnvironmentBadge } from "../servers/ServerSettings";
import { APPROVAL_STATUS_LABEL, timeLeft } from "../../guard/format";

/**
 * One request that needs the owner: who wants to do what, exactly, why it needs you, and the
 * time left. Nothing happens until you answer.
 */
export function ApprovalCard({
  approval: a,
  now,
  pending,
  onAnswer,
}: {
  approval: ApprovalView;
  now: number;
  pending: boolean;
  onAnswer: (approve: boolean) => void;
}) {
  const where = [a.role, a.project ? `${a.project} project` : null].filter(Boolean).join(" · ");
  return (
    <article
      className={`approval${a.environment === "production" ? " approval--production" : ""}`}
      aria-label={`${a.worker} wants to ${a.summary}`}
    >
      {a.server && a.environment && (
        <p className="approval__server">
          <EnvironmentBadge environment={a.environment} /> <strong>{a.server}</strong>
          {a.address && <span className="path"> {a.address}</span>}
          {a.environment === "production" && (
            <span> — a production server: check exactly what will run.</span>
          )}
        </p>
      )}
      <header className="approval__header">
        <h3 className="approval__title">
          {a.worker} wants to {a.summary}
        </h3>
        <span className="pill pill--warn">{timeLeft(a.expiresAt, now)}</span>
      </header>
      <p className="approval__meta">
        {where}
        {a.folder && (
          <>
            {" · "}
            <span className="path">{a.folder}</span>
          </>
        )}
      </p>
      <pre className="approval__detail" aria-label="Exactly what it will do">
        {a.detail || a.summary}
      </pre>
      {a.url && (
        <p className="approval__meta">
          On the page <span className="path">{a.url}</span>
        </p>
      )}
      {a.screenshot && (
        <ScreenshotView
          id={a.screenshot}
          label={`The page when ${a.worker} asked`}
          startOpen={a.status === "pending"}
        />
      )}
      <p className="approval__why">
        <strong>Why it needs you:</strong> {a.reason}
      </p>
      <p className="approval__tags">
        <span className="pill">{a.capabilityLabel}</span>{" "}
        {a.riskLabel && <span className="pill">{a.riskLabel}</span>}{" "}
        {a.sensitiveLabel && <span className="pill pill--bad">{a.sensitiveLabel}</span>}
      </p>
      {!a.waiting && (
        <p className="muted">
          No worker is waiting for this answer any more (its step ended); answering records your
          decision only.
        </p>
      )}
      <div className="actions">
        <button type="button" className="button" disabled={pending} onClick={() => onAnswer(true)}>
          Approve
        </button>
        <button
          type="button"
          className="button button--danger"
          disabled={pending}
          onClick={() => onAnswer(false)}
        >
          Deny
        </button>
        <span className="muted">Asked {ago(a.requestedAt, now)}</span>
      </div>
    </article>
  );
}

/** A settled request, in a line. */
export function ApprovalOutcome({ approval: a, now }: { approval: ApprovalView; now: number }) {
  const tone =
    a.status === "approved" ? "pill--ok" : a.status === "rejected" ? "pill--bad" : "pill--warn";
  return (
    <li className="approval-outcome">
      <span className={`pill ${tone}`}>{APPROVAL_STATUS_LABEL[a.status]}</span>{" "}
      <strong>{a.worker}</strong>: {a.summary}
      <span className="table__sub">
        {a.resolvedAt ? ago(a.resolvedAt, now) : ""}
        {a.note ? ` · ${a.note}` : ""}
      </span>
    </li>
  );
}
