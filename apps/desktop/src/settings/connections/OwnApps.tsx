import { useState } from "react";
import type { ConnectionCard as Card, ConnectionsPage } from "@plenipo/types";
import { Button, TextField } from "@plenipo/ui";

import { saveConnectionApp, setConnectionOwnApp } from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import { useRun } from "../../guard/useRun";

/** Where the setup steps for the owner's own Slack and Google apps are. */
export const APP_STEPS = "Setting up your Slack and Google apps, in Plenipo's documentation";

/** Why the app cannot change now, or null. */
function lockedWhy(card: Card): string | null {
  if (card.signingIn)
    return "Finish or cancel the sign-in first: it belongs to the app it started with.";
  if (card.connection.state !== "notConnected") {
    return "Disconnect first to change it: a sign-in belongs to the app it was made with.";
  }
  return null;
}

/**
 * Advanced (Microsoft 365): sign in with the organization's own Microsoft app (its app ID is
 * not a secret).
 */
export function MicrosoftOwnApp({
  card,
  onApply,
}: {
  card: Card;
  onApply: (page: ConnectionsPage) => void;
}) {
  const c = card.connection;
  const [appId, setAppId] = useState(c.ownApp?.appId ?? "");
  const [tenant, setTenant] = useState(c.ownApp?.tenant ?? "");
  const { pending, error, run } = useRun(onApply);
  const locked = lockedWhy(card);
  return (
    <details className="connection__section connection__advanced">
      <summary>Advanced</summary>
      <p className="muted">
        Plenipo signs in with 8 West&apos;s Microsoft app. An organization that wants its own can
        register one (the steps are in Plenipo&apos;s documentation) and give its Microsoft app ID
        and its domain here. An app ID is not a secret; Plenipo never asks for a Microsoft
        app&apos;s secret.
      </p>
      {locked ? (
        <p className="muted">
          {c.ownApp
            ? `Using your organization's own app (${c.ownApp.appId}, ${c.ownApp.tenant ?? ""}).`
            : "Using 8 West's app."}{" "}
          {locked}
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
                  setConnectionOwnApp(c.id, {
                    appId: appId.trim(),
                    tenant: tenant.trim(),
                    secretKept: false,
                  }),
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

/**
 * Advanced (Slack): the workspace's own Slack app, made from Plenipo's app description (Slack's
 * "manifest"), by its client ID (not a secret). It reads at Slack's normal speed.
 */
export function SlackOwnApp({
  card,
  page,
  onApply,
}: {
  card: Card;
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
}) {
  const c = card.connection;
  const [clientId, setClientId] = useState(c.ownApp?.appId ?? "");
  const [copied, setCopied] = useState(false);
  const { pending, error, run } = useRun(onApply);
  const locked = lockedWhy(card);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(page.slackManifest);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return (
    <details className="connection__section connection__advanced">
      <summary>Advanced</summary>
      <p className="muted">
        <strong>Use your workspace&apos;s own Slack app</strong> instead of 8 West&apos;s. Slack
        reads at its normal speed for an app a workspace makes for itself. Go to api.slack.com/apps,
        choose Create New App, then From a manifest (Slack&apos;s words), pick your workspace, and
        paste Plenipo&apos;s app description below. Then copy the new app&apos;s Client ID (Basic
        Information, App Credentials) here. A client ID is not a secret; Plenipo never asks for a
        Slack app&apos;s secret. The steps: {APP_STEPS}.
      </p>
      <pre
        className="connection__manifest"
        role="region"
        tabIndex={0}
        aria-label="Plenipo's app description for Slack"
      >
        {page.slackManifest}
      </pre>
      <Button variant="secondary" size="sm" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy the app description"}
      </Button>
      {locked ? (
        <p className="muted">
          {c.ownApp ? `Using your workspace's own app (${c.ownApp.appId}).` : "Using 8 West's app."}{" "}
          {locked}
        </p>
      ) : (
        <>
          <TextField
            label="Your Slack app's client ID"
            value={clientId}
            placeholder="1234567890.9876543210"
            onChange={setClientId}
          />
          <div className="actions">
            <Button
              variant="primary"
              size="sm"
              disabled={pending || clientId.trim() === ""}
              onClick={() => void run(() => saveConnectionApp(c.id, { clientId: clientId.trim() }))}
            >
              Use this app
            </Button>
            {c.ownApp && (
              <Button
                variant="quiet"
                size="sm"
                disabled={pending}
                onClick={() =>
                  void run(() => saveConnectionApp(c.id, null)).then((ok) => {
                    if (ok) setClientId("");
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

/**
 * **Your Google app** (ADR-069 §4): the owner's own Google app, by its client ID and its secret.
 * The secret goes straight to the Vault and is never shown again; the box hides what you type.
 */
export function GoogleApp({
  card,
  vaultLabel,
  onApply,
}: {
  card: Card;
  vaultLabel: string;
  onApply: (page: ConnectionsPage) => void;
}) {
  const c = card.connection;
  const [clientId, setClientId] = useState("");
  const [secret, setSecret] = useState("");
  const { pending, error, run } = useRun(onApply);
  const locked = lockedWhy(card);
  const saved = c.ownApp?.secretKept ? c.ownApp : null;
  return (
    <section className="connection__section" aria-labelledby={`${c.id}-app`}>
      <h4 id={`${c.id}-app`}>Your Google app</h4>
      {saved ? (
        <>
          <p>
            Client ID: <span className="path">{saved.appId}</span>. Its secret is kept in{" "}
            {vaultLabel}.
          </p>
          {locked ? (
            <p className="muted">{locked}</p>
          ) : (
            <Button
              variant="quiet"
              size="sm"
              disabled={pending}
              onClick={() => void run(() => saveConnectionApp(c.id, null))}
            >
              Remove this app
            </Button>
          )}
        </>
      ) : (
        <form
          onSubmit={(ev) => {
            ev.preventDefault();
            void run(() =>
              saveConnectionApp(c.id, { clientId: clientId.trim(), secret: secret.trim() }),
            ).then(() => setSecret(""));
          }}
        >
          <p className="muted">
            Google connects through your own Google app, made in your own Google Cloud project (a
            Desktop app; Internal if you use Google Workspace). The steps: {APP_STEPS}. Its secret
            goes only to {vaultLabel}, and is never shown again.
          </p>
          <TextField
            label="Client ID"
            value={clientId}
            placeholder="1234567890-abc123.apps.googleusercontent.com"
            onChange={setClientId}
          />
          <label className="field">
            <span>Client secret</span>
            <input
              type="password"
              autoComplete="off"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
            />
          </label>
          <div className="actions">
            <Button
              type="submit"
              variant="primary"
              size="sm"
              disabled={pending || clientId.trim() === "" || secret.trim() === ""}
            >
              Save
            </Button>
          </div>
        </form>
      )}
      <Refusal error={error} />
    </section>
  );
}
