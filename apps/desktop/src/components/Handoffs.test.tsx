import type { HandoffView } from "@plenipo/types";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { HandoffCard } from "./Handoffs";

const view = (reply: Partial<NonNullable<HandoffView["reply"]>> = {}): HandoffView => ({
  messageId: "m1",
  correlationId: "c1",
  state: "answered",
  requesterTaskId: "t1",
  requester: "session:s1",
  requesterRuntimeId: "codex",
  step: 1,
  destination: "role:Senior Developer",
  destinationLabel: "Senior Developer (Claude Code)",
  objective: "Fix the login bug",
  acceptanceCriteria: "The tests pass.",
  priority: 2,
  depth: 1,
  context: [],
  artifacts: [],
  capabilitiesRequested: [],
  rejection: null,
  childTaskId: "c-task",
  childSessionId: "s2",
  childState: "succeeded",
  reply: {
    messageId: "r1",
    state: "delivered",
    outcome: "completed",
    summary: "Fixed",
    text: "Fixed it. All tests pass.",
    error: null,
    source: "session:s2",
    createdAt: 5,
    ...reply,
  },
  createdAt: 2,
  updatedAt: 5,
});

function card(v: HandoffView) {
  render(<HandoffCard view={v} canOpen={() => false} onOpenSession={() => {}} />);
  return screen.getByRole("listitem", {
    name: "Handoff to Senior Developer (Claude Code): Fix the login bug",
  });
}

// Phase 25, item 4.7: an answer that doesn't match Plenipo's record says so, in plain words.
describe("HandoffCard — answers checked against Plenipo's record", () => {
  it("marks a reply that doesn't match the record, and says it was sent back", () => {
    const c = card(view({ mismatches: ["says tests passed, but no test ran"], sentBack: true }));
    expect(within(c).getByText(/doesn't match the record/)).toBeInTheDocument();
    expect(within(c).getByRole("note")).toHaveTextContent(
      "Doesn't match Plenipo's record: says tests passed, but no test ran. It was sent back to the worker once to check.",
    );
  });

  it("says nothing more for a reply that matches", () => {
    const c = card(view());
    expect(within(c).queryByRole("note")).not.toBeInTheDocument();
    expect(within(c).queryByText(/doesn't match/)).not.toBeInTheDocument();
    expect(within(c).getByText("Fixed it. All tests pass.")).toBeInTheDocument();
  });
});
