import type { ApprovalView } from "@plenipo/types";
import { Button, StatusPill, Tag } from "@plenipo/ui";

import { ago } from "../../org/format";
import { ScreenshotView } from "../ScreenshotView";
import { EnvironmentBadge } from "../servers/ServerSettings";
import { PILL_TONE } from "../tones";
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
        <StatusPill status={PILL_TONE.warn} label={timeLeft(a.expiresAt, now)} />
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
          label={a.url ? `The page when ${a.worker} asked` : `The screen when ${a.worker} asked`}
          startOpen={a.status === "pending"}
        />
      )}
      <p className="approval__why">
        <strong>Why it needs you:</strong> {a.reason}
      </p>
      <p className="approval__tags">
        <Tag label={a.capabilityLabel} /> {a.riskLabel && <Tag label={a.riskLabel} />}{" "}
        {a.sensitiveLabel && <StatusPill status={PILL_TONE.bad} label={a.sensitiveLabel} />}
      </p>
      {!a.waiting && (
        <p className="muted">
          No worker is waiting for this answer any more (its step ended); answering records your
          decision only.
        </p>
      )}
      <div className="actions">
        <Button variant="primary" disabled={pending} onClick={() => onAnswer(true)}>
          Approve
        </Button>
        <Button variant="danger" disabled={pending} onClick={() => onAnswer(false)}>
          Deny
        </Button>
        <span className="muted">Asked {ago(a.requestedAt, now)}</span>
      </div>
    </article>
  );
}

/** A settled request, in a line. */
export function ApprovalOutcome({ approval: a, now }: { approval: ApprovalView; now: number }) {
  const tone =
    a.status === "approved"
      ? PILL_TONE.ok
      : a.status === "rejected"
        ? PILL_TONE.bad
        : PILL_TONE.warn;
  return (
    <li className="approval-outcome">
      <StatusPill status={tone} label={APPROVAL_STATUS_LABEL[a.status]} />{" "}
      <strong>{a.worker}</strong>: {a.summary}
      <span className="table__sub">
        {a.resolvedAt ? ago(a.resolvedAt, now) : ""}
        {a.note ? ` · ${a.note}` : ""}
      </span>
    </li>
  );
}
