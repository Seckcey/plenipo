/**
 * What a task cost, in plain words (I2): its tokens (pieces of words) and, on paid keys, its
 * money. The owner's rules: a subscription's tools show tokens only (no money was spent); paid
 * keys show real dollars and how they were priced; a run stopped before it reported says "at
 * least"; while any of it runs, "so far".
 */
import type { CostPricedBy, TaskCost } from "@plenipo/types";

import { tokens } from "../routing/format";
import { dollars } from "./words";

/** Its tokens: "48K tokens", "at least 48K tokens"; `null` when no run reported any. */
export function tokensWords(cost: TaskCost): string | null {
  if (cost.counted === 0) return null;
  const total = cost.read + cost.written;
  return `${cost.atLeast ? "at least " : ""}${tokens(total)} tokens`;
}

/** What they were: "9,000 read (7,000 reused) · 3,000 written". */
export function tokensDetail(cost: TaskCost): string {
  const reused = cost.reused > 0 ? ` (${tokens(cost.reused)} reused)` : "";
  return `${tokens(cost.read)} read${reused} · ${tokens(cost.written)} written`;
}

/** Its money on paid keys, the amount alone: "$0.31"; `null` when nothing went through one. */
export function moneyAmount(cost: TaskCost): string | null {
  const counted = cost.spentMicros + cost.notPricedMicros;
  if (counted === 0 && cost.setAsideMicros === 0 && cost.notPriced === 0) return null;
  return dollars(counted);
}

/** Its money on paid keys: "$0.31", with "so far" while a paid request still runs. */
export function moneyWords(cost: TaskCost): string | null {
  const amount = moneyAmount(cost);
  return amount && cost.setAsideMicros > 0 ? `${amount} so far` : amount;
}

const PRICED_BY: Record<CostPricedBy, string> = {
  service: "priced by the AI company's own bill",
  priceList: "priced from Plenipo's price list",
  both: "priced by the AI company's bill and Plenipo's price list",
};

/** How its money was worked out, for when its line is opened. `null` without a paid key. */
export function pricedWords(cost: TaskCost): string | null {
  const parts: string[] = [];
  if (cost.spentMicros > 0 || cost.pricedBy) {
    parts.push(
      `spent ${dollars(cost.spentMicros)}${cost.pricedBy ? `, ${PRICED_BY[cost.pricedBy]}` : ""}`,
    );
  }
  if (cost.notPriced > 0) {
    parts.push(`not priced yet: counted as ${dollars(cost.notPricedMicros)}`);
  }
  if (cost.setAsideMicros > 0) {
    parts.push(`still running: up to ${dollars(cost.setAsideMicros)} set aside`);
  }
  return parts.length > 0 ? parts.join(" · ") : null;
}
