import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TerminalContext, type TerminalApi } from "../../../terminal/context";
import { sampleOrganization } from "../../../test/orgFixtures";
import { OverviewTab } from "./OverviewTab";
import type { InspectorActions } from "./types";

const actions = { openTask: vi.fn(), openSession: vi.fn() } as unknown as InspectorActions;

function show(id: string, openWatch?: TerminalApi["openWatch"]) {
  const snapshot = sampleOrganization();
  const p = snapshot.positions.find((x) => x.id === id)!;
  const tab = <OverviewTab p={p} snapshot={snapshot} actions={actions} onSelect={vi.fn()} />;
  const wrap = (children: ReactNode) =>
    openWatch ? (
      <TerminalContext.Provider value={{ openWatch } as unknown as TerminalApi}>
        {children}
      </TerminalContext.Provider>
    ) : (
      children
    );
  render(wrap(tab));
}

afterEach(cleanup);

describe("the Overview tab's Watch button", () => {
  it("opens the agent's Watch tab in the terminal panel", async () => {
    const user = userEvent.setup();
    const openWatch = vi.fn();
    show("p-dev", openWatch);
    const watch = screen.getByRole("button", { name: "Watch Senior Developer" });
    expect(watch).toHaveTextContent("Watch");
    expect(watch).toHaveAccessibleDescription(
      "See the code it writes as it writes it, in the terminal panel. Read-only.",
    );
    await user.click(watch);
    expect(openWatch).toHaveBeenCalledWith("p-dev", "Senior Developer");
  });

  it("is not there without a terminal panel, nor for a vacant position", () => {
    show("p-dev");
    expect(screen.queryByRole("button", { name: /^Watch/ })).toBeNull();
    cleanup();
    show("p-mkt", vi.fn());
    expect(screen.queryByRole("button", { name: /^Watch/ })).toBeNull();
  });
});
