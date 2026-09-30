import { useState } from "react";
import type { ConnectionCard as Card, ConnectionsPage, KeyInput, Service } from "@plenipo/types";
import { Button, TextField } from "@plenipo/ui";

import { saveConnectionKey } from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import { useRun } from "../../guard/useRun";

/** Where the click-by-click steps for making each key are. */
export const KEY_STEPS =
  "Making keys for HubSpot, Stripe, and your website, in Plenipo's documentation";

/** A box that hides what you type: its value is sent once, to the Vault, and never shown. */
function SecretBox({
  label,
  value,
  onChange,
  hint,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  hint?: string;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <input
        type="password"
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      {hint && <small className="muted">{hint}</small>}
    </label>
  );
}

/** How each keyed service's key is made, in a sentence. */
function whereWords(service: Service): string {
  switch (service) {
    case "hubspot":
      return "Make a service key in HubSpot (Development → Keys → Service keys) with only the permissions below. Service keys are a HubSpot beta; a private app's key you already have works too.";
    case "stripe":
      return "Make a restricted key in Stripe (Developers → API keys → Create restricted key) with only the permissions below, and choose Authorizing agent access when Stripe asks what it is for. Start with a test-mode key (it starts rk_test_). A live key moves real money.";
    default:
      return "Make a WordPress user just for Plenipo (Editor, or Shop Manager for the store), then an Application Password for it (Users → Profile → Application Passwords). It can do everything that user can. A WooCommerce key (WooCommerce → Settings → Advanced → REST API) is optional: a Read key keeps the store read-only.";
  }
}

/**
 * **Save and check** a key (ADR-071 §1): HubSpot's service key, Stripe's restricted key, or the
 * website's address, user, and Application Password (and an optional WooCommerce key). Plenipo
 * checks it with one reading call and keeps it only in the Vault; it is never shown again. While
 * connected, **Replace the key** takes a new key for the same account.
 */
export function KeyForm({
  card,
  page,
  onApply,
}: {
  card: Card;
  page: ConnectionsPage;
  onApply: (page: ConnectionsPage) => void;
}) {
  const c = card.connection;
  const service = c.service;
  const connected = c.state === "connected";
  const [key, setKey] = useState("");
  const [site, setSite] = useState(c.site ?? "");
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  const [storeKey, setStoreKey] = useState("");
  const [storeSecret, setStoreSecret] = useState("");
  const { pending, error, run } = useRun(onApply);
  const website = service === "wordpress";
  const partsOn = card.parts.some((p) => p.available && p.level !== "off");
  const typed = website
    ? site.trim() !== "" && user.trim() !== "" && password.trim() !== ""
    : key.trim() !== "";
  const clear = () => {
    setKey("");
    setPassword("");
    setStoreKey("");
    setStoreSecret("");
  };
  const input: KeyInput = website
    ? {
        site: site.trim(),
        user: user.trim(),
        password,
        ...(storeKey.trim() || storeSecret.trim()
          ? { storeKey: storeKey.trim(), storeSecret: storeSecret.trim() }
          : {}),
      }
    : { key: key.trim() };
  return (
    <section className="connection__section" aria-labelledby={`${c.id}-key`}>
      <h4 id={`${c.id}-key`}>{connected ? "Replace the key" : "Its key"}</h4>
      <p className="muted">
        {whereWords(service)} The steps: {KEY_STEPS}. What you type goes only to {page.vaultLabel},
        and is never shown again. Never paste a key into a chat.
      </p>
      {card.keyNeeds.length > 0 && (
        <p className="muted">
          Give the key these permissions in {service === "hubspot" ? "HubSpot" : "Stripe"}, for the
          parts you turned on: <span className="path">{card.keyNeeds.join(", ")}</span>
          {service === "hubspot" && " (a note on any record needs crm.objects.contacts.write)"}.
        </p>
      )}
      <form
        onSubmit={(ev) => {
          ev.preventDefault();
          void run(() => saveConnectionKey(c.id, input)).then((ok) => {
            if (ok) clear();
          });
        }}
      >
        {website ? (
          <>
            {connected ? (
              <p>
                Your site: <span className="path">{c.site}</span>. To change it, disconnect first.
              </p>
            ) : (
              <TextField
                label="Your site's address"
                value={site}
                placeholder="https://example.com"
                hint="As your browser shows it once the site has loaded, with https://."
                onChange={setSite}
              />
            )}
            <TextField label="WordPress user name" value={user} onChange={setUser} />
            <SecretBox label="Application Password" value={password} onChange={setPassword} />
            <details className="connection__advanced">
              <summary>WooCommerce key (optional)</summary>
              <SecretBox label="Consumer key (ck_…)" value={storeKey} onChange={setStoreKey} />
              <SecretBox
                label="Consumer secret (cs_…)"
                value={storeSecret}
                onChange={setStoreSecret}
              />
            </details>
          </>
        ) : (
          <SecretBox
            label={service === "hubspot" ? "Service key" : "Restricted key"}
            value={key}
            onChange={setKey}
          />
        )}
        <div className="actions">
          <Button
            type="submit"
            variant="primary"
            size="sm"
            disabled={pending || !typed || !page.vaultAvailable || !partsOn}
          >
            {pending ? "Checking…" : connected ? "Replace the key" : "Save and check"}
          </Button>
        </div>
      </form>
      <Refusal error={error} />
    </section>
  );
}
