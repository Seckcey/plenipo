import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type ChangeEvent,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import type {
  Mood,
  OwnerProfile,
  OwnerProfileInput,
  OwnerStatus,
  PictureChange,
} from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { toCommandError } from "../api/commands";
import { useOwnerProfile } from "./context";
import { OwnerFace, OwnerLight } from "./OwnerFace";
import { CANNOT_READ, PICTURE_ACCEPT, PictureError, shrinkToPng } from "./picture";
import {
  doNotDisturbHint,
  MAX_MESSAGE,
  MOOD_FACES,
  MOOD_ORDER,
  MOOD_WORDS,
  NO_MOOD_WORD,
  pictureNote,
  READING_DETAILS,
  STATUS_ORDER,
  STATUS_WORDS,
  cannotReadDetails,
  describeOwner,
  oneLineMessage,
} from "./words";

export const OWNER_BUTTON_ID = "owner-button";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * What Tab can reach in the panel, in order: not the hidden file input, nothing turned off, and
 * of each group of choices only the chosen one (Tab goes to it; the arrow keys move in the group).
 */
function focusables(root: HTMLElement | null): HTMLElement[] {
  return [...(root?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [])].filter(
    (el) =>
      el.tabIndex >= 0 &&
      !el.closest("[hidden]") &&
      !el.matches(":disabled") &&
      !(el instanceof HTMLInputElement && el.type === "radio" && !el.checked),
  );
}

/**
 * Your button in the top bar (Phase 18, ADR-056): your picture with its status light. It opens a
 * small panel to change your picture, status, mood, and message.
 */
