import { useId, useState } from "react";
import type { MessageView } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { MessageText } from "./MessageText";
import { REACTIONS, deliveryWords, messageIso, messageTime } from "./messageWords";
import { oneLine } from "./safeText";

/**
 * One message in a conversation (ADR-164): who wrote it, when, its words as safe text, where it
 * is (yours), its reactions, and what you can do with it. Everything in it that someone else
 * wrote is text only (`MessageText`, `oneLine`).
 *
 * What can be done is one row of buttons, in order: **React**, **Reply**, **Delete for me**,
 * **Give to a worker**, **Report** (ADR-167). **Report** is only on a message that can be
 * reported (theirs, with its proof), and only where `onReport` is given.
 */
export function MessageItem({
  message,
  who,
  quote,
  canWrite,
  onReact,
  onReply,
  onDelete,
  onGive,
  onReport,
  onOpenLink,
}: {
  message: MessageView;
  /** What to call the other person ("Pat"), already safe to show. */
  who: string;
  /**
   * The message this one answers, as a short quote: `undefined` when it answers none, `null`
   * when it answers one that is not shown here.
   */
  quote: string | null | undefined;
  /** You can write in this conversation, so you can react and reply. */
  canWrite: boolean;
  /** `emoji` is one of the reactions, or `null` to take yours back. */
  onReact: (message: MessageView, emoji: string | null) => void;
  onReply: (message: MessageView) => void;
  onDelete: (message: MessageView) => void;
  onGive: (message: MessageView) => void;
  /** Report this message (it is ticked in the Report window). */
  onReport?: ((message: MessageView) => void) | undefined;
  onOpenLink: (address: string) => void;
}) {
  const pickerId = useId();
  const [picking, setPicking] = useState(false);
  const mine = message.reactions.find((r) => r.mine)?.emoji ?? null;
  const at = message.acceptedAt ?? message.sentAt;
  const delivery = message.outgoing ? deliveryWords(message.state) : null;
  const hasWords = message.text !== null && message.text !== "";

  return (
    <li className={`message ${message.outgoing ? "message--mine" : "message--theirs"}`}>
      <p className="message__meta">
        <span className="message__author">{message.outgoing ? "You" : who}</span>
        <time dateTime={messageIso(at)}>{messageTime(at)}</time>
        {delivery && (
          <span className={`message__state message__state--${message.state}`}>{delivery}</span>
        )}
      </p>
      {quote !== undefined && (
        <p className="message__quote">
          {quote === null ? "In reply to an earlier message" : <q>{quote}</q>}
        </p>
      )}
      {hasWords && <MessageText text={message.text ?? ""} onOpenLink={onOpenLink} />}
      {message.hasGif && <p className="message__note">A GIF (this version doesn't show GIFs)</p>}
      {message.hasSticker && <p className="message__note">A sticker</p>}
      {!hasWords && !message.hasGif && !message.hasSticker && (
        <p className="message__note">This message can't be shown in this version of Plenipo.</p>
      )}
      {message.reactions.length > 0 && (
        <ul className="message__reactions" aria-label="Reactions">
          {message.reactions.map((r) => (
            <li key={`${r.mine}`} className={r.mine ? "message__reaction--mine" : undefined}>
              <span>{oneLine(r.emoji)}</span>
              {r.mine && <span className="message__reactor">You</span>}
            </li>
          ))}
        </ul>
      )}
      <div className="message__actions" role="group" aria-label="What you can do with this message">
        {canWrite && (
          <Button
            size="sm"
            variant="quiet"
            aria-expanded={picking}
            aria-controls={picking ? pickerId : undefined}
            onClick={() => setPicking((p) => !p)}
          >
            React
          </Button>
        )}
        {canWrite && (
          <Button size="sm" variant="quiet" onClick={() => onReply(message)}>
            Reply
          </Button>
        )}
        <Button size="sm" variant="quiet" onClick={() => onDelete(message)}>
          Delete for me
        </Button>
        {hasWords && (
          <Button size="sm" variant="quiet" onClick={() => onGive(message)}>
            Give to a worker
          </Button>
        )}
        {message.reportable && onReport && (
          <Button size="sm" variant="quiet" onClick={() => onReport(message)}>
            Report
          </Button>
        )}
      </div>
      {canWrite && picking && (
        <div id={pickerId} className="message__picker" role="group" aria-label="Pick a reaction">
          {REACTIONS.map((emoji) => (
            <button
              key={emoji}
              type="button"
              className="message__emoji"
              aria-label={`React with ${emoji}`}
              aria-pressed={mine === emoji}
              title={mine === emoji ? "Take your reaction back" : `React with ${emoji}`}
              onClick={() => {
                setPicking(false);
                onReact(message, mine === emoji ? null : emoji);
              }}
            >
              {emoji}
            </button>
          ))}
        </div>
      )}
    </li>
  );
}
