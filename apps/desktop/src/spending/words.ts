import type { CapCovers, CapStatus, Price, SpendingRecord } from "@plenipo/types";

/**
 * Words and money for the Spending caps page (Phase 16 Wave 3, ADR-085). Money comes from Core
 * as whole millionths of a dollar ("micros") and is shown the way the Ledger writes it.
 */

export const MICROS_PER_DOLLAR = 1_000_000;
/** The smallest and largest caps (as the Ledger checks them). */
export const MIN_CAP_MICROS = 10_000;
export const MAX_CAP_MICROS = 1_000_000 * MICROS_PER_DOLLAR;

/** What a paid model costs: "$3.00 a million tokens read, $15.00 a million written". */
export function priceWords(price: Price): string {
  return `${dollars(price.input)} a million tokens read, ${dollars(price.output)} a million written`;
}

/** Money: "$12.34"; under a cent, "$0.0042"; nothing, "$0.00" (as the Ledger writes it). */
export function dollars(micros: number): string {
  if (micros > 0 && micros < 10_000) {
    const tenThousandths = Math.max(1, Math.min(9_999, Math.ceil(micros / 100)));
    return `$0.${String(tenThousandths).padStart(4, "0")}`;
  }
  const cents = Math.floor((micros + 5_000) / 10_000);
  const whole = Math.floor(cents / 100);
  return `$${whole.toLocaleString("en-US")}.${String(cents % 100).padStart(2, "0")}`;
}

/**
 * What the owner typed as a monthly amount ("50", "12.50", "$1,000"), in micros; null when it
 * is not an amount of dollars and cents from $0.01 to $1,000,000.
 */
export function parseDollars(text: string): number | null {
  const typed = text.trim().replace(/^\$/, "");
  // A comma only between groups of three digits ("1,000"), never as a decimal point ("12,50").
  if (typed.includes(",") && !/^\d{1,3}(,\d{3})+(\.\d{1,2})?$/.test(typed)) return null;
  const cleaned = typed.replace(/,/g, "");
  const match = /^(\d{1,7})(?:\.(\d{1,2}))?$/.exec(cleaned);
  if (!match) return null;
  const micros =
    Number(match[1]) * MICROS_PER_DOLLAR + Number((match[2] ?? "").padEnd(2, "0")) * 10_000;
  return micros >= MIN_CAP_MICROS && micros <= MAX_CAP_MICROS ? micros : null;
}

/** An amount as the owner would type it again: "50.00" → "50", "12.50" stays. */
export function typedAmount(micros: number): string {
  const cents = Math.round(micros / 10_000);
  return cents % 100 === 0 ? String(cents / 100) : (cents / 100).toFixed(2);
}

/**
 * How long until `resetsAt` (the month starts over), for a timer that reloads then: at most an
 * hour (a browser timer longer than about 24 days fires at once), at least a second.
 */
export function untilTurnover(resetsAt: number, now = Date.now()): number {
  return Math.min(Math.max(resetsAt - now + 1_000, 1_000), 60 * 60 * 1_000);
}

export const AMOUNT_HELP =
  "Type an amount in dollars, like 50 or 12.50 (from $0.01 to $1,000,000 a month).";

const PACIFIC = "America/Los_Angeles";

/** "November 1" (Pacific time). */
export function resetDay(ms: number): string {
  return new Intl.DateTimeFormat("en-US", {
    timeZone: PACIFIC,
    month: "long",
    day: "numeric",
  }).format(new Date(ms));
}

/** "October 2026" from "2026-10". */
export function monthName(month: string): string {
  const [year, number] = month.split("-").map(Number);
  if (!year || !number) return month;
  return new Intl.DateTimeFormat("en-US", {
    timeZone: "UTC",
    month: "long",
    year: "numeric",
  }).format(new Date(Date.UTC(year, number - 1, 15)));
}

/** "Oct 3, 2:15 PM" (Pacific time). */
export function when(ms: number): string {
  return new Intl.DateTimeFormat("en-US", {
    timeZone: PACIFIC,
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(ms));
}

/** Where a cap stands, in words and as a status mark. */
export function capStateWords(status: CapStatus): {
  word: string;
  status: "ok" | "warn" | "error";
} {
  switch (status.state) {
    case "stopped":
      return { word: "Stopped", status: "error" };
    case "warning":
      return { word: "80% or more used", status: "warn" };
    default:
      return { word: "Under 80%", status: "ok" };
  }
}

/** The bar's caption: "$12.00 of $50.00 spent · $1.20 set aside for running tasks". */
export function capLine(status: CapStatus): string {
  const spent = `${dollars(status.spentMicros)} of ${dollars(status.cap.monthlyMicros)} spent`;
  return status.setAsideMicros > 0
    ? `${spent} · ${dollars(status.setAsideMicros)} set aside for running tasks`
    : spent;
}

/** A stable key for what a cap covers. */
export function coversKey(covers: CapCovers): string {
  return covers.kind === "business" ? "business" : `${covers.kind}:${covers.id}`;
}

/** What a paid task cost, in words. */
export function recordCost(record: SpendingRecord): string {
  switch (record.state) {
    case "spent":
      return dollars(record.spentMicros ?? 0);
    case "notPriced":
      return `Not priced yet (counted as ${dollars(record.setAsideMicros)})`;
    case "released":
      return "Not sent: nothing spent";
    default:
      return `Running: up to ${dollars(record.setAsideMicros)} set aside`;
  }
}

/** Who a paid task was for: "Chief of Staff · Operations", or "A worker". */
export function recordWho(record: SpendingRecord): string {
  const parts = [record.positionTitle, record.departmentName].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : "A worker";
}
