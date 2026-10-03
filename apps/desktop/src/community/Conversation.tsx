import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import type { ConversationSummary, ConversationView, MessageView } from "@plenipo/types";
import { Button, Icon } from "@plenipo/ui";

import {
  acceptCommunityRequest,
  communityConversation,
  communitySafetyCodeChecked,
  deleteCommunityMessage,
  leaveCommunityConversation,
  openCommunityLink,
  reactInCommunity,
  sendCommunityMessage,
  toCommandError,
} from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import type { Go } from "../components/views";
import { Composer, type SendProblem } from "./Composer";
import { GiveToWorkerDialog } from "./GiveToWorker";
import { MessageItem } from "./MessageItem";
import {
  UNDER_18_NOTE,
  changedWords,
  deleteWords,
  describeMessage,
  handle,
  leaveWords,
  safetyWords,
  sealedWords,
  stateOf,
  whoWords,
} from "./messageWords";
import { oneLine } from "./safeText";

/** Who a conversation is with, when it is opened: it may not exist on this PC yet. */
export interface Target {
  memberId: string;
  /** Their Community name, without the "@". */
  name: string;
}

/**
 * Opening a conversation tells every window ("messages changed", so the unseen counts move). So a
 * change that arrives right after this window read the conversation is most likely that very
 * echo, and is not read again: or two windows, each answering the other's echo, would never stop.
 * The list is read for every change, and it tells when something is really new (`behind`).
 */
export const QUIET_MS = 2000;

/** The most times in a row the list may send the conversation to be read again (a safety stop). */
const MOST_CATCH_UPS = 3;
const CATCH_UP_SPAN_MS = 5000;

/** What is kept of an open conversation: its newest page, the older pages, and how reading goes. */
function useConversation(
  memberId: string,
  summary: ConversationSummary | null,
  listAt: number,
  changes: number,
) {
  const [view, setView] = useState<ConversationView | null | undefined>(undefined);
  const [older, setOlder] = useState<MessageView[]>([]);
  const [noMoreOlder, setNoMoreOlder] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // A read is going on, when the last one began and ended, and an answer's place in line.
  const reading = useRef(0);
  const readStarted = useRef(0);
  const readEnded = useRef(0);
  const turn = useRef(0);
  const alive = useRef(true);
  const catchUps = useRef<number[]>([]);
  // Counts the reads that ended, so the list's word is weighed again after each.
  const [ended, setEnded] = useState(0);

  const track = useCallback(<T,>(read: Promise<T>): Promise<T> => {
    reading.current += 1;
    readStarted.current = Date.now();
    return read.finally(() => {
      reading.current -= 1;
      readEnded.current = Date.now();
      if (alive.current) setEnded((n) => n + 1);
    });
  }, []);

  /** Read the newest page (and mark it seen). */
  const load = useCallback(() => {
    const mine = ++turn.current;
    return track(communityConversation(memberId, null)).then(
      (next) => {
        if (!alive.current || mine !== turn.current) return;
        setView(next);
        setError(null);
      },
      (reason: unknown) => {
        if (alive.current && mine === turn.current) setError(toCommandError(reason).message);
      },
    );
  }, [memberId, track]);

  useEffect(() => {
    alive.current = true;
    void load();
    return () => {
      alive.current = false;
    };
  }, [load]);

  // A change was heard: read again, unless this window just read (see `QUIET_MS`).
  const heard = useRef(changes);
  useEffect(() => {
    if (heard.current === changes) return;
    heard.current = changes;
    if (reading.current > 0 || Date.now() - readEnded.current < QUIET_MS) return;
    void load();
  }, [changes, load]);

  // The list says something the open conversation does not show: a message you have not seen, a
  // later time, or another state. Only a list read after this conversation was read counts: an
  // older one is simply behind.
  useEffect(() => {
    if (summary === null || view === undefined || view === null || reading.current > 0) return;
    if (listAt < readStarted.current) return;
    const shown = view.person;
    const behind =
      summary.unseen > 0 ||
      summary.lastAt > shown.lastAt ||
      summary.state !== shown.state ||
      summary.computersChanged !== shown.computersChanged;
    if (!behind) return;
    const now = Date.now();
    catchUps.current = catchUps.current.filter((at) => now - at < CATCH_UP_SPAN_MS);
    if (catchUps.current.length >= MOST_CATCH_UPS) return;
    catchUps.current.push(now);
    void load();
  }, [summary, view, listAt, ended, load]);

  const messages = useMemo(() => {
    const newest = view?.messages ?? [];
    const ids = new Set(newest.map((m) => m.itemId));
    return [...older.filter((m) => !ids.has(m.itemId)), ...newest];
  }, [view, older]);

  /** Read the page before the oldest message shown. */
  const loadOlder = useCallback(() => {
    const first = messages[0];
    if (!first) return;
    setLoadingOlder(true);
    track(communityConversation(memberId, first.itemId))
      .then((page) => {
        if (!alive.current) return;
        const got = page?.messages ?? [];
        setOlder((before) => {
          const have = new Set(before.map((m) => m.itemId));
          return [...got.filter((m) => !have.has(m.itemId)), ...before];
        });
        if (got.length === 0) setNoMoreOlder(true);
        setError(null);
      })
      .catch((reason: unknown) => {
        if (alive.current) setError(toCommandError(reason).message);
      })
      .finally(() => {
        if (alive.current) setLoadingOlder(false);
      });
  }, [memberId, messages, track]);

  return {
    view,
    messages,
    error,
    load,
    loadOlder,
    loadingOlder,
    noMoreOlder,
    /** A message deleted from this PC leaves the older pages too. */
    forget: (itemId: string) => setOlder((before) => before.filter((m) => m.itemId !== itemId)),
    forgetAll: () => setOlder([]),
  };
}

