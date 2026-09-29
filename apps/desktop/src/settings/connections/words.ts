import type {
  AccessLevel,
  Account,
  AccountKind,
  Connection,
  ConnectionState,
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

/** "frankie@8westit.com (a work or school account, 8 West IT)". */
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
    default:
      // Google's own app is asked for on its card.
      return null;
  }
}

/** Slack's limit on 8 West's app while it is outside Slack's Marketplace (ADR-070 §3). */
export const SLACK_SLOW =
  "With 8 West's Slack app, Slack lets Plenipo read one channel or thread a minute, 15 messages at a time. Your workspace's own Slack app (Advanced) reads at Slack's normal speed.";

/** "Mail, Calendar, and Teams". */
export function andList(items: readonly string[]): string {
  if (items.length <= 1) return items.join("");
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}
