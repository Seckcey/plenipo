import { IconRail, PlenipoLogo, type RailItem } from "@plenipo/ui";

import { VIEWS, type ViewId } from "./views";

/**
 * The left strip (Phase 12A): each section's icon with its name under it (the owner's choice),
 * the current one marked, and counts for what is running or waiting.
 */
export function Sidebar({
  current,
  onNavigate,
  activeCount,
  workingCount,
  approvalCount = 0,
  communityOn = false,
}: {
  current: ViewId;
  onNavigate: (view: ViewId) => void;
  activeCount: number;
  /** Agent turns in progress. */
  workingCount: number;
  /** Requests waiting for the owner's approval. */
  approvalCount?: number;
  /**
   * Community's switch is on. Only then is the Community section on the strip: someone who never
   * turns Community on never sees it (Phase 24).
   */
  communityOn?: boolean;
}) {
  const badge = (id: ViewId): RailItem<ViewId>["badge"] => {
    if (id === "runtimes" && activeCount > 0)
      return { count: activeCount, label: "active", tone: "ok" };
    if (id === "workers" && workingCount > 0)
      return { count: workingCount, label: "working", tone: "ok" };
    if (id === "approvals" && approvalCount > 0) {
      return { count: approvalCount, label: "waiting for you", tone: "pending" };
    }
    return undefined;
  };
  return (
    <IconRail<ViewId>
      current={current}
      onSelect={onNavigate}
      // The logo from the owner's brand kit: the P, "lenipo", and Pip on the n.
      brand={<PlenipoLogo height={28} />}
      items={VIEWS.filter((v) => v.id !== "community" || communityOn).map((v) => {
        const b = badge(v.id);
        return {
          id: v.id,
          label: v.label,
          icon: v.icon,
          tooltip: v.tooltip,
          ...(v.bottom ? { bottom: true } : {}),
          ...(b ? { badge: b } : {}),
        };
      })}
    />
  );
}
