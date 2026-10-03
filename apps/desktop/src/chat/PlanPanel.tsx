import { cx } from "@plenipo/ui";

import { useLiaisonRevision, useTaskHandoffs } from "../agents/useTaskHandoffs";
import { isOver, type ChatTurn } from "./model";
import { chainLine, firstLine, STANDING_WORDS, standingOf, type Standing } from "./plan";
import { useChainOrders } from "./useChainOrders";

/** The circle: dotted while waiting, a turning arc while working, a check when done. */
export function StandingMark({ standing }: { standing: Standing }) {
  return (
    <span
      className={cx("standing", `standing--${standing}`)}
      role="img"
      aria-label={STANDING_WORDS[standing]}
    />
  );
}

/**
 * The agent's plan beside its chat (ADR-200): the tasks it handed to its team for the message
 * it is working on, each with where it stands (choose one to watch that worker's chat), and its
 * chain of command (ADR-202): your orders it was given, passed on, or was told about.
 */
export function PlanPanel({
  turn,
  title,
  positionId,
  onOpenWorker,
}: {
  turn: ChatTurn | undefined;
  title: string;
  /** Its position, when it has one (a worker's conversation has none). */
  positionId: string | null;
  onOpenWorker: (sessionId: string, title: string) => void;
}) {
  const chain = useChainOrders(positionId);
  const revision = useLiaisonRevision();
  const handoffs = useTaskHandoffs(
    turn?.taskId ?? null,
    turn ? `${turn.taskId}:${turn.state}` : "",
    revision,
    turn ? isOver(turn) : true,
  );
  const sent = handoffs?.sent ?? [];
  const asked = handoffs?.received ?? null;
  return (
    <aside className="chat-plan" aria-label={`${title}'s plan`}>
      <h3 className="chat-plan__head">Tasks</h3>
      {sent.length === 0 ? (
        <p className="chat-plan__empty">
          {turn
            ? `When ${title} hands parts of this to its team, each task shows here as it goes.`
            : `${title}'s team tasks show here once you send a message.`}
        </p>
      ) : (
        <ul className="chat-plan__list">
          {sent.map((h) => {
            const standing = standingOf(h);
            const sessionId = h.childSessionId;
            const label = `${h.destinationLabel}: ${firstLine(h.objective)}`;
            return (
              <li key={h.messageId} className="chat-plan__item">
                <StandingMark standing={standing} />
                {sessionId ? (
                  <button
                    type="button"
                    className="chat-plan__task"
                    title={`Watch ${h.destinationLabel} work on it`}
                    onClick={() => onOpenWorker(sessionId, h.destinationLabel)}
                  >
                    {label}
                  </button>
                ) : (
                  <span className="chat-plan__task">{label}</span>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {asked && (
        <p className="chat-plan__asked">
          Asked by its lead: <span>{firstLine(asked.objective)}</span>
        </p>
      )}
      {chain && chain.length > 0 && (
        <>
          <h3 className="chat-plan__head">Chain of command</h3>
          <ul className="chat-plan__list" aria-label="Chain of command">
            {chain.map((o) => (
              <li key={o.taskId} className="chat-plan__item">
                <StandingMark standing={o.standing} />
                <div className="chat-plan__order">
                  <span className="chat-plan__task">{chainLine(o)}</span>
                  {o.result && <span className="chat-plan__result">Reported: {o.result}</span>}
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
    </aside>
  );
}
