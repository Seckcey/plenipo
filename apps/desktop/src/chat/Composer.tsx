import { useId, useState, type FormEvent, type KeyboardEvent } from "react";
import { Banner, Icon, IconButton, cx } from "@plenipo/ui";

import { ENTER_SENDS, isSendKey } from "../components/enterSends";

/** The longest message (the same limit as an objective). */
export const MAX_MESSAGE = 20_000;

/**
 * Where you write to an agent (ADR-200): Enter sends, Shift+Enter starts a new line. While the
 * agent works, what you send waits its turn (listed above the box, each with a button to take it
 * back), and Stop stops the work now. The line under the box says which AI tool, model, and
 * effort answer.
 */
export function Composer({
  title,
  busy,
  sending,
  queued,
  problem,
  disabledReason,
  facts,
  onSend,
  onStop,
  onUnqueue,
  onDismissProblem,
}: {
  title: string;
  busy: boolean;
  sending: boolean;
  queued: readonly string[];
  problem: string | null;
  /** Why nothing can be sent here (a worker that takes work only from its lead). */
  disabledReason: string | null;
  /** The AI tool, model, and effort, in plain words. */
  facts: readonly string[];
  onSend: (text: string) => void;
  onStop: () => void;
  onUnqueue: (index: number) => void;
  onDismissProblem: () => void;
}) {
  const [text, setText] = useState("");
  const hint = useId();
  const blocked = disabledReason !== null;
  const send = (e?: FormEvent) => {
    e?.preventDefault();
    if (blocked || text.trim() === "") return;
    onSend(text);
    setText("");
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter sends; Shift+Enter is a new line; a word being put together (an IME) is not sent.
    if (isSendKey(e)) {
      e.preventDefault();
      send();
    }
  };
  return (
    <form className="chat-composer" aria-label={`Message ${title}`} onSubmit={send}>
      {queued.length > 0 && (
        <ol className="chat-composer__queue" aria-label="Waiting to send">
          {queued.map((message, i) => (
            <li key={`${i}:${message.slice(0, 20)}`}>
              <Icon name="clock" size={13} />
              <span className="chat-composer__queued">{message}</span>
              <span className="muted">goes when {title} finishes</span>
              <IconButton
                icon="close"
                label="Do not send this"
                className="chat-composer__unqueue"
                onClick={() => onUnqueue(i)}
              />
            </li>
          ))}
        </ol>
      )}
      {problem && (
        <Banner
          tone="error"
          role="alert"
          title="That message did not go"
          onDismiss={onDismissProblem}
        >
          {problem}
        </Banner>
      )}
      {disabledReason && <p className="chat-composer__blocked">{disabledReason}</p>}
      <div className={cx("chat-composer__box", blocked && "is-blocked")}>
        <textarea
          className="chat-composer__input"
          value={text}
          rows={2}
          maxLength={MAX_MESSAGE}
          disabled={blocked}
          aria-label={`Message to ${title}`}
          aria-describedby={hint}
          placeholder={
            blocked
              ? ""
              : busy
                ? `Write your next message; it goes when ${title} finishes`
                : `Message ${title}`
          }
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKey}
        />
        <div className="chat-composer__foot">
          <span id={hint} className="chat-composer__facts">
            {facts.map((f) => (
              <span key={f} className="chat-composer__fact">
                {f}
              </span>
            ))}
            <span className="visually-hidden">{ENTER_SENDS}</span>
          </span>
          {busy && (
            <IconButton
              icon="stop"
              label={`Stop ${title}`}
              className="chat-composer__stop"
              onClick={onStop}
            />
          )}
          <IconButton
            icon="arrowUp"
            type="submit"
            label={busy ? "Send when it finishes" : "Send"}
            className="chat-composer__send"
            disabled={blocked || sending || text.trim() === ""}
          />
        </div>
      </div>
    </form>
  );
}
