import type { ExecutionRecord, OutputLine, RuntimeOverview } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import { initialRuntimeState, MAX_UI_LINES, mergeLines, runtimeReducer } from "./store";

const line = (seq: number, stream: "stdout" | "stderr" = "stdout"): OutputLine => ({
  seq,
  stream,
  text: `line ${seq}`,
  truncated: false,
  ts: seq,
});

export const record = (id: string, patch: Partial<ExecutionRecord> = {}): ExecutionRecord => ({
  id,
  profileId: "diagnostic.echo",
  label: "Echo test",
  executable: "/x",
  args: [],
  workingDir: "/",
  pid: 42,
  state: "running",
  exitCode: null,
  detail: null,
  startedAt: 1_000,
  endedAt: null,
  ...patch,
});

const overview = (executions: ExecutionRecord[]): RuntimeOverview => ({
  profiles: [],
  executions,
  activeCount: executions.filter((e) => e.state === "running").length,
  notices: [],
});

describe("mergeLines", () => {
  it("appends in-order lines", () => {
    expect(mergeLines([line(1)], [line(2), line(3)]).lines.map((l) => l.seq)).toEqual([1, 2, 3]);
  });

  it("dedupes overlap between a snapshot and live events", () => {
    const merged = mergeLines([line(3), line(4)], [line(1), line(2), line(3)]);
    expect(merged.lines.map((l) => l.seq)).toEqual([1, 2, 3, 4]);
  });

  it("caps the number of lines", () => {
    const many = Array.from({ length: MAX_UI_LINES + 5 }, (_, i) => line(i + 1));
    const merged = mergeLines([], many);
    expect(merged.lines).toHaveLength(MAX_UI_LINES);
    expect(merged.evicted).toBe(5);
    expect(merged.lines[0]?.seq).toBe(6);
  });
});

describe("runtimeReducer", () => {
  it("loads an overview, newest first", () => {
    const s = runtimeReducer(initialRuntimeState, {
      type: "overviewLoaded",
      overview: overview([record("b"), record("a")]),
    });
    expect(s.status).toBe("ready");
    expect(s.order).toEqual(["b", "a"]);
  });

  it("adds new executions from lifecycle events to the front", () => {
    let s = runtimeReducer(initialRuntimeState, {
      type: "overviewLoaded",
      overview: overview([record("a", { state: "succeeded" })]),
    });
    s = runtimeReducer(s, {
      type: "event",
      at: 0,
      event: { kind: "lifecycle", record: record("b") },
    });
    expect(s.order).toEqual(["b", "a"]);
    s = runtimeReducer(s, {
      type: "event",
      at: 0,
      event: { kind: "lifecycle", record: record("b", { state: "failed", exitCode: 3 }) },
    });
    expect(s.order).toEqual(["b", "a"]);
    expect(s.executions.b?.exitCode).toBe(3);
    expect(s.eventLog).toHaveLength(2);
  });

  it("keeps a terminal state that arrived while an older snapshot was loading", () => {
    let s = runtimeReducer(initialRuntimeState, {
      type: "event",
      at: 0,
      event: { kind: "lifecycle", record: record("a", { state: "cancelled" }) },
    });
    s = runtimeReducer(s, { type: "overviewLoaded", overview: overview([record("a")]) });
    expect(s.executions.a?.state).toBe("cancelled");
  });

  it("merges output events and snapshots", () => {
    let s = runtimeReducer(initialRuntimeState, {
      type: "event",
      at: 0,
      event: { kind: "output", executionId: "a", lines: [line(3, "stderr")] },
    });
    s = runtimeReducer(s, {
      type: "outputLoaded",
      output: { executionId: "a", lines: [line(1), line(2)], dropped: 0, available: true },
    });
    expect(s.outputs.a?.lines.map((l) => [l.seq, l.stream])).toEqual([
      [1, "stdout"],
      [2, "stdout"],
      [3, "stderr"],
    ]);
  });

  it("records load failures", () => {
    const s = runtimeReducer(initialRuntimeState, { type: "loadFailed", message: "down" });
    expect(s).toMatchObject({ status: "error", error: "down" });
  });
});
