import { useId, useMemo } from "react";
import type { ConversationSummary } from "@plenipo/types";
import { CountBadge, Icon } from "@plenipo/ui";

import type { Go } from "../components/views";
import { Conversation, type Target } from "./Conversation";
import { changedWords, handle, stateOf, whoWords } from "./messageWords";
import { oneLine } from "./safeText";
import type { useMessages } from "./useMessages";

/** What a row says about a conversation that is not a plain, accepted one (or nothing). */
function standing(state: string): string | null {
  const known = stateOf({ state });
  if (known === "requestedByMe") return "Waiting to be accepted";
  if (known === "leftByMe") return "You left";
  if (known === "leftByThem") return "They left";
  return null;
}

/** One line of the list: who, how many messages are new, and whether their computers changed. */
function Row({
  person,
  open,
  onOpen,
}: {
  person: ConversationSummary;
  open: boolean;
  onOpen: () => void;
}) {
  const display = person.displayName === null ? "" : oneLine(person.displayName.trim());
  const note = standing(person.state);
  return (
    <button
      type="button"
      className="messages__row"
      aria-current={open ? "true" : undefined}
      onClick={onOpen}
    >
      <span className="messages__lines">
        {display && <span className="messages__name">{display}</span>}
        <span className={display ? "messages__handle" : "messages__name"}>
          {handle(person.name)}
        </span>
        {note && <span className="messages__note">{note}</span>}
        {person.computersChanged && (
          <span className="messages__changed">
            <Icon name="alert" size={12} />
            {changedWords(whoWords(person))}
          </span>
        )}
      </span>
      <CountBadge count={person.unseen} label="unseen" />
    </button>
  );
}

/** A titled list of conversations. */
function Group({
  title,
  people,
  openId,
  onOpen,
}: {
  title: string;
  people: ConversationSummary[];
  openId: string | null;
  onOpen: (person: ConversationSummary) => void;
}) {
  return (
    <ul className="messages__group" aria-label={title}>
      {people.map((p) => (
        <li key={p.memberId}>
          <Row person={p} open={p.memberId === openId} onOpen={() => onOpen(p)} />
        </li>
      ))}
    </ul>
  );
}

/**
 * The Messages tab (Phase 24, ADR-164): your conversations, newest first, with **Requests** from
 * people you have not talked with in their own group; and the open conversation beside them. On a
 * narrow window one shows at a time. The conversation can be one that does not exist yet (from a
 * **Message** button on a card): it is opened with its person's ID and name, and the first message
 * makes it.
 */
export function Messages({
  messages,
  target,
  onTarget,
  adult,
  go,
}: {
  messages: ReturnType<typeof useMessages>;
  /** The conversation that is open, if any. */
  target: Target | null;
  onTarget: (target: Target | null) => void;
  /** The viewer is 18 or older. */
  adult: boolean;
  go: Go;
}) {
  const requestsId = useId();
  const listId = useId();
  const { list, error, listAt, reload, changes } = messages;
  const { requests, rest } = useMemo(() => {
    const shown = (list ?? [])
      .filter((p) => stateOf(p) !== "none")
      .sort((a, b) => b.lastAt - a.lastAt);
    return {
      requests: shown.filter((p) => stateOf(p) === "requestedByThem"),
      rest: shown.filter((p) => stateOf(p) !== "requestedByThem"),
    };
  }, [list]);
  const summary = target
    ? ((list ?? []).find((p) => p.memberId === target.memberId) ?? null)
    : null;
  const open = (p: ConversationSummary) => onTarget({ memberId: p.memberId, name: p.name });

  return (
    <div className={`messages${target ? " messages--open" : ""}`}>
      <div className="messages__side">
        {list === null && error === null && (
          <p className="muted" role="status">
            Loading…
          </p>
        )}
        {error !== null && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        {list !== null && requests.length > 0 && (
          <section aria-labelledby={requestsId}>
            <h2 id={requestsId}>Requests</h2>
            <p className="muted">
              People who wrote to you first. They can't send another message until you Accept.
            </p>
            <Group
              title="Requests"
              people={requests}
              openId={target?.memberId ?? null}
              onOpen={open}
            />
          </section>
        )}
        {list !== null && (
          <section aria-labelledby={listId}>
            <h2 id={listId}>Conversations</h2>
            {rest.length > 0 ? (
              <Group
                title="Conversations"
                people={rest}
                openId={target?.memberId ?? null}
                onOpen={open}
              />
            ) : (
              <p className="muted">
                No conversations yet. Find someone on the People tab, then press Message.
              </p>
            )}
          </section>
        )}
      </div>
      <div className="messages__main">
        {target ? (
          <Conversation
            key={target.memberId}
            target={target}
            summary={summary}
            adult={adult}
            listAt={listAt}
            changes={changes}
            go={go}
            onBack={() => onTarget(null)}
            onChanged={reload}
          />
        ) : (
          <p className="muted messages__none">Pick a conversation to read it.</p>
        )}
      </div>
    </div>
  );
}
