import type { HandoffView } from "@plenipo/types";

import {
  HANDOFF_OUTCOME_LABEL,
  HANDOFF_STATE_LABEL,
  handoffOutcomeTone,
  handoffStateTone,
} from "../agents/format";

function contextText(view: HandoffView): string {
  return view.context.length > 0
    ? `Context: ${view.context.map((c) => c.title).join(" · ")}`
    : "No context passed";
}

function capabilitiesText(view: HandoffView): string | null {
  return view.capabilitiesRequested.length > 0
    ? `asked for ${view.capabilitiesRequested.join(", ")} (not granted)`
    : null;
}

/** One handoff a worker requested, with its worker and reply. */
export function HandoffCard({
  view,
  canOpen,
  onOpenSession,
}: {
  view: HandoffView;
  canOpen: (sessionId: string) => boolean;
  onOpenSession: (sessionId: string) => void;
}) {
  const reply = view.reply;
  const worker = view.childSessionId;
  const caps = capabilitiesText(view);
  return (
    <li
      className="handoff"
      data-state={view.state}
      aria-label={`Handoff to ${view.destinationLabel}: ${view.objective}`}
    >
      <div className="handoff__header">
        <span className="handoff__to">→ {view.destinationLabel}</span>
        <span className="handoff__objective">{view.objective}</span>
        <span className={`badge badge--task-${handoffStateTone(view.state)}`}>
          {HANDOFF_STATE_LABEL[view.state]}
        </span>
      </div>
      {view.rejection && <p className="handoff__refusal">Refused: {view.rejection}</p>}
      <div className="card__meta">
        {contextText(view)}
        {caps && <> · {caps}</>}
        {view.state !== "rejected" && <> · depth {view.depth}</>}
      </div>
      {reply && reply.source !== "liaison" && (
        <details className="handoff__reply">
          <summary>
            Reply:{" "}
            <span className={`badge badge--task-${handoffOutcomeTone(reply.outcome)}`}>
              {HANDOFF_OUTCOME_LABEL[reply.outcome]}
            </span>
            {reply.state === "pending" && " · not delivered yet"}
            {reply.state === "discarded" && " · not delivered (the requester stopped waiting)"}
          </summary>
          <div className="turn__text">{reply.text ?? reply.summary}</div>
          {reply.error && reply.error !== reply.summary && (
            <pre className="turn__error">{reply.error}</pre>
          )}
        </details>
      )}
      {worker && canOpen(worker) && (
        <button type="button" className="link" onClick={() => onOpenSession(worker)}>
          Open worker session
        </button>
      )}
    </li>
  );
}

/** The request a handoff worker was started for (its session header links to the requester). */
export function ReceivedHandoff({
  view,
  requesterLabel,
}: {
  view: HandoffView;
  requesterLabel: string;
}) {
  const caps = capabilitiesText(view);
  return (
    <div className="handoff handoff--received" aria-label="Handoff request">
      <div className="card__meta">
        Asked by {requesterLabel} through Plenipo Liaison · depth {view.depth}
      </div>
      <div className="card__meta">
        Acceptance criteria: {view.acceptanceCriteria.trim() || "none given"}
      </div>
      <div className="card__meta">
        {contextText(view)}
        {caps && <> · {caps}</>}
      </div>
    </div>
  );
}
