import { useCallback, useEffect, useId, useRef, useState } from "react";
import type { BlockedPerson } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { communityBlocked, toCommandError, unblockInCommunity } from "../api/commands";
import { handle, onDate } from "./messageWords";

/** One blocked person: their name, when you blocked them, and **Unblock**. */
function Row({
  person,
  busy,
  onUnblock,
}: {
  person: BlockedPerson;
  busy: boolean;
  onUnblock: (person: BlockedPerson) => void;
}) {
  const nameId = useId();
  return (
    <li className="blocked__row">
      <span className="blocked__who">
        <strong id={nameId}>{handle(person.name)}</strong>
        <span className="muted">Blocked on {onDate(person.blockedAt)}</span>
      </span>
      {/* The button's name is "Unblock"; the name beside it says who. */}
      <Button size="sm" disabled={busy} aria-describedby={nameId} onClick={() => onUnblock(person)}>
        Unblock
      </Button>
    </li>
  );
}

/**
 * Settings → Community → **Blocked** (ADR-167 §3): the people you blocked, each with **Unblock**.
 * It is read when it shows. Names are other people's words: always `handle` (`oneLine`). Links
 * and collaborations that a block ended do not come back by themselves, and it says so.
 */
export function BlockedPeople() {
  const titleId = useId();
  const [people, setPeople] = useState<BlockedPerson[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState<string | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  // An answer that comes after this has gone is dropped.
  const alive = useRef(true);
  const read = useCallback(() => {
    communityBlocked().then(
      (list) => {
        if (alive.current) setPeople(list);
      },
      (reason: unknown) => {
        if (alive.current) setError(toCommandError(reason).message);
      },
    );
  }, []);
  useEffect(() => {
    alive.current = true;
    read();
    return () => {
      alive.current = false;
    };
  }, [read]);
  const retry = () => {
    setError(null);
    read();
  };

  const unblock = (person: BlockedPerson) => {
    setWorking(person.memberId);
    setError(null);
    setSaid(null);
    unblockInCommunity(person.memberId)
      .then(() => {
        setPeople((before) => before?.filter((p) => p.memberId !== person.memberId) ?? null);
        setSaid(
          `Unblocked ${handle(person.name)}. Links the block ended don't come back by themselves.`,
        );
      })
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setWorking(null));
  };

  return (
    <section className="blocked" aria-labelledby={titleId}>
      <h3 id={titleId}>Blocked</h3>
      <p className="muted">
        People you blocked can't message you, find your card, or link with you. They aren't told.
      </p>
      {people === null && error === null && (
        <p className="muted" role="status">
          Looking…
        </p>
      )}
      {people !== null && people.length === 0 && <p>No one is blocked.</p>}
      {people !== null && people.length > 0 && (
        <ul className="blocked__list" aria-label="People you blocked">
          {people.map((p) => (
            <Row key={p.memberId} person={p} busy={working !== null} onUnblock={unblock} />
          ))}
        </ul>
      )}
      {said && (
        <p role="status" className="muted">
          {said}
        </p>
      )}
      {error && (
        <div className="form-error" role="alert">
          <p>{error}</p>
          {people === null && (
            <Button size="sm" onClick={retry}>
              Try again
            </Button>
          )}
        </div>
      )}
    </section>
  );
}
