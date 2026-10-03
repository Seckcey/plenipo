import { cx } from "@plenipo/ui";

import { useLiaisonRevision, useTaskHandoffs } from "../agents/useTaskHandoffs";
import { isOver, type ChatTurn } from "./model";
import { firstLine, STANDING_WORDS, standingOf, type Standing } from "./plan";

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
 * it is working on, each with where it stands. Choose one to watch that worker's chat.
 */
export function PlanPanel({
  turn,
  title,
  onOpenWorker,
}: {
  turn: ChatTurn | undefined;
  title: string;
  onOpenWorker: (sessionId: string, title: string) => void;
}) {
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
    </aside>
  );
}
