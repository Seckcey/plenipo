import {
  averageDown,
  axisLabels,
  bucketActivity,
  bucketLevel,
  DAY_MS,
  DEFAULT_BUCKETS,
  describeActivity,
  describeRange,
  downsample,
  totals,
} from "./activity";

const now = Date.UTC(2026, 8, 27, 18, 0);
const from = now - DAY_MS;

describe("bucketActivity", () => {
  it("counts events, problems, and approvals into 96 fifteen-minute buckets", () => {
    const series = bucketActivity(
      [
        { at: from, kind: "event" },
        { at: from + 1, kind: "problem" },
        { at: from + 15 * 60_000, kind: "waiting" },
        { at: now - 1 },
        { at: now }, // the end is not included
        { at: from - 1 }, // before the start
      ],
      from,
      now,
    );
    expect(series.buckets).toHaveLength(DEFAULT_BUCKETS);
    expect(series.bucketMs).toBe(15 * 60_000);
    expect(series.buckets[0]).toEqual({ events: 2, problems: 1, waiting: 0 });
    expect(series.buckets[1]).toEqual({ events: 1, problems: 0, waiting: 1 });
    expect(series.buckets[95]).toEqual({ events: 1, problems: 0, waiting: 0 });
    expect(totals(series)).toEqual({ events: 4, problems: 1, waiting: 1 });
  });

  it("widens buckets for a longer range and drops nothing (downsampling)", () => {
    const week = Array.from({ length: 700 }, (_, i) => ({ at: now - 7 * DAY_MS + i * 864_000 }));
    const series = bucketActivity(week, now - 7 * DAY_MS, now);
    expect(series.buckets).toHaveLength(96);
    expect(series.bucketMs).toBe(Math.ceil((7 * DAY_MS) / 96));
    expect(totals(series).events).toBe(700);
  });
});

describe("downsample", () => {
  it("adds neighbours together, keeping every count", () => {
    const series = bucketActivity(
      Array.from({ length: 96 }, (_, i) => ({
        at: from + i * 15 * 60_000,
        kind: i % 10 === 0 ? ("problem" as const) : ("event" as const),
      })),
      from,
      now,
    );
    const small = downsample(series, 24);
    expect(small.buckets).toHaveLength(24);
    expect(small.bucketMs).toBe(series.bucketMs * 4);
    expect(totals(small)).toEqual(totals(series));
  });

  it("leaves a series that is already small enough", () => {
    const series = bucketActivity([], from, now, 12);
    expect(downsample(series, 24)).toBe(series);
  });
});

describe("bucketLevel", () => {
  it("puts problems first, then waiting, then how busy", () => {
    expect(bucketLevel({ events: 5, problems: 1, waiting: 1 }, 10)).toBe("problem");
    expect(bucketLevel({ events: 5, problems: 0, waiting: 1 }, 10)).toBe("waiting");
    expect(bucketLevel({ events: 0, problems: 0, waiting: 0 }, 10)).toBe("none");
    expect(bucketLevel({ events: 1, problems: 0, waiting: 0 }, 10)).toBe("low");
    expect(bucketLevel({ events: 5, problems: 0, waiting: 0 }, 10)).toBe("mid");
    expect(bucketLevel({ events: 10, problems: 0, waiting: 0 }, 10)).toBe("high");
  });
});

describe("labels", () => {
  it("ends the axis with Now when the series ends now", () => {
    expect(axisLabels({ from, to: now }, now)[2]).toBe("Now");
    expect(axisLabels({ from, to: now - 3 * 3_600_000 }, now)[2]).not.toBe("Now");
  });

  it("describes ranges and totals in words", () => {
    expect(describeRange(DAY_MS)).toBe("24 hours");
    expect(describeRange(7 * DAY_MS)).toBe("7 days");
    expect(describeRange(3_600_000)).toBe("1 hour");
    const series = bucketActivity([{ at: from + 1, kind: "problem" }, { at: from + 2 }], from, now);
    expect(describeActivity(series)).toBe("Last 24 hours: 2 events, 1 problem");
  });

  it("averages values down for sparklines, always ending with the newest value", () => {
    expect(averageDown([1, 3, 5, 7], 2)).toEqual([2, 6]);
    expect(averageDown([1, 2], 10)).toEqual([1, 2]);
    // 96 values into 48/1.1 points: the last point still includes value 95.
    const values = Array.from({ length: 96 }, (_, i) => (i === 95 ? 1000 : 0));
    const points = averageDown(values, 96 / 2.2);
    expect(points[points.length - 1]).toBeGreaterThan(0);
  });
});
