import { useId } from "react";
import type { ProfileDraft, ProfileShown } from "@plenipo/types";
import { Checkbox, Select } from "@plenipo/ui";

import { useOwnerProfile } from "../owner/context";
import { OwnerLight } from "../owner/OwnerFace";
import { MOOD_FACES, MOOD_WORDS, STATUS_WORDS } from "../owner/words";
import {
  BUSINESS_KINDS,
  HIDDEN_PART_WORDS,
  hiddenNote,
  kindLabel,
  MOST_KINDS,
  MOST_LINE,
  MOST_NAME,
  PARTS,
  regionLabel,
  regionOptions,
} from "./profileWords";
import { oneLine } from "./safeText";

/** One line you type: a control character (a line break, a tab) becomes a space. */
function typedLine(text: string): string {
  let out = "";
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    out += code < 0x20 || (code >= 0x7f && code <= 0x9f) ? " " : ch;
  }
  return out;
}

/** A text box with its label and a short hint under it. */
function Line({
  label,
  hint,
  value,
  most,
  disabled,
  onChange,
}: {
  label: string;
  hint?: string;
  value: string;
  most: number;
  disabled: boolean;
  onChange: (next: string) => void;
}) {
  const id = useId();
  return (
    <div className="ui-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="text"
        value={value}
        maxLength={most}
        disabled={disabled}
        autoComplete="off"
        aria-describedby={hint ? `${id}-hint` : undefined}
        onChange={(e) => onChange(typedLine(e.target.value))}
      />
      {hint && (
        <div id={`${id}-hint`} className="ui-field__hint">
          {hint}
        </div>
      )}
    </div>
  );
}

/**
 * **What people see** (Phase 24, ADR-163 §1, §2): the card others will see, and the boxes that
 * choose its parts. The picture, status, mood, and message come from your tile (the You button),
 * so only your name, company, what your business does, and where are typed here. A part that is
 * unticked or empty is not on the card at all. Everything typed or kept is shown as plain text,
 * on one line, with any hidden character made visible (`oneLine`).
 *
 * The screen only holds `draft`; the one who uses it keeps it and sends it.
 */
