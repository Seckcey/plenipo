import { useState } from "react";
import type {
  AccountKind,
  ConnectionCard as Card,
  ConnectionsPage,
  PartLevel,
  ServiceCard,
} from "@plenipo/types";
import { Button, Disclosure, Segmented } from "@plenipo/ui";

import {
  cancelConnectionSignIn,
  connectConnection,
  disconnectConnection,
  removeConnection,
  setConnectionParts,
} from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import type { Go } from "../../components/views";
import { useRun } from "../../guard/useRun";
import { KeyForm } from "./KeyForm";
import { GoogleApp, MicrosoftOwnApp, SlackOwnApp } from "./OwnApps";
import { SendList } from "./SendList";
import { WhoMayUse } from "./WhoMayUse";
import {
  PART_LEVEL_LABEL,
  PART_LEVELS,
  SLACK_SLOW,
  STATE_LABEL,
  STATE_TONE,
  accountLine,
  andList,
  cardSummary,
  cardTitle,
  keyedAccountLine,
  keyedDisconnectWords,
  noAppWords,
} from "./words";

/**
 * One connection's card: its state; Connect (Microsoft 365: a work or school account, or a
 * personal one), Reconnect, and Disconnect; while signing in, "Finish signing in in your browser"
 * with Cancel; what it can do, part by part; what Plenipo was allowed; who may use it; the people
 * (and Slack channels) it may send to without asking; and the owner's own app — Google's under
 * **Your Google app**, Microsoft's and Slack's under **Advanced**. HubSpot, Stripe, and the website
 * connect with a key typed into the card instead (ADR-071): **Save and check**, then **Replace the
 * key** or **Disconnect**.
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
  // Slack and Google: Plenipo cancels the sign-in at the service, which can take a moment.
  const [cancelling, setCancelling] = useState(false);
  const connected = c.state === "connected";
  const signedInOnce = c.state !== "notConnected";
  const partsOn = card.parts.some((p) => p.available && p.level !== "off");
  const canConnect = card.hasApp && page.vaultAvailable && partsOn && !card.signingIn && !pending;
  const connect = (kind: AccountKind) => void run(() => connectConnection(id, kind));
  const microsoft = c.service === "microsoft365";
  const keyed = card.usesKey;
  const live = c.service === "stripe" && c.granted.includes("live mode");
  const title = cardTitle(service, c);
  const titleId = `connection-${id}`;
  // Another Slack workspace's card can be removed while it is not connected.
  const removable =
    service.many && c.state === "notConnected" && !card.signingIn && service.connections.length > 1;
  // It opens by itself when it needs you (Phase 25, item 2.2).
  const needsYou =
    card.signingIn ||
    c.state === "needsSignIn" ||
    !!card.problem ||
    !!card.adminLink ||
    (connected && card.reconnectFor.length > 0);
  return (
    <li className={`connection connection--opens connection--${c.state}`} aria-labelledby={titleId}>
      <Disclosure
        title={title}
        headingId={titleId}
        status={{
          status: card.signingIn ? "pending" : STATE_TONE[c.state],
          label: card.signingIn
            ? "Waiting for you in your browser"
            : keyed && c.state === "needsSignIn"
              ? "Needs a new key"
              : STATE_LABEL[c.state],
        }}
        summary={cardSummary(card)}
        openWhen={needsYou}
        rememberAs={`connection:${id}`}
      >
        {connected && c.account && (
          <p className="connection__account">
            Connected {keyed ? "to" : "as"}{" "}
            <strong>{keyed ? keyedAccountLine(c) : accountLine(c.account, c.accountKind)}</strong>.
          </p>
        )}
        {connected && live && (
          <p className="notice-box" role="alert">
            <strong>Live mode: this key moves real money.</strong> Refunds and invoices still wait
            for you every time, and Stripe asks again for a key tagged for an agent. What an AI
            worker does through Stripe binds you, as Stripe&apos;s terms say.
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
        {c.state === "needsSignIn" && keyed && (
          <p className="notice-box" role="alert">
            <strong>{service.label} needs a new key.</strong> It no longer accepts the key (it was
            deleted, replaced, or revoked), so Plenipo removed it. Workers cannot use it until you
            type a new one below.
          </p>
        )}
        {c.state === "needsSignIn" && !keyed && (
          <p className="notice-box" role="alert">
            <strong>{service.label} needs you to sign in again.</strong> It no longer accepts
            Plenipo&apos;s sign-in (it expired, was removed, or your password changed
            {c.service === "slack" ? "; a Slack sign-in not used for 30 days ends" : ""}). Workers
            cannot use it until you sign in again.
          </p>
        )}
        {!card.hasApp && noAppWords(c.service) && (
          <p className="form-error">{noAppWords(c.service)}</p>
        )}
        {c.service === "slack" && !c.ownApp && card.builtInApp && (
          <p className="muted">{SLACK_SLOW}</p>
        )}
        {connected && card.reconnectFor.length > 0 && (
          <p className="notice-box" role="note">
            <strong>Reconnect to allow {andList(card.reconnectFor)}.</strong> You turned{" "}
            {card.reconnectFor.length === 1 ? "it" : "them"} on (or up to Full access) after
            connecting, and {service.label} has not allowed that yet.
          </p>
        )}
        {!partsOn && !connected && (
          <p className="muted">
            Turn on at least one part below, so Plenipo knows what to ask for.
          </p>
        )}
        {c.service === "google" && (
          <GoogleApp card={card} vaultLabel={page.vaultLabel} onApply={onApply} />
        )}
        {keyed && <KeyForm card={card} page={page} onApply={onApply} />}
        <div className="actions">
          {keyed ? null : connected ? (
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
                {c.state === "needsSignIn"
                  ? "Sign in again"
                  : microsoft
                    ? "Connect a work or school account"
                    : "Connect"}
              </Button>
              {microsoft && c.state === "notConnected" && !c.ownApp && (
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
                  {keyed ? (
                    keyedDisconnectWords(c.service, title, page.vaultLabel)
                  ) : (
                    <>
                      Disconnect {title}? Its tools stop now, and its sign-in is removed from{" "}
                      {page.vaultLabel}
                      {microsoft ? "" : ` and cancelled at ${service.label}`}.
                    </>
                  )}
                </span>
                <Button
                  variant="danger"
                  size="sm"
                  disabled={pending}
                  onClick={() => {
                    setCancelling(!microsoft && c.service !== "hubspot" && c.service !== "stripe");
                    void run(() => disconnectConnection(id)).then(() => {
                      setCancelling(false);
                      setConfirmDisconnect(false);
                    });
                  }}
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
          {removable && (
            <Button
              variant="quiet"
              size="sm"
              disabled={pending}
              onClick={() => void run(() => removeConnection(id))}
            >
              Remove this workspace
            </Button>
          )}
        </div>
        {cancelling && pending && (
          <p className="muted" role="status">
            {c.service === "wordpress"
              ? "Revoking the Application Password at your site…"
              : `Cancelling the sign-in at ${service.label}…`}
          </p>
        )}
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
                      options={PART_LEVELS.filter((l) => p.fullAccess || l !== "fullAccess").map(
                        (l) => ({ value: l, label: PART_LEVEL_LABEL[l] }),
                      )}
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
                    Read only: {p.reads}
                    {p.fullAccess && ` Full access: ${p.changes}`}
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
        {c.service !== "hubspot" && c.service !== "stripe" && (
          <SendList
            id={id}
            service={c.service}
            list={c.sendList}
            switchOn={page.sendSwitchOn}
            go={go}
            onApply={onApply}
          />
        )}
        {microsoft && <MicrosoftOwnApp card={card} onApply={onApply} />}
        {c.service === "slack" && <SlackOwnApp card={card} page={page} onApply={onApply} />}
      </Disclosure>
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
