import { useCallback, useEffect, useState } from "react";
import type {
  ConnectionCard as Card,
  ConnectionsPage,
  GithubRepositories,
  ServiceCard,
  SignInCode,
} from "@plenipo/types";
import { Button, Disclosure } from "@plenipo/ui";

import {
  cancelConnectionSignIn,
  connectConnection,
  disconnectConnection,
  listGithubRepositories,
  openGithubPage,
  toCommandError,
  type GithubPage,
} from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import { useRun } from "../../guard/useRun";
import {
  GITHUB_SEES,
  STATE_LABEL,
  STATE_TONE,
  githubAccountLine,
  githubCardSummary,
  noAppWords,
  repositoryCount,
} from "./words";

/** How many repositories the card lists before "and N more" (the picker shows them all). */
export const SHOWN_REPOSITORIES = 20;

/**
 * GitHub's card (Phase 25, ADR-204): the owner's alone, free, and read-only. **Connect** shows a
 * short code to type on GitHub's own page (no password or key ever comes to Plenipo); once
 * connected, the accounts and organizations Plenipo may list, **Add an account or organization**
 * (GitHub's page), and the repositories' names. No parts, no **Who may use it**, no send list:
 * no worker ever uses it. **Disconnect** removes the sign-in here and says where to remove Plenipo
 * at GitHub, which Plenipo cannot do by itself.
 */
export function GithubCard({
  service,
  card,
  page,
  onApply,
}: {
  service: ServiceCard;
  card: Card;
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
}) {
  const c = card.connection;
  const id = c.id;
  const { pending, error, run } = useRun(onApply);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  // Just disconnected: the card still says where to remove Plenipo at GitHub.
  const [disconnected, setDisconnected] = useState(false);
  const connected = c.state === "connected";
  const canConnect = card.hasApp && page.vaultAvailable && !card.signingIn && !pending;
  const titleId = `connection-${id}`;
  const needsYou = card.signingIn || c.state === "needsSignIn" || !!card.problem;
  return (
    <li className={`connection connection--opens connection--${c.state}`} aria-labelledby={titleId}>
      <Disclosure
        title={service.label}
        headingId={titleId}
        status={{
          status: card.signingIn ? "pending" : STATE_TONE[c.state],
          label: card.signingIn ? "Waiting for you to type the code" : STATE_LABEL[c.state],
        }}
        summary={githubCardSummary(card)}
        openWhen={needsYou}
        rememberAs={`connection:${id}`}
      >
        <p className="muted">
          Plenipo lists your GitHub repositories when you set up a project, so you can pick one
          instead of typing its address. GitHub lets it see {GITHUB_SEES}. It is yours alone: no
          worker ever uses it. Free with every plan.
        </p>
        {connected && c.account && (
          <p className="connection__account">
            Connected as <strong>{githubAccountLine(c.account)}</strong>.
          </p>
        )}
        {card.code && card.signingIn && (
          <CodeWaiting code={card.code} cancel={() => void run(() => cancelConnectionSignIn(id))} />
        )}
        {card.problem && (
          <p className={connected ? "form-error" : "notice-box"} role="alert">
            {card.problem}
          </p>
        )}
        {c.state === "needsSignIn" && (
          <p className="notice-box" role="alert">
            <strong>GitHub needs you to sign in again.</strong> It no longer accepts Plenipo&apos;s
            sign-in (it expired, or it was removed on GitHub). Plenipo cannot list your repositories
            until you sign in again.
          </p>
        )}
        {!card.hasApp && <p className="form-error">{noAppWords(c.service)}</p>}
        <div className="actions">
          {!card.signingIn && !connected && (
            <Button
              variant="primary"
              size="sm"
              disabled={!canConnect}
              onClick={() => void run(() => connectConnection(id, "work"))}
            >
              {c.state === "needsSignIn" ? "Sign in again" : "Sign in with GitHub"}
            </Button>
          )}
          {c.state !== "notConnected" &&
            (confirmDisconnect ? (
              <>
                <span className="muted">
                  Disconnect GitHub? Plenipo stops listing your repositories, and its sign-in is
                  removed from {page.vaultLabel}. GitHub has no way for Plenipo to remove itself
                  there: the card shows where to do it.
                </span>
                <Button
                  variant="danger"
                  size="sm"
                  disabled={pending}
                  onClick={() =>
                    void run(() => disconnectConnection(id)).then((done) => {
                      setConfirmDisconnect(false);
                      if (done) setDisconnected(true);
                    })
                  }
                >
                  Yes, disconnect
                </Button>
                <Button variant="quiet" size="sm" onClick={() => setConfirmDisconnect(false)}>
                  Keep it
                </Button>
              </>
            ) : (
              <Button variant="danger" size="sm" onClick={() => setConfirmDisconnect(true)}>
                Disconnect
              </Button>
            ))}
        </div>
        <Refusal error={error} />
        {connected && <Repositories />}
        {(disconnected || c.state !== "notConnected") && <RemoveAtGithub />}
      </Disclosure>
    </li>
  );
}

