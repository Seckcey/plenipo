import { useEffect } from "react";
import type { SpendingPage } from "@plenipo/types";
import { Banner, Button } from "@plenipo/ui";

import { getSpending } from "../api/commands";
import type { Go } from "../components/views";
import { useLive } from "../pages/useLive";
import { dollars, resetDay, untilTurnover } from "./words";

/**
 * The banner on every page when a spending cap needs the owner (Phase 16 Wave 3, ADR-085): paid
 * AI work stopped under a cap, or 80% of a cap is used. It stays until the cap is raised or the
 * month starts over; Spending caps opens the page to change it.
 */
export function SpendingBanner({ go }: { go: Go }) {
  const live = useLive<SpendingPage>(
    "spending-banner",
    () => getSpending(),
    (e) => e.eventType.startsWith("spending.") || e.eventType === "org.settings_changed",
  );
  const page = live.value;
  // The month starts over while Plenipo is open: the banner goes with it.
  const resetsAt = page?.resetsAt;
  const { reload } = live;
  useEffect(() => {
    if (resetsAt === undefined) return undefined;
    const timer = setTimeout(reload, untilTurnover(resetsAt));
    return () => clearTimeout(timer);
  }, [resetsAt, reload]);
  if (!page) return null;
  const stopped = page.caps.filter((c) => c.state === "stopped" && !c.gone);
  const warned = page.caps.filter((c) => c.state === "warning" && !c.gone);
  if (stopped.length === 0 && warned.length === 0) return null;
  const open = (
    <Button size="sm" variant="primary" onClick={() => go({ view: "settings", id: "spending" })}>
      Spending caps
    </Button>
  );
  const first = stopped[0] ?? warned[0]!;
  const name =
    first.cap.covers.kind === "business" ? "the business's cap" : `the cap for ${first.label}`;
  if (stopped.length > 0) {
    return (
      <Banner
        tone="error"
        role="alert"
        className="banner--spending"
        title={
          stopped.length === 1
            ? `Paid AI work stopped under ${name}`
            : `Paid AI work stopped under ${stopped.length} spending caps`
        }
        action={open}
      >
        <div className="muted">
          {stopped.length === 1 && first.stoppedWhy ? `${first.stoppedWhy} ` : ""}
          Raise the cap, or wait until {resetDay(page.resetsAt)}, when the month starts over.
        </div>
      </Banner>
    );
  }
  return (
    <Banner
      tone="warn"
      role="status"
      className="banner--spending"
      title={
        warned.length === 1
          ? `80% or more of ${name} is used`
          : `80% or more of ${warned.length} spending caps is used`
      }
      action={open}
    >
      {warned.length === 1 && (
        <div className="muted">
          {dollars(first.spentMicros)} of {dollars(first.cap.monthlyMicros)} spent this month. Paid
          AI work stops at the cap.
        </div>
      )}
    </Banner>
  );
}
