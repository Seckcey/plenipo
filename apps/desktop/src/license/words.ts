import type { FreeLimits, LicenseView, Plan, SubscriptionState } from "@plenipo/types";

/**
 * Words for Free and Pro (Phase 11A, ADR-021, ADR-022): Settings → License, the "part of Pro"
 * notes, and the Activity trail. Plain words: "Free" and "Plenipo Pro", "license key", "the
 * weekly check with 8 West" (docs/design/vocabulary.md).
 */

/** A day, as the screen shows it: "Oct 30, 2026". */
export function day(ms: number): string {
  return new Date(ms).toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });
}

/** What Free includes, in one line: "1 organization, 1 department, 1 project, and 3 workers…". */
export function freeLine(limits: FreeLimits): string {
  const n = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;
  return `${n(limits.organizations, "organization", "organizations")}, ${n(
    limits.departments,
    "department",
    "departments",
  )}, ${n(limits.projects, "project", "projects")}, and ${n(
    limits.workersAtOnce,
    "worker",
    "workers",
  )} on the job at a time`;
}

/** "Plenipo Pro" or "Free". */
export function editionName(view: Pick<LicenseView, "edition">): string {
  return view.edition === "pro" ? "Plenipo Pro" : "Free";
}

export const PLAN_WORDS: Record<Plan, string> = {
  monthly: "Monthly",
  yearly: "Yearly",
};

/** Where to buy or renew Pro. */
export const WHERE_TO_BUY = "getplenipo.com";

/** Why the edition is what it is, in a sentence or two. */
export function reasonWords(view: LicenseView, now: number = Date.now()): string {
  switch (view.reason) {
    case "noKey":
      return `You're on Free: ${freeLine(view.freeLimits)}. Every safety feature and every AI tool is included. Plenipo Pro adds more organizations, departments, projects, and workers, business departments, lessons, Connections, and add-on tools. Get it at ${WHERE_TO_BUY}.`;
    case "notCheckedYet":
      return view.lastTried
        ? `Pro is on. Plenipo hasn't reached 8 West yet to check the key, and keeps trying${
            view.graceEnds ? `; Pro stays on until ${day(view.graceEnds)} without a check` : ""
          }.`
        : "Pro is on. Plenipo is checking the key with 8 West.";
    case "active":
      return view.paidThrough && view.paidThrough > now
        ? `Pro is paid through ${day(view.paidThrough)}.`
        : "Pro is paid.";
    case "cancelling":
      return `Pro was cancelled and stays on until ${
        view.endsAt ? day(view.endsAt) : "the end of the paid period"
      }. Then Plenipo goes back to Free, and nothing you made is taken away.`;
    case "ended":
      return `Your Pro subscription ended${
        view.endsAt ? ` on ${day(view.endsAt)}` : ""
      }, so Plenipo is on Free. Everything you made is still here. Renew Pro at ${WHERE_TO_BUY}, and the same key works again.`;
    case "noCheck":
      return "Plenipo is on Free for now: it hasn't reached 8 West for 30 days. It keeps trying, and Pro comes back by itself when a check goes through. Everything you made is still here.";
  }
}

/** A subscription's state, as the Activity trail says it. */
const STATE_WORDS: Record<SubscriptionState, string> = {
  active: "Pro is paid",
  cancelled: "Pro was cancelled and ends at the end of the paid period",
  ended: "the Pro subscription has ended",
  unknown: "8 West doesn't know this key yet",
};

const REASON_WORDS: Record<string, string> = {
  noKey: "no license key",
  notCheckedYet: "a new key",
  active: "paid",
  cancelling: "cancelled, until the paid period ends",
  ended: "the subscription ended",
  noCheck: "no check with 8 West for 30 days",
};

const str = (v: unknown): string | null => (typeof v === "string" && v !== "" ? v : null);

/** The license's events on the Activity trail (never the key: its ID only). */
export function describeLicenseEvent(type: string, p: Record<string, unknown>): string | null {
  const id = str(p.keyId) ? ` (${str(p.keyId)})` : "";
  switch (type) {
    case "license.key_entered":
      return `You entered a license key${id}`;
    case "license.key_removed":
      return `You removed the license key${id}`;
    case "license.checked":
      return `The weekly check with 8 West: ${
        STATE_WORDS[(str(p.state) ?? "unknown") as SubscriptionState] ?? "answered"
      }`;
    case "license.check_failed":
      return `The weekly check with 8 West didn't go through${
        str(p.problem) ? `: ${str(p.problem)}` : ""
      }. Plenipo tries again later.`;
    case "license.edition_changed": {
      const why = REASON_WORDS[str(p.reason) ?? ""];
      return p.to === "pro"
        ? "Plenipo Pro is on"
        : `Plenipo is on Free now${why ? ` (${why})` : ""}. Nothing you made was taken away.`;
    }
  }
  return null;
}