/** What a conversation is asking about, in a window over it. */
type Dialog =
  | { kind: "delete"; message: MessageView }
  | { kind: "give"; message: MessageView }
  | { kind: "leave" }
  | { kind: "link"; address: string };

/** Where the other person stands, as far as this PC knows, before it has heard of them. */
function unknownPerson(target: Target): ConversationSummary {
  return {
    memberId: target.memberId,
    name: target.name,
    displayName: null,
    state: "none",
    unseen: 0,
    lastAt: 0,
    computersChanged: false,
  };
}

/**
 * One conversation (Phase 24, ADR-164): who it is with, the safety code, the messages (oldest
 * first, as safe text), what you can do with each, and the box to write in. It is read when it
 * opens, which marks its messages seen.
 *
 * What it offers depends on where it stands: a **request** from someone new has **Accept** and
 * **Leave this conversation**; your own request waits for them; after a leave the box is off or on
 * as the sentences below say.
 */
export function Conversation({
  target,
  summary,
  adult,
  listAt,
  changes,
  go,
  onBack,
  onChanged,
}: {
  target: Target;
  /** The list's line for this person, when there is one. */
  summary: ConversationSummary | null;
  /** The viewer is 18 or older (a member under 18 is warned about people they don't know). */
  adult: boolean;
  /** When the list was last read (Date.now()), and how many times messages changed. */
  listAt: number;
  changes: number;
  go: Go;
  /** Back to the list (on a narrow window the list is hidden while a conversation is open). */
  onBack: () => void;
  /** Something changed that the list should read again. */
  onChanged: () => void;
}) {
  const c = useConversation(target.memberId, summary, listAt, changes);
  const titleId = useId();
  const safetyId = useId();
  const person = c.view?.person ?? summary ?? unknownPerson(target);
  const state = stateOf(person);
  const who = whoWords(person);
  const at = handle(person.name);
  const display = person.displayName === null ? "" : oneLine(person.displayName.trim());

  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [replyTo, setReplyTo] = useState<MessageView | null>(null);
  const [showCode, setShowCode] = useState(false);
  const [matched, setMatched] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [given, setGiven] = useState<string | null>(null);

  const known = c.view !== undefined || summary !== null;
  const canWrite = known && (state === "none" || state === "accepted" || state === "leftByMe");
  const request = state === "requestedByThem";

  // What changed (the list reads again too).
  const after = () => {
    void c.load();
    onChanged();
  };
  /** Do something to this conversation; say why in plain words if it can't be done. */
  const act = (work: () => Promise<void>, done?: () => void) => {
    setProblem(null);
    setGiven(null);
    work().then(
      () => {
        done?.();
        after();
      },
      (reason: unknown) => setProblem(toCommandError(reason).message),
    );
  };

  const send = async (text: string): Promise<SendProblem | null> => {
    try {
      await sendCommunityMessage(person.memberId, person.name, text, replyTo?.itemId ?? null);
      setReplyTo(null);
      setProblem(null);
      setGiven(null);
      after();
      return null;
    } catch (reason) {
      const failed = toCommandError(reason);
      return { message: failed.message, partOfPro: failed.kind === "partOfPro" };
    }
  };

  const byId = useMemo(() => new Map(c.messages.map((m) => [m.itemId, m])), [c.messages]);
  const quote = (m: MessageView): string | null | undefined => {
    if (m.replyTo === null) return undefined;
    const answered = byId.get(m.replyTo);
    return answered ? describeMessage(answered) : null;
  };

  // The newest message comes into view when the conversation opens and when one arrives.
  const end = useRef<HTMLDivElement>(null);
  const newest = c.messages[c.messages.length - 1]?.itemId ?? null;
  useEffect(() => {
    if (newest !== null) end.current?.scrollIntoView?.({ block: "end" });
  }, [newest]);

  const code = c.view?.safetyCode ?? null;
  const leave = (
    <Button variant="quiet" onClick={() => setDialog({ kind: "leave" })}>
      Leave this conversation
    </Button>
  );

  return (
    <section className="conversation" aria-labelledby={titleId}>
      <header className="conversation__header">
        <Button
          size="sm"
          variant="quiet"
          icon="chevronLeft"
          className="messages__back"
          onClick={onBack}
        >
          All messages
        </Button>
        <div className="conversation__who">
          <h2 id={titleId}>{display || at}</h2>
          {display && <p className="conversation__handle">{at}</p>}
        </div>
        <div className="conversation__tools">
          <Button
            aria-expanded={showCode}
            aria-controls={showCode ? safetyId : undefined}
            onClick={() => setShowCode((s) => !s)}
          >
            Check the safety code
          </Button>
          {(state === "accepted" || state === "requestedByMe" || state === "leftByThem") && leave}
        </div>
      </header>

      {showCode && (
        <div id={safetyId} className="conversation__safety">
          {code === null ? (
            <p>The safety code isn't ready yet. It comes once you and {who} can message.</p>
          ) : (
            <>
              <p className="conversation__code" aria-label={`Safety code ${code}`}>
                {oneLine(code)}
              </p>
              <p>{safetyWords(who)}</p>
              <div className="settings-section__actions">
                <Button
                  variant="primary"
                  onClick={() => {
                    setShowCode(false);
                    act(
                      () => communitySafetyCodeChecked(person.memberId),
                      () => setMatched(true),
                    );
                  }}
                >
                  It matches
                </Button>
              </div>
            </>
          )}
        </div>
      )}
      {matched && !showCode && (
        <p className="muted" role="status">
          Thank you. Plenipo tells you if {who}'s computers change again.
        </p>
      )}
      {person.computersChanged && (
        <p className="notice-box" role="note">
          <strong>{changedWords(who)}</strong>. Check the safety code again.
        </p>
      )}
      <p className="conversation__sealed">
        <Icon name="lock" size={14} />
        <strong>{sealedWords(who)}</strong>
      </p>

      {problem && (
        <p className="form-error" role="alert">
          {problem}
        </p>
      )}
      {given && (
        <p role="status" className="muted">
          Given to {oneLine(given)}. It was marked as outside words.
        </p>
      )}

      {request && !adult && (
        <p className="notice-box notice-box--danger" role="note">
          <strong>{UNDER_18_NOTE}</strong>
        </p>
      )}

      <div className="conversation__messages">
        {c.view === undefined && c.error === null && (
          <p className="muted" role="status">
            Loading…
          </p>
        )}
        {c.error !== null && (
          <div className="form-error" role="alert">
            <p>{c.error}</p>
            <Button size="sm" onClick={() => void c.load()}>
              Try again
            </Button>
          </div>
        )}
        {c.messages.length > 0 && !c.noMoreOlder && (
          <div className="settings-section__actions">
            <Button disabled={c.loadingOlder} onClick={c.loadOlder}>
              Load older
            </Button>
          </div>
        )}
        {c.noMoreOlder && <p className="muted">That is every message that is kept here.</p>}
        {c.view !== undefined &&
          c.messages.length === 0 &&
          state !== "requestedByMe" &&
          !request && <p className="muted">No messages yet.</p>}
        {c.messages.length > 0 && (
          <ol className="conversation__items" aria-label="Messages">
            {c.messages.map((m) => (
              <MessageItem
                key={m.itemId}
                message={m}
                who={who}
                quote={quote(m)}
                canWrite={canWrite}
                onReact={(message, emoji) => act(() => reactInCommunity(message.itemId, emoji))}
                onReply={(message) => setReplyTo(message)}
                onDelete={(message) => setDialog({ kind: "delete", message })}
                onGive={(message) => setDialog({ kind: "give", message })}
                onOpenLink={(address) => setDialog({ kind: "link", address })}
              />
            ))}
          </ol>
        )}
        <div ref={end} />
      </div>

      {request && (
        <div className="conversation__request">
          <Button
            variant="primary"
            onClick={() => act(() => acceptCommunityRequest(person.memberId))}
          >
            Accept
          </Button>
          {leave}
        </div>
      )}
      {state === "requestedByMe" && (
        <p className="conversation__state" role="status">
          Waiting for {at} to accept your first message.
        </p>
      )}
      {state === "leftByMe" && (
        <p className="conversation__state" role="status">
          You left this conversation. Writing again opens it on your side.
        </p>
      )}
      {state === "leftByThem" && (
        <p className="conversation__state" role="status">
          {at} left this conversation. Your messages won't be delivered.
        </p>
      )}
      {known && state === "none" && c.messages.length === 0 && (
        <p className="muted">
          Your first message is a request. {who} can answer once they accept it.
        </p>
      )}
      {canWrite && (
        <Composer
          who={person}
          reply={replyTo}
          onCancelReply={() => setReplyTo(null)}
          onSend={send}
          onLicense={() => go({ view: "settings", id: "license" })}
        />
      )}

      {dialog?.kind === "delete" && (
        <ConfirmDialog
          title="Delete this message?"
          message={<p>{deleteWords(who)}</p>}
          confirmLabel="Delete for me"
          danger
          onCancel={() => setDialog(null)}
          onConfirm={async () => {
            const gone = dialog.message.itemId;
            try {
              await deleteCommunityMessage(gone);
            } catch (reason) {
              return toCommandError(reason).message;
            }
            c.forget(gone);
            if (replyTo?.itemId === gone) setReplyTo(null);
            setDialog(null);
            after();
            return null;
          }}
        />
      )}
      {dialog?.kind === "leave" && (
        <ConfirmDialog
          title="Leave this conversation?"
          message={<p>{leaveWords(who)}</p>}
          confirmLabel="Leave this conversation"
          danger
          onCancel={() => setDialog(null)}
          onConfirm={async () => {
            try {
              await leaveCommunityConversation(person.memberId);
            } catch (reason) {
              return toCommandError(reason).message;
            }
            c.forgetAll();
            setReplyTo(null);
            setDialog(null);
            after();
            return null;
          }}
        />
      )}
      {dialog?.kind === "link" && (
        <ConfirmDialog
          title="Open this link in your web browser?"
          message={
            <>
              <p>
                <code className="message__address">{dialog.address}</code>
              </p>
              <p className="muted">
                It opens in your own web browser, never inside Plenipo. Only open it if you trust{" "}
                {who}.
              </p>
            </>
          }
          confirmLabel="Yes, open it"
          onCancel={() => setDialog(null)}
          onConfirm={async () => {
            try {
              await openCommunityLink(dialog.address);
            } catch (reason) {
              return toCommandError(reason).message;
            }
            setDialog(null);
            return null;
          }}
        />
      )}
      {dialog?.kind === "give" && (
        <GiveToWorkerDialog
          itemId={dialog.message.itemId}
          text={dialog.message.text}
          onCancel={() => setDialog(null)}
          onGiven={(title) => {
            setDialog(null);
            setGiven(title);
          }}
        />
      )}
    </section>
  );
}
