import type { PositionInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

/** Watch a worker's file changes and live conversation, beside a row (Phase 25, item 3.1). */
export function WatchButton({
  p,
  watch,
}: {
  p: PositionInfo;
  watch: (positionId: string, title: string) => void;
}) {
  return (
    <Button size="sm" aria-label={`Watch ${p.title}`} onClick={() => watch(p.id, p.title)}>
      Watch
    </Button>
  );
}
