import type { AccessLevel, Account, AccountKind, ConnectionState, PartLevel } from "@plenipo/types";
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

/** "Mail, Calendar, and Teams". */
export function andList(items: readonly string[]): string {
  if (items.length <= 1) return items.join("");
  if (items.length === 2) return `${items[0]} and ${items[1]}`;
  return `${items.slice(0, -1).join(", ")}, and ${items[items.length - 1]}`;
}
