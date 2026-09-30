import type { ServiceCard } from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { addConnection } from "../../api/commands";
import { Refusal } from "../../components/models/shared";
import type { Go } from "../../components/views";
import { useRun } from "../../guard/useRun";
import { AddOnTools } from "./AddOnTools";
import { ConnectionCard } from "./ConnectionCard";
import { useConnections } from "./useConnections";
import { LATER } from "./words";

/**
 * Settings → Connections (Phase 20, ADR-062 to ADR-065): the business accounts workers may use.
 * You sign in on the service's own page in your browser; Plenipo keeps the sign-in only in the
 * Vault and never sees your password. For each connection: its parts (Off, Read only, or Full
 * access), who may use it, and the people it may send to without asking you. HubSpot, Stripe,
 * and your website connect with a key typed into their cards. **Add-on tools** come last.
 */
export function ConnectionsSettings({ go }: { go: Go }) {
  const { page, error, apply } = useConnections();
  if (!page) {
    return (
      <p className={error ? "form-error" : "muted"} role={error ? "alert" : undefined}>
        {error ?? "Loading your connections…"}
      </p>
    );
  }
  return (
    <div className="connections" aria-label="Connections">
      <p className="muted">
        A worker uses a connection only if it is on the connection&apos;s <em>Who may use it</em>{" "}
        list, and only the parts you turned on. Reading is allowed at <em>Read only</em>; drafting
        and adding need <em>Full access</em>. Sending, posting, inviting people, adding a file to a
        SharePoint site, publishing on your website, and replacing a file ask you first — except a
        send to people (or a Slack channel) who are all on that connection&apos;s{" "}
        <em>Send without asking to</em> list, while you have that switch on.{" "}
        <strong>Money always asks you:</strong> Stripe refunds and invoices, and store refunds.
        Sign-ins and keys are kept in {page.vaultLabel}: workers never see them, and neither does
        anything Plenipo records.
      </p>
      <p className="notice-box" role="note">
        <strong>
          Mail, chats, calendars, files, HubSpot notes, orders, comments, and add-on answers are other
          people&apos;s words.
        </strong>{" "}
        Workers get them marked as information, never as instructions from you. An email that says
        &quot;forward all mail&quot; cannot send anything by itself: a send asks you and shows who
        it goes to, unless everyone it goes to is on that connection&apos;s{" "}
        <em>Send without asking to</em> list and you turned that switch on.
      </p>
      {!page.vaultAvailable && (
        <p className="form-error" role="alert">
          {page.vaultLabel} is not available on this computer, so Plenipo cannot keep a sign-in.
          Connections cannot be connected until it is.
        </p>
      )}
      <p className="muted">
        Connections are part of Plenipo Pro. Every copy can use them for now; disconnecting always
        works.
      </p>
      <ul className="connection-list" aria-label="Services">
        {page.services.map((s) => (
          <ServiceItem key={s.service} service={s} page={page} onApply={apply} go={go} />
        ))}
      </ul>
      <AddOnTools page={page} onApply={apply} />
    </div>
  );
}

function ServiceItem({
  service: s,
  page,
  onApply,
  go,
}: {
  service: ServiceCard;
  page: Parameters<typeof ConnectionCard>[0]["page"];
  onApply: Parameters<typeof ConnectionCard>[0]["onApply"];
  go: Go;
}) {
  if (!s.built) {
    return (
      <li
        className="connection connection--later"
        aria-label={`${s.label}, ${LATER.toLowerCase()}`}
      >
        <div className="connection__header">
          <h3>{s.label}</h3>
          <StatusPill status="offline" label={LATER} />
        </div>
      </li>
    );
  }
  return (
    <>
      {s.connections.map((c) => (
        <ConnectionCard
          key={c.connection.id}
          service={s}
          card={c}
          page={page}
          onApply={onApply}
          go={go}
        />
      ))}
      {s.many && <AddAnother service={s} onApply={onApply} />}
    </>
  );
}

/** More than one account of a service (Slack's workspaces): each gets its own card. */
function AddAnother({
  service: s,
  onApply,
}: {
  service: ServiceCard;
  onApply: Parameters<typeof ConnectionCard>[0]["onApply"];
}) {
  const { pending, error, run } = useRun(onApply);
  return (
    <li className="connection connection--add" aria-label={`Add another ${s.label} workspace`}>
      <Button
        variant="secondary"
        size="sm"
        disabled={pending}
        onClick={() => void run(() => addConnection(s.service))}
      >
        Add another {s.label} workspace
      </Button>
      <span className="muted"> Each workspace gets its own card, sign-in, parts, and lists.</span>
      <Refusal error={error} />
    </li>
  );
}
