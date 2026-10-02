import { describe as group, expect, it } from "vitest";
import type { LedgerEvent } from "@plenipo/types";

import { describe } from "./words";

function changed(to: string): LedgerEvent {
  return {
    seq: 1,
    id: "e1",
    taskId: "t1",
    executionId: null,
    source: "liaison",
    destination: null,
    eventType: "task.state_changed",
    payload: { from: "running", to },
    createdAt: 0,
  };
}

group("Activity's words on the phone", () => {
  it("says what a task became in a whole sentence", () => {
    expect(describe(changed("running"))).toBe("A task is working");
    expect(describe(changed("failed"))).toBe("A task didn't finish");
    expect(describe(changed("cancelled"))).toBe("A task was stopped");
    expect(describe(changed("succeeded"))).toBe("A task is done");
    expect(describe(changed("something-new"))).toBe("A task changed");
  });
});