export function WhatPeopleSee({
  draft,
  onChange,
  disabled = false,
  hiddenParts = [],
  level = 3,
}: {
  draft: ProfileDraft;
  onChange: (next: ProfileDraft) => void;
  /** Everything is turned off (while something is being saved). */
  disabled?: boolean;
  /** Parts 8 West hid (`picture`, `display_name`, `message`, `company`, `business_line`). */
  hiddenParts?: readonly string[];
  /** The heading level of the card's title: one below the section it sits in. */
  level?: 3 | 4;
}) {
  const { profile: tile } = useOwnerProfile();
  const titleId = useId();
  const kindsId = useId();
  const kindsHintId = `${kindsId}-hint`;
  const Title = level === 3 ? "h3" : "h4";
  const { shown } = draft;
  const set = (patch: Partial<ProfileDraft>) => onChange({ ...draft, ...patch });
  const setShown = (key: keyof ProfileShown, on: boolean) =>
    set({ shown: { ...shown, [key]: on } });

  const hid = (part: string) => hiddenParts.includes(part);
  const notes = [...HIDDEN_PART_WORDS].filter(([part]) => hid(part));

  const chosen = draft.businessKinds;
  const full = chosen.length >= MOST_KINDS;
  const toggleKind = (value: string, on: boolean) =>
    set({
      businessKinds: on ? [...chosen, value] : chosen.filter((k) => k !== value),
    });

  // The card: only what is ticked and not empty (and not hidden by 8 West).
  const picture = shown.picture && !hid("picture") ? (tile?.picture ?? null) : null;
  const name = shown.name && !hid("display_name") ? oneLine(draft.displayName.trim()) : "";
  // Do not disturb is shown to others as Busy (ADR-163 §8).
  const status =
    shown.status && tile ? (tile.status === "doNotDisturb" ? "busy" : tile.status) : null;
  const mood = shown.mood && tile?.mood ? tile.mood : null;
  const message = shown.message && !hid("message") && tile ? oneLine(tile.message.trim()) : "";
  const company = shown.company && !hid("company") ? oneLine(draft.company.trim()) : "";
  const kinds = shown.business
    ? chosen.flatMap((k) => {
        const label = kindLabel(k);
        return label ? [label] : [];
      })
    : [];
  const line = shown.business && !hid("business_line") ? oneLine(draft.businessLine.trim()) : "";
  const place = shown.region && draft.region !== "" ? oneLine(regionLabel(draft.region)) : "";
  const empty =
    !picture &&
    !name &&
    !status &&
    !mood &&
    !message &&
    !company &&
    !kinds.length &&
    !line &&
    !place;

  return (
    <div className="what-people-see">
      {/* One fieldset turns every box off together, even the ones that cannot turn themselves off. */}
      <fieldset className="what-people-see__editor" disabled={disabled}>
        <Line
          label="Your name"
          value={draft.displayName}
          most={MOST_NAME}
          disabled={disabled}
          onChange={(displayName) => set({ displayName })}
        />
        <Line
          label="Company"
          value={draft.company}
          most={MOST_LINE}
          disabled={disabled}
          onChange={(company) => set({ company })}
        />
        <fieldset className="what-people-see__kinds" aria-describedby={kindsHintId}>
          <legend>What your business does</legend>
          <p id={kindsHintId} className="ui-field__hint">
            Choose up to {MOST_KINDS}.
          </p>
          <div className="what-people-see__kind-list">
            {BUSINESS_KINDS.map((kind) => {
              const on = chosen.includes(kind.value);
              return (
                <Checkbox
                  key={kind.value}
                  label={kind.label}
                  checked={on}
                  disabled={disabled || (full && !on)}
                  onChange={(next) => toggleKind(kind.value, next)}
                />
              );
            })}
          </div>
        </fieldset>
        <Line
          label="In your own words"
          hint="One line about what your business does."
          value={draft.businessLine}
          most={MOST_LINE}
          disabled={disabled}
          onChange={(businessLine) => set({ businessLine })}
        />
        <div className="what-people-see__where">
          <Select
            label="Where"
            value={draft.region}
            options={regionOptions(draft.region)}
            onChange={(region) => set({ region })}
          />
          <div className="ui-field__hint">A state or a country. Never a town or an address.</div>
        </div>
        <fieldset className="what-people-see__parts">
          <legend>Tick what you want people to see</legend>
          <div className="what-people-see__part-list">
            {PARTS.map((part) => (
              <Checkbox
                key={part.key}
                label={part.label}
                checked={shown[part.key]}
                disabled={disabled}
                onChange={(on) => setShown(part.key, on)}
              />
            ))}
          </div>
          <p className="ui-field__hint">
            Your picture, status, mood, and message come from the You button at the top.
          </p>
        </fieldset>
      </fieldset>

      {notes.map(([part, word]) => (
        <p key={part} className="notice-box" role="note">
          {hiddenNote(word)}
        </p>
      ))}

      <section className="what-people-see__preview" aria-labelledby={titleId}>
        <Title id={titleId} className="what-people-see__title">
          What people see
        </Title>
        {empty ? (
          <p className="muted">Nothing to show yet. Tick a box or fill in a part to see it here.</p>
        ) : (
          <div className="what-people-see__card">
            {picture && (
              <img
                className="what-people-see__picture"
                src={`data:image/png;base64,${picture}`}
                alt="Your picture"
                width={56}
                height={56}
                draggable={false}
              />
            )}
            <div className="what-people-see__lines">
              {name && <p className="what-people-see__name">{name}</p>}
              {(status || mood) && (
                <p className="what-people-see__tile">
                  {status && (
                    <span className="what-people-see__item">
                      <OwnerLight status={status} />
                      <span>{STATUS_WORDS[status]}</span>
                    </span>
                  )}
                  {mood && (
                    <span className="what-people-see__item">
                      <span aria-hidden="true">{MOOD_FACES[mood]}</span>
                      <span>{MOOD_WORDS[mood]}</span>
                    </span>
                  )}
                </p>
              )}
              {message && <p className="what-people-see__message">{message}</p>}
              {company && <p className="what-people-see__company">{company}</p>}
              {kinds.length > 0 && <p className="what-people-see__business">{kinds.join(", ")}</p>}
              {line && <p className="what-people-see__line">{line}</p>}
              {place && <p className="what-people-see__place">{place}</p>}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