/** The code to type on GitHub's page, with Copy, Open GitHub's page, and Cancel. */
function CodeWaiting({ code, cancel }: { code: SignInCode; cancel: () => void }) {
  const [copied, setCopied] = useState(false);
  const opener = useOpener();
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code.code);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  const until = new Date(code.expiresAt).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
  return (
    <div className="notice-box github-code" role="status">
      <strong>Type this code on GitHub&apos;s page.</strong> GitHub&apos;s page opened in your
      browser ({code.page.replace(/^https:\/\//, "")}). Type the code there and approve Plenipo;
      this page updates by itself. Plenipo never sees your password.
      <p className="github-code__code" aria-label="Your code">
        {code.code}
      </p>
      <p>
        <strong>{code.words}</strong> If someone else sent you a code, don&apos;t type it.
      </p>
      <p className="muted">The code runs out at {until}.</p>
      <div className="actions">
        <Button variant="primary" size="sm" onClick={() => void copy()}>
          {copied ? "Copied" : "Copy the code"}
        </Button>
        <Button variant="secondary" size="sm" onClick={() => opener.open("device")}>
          Open GitHub&apos;s page
        </Button>
        <Button variant="quiet" size="sm" onClick={cancel}>
          Cancel
        </Button>
      </div>
      <Refusal error={opener.error} />
    </div>
  );
}

/** Opens one of GitHub's own pages in the owner's browser, keeping a refusal to show. */
function useOpener() {
  const [error, setError] = useState<string | null>(null);
  const open = (which: GithubPage) => {
    setError(null);
    openGithubPage(which).catch((reason: unknown) => setError(toCommandError(reason).message));
  };
  return { error, open };
}

/** Where to remove Plenipo at GitHub, which Plenipo cannot do by itself. */
function RemoveAtGithub() {
  const opener = useOpener();
  return (
    <section className="connection__section" aria-labelledby="github-remove">
      <h4 id="github-remove">Remove Plenipo at GitHub</h4>
      <p className="muted">
        Disconnect removes Plenipo&apos;s sign-in from this computer. To remove Plenipo from GitHub
        too, open these two pages and remove it on each.
      </p>
      <div className="actions">
        <Button variant="secondary" size="sm" onClick={() => opener.open("authorizations")}>
          Open Authorized GitHub Apps
        </Button>
        <Button variant="secondary" size="sm" onClick={() => opener.open("installations")}>
          Open Installed GitHub Apps
        </Button>
      </div>
      <Refusal error={opener.error} />
    </section>
  );
}

/**
 * The accounts and organizations Plenipo may list, and their repositories' names: asked of GitHub
 * when the card opens, kept ten minutes, and asked again with **Look again**.
 */
function Repositories() {
  const [list, setList] = useState<GithubRepositories | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [looking, setLooking] = useState(true);
  const opener = useOpener();
  const settle = useCallback((asked: Promise<GithubRepositories>) => {
    asked
      .then(
        (answer) => setList(answer),
        (reason: unknown) => setError(toCommandError(reason).message),
      )
      .finally(() => setLooking(false));
  }, []);
  useEffect(() => {
    settle(listGithubRepositories(false));
  }, [settle]);
  const lookAgain = () => {
    setLooking(true);
    setError(null);
    settle(listGithubRepositories(true));
  };
  const shown = list?.repositories.slice(0, SHOWN_REPOSITORIES) ?? [];
  const hidden = (list?.repositories.length ?? 0) - shown.length;
  return (
    <section className="connection__section" aria-labelledby="github-accounts">
      <h4 id="github-accounts">Accounts Plenipo may list</h4>
      {looking && !list && (
        <p className="muted" role="status">
          Asking GitHub…
        </p>
      )}
      {list && list.accounts.length === 0 && (
        <p className="notice-box" role="note">
          <strong>One more step: choose which accounts Plenipo may list.</strong> Press{" "}
          <em>Choose on GitHub</em> and pick your own account and each organization. Pick{" "}
          <em>All repositories</em> to see them all.
        </p>
      )}
      {list && list.accounts.length > 0 && (
        <ul className="github-accounts">
          {list.accounts.map((a) => (
            <li key={a.login}>
              <strong>{a.login}</strong>{" "}
              <span className="muted">
                ({a.organization ? "an organization" : "your account"},{" "}
                {a.allRepositories ? "every repository" : "only the repositories you picked"})
              </span>
              {a.refused && (
                <p className="form-error" role="alert">
                  {a.refused}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
      <div className="actions">
        {list?.installPage && (
          <Button
            variant={list.accounts.length === 0 ? "primary" : "secondary"}
            size="sm"
            onClick={() => opener.open("install")}
          >
            {list.accounts.length === 0 ? "Choose on GitHub" : "Add an account or organization"}
          </Button>
        )}
        <Button variant="quiet" size="sm" disabled={looking} onClick={lookAgain}>
          Look again
        </Button>
      </div>
      {list?.installPage && (
        <p className="muted">
          For an organization you don&apos;t own, GitHub asks its owners first, and it shows up here
          once they say yes.
        </p>
      )}
      <Refusal error={error ?? opener.error} />
      {list && list.repositories.length > 0 && (
        <>
          <h4 id="github-repositories">{repositoryCount(list)}</h4>
          <ul className="github-repositories" aria-labelledby="github-repositories">
            {shown.map((r) => (
              <li key={`${r.owner}/${r.name}`}>
                <span className="path">
                  {r.owner}/{r.name}
                </span>
                {r.private && <span className="muted"> (private)</span>}
                {r.description && <span className="muted"> — {r.description}</span>}
              </li>
            ))}
          </ul>
          {hidden > 0 && (
            <p className="muted">
              And {hidden} more. You pick from all of them when you set up a project.
            </p>
          )}
        </>
      )}
    </section>
  );
}
