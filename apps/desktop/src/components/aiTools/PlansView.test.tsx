import type { SpendingPage, ToolPaces, WindowPace } from "@plenipo/types";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { PlansView } from "./PlansView";
import { paceWords } from "./words";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getPlanPaces: vi.fn(),
    getSpending: vi.fn(),
    setPlanBudget: vi.fn(),
  };
});
const api = vi.mocked(commands);

const week = (patch: Partial<WindowPace> = {}): WindowPace => ({
  minutes: 10080,
  models: null,
  usedPercent: 45,
  fairPercent: 27,
  pace: "ahead",
  resetsAt: null,
  estimated: false,
  ...patch,
});

const claude: ToolPaces = {
  runtimeId: "claude-code",
  label: "Claude Code",
  windows: [
    week(),
    week({ minutes: 300, usedPercent: 20, fairPercent: 60, pace: "behind", models: null }),
  ],
  weeklyBudget: null,
};
const kimi: ToolPaces = { runtimeId: "kimi", label: "Kimi Code", windows: [], weeklyBudget: null };

const spending = {
  spentMicros: 3_200_000,
  setAsideMicros: 0,
  caps: [{ cap: { covers: { kind: "business" }, monthlyMicros: 50_000_000 } }],
} as unknown as SpendingPage;

beforeEach(() => {
  api.getPlanPaces.mockResolvedValue([claude, kimi]);
  api.getSpending.mockResolvedValue(spending);
  api.setPlanBudget.mockResolvedValue({} as never);
});

// Phase 25, item 4.6: every plan, its pace, and paid spending, in one place.
describe("Your plans", () => {
  it("shows each plan's pace in plain words", async () => {
    render(<PlansView />);
    const plan = await screen.findByRole("listitem", { name: "Claude Code" });
    const rows = within(plan).getAllByRole("row");
    expect(rows[1]).toHaveTextContent("Week");
    expect(rows[1]).toHaveTextContent("45% used, ahead of pace (27% by now)");
    expect(rows[1]).toHaveTextContent("Ahead of pace");
    expect(rows[1]).toHaveTextContent("Work steps down early to make it last.");
    expect(rows[2]).toHaveTextContent("5-hour");
    expect(rows[2]).toHaveTextContent("Room to spare: the best model is used freely.");
    expect(
      await screen.findByText("Paid AI this month: $3.20 of your $50.00 cap."),
    ).toBeInTheDocument();
  });

  it("paces an AI tool that reports nothing against a weekly budget you set", async () => {
    const user = userEvent.setup();
    render(<PlansView />);
    const plan = await screen.findByRole("listitem", { name: "Kimi Code" });
    expect(plan).toHaveTextContent("doesn't report how much of its plan is used");
    await user.type(
      within(plan).getByRole("textbox", { name: "Weekly budget for Kimi Code (tokens)" }),
      "2,000,000",
    );
    await user.click(within(plan).getByRole("button", { name: "Save" }));
    expect(api.setPlanBudget).toHaveBeenCalledWith("kimi", 2_000_000);
    expect(api.getPlanPaces).toHaveBeenCalledTimes(2);
  });

  it("says when a share is estimated", () => {
    expect(paceWords(week({ estimated: true, pace: "onPace", fairPercent: 40 }))).toBe(
      "About 45% used, on pace (40% by now)",
    );
    expect(paceWords(week({ pace: "unknown", fairPercent: null }))).toBe("45% used");
  });
});
