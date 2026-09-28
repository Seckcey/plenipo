import { useState } from "react";
import type {
  AccountKind,
  ConnectionCard as Card,
  ConnectionsPage,
  PartLevel,
  ServiceCard,
} from "@plenipo/types";
import { Button, Segmented, StatusPill, TextField } from "@plenipo/ui";

import {
  cancelConnectionSignIn,
  connectConnection,
  disconnectConnection,
  setConnectionOwnApp,
  setConnectionParts,
} from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import type { Go } from "../../components/views";
import { useRun } from "../../guard/useRun";
import { SendList } from "./SendList";
import { WhoMayUse } from "./WhoMayUse";
import {
  PART_LEVEL_LABEL,
  PART_LEVELS,
  STATE_LABEL,
  STATE_TONE,
  accountLine,
  andList,
} from "./words";

/**
 * One connection's card: its state; Connect (a work or school account, or a personal one),
 * Reconnect, and Disconnect; while signing in, "Finish signing in in your browser" with Cancel;
 * what it can do, part by part; what Plenipo was allowed; who may use it; the people it may send
 * to without asking; and, under Advanced, the organization's own app.
 */
export function ConnectionCard({
  service,
  card,
  page,
  onApply,
  go,
}: {
  service: ServiceCard;
  card: Card;
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
  go: Go;
}) {
  const c = card.connection;
  const id = c.id;
  const { pending, error, run } = useRun(onApply);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  const connected = c.state === "connected";
  const signedInOnce = c.state !== "notConnected";
  const partsOn = card.parts.some((p) => p.available && p.level !== "off");
  const canConnect = card.hasApp && page.vaultAvailable && partsOn && !card.signingIn && !pending;
  const connect = (kind: AccountKind) => void run(() => connectConnection(id, kind));
  const titleId = `connection-${id}`;
  return (
    <li className={`connection connection--${c.state}`} aria-labelledby={titleId}>
      <div className="connection__header">
        <h3 id={titleId}>{service.label}</h3>
        <StatusPill
          status={card.signingIn ? "pending" : STATE_TONE[c.state]}
          label={card.signingIn ? "Waiting for you in your browser" : STATE_LABEL[c.state]}
        />
      </div>
      {connected && c.account && (
        <p className="connection__account">
          Connected as <strong>{accountLine(c.account, c.accountKind)}</strong>.
        </p>
      )}
      {card.signingIn && (
        <div className="notice-box" role="status">
          <strong>Finish signing in in your browser.</strong> {service.label}&apos;s sign-in page
          opened in your browser. Sign in there and approve Plenipo; this page updates by itself.
          Plenipo never sees your password.{" "}
          <Button
            variant="quiet"
            size="sm"
            disabled={pending}
            onClick={() => void run(() => cancelConnectionSignIn(id))}
          >
            Cancel
          </Button>
        </div>
      )}
      {card.adminLink ? (
        <AdminLink link={card.adminLink} />
      ) : (
        card.problem && (
          <p className="form-error" role="alert">
            {card.problem}
          </p>
        )
      )}
      {c.state === "needsSignIn" && (
        <p className="notice-box" role="alert">
          <strong>{service.label} needs you to sign in again.</strong> It no longer accepts
          Plenipo&apos;s sign-in (it expired, was removed, or your password changed). Workers cannot
          use it until you sign in again.
        </p>
      )}
      {!card.hasApp && (
        <p className="form-error">
          This copy of Plenipo has no Microsoft app ID yet, so it cannot sign in. Your organization
          can use its own app ID under <em>Advanced</em> below.
        </p>
      )}
      {connected && card.reconnectFor.length > 0 && (
        <p className="notice-box" role="note">
          <strong>Reconnect to allow {andList(card.reconnectFor)}.</strong> You turned{" "}
          {card.reconnectFor.length === 1 ? "it" : "them"} on (or up to Full access) after
          connecting, and Microsoft has not allowed that yet.
        </p>
      )}
      {!partsOn && !connected && (
        <p className="muted">Turn on at least one part below, so Plenipo knows what to ask for.</p>
      )}
      <div className="actions">
        {connected ? (
          <Button
            variant="secondary"
            size="sm"
            disabled={!canConnect}
            onClick={() => connect(c.accountKind ?? "work")}
          >
            Reconnect
          </Button>
        ) : (
          <>
            <Button
              variant="primary"
              size="sm"
              disabled={!canConnect}
              onClick={() => connect(c.accountKind ?? "work")}
            >
              {c.state === "needsSignIn" ? "Sign in again" : "Connect a work or school account"}
            </Button>
            {c.state === "notConnected" && !c.ownApp && (
              <Button
                variant="secondary"
                size="sm"
                disabled={!canConnect}
                onClick={() => connect("personal")}
              >
                Connect a personal account
              </Button>
            )}
          </>
        )}
        {signedInOnce &&
          (confirmDisconnect ? (
            <>
              <span className="muted">
                Disconnect {service.label}? Its tools stop now, and its sign-in is removed from{" "}
                {page.vaultLabel}.
              </span>
              <Button
                variant="danger"
                size="sm"
                disabled={pending}
                onClick={() =>
                  void run(() => disconnectConnection(id)).then(() => setConfirmDisconnect(false))
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

      <section className="connection__section" aria-labelledby={`${id}-parts`}>
        <h4 id={`${id}-parts`}>What it can do</h4>
        <ul className="connection-parts">
          {card.parts.map((p) => (
            <li key={p.part} className="connection-part">
              <div className="connection-part__head">
                <strong>{p.label}</strong>
                {p.available ? (
                  <Segmented<PartLevel>
                    label={`${p.label}: what workers may do`}
                    value={p.level}
                    options={PART_LEVELS.map((l) => ({ value: l, label: PART_LEVEL_LABEL[l] }))}
                    onChange={(level) => {
                      if (level !== p.level && !pending) {
                        void run(() => setConnectionParts(id, { [p.part]: level }));
                      }
                    }}
                  />
                ) : (
                  <span className="muted">Not in personal accounts</span>
                )}
              </div>
              {p.available && (
                <p className="connection-part__words">
                  Read only: {p.reads} Full access: {p.changes}
                  {p.needsAdmin && " Your organization's admin approves it once."}
                </p>
              )}
            </li>
          ))}
        </ul>
      </section>

      {card.granted.length > 0 && (
        <section className="connection__section" aria-labelledby={`${id}-granted`}>
          <h4 id={`${id}-granted`}>What Plenipo was allowed</h4>
          <ul className="connection-granted">
            {card.granted.map((g) => (
              <li key={g.name}>
                {g.words || g.name} <code>{g.name}</code>
              </li>
            ))}
          </ul>
        </section>
      )}

      <WhoMayUse id={id} access={c.access} people={page.people} onApply={onApply} />
      <SendList id={id} list={c.sendList} switchOn={page.sendSwitchOn} go={go} onApply={onApply} />
      <OwnApp card={card} onApply={onApply} />
    </li>
  );
}

/** Microsoft says the organization's admin must approve Plenipo first: the link to send them. */
function AdminLink({ link }: { link: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className="notice-box" role="alert">
      <strong>Your organization&apos;s admin needs to approve Plenipo first.</strong> Send them this
      link; after they approve, press Connect again.
      <p className="path">{link}</p>
      <Button variant="secondary" size="sm" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy the approval link for your admin"}
      </Button>
    </div>
  );
}

/** Advanced: sign in with the organization's own Microsoft app (its app ID is not a secret). */
function OwnApp({ card, onApply }: { card: Card; onApply: (page: ConnectionsPage) => void }) {
  const c = card.connection;
  const [appId, setAppId] = useState(c.ownApp?.appId ?? "");
  const [tenant, setTenant] = useState(c.ownApp?.tenant ?? "");
  const { pending, error, run } = useRun(onApply);
  const locked = c.state !== "notConnected";
  return (
    <details className="connection__section connection__advanced">
      <summary>Advanced</summary>
      <p className="muted">
        Plenipo signs in with 8 West&apos;s Microsoft app. An organization that wants its own can
        register one (the steps are in Plenipo&apos;s documentation) and give its Microsoft app ID
        and its domain here. An app ID is not a secret; Plenipo never asks for an app&apos;s secret.
      </p>
      {locked ? (
        <p className="muted">
          {c.ownApp
            ? `Using your organization's own app (${c.ownApp.appId}, ${c.ownApp.tenant}).`
            : "Using 8 West's app."}{" "}
          Disconnect first to change it: a sign-in belongs to the app it was made with.
        </p>
      ) : (
        <>
          <TextField
            label="Microsoft app ID"
            value={appId}
            placeholder="12345678-abcd-4ef0-9abc-0123456789ab"
            onChange={setAppId}
          />
          <TextField
            label="Your organization's domain or ID"
            value={tenant}
            placeholder="contoso.com"
            onChange={setTenant}
          />
          <div className="actions">
            <Button
              variant="primary"
              size="sm"
              disabled={pending || appId.trim() === "" || tenant.trim() === ""}
              onClick={() =>
                void run(() =>
                  setConnectionOwnApp(c.id, { appId: appId.trim(), tenant: tenant.trim() }),
                )
              }
            >
              Use this app
            </Button>
            {c.ownApp && (
              <Button
                variant="quiet"
                size="sm"
                disabled={pending}
                onClick={() =>
                  void run(() => setConnectionOwnApp(c.id, null)).then((ok) => {
                    if (ok) {
                      setAppId("");
                      setTenant("");
                    }
                  })
                }
              >
                Use 8 West&apos;s app
              </Button>
            )}
          </div>
          <Refusal error={error} />
        </>
      )}
    </details>
  );
}
