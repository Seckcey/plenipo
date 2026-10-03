import { useEffect, useId, useState } from "react";
import type { OrgSnapshot, PositionInfo } from "@plenipo/types";

import { getOrganization, giveCommunityMessageToWorker, toCommandError } from "../api/commands";
import { ConfirmDialog } from "../components/org/Modal";
import { canTakeObjective } from "../org/rules";
import { rankName, titlesOf } from "../org/titles";
import { OUTSIDE_WORDS, quoteOf } from "./messageWords";

/** The longest note: Plenipo checks the whole objective too (the note and the message's words). */
const MAX_NOTE = 2000;

/**
 * **Give to a worker** (ADR-164 §9): a message's words go to one of your full-time positions as
 * an objective. The worker gets the words marked as outside words, so it treats them as
 * information, never as orders. Workers never read your messages on their own.
 *
 * The positions to pick from are the ones that can be given an objective anywhere else in
 * Plenipo (`canTakeObjective`: full-time, staffed, and active), named with their rank as the
 * owner's Titles setting says. `onGiven` is told the position's title.
 */
export function GiveToWorkerDialog({
  itemId,
  text,
  onGiven,
  onCancel,
}: {
  itemId: string;
  /** The message's words, for the short quote. */
  text: string | null;
  onGiven: (title: string) => void;
  onCancel: () => void;
}) {
  const pickId = useId();
  const noteId = useId();
  const [org, setOrg] = useState<OrgSnapshot | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [positionId, setPositionId] = useState("");
  const [note, setNote] = useState("");

  useEffect(() => {
    let alive = true;
    getOrganization().then(
      (snapshot) => {
        if (!alive) return;
        setOrg(snapshot);
        setPositionId(snapshot.positions.find(canTakeObjective)?.id ?? "");
      },
      (reason: unknown) => {
        if (alive) setProblem(toCommandError(reason).message);
      },
    );
    return () => {
      alive = false;
    };
  }, []);

  const takers: PositionInfo[] = org ? org.positions.filter(canTakeObjective) : [];
  const titles = org ? titlesOf(org) : null;

  return (
    <ConfirmDialog
      title="Give to a worker"
      confirmLabel="Give it"
      onCancel={onCancel}
      message={
        <div className="give-to-worker">
          <p>
            Message: <q>{quoteOf(text)}</q>
          </p>
          <p className="muted">{OUTSIDE_WORDS}</p>
          {!org && !problem && <p role="status">Looking for workers…</p>}
          {problem && (
            <p className="form-error" role="alert">
              {problem}
            </p>
          )}
          {org && takers.length === 0 && (
            <p role="status">
              No one can be given this yet. Give it to a full-time position that has an agent in it
              (you can hire one on the Organization page).
            </p>
          )}
          {org && titles && takers.length > 0 && (
            <>
              <div className="ui-field">
                <label htmlFor={pickId}>Give it to</label>
                <select
                  id={pickId}
                  value={positionId}
                  onChange={(e) => setPositionId(e.target.value)}
                >
                  {takers.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.title} ({rankName(titles, p.kind)})
                    </option>
                  ))}
                </select>
              </div>
              <div className="ui-field">
                <label htmlFor={noteId}>What should they do with it?</label>
                <textarea
                  id={noteId}
                  value={note}
                  rows={3}
                  maxLength={MAX_NOTE}
                  placeholder="Optional. If you leave this empty, they will read it and say what they think."
                  onChange={(e) => setNote(e.target.value)}
                />
              </div>
            </>
          )}
        </div>
      }
      onConfirm={async () => {
        const taker = takers.find((p) => p.id === positionId);
        if (!taker) return "Pick who gets it first.";
        try {
          await giveCommunityMessageToWorker(itemId, taker.id, note.trim());
          onGiven(taker.title);
          return null;
        } catch (reason) {
          return toCommandError(reason).message;
        }
      }}
    />
  );
}