export function OwnerButton() {
  const { profile, save, reload, loadError } = useOwnerProfile();
  const [open, setOpen] = useState(false);
  // While Save is on its way, the panel stays open: only Plenipo's answer closes it.
  const [saving, setSaving] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const panelId = useId();

  const close = useCallback((refocus: boolean) => {
    setOpen(false);
    setSaving(false);
    if (refocus) button.current?.focus();
  }, []);

  const toggle = () => {
    if (open) {
      if (!saving) close(false);
      return;
    }
    setOpen(true);
    // Your details are not loaded yet (or reading them failed): read them again.
    if (!profile) reload?.();
  };

  // A click outside the panel closes it, like Cancel (the focus stays where you clicked).
  useEffect(() => {
    if (!open || saving) return;
    const outside = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open, saving]);

  const now = profile
    ? `${STATUS_WORDS[profile.status]}${profile.mood ? `, feeling ${MOOD_WORDS[profile.mood]}` : ""}`
    : null;
  const label = `You${now ? `: ${now}` : ""} — change your picture, status, mood, and message`;

  return (
    <div className="owner-button" ref={root}>
      <button
        ref={button}
        id={OWNER_BUTTON_ID}
        type="button"
        className="ui-icon-button owner-button__open"
        aria-label={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        title={profile ? `You: ${describeOwner(profile)}` : "You"}
        onClick={toggle}
      >
        <OwnerFace profile={profile} size={22} />
      </button>
      {open && (
        <OwnerPanel
          id={panelId}
          profile={profile}
          loadError={loadError ?? null}
          reload={reload}
          save={save}
          pending={saving}
          onPending={setSaving}
          onClose={close}
        />
      )}
    </div>
  );
}

/** What you changed in the panel. The rest shows your details as kept, even as they arrive. */
interface Edits {
  status?: OwnerStatus;
  mood?: Mood | null;
  message?: string;
}

/**
 * The panel: your picture, status, mood, and message, kept by Save. Until your details are read,
 * Save is off (it would replace what is kept with the panel's starting choices); when they
 * arrive, they fill in what you have not changed yet.
 */
function OwnerPanel({
  id,
  profile,
  loadError,
  reload,
  save,
  pending,
  onPending,
  onClose,
}: {
  id: string;
  profile: OwnerProfile | null;
  /** Why your details could not be read (while `profile` is `null`). */
  loadError: string | null;
  reload: (() => void) | undefined;
  save: (input: OwnerProfileInput) => Promise<OwnerProfile>;
  /** Save is on its way: Cancel, Escape, and clicks outside wait for it. */
  pending: boolean;
  onPending: (pending: boolean) => void;
  /** `true`: the focus goes back to the button. */
  onClose: (refocus: boolean) => void;
}) {
  const uid = useId();
  const titleId = `${uid}-title`;
  const chooseId = `${uid}-choose`;
  const hintId = `${uid}-dnd`;
  const messageId = `${uid}-message`;
  const countId = `${uid}-count`;
  const box = useRef<HTMLDivElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const [edits, setEdits] = useState<Edits>({});
  const status = edits.status ?? profile?.status ?? "available";
  const mood = edits.mood !== undefined ? edits.mood : (profile?.mood ?? null);
  const message = edits.message ?? profile?.message ?? "";
  const setStatus = (next: OwnerStatus) => setEdits((e) => ({ ...e, status: next }));
  const setMood = (next: Mood | null) => setEdits((e) => ({ ...e, mood: next }));
  const setMessage = (next: string) => setEdits((e) => ({ ...e, message: next }));
  const [picture, setPicture] = useState<PictureChange>({ kind: "keep" });
  const [reading, setReading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const shown =
    picture.kind === "set"
      ? picture.png
      : picture.kind === "remove"
        ? null
        : (profile?.picture ?? null);

  // The keyboard starts on the panel's first control.
  useEffect(() => {
    focusables(box.current)[0]?.focus();
  }, []);

  const choose = async (e: ChangeEvent<HTMLInputElement>) => {
    const chosen = e.target.files?.[0];
    // Choosing the same file again reads it again.
    e.target.value = "";
    if (!chosen) return;
    setReading(true);
    setError(null);
    try {
      const png = await shrinkToPng(chosen);
      setPicture({ kind: "set", png });
    } catch (reason) {
      setError(reason instanceof PictureError ? reason.message : CANNOT_READ);
    } finally {
      setReading(false);
    }
  };

  const remove = () => {
    setPicture({ kind: "remove" });
    setError(null);
    // Remove picture goes away with the picture: the keyboard moves to Choose a picture….
    document.getElementById(chooseId)?.focus();
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pending || reading || !profile) return;
    onPending(true);
    setError(null);
    try {
      await save({ status, mood, message: message.trim(), picture });
      onClose(true);
    } catch (reason) {
      setError(`Couldn't save your changes: ${toCommandError(reason).message}`);
      onPending(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      // While Save is on its way, the panel waits for Plenipo's answer.
      if (!pending) onClose(true);
      return;
    }
    if (e.key !== "Tab") return;
    // Tab stays inside the panel until it closes.
    const items = focusables(box.current);
    const first = items[0];
    const last = items[items.length - 1];
    if (!first || !last) return;
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div
      ref={box}
      id={id}
      className="owner-panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      onKeyDown={onKeyDown}
    >
      <form className="owner-panel__form" onSubmit={(e) => void submit(e)}>
        <h2 id={titleId} className="owner-panel__title">
          You
        </h2>

        {!profile && (
          <div className="owner-panel__loading">
            {loadError ? (
              <>
                <p role="alert">{reload ? cannotReadDetails(loadError) : loadError}</p>
                {reload && (
                  <Button
                    size="sm"
                    onClick={() => {
                      reload();
                      // Try again goes away as it reads: the keyboard moves to Choose a picture….
                      document.getElementById(chooseId)?.focus();
                    }}
                  >
                    Try again
                  </Button>
                )}
              </>
            ) : (
              <p role="status">{READING_DETAILS}</p>
            )}
          </div>
        )}

        <fieldset className="owner-panel__group">
          <legend>Your picture</legend>
          <div className="owner-panel__picture">
            <OwnerFace
              profile={{ status, mood, message, picture: shown }}
              size={56}
              light={false}
            />
            <div className="owner-panel__picture-actions">
              <Button
                id={chooseId}
                size="sm"
                disabled={reading || pending}
                onClick={() => file.current?.click()}
              >
                {reading ? "Reading the picture…" : "Choose a picture…"}
              </Button>
              {shown && (
                <Button size="sm" variant="quiet" disabled={reading || pending} onClick={remove}>
                  Remove picture
                </Button>
              )}
              {/* Windows' own "Open" box: Plenipo gets only the picture you pick, never a path. */}
              <input
                ref={file}
                type="file"
                accept={PICTURE_ACCEPT}
                hidden
                tabIndex={-1}
                aria-label="Your picture"
                onChange={(e) => void choose(e)}
              />
            </div>
          </div>
          <p className="owner-panel__note">{pictureNote()}</p>
        </fieldset>

        <fieldset className="owner-panel__group">
          <legend>Status</legend>
          <div className="owner-panel__choices">
            {STATUS_ORDER.map((s) => (
              <label key={s} className="owner-choice">
                <input
                  type="radio"
                  name={`${uid}-status`}
                  value={s}
                  checked={status === s}
                  aria-describedby={
                    s === "doNotDisturb" && status === "doNotDisturb" ? hintId : undefined
                  }
                  onChange={() => setStatus(s)}
                />
                <OwnerLight status={s} />
                <span>{STATUS_WORDS[s]}</span>
              </label>
            ))}
          </div>
          {status === "doNotDisturb" && (
            <p id={hintId} className="owner-panel__hint">
              {doNotDisturbHint()}
            </p>
          )}
        </fieldset>

        <fieldset className="owner-panel__group">
          <legend>Mood</legend>
          <div className="owner-panel__choices owner-panel__choices--moods">
            <label className="owner-choice">
              <input
                type="radio"
                name={`${uid}-mood`}
                value=""
                checked={mood === null}
                onChange={() => setMood(null)}
              />
              <span>{NO_MOOD_WORD}</span>
            </label>
            {MOOD_ORDER.map((m) => (
              <label key={m} className="owner-choice">
                <input
                  type="radio"
                  name={`${uid}-mood`}
                  value={m}
                  checked={mood === m}
                  onChange={() => setMood(m)}
                />
                <span className="owner-choice__face" aria-hidden="true">
                  {MOOD_FACES[m]}
                </span>
                <span>{MOOD_WORDS[m]}</span>
              </label>
            ))}
          </div>
        </fieldset>

        <div className="owner-panel__group owner-panel__message">
          <label htmlFor={messageId}>Message</label>
          <input
            id={messageId}
            type="text"
            value={message}
            maxLength={MAX_MESSAGE}
            placeholder="Feeling great!"
            autoComplete="off"
            aria-describedby={countId}
            onChange={(e) => setMessage(oneLineMessage(e.target.value))}
          />
          <span id={countId} className="owner-panel__count">
            {message.length} of {MAX_MESSAGE}
          </span>
        </div>

        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}

        <footer className="owner-panel__footer">
          <Button variant="quiet" disabled={pending} onClick={() => onClose(true)}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={pending || reading || !profile}>
            {pending ? "Saving…" : "Save"}
          </Button>
        </footer>
      </form>
    </div>
  );
}
