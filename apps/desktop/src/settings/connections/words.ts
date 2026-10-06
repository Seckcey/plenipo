import type {
  AccessLevel,
  Account,
  AccountKind,
  Connection,
  ConnectionCard,
  ConnectionState,
  GithubRepositories,
  PartLevel,
  Service,
  ServiceCard,
} from "@plenipo/types";
import type { Status } from "@plenipo/ui";

/** Where a connection stands, in plain words (docs/design/vocabulary.md). */
export const STATE_LABEL: Record<ConnectionState, string> = {
  notConnected: "Not connected",
  connected: "Connected",
  needsSignIn: "Needs you to sign in again",
};

export const STATE_TONE: Record<ConnectionState, Status> = {
  notConnected: "offline",
  connected: "ok",
  needsSignIn: "warn",
};

/** A service built into a later update of Plenipo. */
export const LATER = "Coming in a later update";

export const PART_LEVEL_LABEL: Record<PartLevel, string> = {
  off: "Off",
  readOnly: "Read only",
  fullAccess: "Full access",
};

export const PART_LEVELS: readonly PartLevel[] = ["off", "readOnly", "fullAccess"];

export const ACCESS_LABEL: Record<AccessLevel, string> = {
  readOnly: "Read only",
  readWrite: "Read and write",
};

export const ACCOUNT_KIND_LABEL: Record<AccountKind, string> = {
  work: "a work or school account",
  personal: "a personal account",
};

/** "alex@8westit.com (a work or school account, 8 West IT)". */
export function accountLine(
  account: Account | null | undefined,
  kind: AccountKind | null | undefined,
): string {
  if (!account) return "";
  const about = [kind ? ACCOUNT_KIND_LABEL[kind] : null, account.organization ?? null].filter(
    (x): x is string => !!x,
  );
  const who = account.address || account.name;
  return about.length > 0 ? `${who} (${about.join(", ")})` : who;
}

/**
 * A card's heading: the service's name; for a Slack workspace, its name once connected, or which
 * card it is before ("Slack", "Slack — 8 West IT", "Slack — workspace 2").
 */
export function cardTitle(service: ServiceCard, c: Connection): string {
  if (c.service !== "slack") return service.label;
  const workspace = c.account?.organization;
  if (workspace) return `${service.label} — ${workspace}`;
  const n = /^slack-(\d+)$/.exec(c.id)?.[1];
  return n ? `${service.label} — workspace ${n}` : service.label;
}

/** A copy of Plenipo with no app to sign in to `service` with, in plain words. */
export function noAppWords(service: Service): string | null {
  switch (service) {
    case "microsoft365":
      return "This copy of Plenipo has no Microsoft app ID yet, so it cannot sign in. Your organization can use its own app ID under Advanced below.";
    case "slack":
      return "This copy of Plenipo has no Slack app yet, so it cannot sign in with 8 West's. Your workspace can use its own Slack app under Advanced below.";
    case "github":
      return "This copy of Plenipo has no GitHub app yet, so it cannot sign in. An update brings it.";
    default:
      // Google's own app is asked for on its card.
      return null;
  }
}

/**
 * Who a keyed connection is connected to: "HubSpot account 24681357", "8 West IT (Test mode)",
 * "Plenipo (a WordPress user) at https://shop.example.com".
 */
export function keyedAccountLine(c: Connection): string {
  const name = c.account?.name ?? "";
  switch (c.service) {
    case "stripe":
      return c.account?.organization ? `${name} (${c.account.organization})` : name;
    case "wordpress":
      return `${name} (a WordPress user) at ${c.site ?? ""}`;
    default:
      return name;
  }
}

/** The ID of the warning about the "Send without asking to" lists, said once on the page. */
export const SEND_LIST_NOTE = "send-list-note";

/** How many roles or agents may use something, in plain words. */
export function whoMayUseWords(count: number): string {
  if (count === 0) return "Nobody may use it yet";
  return count === 1 ? "1 role or agent may use it" : `${count} roles or agents may use it`;
}

/**
 * One line for a card while it is closed (Phase 25, item 2.2): who it is connected as, how many
 * of its parts are on, and how many roles or agents may use it.
 */
export function cardSummary(card: ConnectionCard): string {
  const c = card.connection;
  const parts = card.parts.filter((p) => p.available);
  const on = parts.filter((p) => p.level !== "off").length;
  const account =
    c.state === "connected" && c.account
      ? card.usesKey
        ? keyedAccountLine(c)
        : accountLine(c.account, c.accountKind)
      : null;
  return [
    account,
    parts.length > 0 ? `${on} of ${parts.length} parts on` : null,
    whoMayUseWords(c.access.length),
  ]
    .filter((x): x is string => !!x)
    .join(" · ");
}

/** What Disconnect does for a keyed connection, in plain words. */
export function keyedDisconnectWords(service: Service, title: string, vault: string): string {
  switch (service) {
    case "wordpress":
      return `Disconnect ${title}? Its tools stop now, the Application Password and any WooCommerce key are removed from ${vault}, and the password is revoked at your site. If you gave a WooCommerce key, revoke it in WooCommerce too (WooCommerce → Settings → Advanced → REST API).`;
    case "hubspot":
      return `Disconnect ${title}? Its tools stop now, and its key is removed from ${vault}. HubSpot has no way to cancel a key from outside: delete it in HubSpot too (Development → Keys → Service keys).`;
    default:
      return `Disconnect ${title}? Its tools stop now, and its key is removed from ${vault}. Stripe has no way to cancel a key from outside: delete it in Stripe too (Developers → API keys).`;
  }
}

/** Slack's limit on 8 West's app while it is outside Slack's Marketplace (ADR-070 §3). */
export const SLACK_SLOW =
  "With 8 West's Slack app, Slack lets Plenipo read one channel or thread a minute, 15 messages at a time. Your workspace's own Slack app (Advanced) reads at Slack's normal speed.";

/** What GitHub lets Plenipo see (ADR-204, word for word on the card). */
export const GITHUB_SEES =
  "names, descriptions, branch and tag names, and who collaborates; never code";

/** "Frankie G (frankieg)", or just "frankieg" when GitHub has no other name. */
export function githubAccountLine(account: Account): string {
  const login = account.address;
  return account.name && account.name !== login ? `${account.name} (${login})` : login;
}

/** GitHub's card while it is closed: who it is connected as, that it is yours alone, and free. */
export function githubCardSummary(card: ConnectionCard): string {
  const c = card.connection;
  const who = c.state === "connected" && c.account ? githubAccountLine(c.account) : null;
  return [who, "Yours alone", "Free"].filter((x): x is string => !!x).join(" · ");
}

/** "1 repository", "48 repositories", "More than 1000 repositories". */
export function repositoryCount(list: GithubRepositories): string {
  const n = list.repositories.length;
  if (list.more) return `More than ${n} repositories`;
  return n === 1 ? "1 repository" : `${n} repositories`;
}

/** "Mail, Calendar, and Teams". */
export function andList(items: readonly string[]): string {
  if (items.length <= 1) return items.join("");
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}
