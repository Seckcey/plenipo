import { useId, useState } from "react";
import type { MessageView, ReportOf } from "@plenipo/types";
import { Button, Checkbox } from "@plenipo/ui";

import { reportInCommunity, toCommandError, type ReportReason } from "../api/commands";
import { Modal } from "../components/org/Modal";
import {
  MAX_NOTE_CHARS,
  MAX_REPORT_MESSAGES,
  REASONS,
  REPORT_EXPLAINED,
  REPORT_THANKS,
  UNBLOCK_IN_SETTINGS,
} from "./blockReportWords";
import { MessageParts } from "./MessageText";
import { countChars, cutChars, handle, messageIso, messageTime } from "./messageWords";

/** What a report can be about: the person, their profile, or messages they sent you. */
export type ReportAbout = "person" | "profile" | "messages";

/** What each choice is called, when there is more than one to pick from. */
const ABOUT_WORDS: Record<ReportAbout, string> = {
  person: "The person",
  profile: "Their profile",
  messages: "Messages from them",
};

/** A message with no words, in a few words: what the box says in place of them. */
function noWords(message: MessageView): string {
  if (message.hasGif) return "A GIF (this version doesn't show GIFs)";
  if (message.hasSticker) return "A sticker";
  return "This message can't be shown in this version of Plenipo.";
}

/**
 * **Report** (ADR-167 §5, §6): **What is wrong?**, an optional note of up to 1,000 characters, and
 * whether to **Also block** the person. A report carries only what is ticked here, so for
 * messages the window lists the ones that can be reported (`messages`: theirs, with their proof),
 * up to 20 ticked. A web address in one is plain text: nothing here opens, and nothing is a link.
 *
 * `about` is what can be picked, in order: one choice is not asked, it is just what the report is
 * about. `ticked` are the messages ticked to begin with. `onSent` is told when 8 West has the
 * report, and whether the person was blocked too; the window then says "Thanks" until it is
 * closed. The person's `name` is sent as it is (never the shown, made-safe one).
 */
