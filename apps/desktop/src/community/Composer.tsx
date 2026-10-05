import {
  useEffect,
  useId,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { Button } from "@plenipo/ui";

import { isSendKey } from "../components/enterSends";
import { ATTACH_EVENT, DROP_ATTRIBUTE } from "../files/refs";
import {
  MAX_MESSAGE_CHARS,
  PHOTOS_REFUSED,
  countChars,
  limitChars,
  quoteOf,
  whoWords,
} from "./messageWords";

/** Why a message did not go: its plain words, and whether it is a part of Pro. */
export interface SendProblem {
  message: string;
  partOfPro: boolean;
}

/** Does this clipboard or drag hold a picture or a file? (Looks only at what it is, never in it.) */
function holdsFiles(data: DataTransfer | null | undefined): boolean {
  if (!data) return false;
  if ((data.files?.length ?? 0) > 0) return true;
  if (Array.from(data.items ?? []).some((item) => item.kind === "file")) return true;
  return Array.from(data.types ?? []).includes("Files");
}

/**
 * The box a message is written in (ADR-164 §4). It holds up to 4,000 characters; Enter sends and
 * Shift+Enter starts a new line. There is no GIF button and no sticker button (ADR-172).
 *
 * A picture or a file pasted or dropped on it is refused with "Photos can't be sent in
 * Community": a sealed picture is one 8 West could never check, and some members are 13. The file
 * is never read, only turned away. `onSend` answers `null` when the message went, or why not.
 */
export function Composer({
  who,
  reply,
  onCancelReply,
  onSend,
  onLicense,
}: {
  /** What to call the other person ("Pat"). */
  who: { name: string; displayName: string | null };
  /** The message being answered: its words, or none. */
  reply: { text: string | null } | null;
  onCancelReply: () => void;
  onSend: (text: string) => Promise<SendProblem | null>;
  /** The way to Settings → License, for a message that is part of Pro. */
  onLicense: () => void;
}) {
  const countId = useId();
  const hintId = useId();
  const box = useRef<HTMLTextAreaElement>(null);
  const form = useRef<HTMLFormElement>(null);
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [problem, setProblem] = useState<SendProblem | null>(null);
  const [refused, setRefused] = useState(false);
  const name = whoWords(who);
  const count = countChars(text);
  const ready = text.trim() !== "" && !sending;

  // Answering a message: the cursor goes to the box.
  const replying = reply !== null;
  useEffect(() => {
    if (replying) box.current?.focus();
  }, [replying]);

  // A file dragged from the Files panel, or dropped from File Explorer, is told to this form by
  // the page. It is turned away the same way.
  useEffect(() => {
    const el = form.current;
    if (!el) return;
    const turnAway = () => setRefused(true);
    el.addEventListener(ATTACH_EVENT, turnAway);
    return () => el.removeEventListener(ATTACH_EVENT, turnAway);
  }, []);

  const send = () => {
    if (!ready) return;
    setSending(true);
    setProblem(null);
    setRefused(false);
    // The spaces and empty lines around the words are not sent.
    void onSend(text.trim())
      .then((failed) => {
        if (failed === null) setText("");
        else setProblem(failed);
      })
      .finally(() => setSending(false));
  };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    send();
  };
  const keys = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter sends; Shift+Enter is a new line. A key that picks a letter in another language
    // (composing, on Windows, a Mac, or Linux) is not a send.
    if (isSendKey(e)) {
      e.preventDefault();
      send();
    }
  };
  const pasted = (e: ClipboardEvent<HTMLTextAreaElement>) => {
    if (holdsFiles(e.clipboardData)) {
      e.preventDefault();
      setRefused(true);
    }
  };
  const over = (e: DragEvent<HTMLElement>) => {
    // Let a drop land here, so it can be turned away (otherwise the window might open the file).
    if (holdsFiles(e.dataTransfer)) e.preventDefault();
  };
  const dropped = (e: DragEvent<HTMLElement>) => {
    if (holdsFiles(e.dataTransfer)) {
      e.preventDefault();
      setRefused(true);
    }
  };

  return (
    <form
      ref={form}
      className="composer"
      aria-label="Write a message"
      {...{ [DROP_ATTRIBUTE]: "" }}
      onSubmit={submit}
      onDragOver={over}
      onDrop={dropped}
    >
      {reply && (
        <div className="composer__reply">
          <p>
            Replying to <q>{quoteOf(reply.text)}</q>
          </p>
          <Button size="sm" variant="quiet" onClick={onCancelReply}>
            Cancel reply
          </Button>
        </div>
      )}
      <textarea
        ref={box}
        aria-label={`Message to ${name}`}
        aria-describedby={`${countId} ${hintId}`}
        value={text}
        rows={3}
        autoComplete="off"
        onChange={(e) => {
          setText(limitChars(e.target.value));
          setRefused(false);
        }}
        onKeyDown={keys}
        onPaste={pasted}
      />
      <div className="composer__row">
        <p id={hintId} className="composer__hint muted">
          Enter sends. Shift+Enter starts a new line.
        </p>
        <p
          id={countId}
          className={`composer__count${count >= MAX_MESSAGE_CHARS ? " composer__count--full" : ""}`}
        >
          {count} of {MAX_MESSAGE_CHARS}
        </p>
        <Button type="submit" variant="primary" disabled={!ready}>
          Send
        </Button>
      </div>
      {refused && (
        <p className="form-error" role="alert">
          {PHOTOS_REFUSED}
        </p>
      )}
      {problem?.partOfPro === false && (
        <p className="form-error" role="alert">
          {problem.message}
        </p>
      )}
      {problem?.partOfPro === true && (
        <div className="notice-box part-of-pro" role="alert">
          <p>{problem.message}</p>
          <div className="settings-section__actions">
            <Button size="sm" icon="key" onClick={onLicense}>
              Open Settings → License
            </Button>
          </div>
        </div>
      )}
    </form>
  );
}
