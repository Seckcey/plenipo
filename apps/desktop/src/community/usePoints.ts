import { useEffect, useState } from "react";
import type { PointsView } from "@plenipo/types";

import { communityPoints, toCommandError } from "../api/commands";

/**
 * Your points (Phase 24, ADR-169), read once each time the page that shows them is shown, and
 * again only when `retry` is called. Never on a timer: 8 West lets the points and the leaderboard
 * be asked 120 times an hour together.
 */
export function usePoints(): {
  points: PointsView | null;
  /** What went wrong, in plain words. */
  error: string | null;
  /** Ask again, after an error. */
  retry: () => void;
} {
  const [points, setPoints] = useState<PointsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tries, setTries] = useState(0);
  useEffect(() => {
    let alive = true;
    communityPoints().then(
      (next) => {
        if (!alive) return;
        setPoints(next);
        setError(null);
      },
      (reason: unknown) => {
        if (alive) setError(toCommandError(reason).message);
      },
    );
    return () => {
      alive = false;
    };
  }, [tries]);
  return {
    points,
    error,
    retry: () => {
      setError(null);
      setTries((n) => n + 1);
    },
  };
}
