/**
 * What agents say to each other, in their chats (B5): a lead's requests to its team as cards where
 * it wrote them, the replies where they came back into its turn, and in a worker's chat who asked
 * it and whether its answer reached them. The words are the ones Liaison keeps (the Ledger,
 * redacted as recorded); nothing new is stored.
 */
import type { HandoffView, ReplyView } from "@plenipo/types";
import { StatusPill } from "@plenipo/ui";

import { HANDOFF_OUTCOME_LABEL, handoffOutcomeTone } from "../agents/format";
import { HandoffCard } from "../components/Handoffs";
import { OUTCOME_TONE } from "../components/tones";
import { sessionOfSource } from "./handoffBlocks";

/** How a chat opens the other side of an exchange, and names who asked. */
export interface ChatLiaison {
  /** Open another conversation: the worker's, or the one that asked. */
  open: (sessionId: string, title: string) => void;
  canOpen: (sessionId: string) => boolean;
  /** Who asked, by name: `session:<id>`, and its AI tool. */
  nameOf: (source: string, runtimeId: string | null) => string;
}

/** First line of a text, short enough for a line. */
function firstLine(text: string, max = 160): string {
  const line = text.trim().split("\n")[0] ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/**
 * A request the lead wrote, as a card: to whom, what, and how it stands now (with its reply once
 * it came). Before Liaison has it (a moment after it is written), the card says it is going.
 */
export function RequestCard({
  to,
  objective,
  view,
  liaison,
}: {
  to: string;
  objective: string;
  view: HandoffView | null;
  liaison: ChatLiaison;
}) {
  return (
    <ul className="handoffs chat-handoffs">
      {view ? (
        <HandoffCard
          view={view}
          canOpen={liaison.canOpen}
          onOpenSession={(id) => liaison.open(id, view.destinationLabel)}
        />
      ) : (
        <li className="handoff" aria-label={`Handoff to ${to}: ${objective}`}>
          <div className="handoff__header">
            <span className="handoff__to">→ {to}</span>
            <span className="handoff__objective">{objective}</span>
            <span className="muted">Sending…</span>
          </div>
        </li>
      )}
    </ul>
  );
}

/** The replies that came back into a lead's turn: each worker's answer, as it was sent. */
export function RepliesBack({ views }: { views: readonly HandoffView[] }) {
  if (views.length === 0) return null;
  return (
    <ul className="chat-replies" aria-label="Replies from its team">
      {views.map((v) =>
        v.reply ? (
          <li key={v.messageId} className="chat-reply">
            <details>
              <summary>
                <strong>{v.destinationLabel} replied</strong>{" "}
                <StatusPill
                  status={OUTCOME_TONE[handoffOutcomeTone(v.reply.outcome)]}
                  label={HANDOFF_OUTCOME_LABEL[v.reply.outcome]}
                />{" "}
                <span className="chat-reply__line">
                  {firstLine(v.reply.text ?? v.reply.summary)}
                </span>
              </summary>
              <p className="chat-reply__text">{v.reply.text ?? v.reply.summary}</p>
            </details>
          </li>
        ) : null,
      )}
    </ul>
  );
}

/** In a worker's chat: who asked it, and how to see their side. */
export function AskedBy({
  view,
  worker,
  liaison,
}: {
  view: HandoffView;
  worker: string;
  liaison: ChatLiaison;
}) {
  const name = liaison.nameOf(view.requester, view.requesterRuntimeId);
  const from = sessionOfSource(view.requester);
  const criteria = view.acceptanceCriteria.trim();
  return (
    <div className="chat-asked" aria-label={`Request from ${name}`}>
      <span className="chat-turn__from">
        {name} asked {worker}
      </span>
      {from && liaison.canOpen(from) && (
        <button type="button" className="link" onClick={() => liaison.open(from, name)}>
          Open {name}&apos;s conversation
        </button>
      )}
      {criteria && <p className="chat-asked__meta">Done when: {criteria}</p>}
      {view.context.length > 0 && (
        <p className="chat-asked__meta">With: {view.context.map((c) => c.title).join(" · ")}</p>
      )}
    </div>
  );
}

function backWords(reply: ReplyView, name: string): string {
  if (reply.state === "delivered") return `${name} got it`;
  if (reply.state === "discarded") return `${name} had stopped waiting, so it was not passed on`;
  return `on its way to ${name}`;
}

/** In a worker's chat, after its answer: that it went back to who asked, and whether it arrived. */
export function ReplyBack({
  view,
  worker,
  liaison,
}: {
  view: HandoffView;
  worker: string;
  liaison: ChatLiaison;
}) {
  const reply = view.reply;
  if (!reply) return null;
  const name = liaison.nameOf(view.requester, view.requesterRuntimeId);
  return (
    <p className="chat-reply-back" aria-label={`Reply to ${name}`}>
      {worker} replied to {name} · {backWords(reply, name)}
      {reply.sentBack && " · sent back once to check against the record"}
    </p>
  );
}
