import { useCallback, useEffect, useId, useMemo, useState, type KeyboardEvent } from "react";
import type { GithubRepositories, GithubRepository } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { listGithubRepositories, openGithubPage, toCommandError } from "../../api/commands";
import { SHOWN, addressOf, matching, updatedWords } from "./repositoryWords";

type Loaded =
  | { state: "looking" }
  | { state: "listed"; list: GithubRepositories }
  | { state: "notConnected" }
  | { state: "refused"; why: string };

/**
 * The repository box on New project, Edit project, and Set up a Development project (ADR-204):
 * typing searches your GitHub repositories (from Settings → Connections → GitHub), grouped by
 * account, private ones marked; arrow keys and Enter pick one, Escape closes the list. An
 * address typed or pasted always works, on any host. Without GitHub connected, it is the plain
 * box, with a line saying where to connect it.
 */
export function RepositoryPicker({
  value,
  onChange,
}: {
  value: string;
  onChange: (address: string) => void;
}) {
  const [loaded, setLoaded] = useState<Loaded>({ state: "looking" });
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const [opener, setOpener] = useState<string | null>(null);
  const listId = useId();
  const settle = useCallback((asked: Promise<GithubRepositories>) => {
    asked
      .then((list) =>
        setLoaded(list ? { state: "listed", list } : { state: "refused", why: "No answer." }),
      )
      .catch((reason: unknown) => {
        const why = toCommandError(reason).message;
        setLoaded(
          /isn't connected/.test(why) ? { state: "notConnected" } : { state: "refused", why },
        );
      });
  }, []);
  useEffect(() => {
    settle(Promise.resolve().then(() => listGithubRepositories(false)));
  }, [settle]);
  const lookAgain = () => {
    setLoaded({ state: "looking" });
    settle(Promise.resolve().then(() => listGithubRepositories(true)));
  };
  const list = loaded.state === "listed" ? loaded.list : null;
  const found = useMemo(() => (list ? matching(list.repositories, value) : []), [list, value]);
  const shown = found.slice(0, SHOWN);
  const showing = open && list !== null && list.repositories.length > 0;
  const pick = (r: GithubRepository) => {
    onChange(addressOf(r));
    setOpen(false);
    setActive(-1);
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!list) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setOpen(true);
      if (shown.length === 0) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((a) => (a + step + shown.length) % shown.length);
    } else if (e.key === "Enter" && showing && active >= 0 && shown[active]) {
      e.preventDefault();
      pick(shown[active]);
    } else if (e.key === "Escape" && showing) {
      e.preventDefault();
      setOpen(false);
      setActive(-1);
    }
  };
  // Grouped by account, in the order GitHub's list has them (your own first).
  const groups = useMemo(() => {
    const out: { owner: string; items: { r: GithubRepository; index: number }[] }[] = [];
    shown.forEach((r, index) => {
      const last = out[out.length - 1];
      if (last && last.owner === r.owner) last.items.push({ r, index });
      else out.push({ owner: r.owner, items: [{ r, index }] });
    });
    return out;
  }, [shown]);
  const optionId = (index: number) => `${listId}-${index}`;
  const openPage = () => {
    setOpener(null);
    openGithubPage("install").catch((reason: unknown) => setOpener(toCommandError(reason).message));
  };
  return (
    <div className="repo-picker">
      <label className="field">
        <span>Repository URL (optional)</span>
        <input
          value={value}
          maxLength={2000}
          placeholder={
            list ? "Search your GitHub repositories, or paste an address" : "https://github.com/…"
          }
          role={list ? "combobox" : undefined}
          aria-autocomplete={list ? "list" : undefined}
          aria-expanded={list ? showing : undefined}
          aria-controls={list ? listId : undefined}
          aria-activedescendant={showing && active >= 0 ? optionId(active) : undefined}
          onChange={(e) => {
            onChange(e.target.value);
            setOpen(true);
            setActive(-1);
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => setOpen(false)}
          onKeyDown={keys}
        />
        <small className="field__hint">
          {loaded.state === "notConnected"
            ? "Connect GitHub in Settings → Connections to pick from your repositories. You can always paste an address."
            : loaded.state === "looking"
              ? "Looking at your GitHub repositories…"
              : list
                ? `${list.repositories.length} repositories from GitHub. Type to search, or paste any address.`
                : "Paste the repository's address, from GitHub or anywhere else."}
        </small>
      </label>
      {loaded.state === "refused" && (
        <p className="hint repo-picker__problem">
          {loaded.why} You can still paste an address.{" "}
          <Button size="sm" variant="quiet" onClick={lookAgain}>
            Look again
          </Button>
        </p>
      )}
      {showing && (
        <div className="repo-picker__popup">
          <div role="listbox" id={listId} aria-label="Your GitHub repositories">
            {groups.map((g) => (
              <div key={g.owner} role="group" aria-label={g.owner}>
                <div className="repo-picker__account" aria-hidden="true">
                  {g.owner}
                </div>
                {g.items.map(({ r, index }) => (
                  <div
                    key={`${r.owner}/${r.name}`}
                    id={optionId(index)}
                    role="option"
                    aria-selected={index === active}
                    className={`repo-picker__option${index === active ? " is-active" : ""}`}
                    // Picked before the box loses focus and the list closes.
                    onMouseDown={(e) => {
                      e.preventDefault();
                      pick(r);
                    }}
                  >
                    <span className="repo-picker__name">
                      {r.owner}/{r.name}
                    </span>
                    {r.private && <span className="repo-picker__private">Private</span>}
                    {r.description && (
                      <span className="repo-picker__description">{r.description}</span>
                    )}
                    {updatedWords(r.updatedAt) && (
                      <span className="repo-picker__updated">{updatedWords(r.updatedAt)}</span>
                    )}
                  </div>
                ))}
              </div>
            ))}
          </div>
          {shown.length === 0 && (
            <p className="hint">No repository matches. Paste its address, or look again.</p>
          )}
          {found.length > shown.length && (
            <p className="hint">
              Showing {shown.length} of {found.length}. Type more of the name to narrow it.
            </p>
          )}
          {list?.more && (
            <p className="hint">GitHub has more repositories than Plenipo lists; type to search.</p>
          )}
          <div
            className="repo-picker__actions"
            // Clicking here keeps the box focused, so the list stays open.
            onMouseDown={(e) => e.preventDefault()}
          >
            {list?.installPage && (
              <Button size="sm" variant="quiet" onClick={openPage}>
                Not here? Add an account or organization on GitHub
              </Button>
            )}
            <Button size="sm" variant="quiet" onClick={lookAgain}>
              Look again
            </Button>
          </div>
          {opener && (
            <p className="form-error" role="alert">
              {opener}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
