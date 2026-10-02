import { describe as group, expect, it } from "vitest";
import type { LedgerEvent } from "@plenipo/types";

import { describe } from "./words";

function event(eventType: string, payload: Record<string, unknown>): LedgerEvent {
  return {
    seq: 1,
    id: "e1",
    taskId: "t1",
    executionId: null,
    source: "liaison",
    destination: null,
    eventType,
    payload,
    createdAt: 0,
  };
}

const changed = (to: string) => event("task.state_changed", { from: "running", to });

group("Activity's words on the phone", () => {
  it("says what a task became in a whole sentence", () => {
    expect(describe(changed("running"))).toBe("A task is working");
    expect(describe(changed("failed"))).toBe("A task didn't finish");
    expect(describe(changed("cancelled"))).toBe("A task was stopped");
    expect(describe(changed("succeeded"))).toBe("A task is done");
    expect(describe(changed("something-new"))).toBe("A task changed");
  });

  it("puts what matters in plain words, and leaves out what it has no words for", () => {
    expect(
      describe(event("guard.denied", { worker: "Senior Developer", summary: "git push" })),
    ).toBe("Blocked: Senior Developer tried to git push");
    expect(describe(event("plenipo.recovered", { stoppedTasks: [] }))).toBe(
      "Plenipo closed unexpectedly on your PC, and is running again",
    );
    expect(describe(event("remote.notice_sent", { name: "Pixel" }))).toBe("A notice went to Pixel");
    for (const type of ["agent.result", "guard.grant_closed", "execution.cancelled"]) {
      expect(describe(event(type, {}))).toBeNull();
    }
  });
});
