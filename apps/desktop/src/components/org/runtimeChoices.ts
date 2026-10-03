import type { OrgSnapshot } from "@plenipo/types";

/** "Grok", or "Grok (not ready)". */
export function runtimeChoiceLabel(snapshot: OrgSnapshot, id: string): string {
  const r = snapshot.runtimes.find((x) => x.id === id);
  if (!r) return id;
  return r.ready ? r.label : `${r.label} (not ready)`;
}

/**
 * For a paid AI tool whose key isn't working: what to do, naming the same company's subscription
 * when there is one ("xAI's key isn't set up. To use your Grok subscription, choose Grok.").
 */
export function subscriptionInstead(snapshot: OrgSnapshot, id: string): string | null {
  const r = snapshot.runtimes.find((x) => x.id === id);
  if (!r?.paid || r.ready) return null;
  const subscription = snapshot.runtimes.find((x) => !x.paid && x.company === r.company);
  return subscription
    ? `${r.label}'s key isn't set up. To use your ${subscription.label} subscription, choose ${subscription.label}.`
    : `${r.label}'s key isn't set up: add it on its card on the AI tools page.`;
}
