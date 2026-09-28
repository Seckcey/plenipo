import type { CSSProperties } from "react";
import type { OwnerProfile, OwnerStatus } from "@plenipo/types";
import { cx } from "@plenipo/ui";

import { Glyph } from "../components/org/Glyph";
import { MOOD_FACES, MOOD_WORDS, STATUS_WORDS } from "./words";

/**
 * A status light. Each status has its own shape as well as its color (Available ●, Busy ■,
 * Away ◐, Do not disturb ⊖), and its word is always given: beside it, or as its name.
 */
export function OwnerLight({
  status,
  named = false,
  className,
}: {
  status: OwnerStatus;
  /** Alone (no word beside it): the light says its word to screen readers and as a tooltip. */
  named?: boolean;
  className?: string;
}) {
  const word = STATUS_WORDS[status];
  return (
    <span
      className={cx("owner-light", `owner-light--${status}`, className)}
      data-status={status}
      {...(named ? { role: "img", "aria-label": word, title: word } : { "aria-hidden": true })}
    />
  );
}

/**
 * Your picture (or, without one, the owner glyph) in a circle, with your status light in its
 * corner. Only props: the canvas's tile and the top bar's button both use it.
 */
export function OwnerFace({
  profile,
  size = 32,
  light = true,
  className,
}: {
  profile: OwnerProfile | null;
  /** The circle's size, in pixels. */
  size?: number;
  /** Show the status light in the corner (once your tile is loaded). */
  light?: boolean;
  className?: string;
}) {
  const picture = profile?.picture;
  return (
    <span
      className={cx("owner-face", className)}
      style={{ "--owner-face-size": `${size}px` } as CSSProperties}
    >
      {picture ? (
        <img
          className="owner-face__picture"
          src={`data:image/png;base64,${picture}`}
          alt="Your picture"
          width={size}
          height={size}
          draggable={false}
        />
      ) : (
        <span className="owner-face__glyph">
          <Glyph name="owner" size={Math.max(12, Math.round(size * 0.6))} />
        </span>
      )}
      {light && profile && (
        <OwnerLight status={profile.status} named className="owner-face__light" />
      )}
    </span>
  );
}

/**
 * Your status light and word, your mood's face and word (if you chose one), and your message (if
 * you wrote one), for your tile on the canvas. Nothing is shown by color or face alone. Only
 * `<span>`s, so it fits inside a button.
 */
export function OwnerStatusLine({
  profile,
  className,
}: {
  profile: OwnerProfile | null;
  className?: string;
}) {
  if (!profile) return null;
  const { status, mood, message } = profile;
  return (
    <span className={cx("owner-status", className)}>
      <span className="owner-status__item owner-status__status">
        <OwnerLight status={status} />
        <span>{STATUS_WORDS[status]}</span>
      </span>
      {mood && (
        <span className="owner-status__item owner-status__mood" data-mood={mood}>
          <span className="owner-status__face" aria-hidden="true">
            {MOOD_FACES[mood]}
          </span>
          <span>{MOOD_WORDS[mood]}</span>
        </span>
      )}
      {message && (
        <span className="owner-status__message" title={message}>
          {message}
        </span>
      )}
    </span>
  );
}
