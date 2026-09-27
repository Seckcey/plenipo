import { useMemo } from "react";
import { Gallery, type EntityCardProps, type ThemeName } from "@plenipo/ui";

import { useActivity } from "../ledger/useActivity";
import { organizationCards } from "../org/cards";
import { useOrganization } from "../org/useOrganization";

/**
 * The Gallery (Phase 12A): every building block of the screens, in light and dark. Its first
 * section shows your real departments and projects, with activity from the Ledger.
 */
export function GalleryView({ theme }: { theme: ThemeName }) {
  const org = useOrganization();
  const entries = useMemo(
    () => (org.snapshot ? organizationCards(org.snapshot) : []),
    [org.snapshot],
  );
  const scopes = useMemo(() => entries.map((e) => e.scope), [entries]);
  const activity = useActivity(scopes);
  const cards = entries.map((e, i): EntityCardProps & { id: string } => {
    const series = activity.series[i];
    return {
      id: `${e.scope.kind}:${"id" in e.scope ? e.scope.id : ""}`,
      ...e.card,
      activity:
        activity.status === "loading"
          ? "loading"
          : activity.status === "error"
            ? "error"
            : (series ?? null),
    };
  });
  const state =
    org.status === "loading"
      ? "loading"
      : org.status === "error" && !org.snapshot
        ? "error"
        : "ready";
  return (
    <section className="view view--gallery" aria-labelledby="gallery-title">
      <h1 id="gallery-title">Gallery</h1>
      <Gallery theme={theme} live={{ state, cards, ...(org.error ? { error: org.error } : {}) }} />
    </section>
  );
}
