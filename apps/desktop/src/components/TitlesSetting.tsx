import { useState } from "react";
import type { TitleTheme } from "@plenipo/types";

import { setOrganizationTitles, toCommandError } from "../api/commands";
import { RANKS, TITLE_SETS, titleSet, type TitleSet } from "../org/titles";
import { useOrganization } from "../org/useOrganization";

const GROUPS: TitleSet["group"][] = ["Business", "Military", "Just for fun"];
const BUSINESS = titleSet("business");

/**
 * Personalization: what the app calls each rank of the chain of command. Stored with the
 * organization; only the names shown change — job titles stay as the owner wrote them, and
 * agents always get the plain titles.
 */
export function TitlesSetting() {
  const org = useOrganization();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const current = org.snapshot?.titles ?? "business";
  const t = titleSet(current);

  async function choose(theme: TitleTheme) {
    setPending(true);
    setError(null);
    setSaved(false);
    try {
      org.apply(await setOrganizationTitles(theme));
      setSaved(true);
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="titles">
      <label className="field titles__pick">
        <span>Titles</span>
        <select
          value={current}
          disabled={org.snapshot === null || pending}
          onChange={(e) => void choose(e.target.value as TitleTheme)}
        >
          {GROUPS.map((group) => (
            <optgroup key={group} label={group}>
              {TITLE_SETS.filter((s) => s.group === group).map((s) => (
                <option key={s.theme} value={s.theme}>
                  {s.label}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </label>
      <ol className="titles__chain" aria-label="Chain of command">
        {RANKS.map((rank) => {
          const notes = [
            t.theme !== "business" ? BUSINESS.ranks[rank].one : null,
            rank === "owner" ? "you" : null,
          ].filter(Boolean);
          return (
            <li key={rank}>
              <strong>{t.ranks[rank].one}</strong>
              {notes.length > 0 && <span className="muted"> — {notes.join(" · ")}</span>}
            </li>
          );
        })}
      </ol>
      <p className="muted">
        What Plenipo calls each rank of your organization, from you at the top down to the workers.
        Only the names you see change: job titles stay as you wrote them, and your agents always get
        the plain titles.
      </p>
      {saved && (
        <p className="status status--ok" role="status">
          Saved. The Organization page now uses {t.label} titles.
        </p>
      )}
      {(error ?? (org.status === "error" ? org.error : null)) && (
        <p className="form-error" role="alert">
          {error ?? org.error}
        </p>
      )}
    </div>
  );
}