export function ReportDialog({
  memberId,
  name,
  about,
  messages = [],
  ticked = [],
  onSent,
  onClose,
}: {
  memberId: string;
  /** Their Community name, without the "@". */
  name: string;
  about: readonly ReportAbout[];
  /** Their messages in the open conversation that can be reported. */
  messages?: readonly MessageView[];
  /** The IDs of the messages to tick to begin with. */
  ticked?: readonly string[];
  onSent?: (blocked: boolean) => void;
  onClose: () => void;
}) {
  const groupId = useId();
  const noteId = useId();
  const countId = useId();
  const [subject, setSubject] = useState<ReportAbout>(about[0] ?? "person");
  const [reason, setReason] = useState<ReportReason | null>(null);
  const [note, setNote] = useState("");
  const [block, setBlock] = useState(false);
  const [ids, setIds] = useState<readonly string[]>(() =>
    ticked.filter((id) => messages.some((m) => m.itemId === id)).slice(0, MAX_REPORT_MESSAGES),
  );
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState<{ blocked: boolean } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  const at = handle(name);
  const count = countChars(note);
  // Only messages that are still there are carried, in the order they are shown.
  const chosen = messages.filter((m) => ids.includes(m.itemId)).map((m) => m.itemId);
  const full = chosen.length >= MAX_REPORT_MESSAGES;
  const ready = reason !== null && !sending && (subject !== "messages" || chosen.length > 0);

  const send = () => {
    if (reason === null || !ready) return;
    const of: ReportOf =
      subject === "messages" ? { kind: "messages", itemIds: chosen } : { kind: subject };
    setSending(true);
    setProblem(null);
    // The spaces and empty lines around the note are not sent.
    reportInCommunity(memberId, name, of, reason, note.trim(), block).then(
      () => {
        setSending(false);
        setSent({ blocked: block });
        onSent?.(block);
      },
      (failed: unknown) => {
        setSending(false);
        setProblem(toCommandError(failed).message);
      },
    );
  };
  const tick = (itemId: string, on: boolean) =>
    setIds((before) =>
      on
        ? before.includes(itemId) || before.length >= MAX_REPORT_MESSAGES
          ? before
          : [...before, itemId]
        : before.filter((id) => id !== itemId),
    );

  return (
    <Modal title={`Report ${at}`} onClose={onClose} wide>
      <div className="modal__body report">
        {sent ? (
          <>
            <p role="status">
              <strong>{REPORT_THANKS}</strong>
            </p>
            {sent.blocked && (
              <p>
                {at} is blocked. {UNBLOCK_IN_SETTINGS}
              </p>
            )}
          </>
        ) : (
          <>
            <p className="muted">{REPORT_EXPLAINED}</p>
            {about.length > 1 && (
              <fieldset className="choices report__choices">
                <legend>What are you reporting?</legend>
                {about.map((choice) => (
                  <label key={choice} className="choice">
                    <input
                      type="radio"
                      name={`${groupId}-about`}
                      checked={subject === choice}
                      onChange={() => setSubject(choice)}
                    />
                    <span className="choice__label">{ABOUT_WORDS[choice]}</span>
                  </label>
                ))}
              </fieldset>
            )}
            <fieldset className="choices report__choices report__reasons">
              <legend>What is wrong?</legend>
              {REASONS.map((r) => (
                <label key={r.value} className="choice">
                  <input
                    type="radio"
                    name={`${groupId}-reason`}
                    checked={reason === r.value}
                    onChange={() => setReason(r.value)}
                  />
                  <span className="choice__label">{r.label}</span>
                </label>
              ))}
            </fieldset>
            {subject === "messages" && (
              <fieldset className="report__messages">
                <legend>Which messages?</legend>
                {messages.length === 0 ? (
                  <p className="muted">There are no messages from {at} here to report.</p>
                ) : (
                  <>
                    <p className="muted">
                      Tick the messages to report. You can tick up to {MAX_REPORT_MESSAGES}.
                    </p>
                    <ul className="report__list">
                      {messages.map((m) => {
                        const on = ids.includes(m.itemId);
                        const when = m.acceptedAt ?? m.sentAt;
                        return (
                          <li key={m.itemId}>
                            <label className="report__message">
                              <input
                                type="checkbox"
                                checked={on}
                                disabled={!on && full}
                                onChange={(e) => tick(m.itemId, e.target.checked)}
                              />
                              <span className="report__what">
                                <time dateTime={messageIso(when)} className="report__when">
                                  {messageTime(when)}
                                </time>
                                <span className="message__text report__words">
                                  {m.text !== null && m.text !== "" ? (
                                    <MessageParts text={m.text} />
                                  ) : (
                                    noWords(m)
                                  )}
                                </span>
                              </span>
                            </label>
                          </li>
                        );
                      })}
                    </ul>
                    <p role="status" className="muted">
                      {chosen.length} of {MAX_REPORT_MESSAGES} ticked
                      {full ? ". That is the most one report can carry." : ""}
                    </p>
                  </>
                )}
              </fieldset>
            )}
            <div className="ui-field">
              <label htmlFor={noteId}>Anything else 8 West should know (optional)</label>
              <textarea
                id={noteId}
                value={note}
                rows={4}
                autoComplete="off"
                aria-describedby={countId}
                onChange={(e) => setNote(cutChars(e.target.value, MAX_NOTE_CHARS))}
              />
              <p
                id={countId}
                className={`report__count${count >= MAX_NOTE_CHARS ? " report__count--full" : ""}`}
              >
                {count} of {MAX_NOTE_CHARS}
              </p>
            </div>
            <Checkbox label={`Also block ${at}`} checked={block} onChange={setBlock} />
          </>
        )}
      </div>
      {problem && (
        <p className="form-error" role="alert">
          {problem}
        </p>
      )}
      <footer className="modal__footer">
        {sent ? (
          <Button variant="primary" autoFocus onClick={onClose}>
            Done
          </Button>
        ) : (
          <>
            <Button variant="quiet" onClick={onClose}>
              Cancel
            </Button>
            <Button variant="primary" disabled={!ready} onClick={send}>
              {sending ? "Sending…" : "Send report"}
            </Button>
          </>
        )}
      </footer>
    </Modal>
  );
}
