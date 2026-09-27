import { useMemo } from "react";
import type { ActivityScope, ActivitySeries } from "@plenipo/types";

import { getActivity } from "../api/commands";
import { useActivity } from "../ledger/useActivity";
import { anyEvent, useLive, type Live } from "./useLive";

const DAY_MS = 86_400_000;
/** A week in 6-hour buckets. */
const WEEK_BUCKETS = 28;

/** One part of the company's last day (15-minute buckets) and last week (6-hour buckets). */
export function useStrips(scope: ActivityScope | null): {
  day: Pick<Live<ActivitySeries>, "status" | "value">;
  week: Live<ActivitySeries>;
} {
  const key = scope ? JSON.stringify(scope) : null;
  const scopes = useMemo(() => (key ? [JSON.parse(key) as ActivityScope] : []), [key]);
  const day = useActivity(scopes);
  const week = useLive(
    key,
    async (k) => {
      const to = Date.now();
      const [series] = await getActivity(
        [JSON.parse(k) as ActivityScope],
        to - 7 * DAY_MS,
        to,
        WEEK_BUCKETS,
      );
      if (!series) throw new Error("Plenipo Core returned no activity.");
      return series;
    },
    anyEvent,
    10_000,
  );
  return {
    day: {
      status: day.status === "ready" && !day.series[0] ? "error" : day.status,
      value: day.series[0] ?? null,
    },
    week,
  };
}
