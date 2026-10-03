import { useEffect, useId, useState } from "react";
import type { CardView } from "@plenipo/types";
import { Icon } from "@plenipo/ui";

import { OwnerLight } from "../owner/OwnerFace";
import { MOOD_FACES, MOOD_WORDS } from "../owner/words";
import {
  badgeWords,
  cardStatus,
  knownMood,
  lightWords,
  pointsWords,
  thankedWords,
} from "./peopleWords";
import { loadPicture, peekPicture, pictureKey } from "./pictures";
import { kindLabel, regionLabel } from "./profileWords";
import { oneLine } from "./safeText";

/** One line of another person's words, trimmed and made safe; empty when there is nothing. */
function line(text: string | null): string {
  return text === null ? "" : oneLine(text.trim());
}

/**
 * A member's picture, as the account service sent it and Plenipo checked it. It is asked for only
 * when the card says there is one (`hasPicture`), once for each member and picture version. The
 * result is shown only as `data:image/png;base64,…`: never any other address.
 */
function Picture({ card }: { card: CardView }) {
  const key = pictureKey(card);
  const memberId = card.memberId;
  const [came, setCame] = useState<{ key: string; picture: string | null } | null>(null);
  useEffect(() => {
    if (key === null) return;
    let alive = true;
    loadPicture(memberId, key).then(
      (picture) => {
        if (alive) setCame({ key, picture });
      },
      // A picture that did not come leaves the plain person-shaped mark.
      () => undefined,
    );
    return () => {
      alive = false;
    };
  }, [memberId, key]);
  const picture =
    key === null ? null : came?.key === key ? came.picture : (peekPicture(key) ?? null);
  return picture ? (
    <img
      className="people-card__picture"
      src={`data:image/png;base64,${picture}`}
      alt=""
      width={56}
      height={56}
      draggable={false}
    />
  ) : (
    <span className="people-card__picture people-card__picture--none" aria-hidden="true">
      <Icon name="user" size={28} />
    </span>
  );
}

/**
 * One person in Community (Phase 24, ADR-163 §4, ADR-169): picture, names, status, mood, message,
 * company, what the business does, where, badges, points, and thanks. A part the person hides is
 * simply not there. Everything another person wrote is plain text on one line (`oneLine`): a
 * hidden character shows as a mark, and nothing is ever a web page, a link, or a button.
 */
export function CommunityCard({ card }: { card: CardView }) {
  const nameId = useId();
  const name = oneLine(card.name);
  const display = line(card.displayName);
  const status = cardStatus(card.status);
  const mood = knownMood(card.mood);
  const message = line(card.message);
  const company = line(card.company);
  const kinds = card.businessKinds.flatMap((kind) => {
    const label = kindLabel(kind);
    return label ? [label] : [];
  });
  const about = line(card.businessLine);
  const place = card.region !== null && card.region !== "" ? oneLine(regionLabel(card.region)) : "";
  const badges = [
    ...new Set(
      card.badges.flatMap((badge) => {
        const words = badgeWords(badge);
        return words ? [words] : [];
      }),
    ),
  ];
  const thanked = thankedWords(card.thankedBy);

  return (
    <article className="people-card" aria-labelledby={nameId}>
      <Picture card={card} />
      <div className="people-card__lines">
        {display && <p className="people-card__name">{display}</p>}
        <p id={nameId} className={display ? "people-card__handle" : "people-card__name"}>
          @{name}
        </p>
        {(status || mood) && (
          <p className="people-card__tile">
            {status?.kind === "light" && (
              <span className="people-card__item">
                <OwnerLight status={status.status} />
                <span>{lightWords(status.status)}</span>
              </span>
            )}
            {status?.kind === "offline" && <span className="people-card__item">Offline</span>}
            {mood && (
              <span className="people-card__item">
                <span aria-hidden="true">{MOOD_FACES[mood]}</span>
                <span>{MOOD_WORDS[mood]}</span>
              </span>
            )}
          </p>
        )}
        {message && <p className="people-card__message">{message}</p>}
        {company && <p className="people-card__company">{company}</p>}
        {kinds.length > 0 && <p className="people-card__business">{kinds.join(", ")}</p>}
        {about && <p className="people-card__line">{about}</p>}
        {place && <p className="people-card__place">{place}</p>}
        {badges.length > 0 && (
          <ul className="people-card__badges" aria-label="Badges">
            {badges.map((badge) => (
              <li key={badge}>{badge}</li>
            ))}
          </ul>
        )}
        <p className="people-card__points">
          <span>{pointsWords(card.points)}</span>
          {thanked && <span>{thanked}</span>}
        </p>
      </div>
    </article>
  );
}
