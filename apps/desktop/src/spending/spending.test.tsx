import type { ReactNode } from "react";
import type { CapStatus, SpendingPage, SpendingRecord } from "@plenipo/types";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { a11yProblems } from "../test/a11y";
import { sampleOrganization } from "../test/orgFixtures";
import { SpendingBanner } from "./SpendingBanner";
import { SpendingSettings } from "./SpendingSettings";
import {
  dollars,
  monthName,
  parseDollars,
  recordCost,
  resetDay,
  typedAmount,
  untilTurnover,
} from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getSpending: vi.fn(),
    setSpendingCap: vi.fn(),
    removeSpendingCap: vi.fn(),
    getOrganization: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();
const D = 1_000_000;
/** 1 November 2026, midnight Pacific. */
const NOV_1 = Date.UTC(2026, 10, 1, 7);

function page(patch: Partial<SpendingPage> = {}): SpendingPage {
  return {
    month: "2026-10",
    monthStartsAt: Date.UTC(2026, 9, 1, 7),
    resetsAt: NOV_1,
    hasBusinessCap: false,
    caps: [],
    spentMicros: 0,
    setAsideMicros: 0,
    notPriced: 0,
    recent: [],
    ...patch,
  };
}

function cap(patch: Partial<CapStatus> & { monthly?: number } = {}): CapStatus {
  const { monthly = 50 * D, ...rest } = patch;
  return {
    cap: {
      id: "cap-business",
      covers: { kind: "business" },
      monthlyMicros: monthly,
      setAt: 0,
      setBy: "owner",
    },
    label: "The business",
    gone: false,
    spentMicros: 0,
    setAsideMicros: 0,
    leftMicros: monthly,
    state: "ok",
    stoppedWhy: null,
    ...rest,
  };
}

function record(patch: Partial<SpendingRecord> = {}): SpendingRecord {
  return {
    id: "r1",
    taskId: "t1",
    positionId: "p-dev",
    positionTitle: "Senior Developer",
    departmentId: "d-eng",
    departmentName: "Engineering",
    runtime: "openrouter",
    model: "moonshotai/kimi-k3",
    keyName: "Office key",
    month: "2026-10",
    state: "spent",
    setAsideMicros: 2 * D,
    spentMicros: 1_234_567,
    pricedBy: "service",
    detail: null,
    createdAt: Date.UTC(2026, 9, 3, 21, 15),
    settledAt: Date.UTC(2026, 9, 3, 21, 16),
    ...patch,
  };
}

function inPage(part: ReactNode) {
  return render(
    <main>
      <h1>Settings</h1>
      <h2>Spending caps</h2>
      {part}
    </main>,
  );
}

beforeEach(() => {
  go.mockReset();
  api.getSpending.mockReset();
  api.setSpendingCap.mockReset();
  api.removeSpendingCap.mockReset();
  api.getOrganization.mockResolvedValue(sampleOrganization());
});

describe("words", () => {
  it("writes money the way the Ledger does", () => {
    expect(dollars(0)).toBe("$0.00");
    expect(dollars(42)).toBe("$0.0001");
    expect(dollars(4_200)).toBe("$0.0042");
    expect(dollars(12_345_678)).toBe("$12.35");
    expect(dollars(1_234_567 * D)).toBe("$1,234,567.00");
  });

  it("reads the amount the owner types, in dollars and cents", () => {
    expect(parseDollars("50")).toBe(50 * D);
    expect(parseDollars(" $12.5 ")).toBe(12_500_000);
    expect(parseDollars("1,000.25")).toBe(1_000_250_000);
    expect(parseDollars("0.01")).toBe(10_000);
    expect(parseDollars("1,000,000")).toBe(1_000_000 * D);
    // A comma is never a decimal point, and only goes between groups of three.
    for (const bad of ["12,50", "1,0", "5,00", ",,5,,", "1,,000", "1000,000", ",100"]) {
      expect(parseDollars(bad), bad).toBeNull();
    }
    for (const bad of ["", "0", "0.001", "-5", "abc", "12.345", "1000001", "1e3"]) {
      expect(parseDollars(bad), bad).toBeNull();
    }
    expect(typedAmount(50 * D)).toBe("50");
    expect(typedAmount(12_500_000)).toBe("12.50");
  });

  it("names the month and the day it starts over, in Pacific time", () => {
    expect(monthName("2026-10")).toBe("October 2026");
    expect(resetDay(NOV_1)).toBe("November 1");
    expect(recordCost(record())).toBe("$1.23");
    expect(recordCost(record({ state: "notPriced", spentMicros: null }))).toBe(
      "Not priced yet (counted as $2.00)",
    );
    expect(recordCost(record({ state: "setAside", spentMicros: null }))).toBe(
      "Running: up to $2.00 set aside",
    );
    expect(recordCost(record({ state: "released", spentMicros: null }))).toBe(
      "Not sent: nothing spent",
    );
  });
});

describe("Settings → Spending caps", () => {
  it("says no cap means no dollar limit, and sets one in millionths of a dollar", async () => {
    api.getSpending.mockResolvedValue(page());
    api.setSpendingCap.mockResolvedValue(page({ hasBusinessCap: true, caps: [cap()] }));
    const user = userEvent.setup();
    const { container } = inPage(<SpendingSettings go={go} />);
    expect(
      await screen.findByText(
        "No cap for the business: paid AI keys have no dollar limit. Set one below if you want.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("November 1 (Pacific time)")).toBeInTheDocument();
    const amount = screen.getByLabelText("The business: monthly cap in dollars");
    await user.type(amount, "fifty");
    await user.click(screen.getByRole("button", { name: "Set the cap for the business" }));
    expect(screen.getByText(/Type an amount in dollars/)).toBeInTheDocument();
    expect(api.setSpendingCap).not.toHaveBeenCalled();
    await user.clear(amount);
    await user.type(amount, "50");
    api.getSpending.mockResolvedValue(page({ hasBusinessCap: true, caps: [cap()] }));
    await user.click(screen.getByRole("button", { name: "Set the cap for the business" }));
    expect(api.setSpendingCap).toHaveBeenCalledWith({ kind: "business" }, 50 * D);
    expect(
      await screen.findByRole("meter", { name: "The business: Under 80%" }),
    ).toBeInTheDocument();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("shows each cap's month, why paid work stopped, and the month's paid tasks", async () => {
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        spentMicros: 50 * D,
        notPriced: 1,
        caps: [
          cap({
            spentMicros: 49 * D,
            setAsideMicros: 1 * D,
            leftMicros: 0,
            state: "stopped",
            stoppedWhy: "Not started: this task could cost up to $3.00, and $0.00 is left.",
          }),
          cap({
            cap: {
              id: "cap-eng",
              covers: { kind: "department", id: "d-eng" },
              monthlyMicros: 20 * D,
              setAt: 0,
              setBy: "owner",
            },
            label: "Engineering",
            spentMicros: 17 * D,
            leftMicros: 3 * D,
            state: "warning",
          }),
        ],
        recent: [
          record(),
          record({
            id: "r2",
            state: "notPriced",
            spentMicros: null,
            detail: "Plenipo stopped before this task's bill was read.",
          }),
        ],
      }),
    );
    const { container } = inPage(<SpendingSettings go={go} />);
    const business = await screen.findByRole("meter", { name: "The business: Stopped" });
    expect(business).toHaveAttribute(
      "aria-valuetext",
      "$49.00 of $50.00 spent · $1.00 set aside for running tasks",
    );
    expect(screen.getByText(/could cost up to \$3\.00/)).toBeInTheDocument();
    expect(
      screen.getByRole("meter", { name: "Engineering: 80% or more used" }),
    ).toBeInTheDocument();
    // Marketing has no cap of its own: a place to set one.
    expect(screen.getByLabelText("Marketing: monthly cap in dollars")).toHaveValue("");
    const tasks = screen
      .getByRole("heading", { name: "This month's paid tasks" })
      .closest("section")!;
    expect(within(tasks).getAllByText("Senior Developer · Engineering")).toHaveLength(2);
    expect(screen.getByText("$1.23")).toBeInTheDocument();
    expect(screen.getByText("Not priced yet (counted as $2.00)")).toBeInTheDocument();
    expect(screen.getAllByText(/key: Office key/)).toHaveLength(2);
    expect(a11yProblems(container)).toEqual([]);
  });

  it("adds a cap for one position and removes a cap", async () => {
    const withBusiness = page({ hasBusinessCap: true, caps: [cap()] });
    api.getSpending.mockResolvedValue(withBusiness);
    api.setSpendingCap.mockResolvedValue(withBusiness);
    api.removeSpendingCap.mockResolvedValue(page());
    const user = userEvent.setup();
    inPage(<SpendingSettings go={go} />);
    const which = await screen.findByLabelText("Add a cap for one position");
    await user.selectOptions(which, "p-dev");
    await user.type(screen.getByLabelText("Its monthly cap in dollars"), "10");
    await user.click(screen.getByRole("button", { name: "Add the cap" }));
    expect(api.setSpendingCap).toHaveBeenCalledWith({ kind: "position", id: "p-dev" }, 10 * D);
    await user.click(screen.getByRole("button", { name: "Remove the cap for the business" }));
    expect(api.removeSpendingCap).toHaveBeenCalledWith("cap-business");
  });

  it("says what went wrong when a cap is refused", async () => {
    api.getSpending.mockResolvedValue(page());
    api.setSpendingCap.mockRejectedValue({
      kind: "invalidInput",
      message: "invalid input: a spending cap must be from $0.01 to $1,000,000.00 a month",
    });
    const user = userEvent.setup();
    inPage(<SpendingSettings go={go} />);
    await user.type(await screen.findByLabelText("The business: monthly cap in dollars"), "5");
    await user.click(screen.getByRole("button", { name: "Set the cap for the business" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a spending cap must be from");
  });

  it("opens Switches, where paid keys are turned on", async () => {
    api.getSpending.mockResolvedValue(page());
    const user = userEvent.setup();
    inPage(<SpendingSettings go={go} />);
    await user.click(await screen.findByRole("button", { name: "Open Switches" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "switches" });
  });
});

describe("the spending banner", () => {
  it("shows nothing while every cap is under 80%", async () => {
    api.getSpending.mockResolvedValue(page({ hasBusinessCap: true, caps: [cap()] }));
    const { container } = render(<SpendingBanner go={go} />);
    await waitFor(() => expect(api.getSpending).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("warns at 80%, and opens Spending caps", async () => {
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        caps: [cap({ spentMicros: 41 * D, leftMicros: 9 * D, state: "warning" })],
      }),
    );
    const user = userEvent.setup();
    render(<SpendingBanner go={go} />);
    expect(
      await screen.findByText("80% or more of the business's cap is used"),
    ).toBeInTheDocument();
    expect(screen.getByText(/\$41\.00 of \$50\.00 spent this month/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Spending caps" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "spending" });
  });

  it("says paid work stopped, why, and when the month starts over", async () => {
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        caps: [
          cap({
            spentMicros: 50 * D,
            leftMicros: 0,
            state: "stopped",
            stoppedWhy: "$50.00 of $50.00 is spent this month.",
          }),
        ],
      }),
    );
    render(<SpendingBanner go={go} />);
    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("Paid AI work stopped under the business's cap");
    expect(banner).toHaveTextContent("$50.00 of $50.00 is spent this month.");
    expect(banner).toHaveTextContent("wait until November 1");
  });
});

describe("found in review", () => {
  it("waits for the month's turn at most an hour at a time", () => {
    expect(untilTurnover(10_000, 0)).toBe(11_000);
    expect(untilTurnover(0, 5_000)).toBe(1_000);
    expect(untilTurnover(40 * 24 * 3_600_000, 0)).toBe(3_600_000);
  });

  it("shows a cap on an inactive department, so it can be changed or removed", async () => {
    const org = sampleOrganization();
    org.departments = org.departments.map((d) => (d.id === "d-mkt" ? { ...d, active: false } : d));
    api.getOrganization.mockResolvedValue(org);
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        caps: [
          cap(),
          cap({
            cap: {
              id: "cap-mkt",
              covers: { kind: "department", id: "d-mkt" },
              monthlyMicros: 5 * D,
              setAt: 0,
              setBy: "owner",
            },
            label: "Marketing",
          }),
        ],
      }),
    );
    inPage(<SpendingSettings go={go} />);
    expect(
      await screen.findByLabelText("Marketing (inactive): monthly cap in dollars"),
    ).toHaveValue("5");
    expect(
      screen.getByRole("button", { name: "Remove the cap for Marketing (inactive)" }),
    ).toBeInTheDocument();
  });

  it("tells positions with the same title apart by their department", async () => {
    const org = sampleOrganization();
    const twin = org.positions.find((p) => p.id === "p-dev")!;
    org.positions = [
      ...org.positions,
      { ...twin, id: "p-dev-2", departmentId: "d-mkt", projectId: null },
    ];
    api.getOrganization.mockResolvedValue(org);
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        caps: [
          cap(),
          cap({
            cap: {
              id: "cap-dev",
              covers: { kind: "position", id: "p-dev" },
              monthlyMicros: 3 * D,
              setAt: 0,
              setBy: "owner",
            },
            label: "Senior Developer",
          }),
        ],
      }),
    );
    const { container } = inPage(<SpendingSettings go={go} />);
    expect(
      await screen.findByLabelText("Senior Developer · Engineering: monthly cap in dollars"),
    ).toBeInTheDocument();
    const which = screen.getByLabelText("Add a cap for one position");
    expect(
      within(which).getByRole("option", { name: "Senior Developer · Marketing" }),
    ).toBeTruthy();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("names a department's cap in the banner in plain words", async () => {
    api.getSpending.mockResolvedValue(
      page({
        hasBusinessCap: true,
        caps: [
          cap({
            cap: {
              id: "cap-eng",
              covers: { kind: "department", id: "d-eng" },
              monthlyMicros: 20 * D,
              setAt: 0,
              setBy: "owner",
            },
            label: "Engineering",
            spentMicros: 18 * D,
            state: "warning",
          }),
        ],
      }),
    );
    render(<SpendingBanner go={go} />);
    expect(
      await screen.findByText("80% or more of the cap for Engineering is used"),
    ).toBeInTheDocument();
  });

  it("goes away by itself when the month starts over", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const now = Date.now();
      api.getSpending.mockResolvedValueOnce(
        page({
          hasBusinessCap: true,
          resetsAt: now + 5_000,
          caps: [cap({ spentMicros: 50 * D, leftMicros: 0, state: "stopped" })],
        }),
      );
      api.getSpending.mockResolvedValue(
        page({ hasBusinessCap: true, resetsAt: now + 31 * 86_400_000, caps: [cap()] }),
      );
      const { container } = render(<SpendingBanner go={go} />);
      expect(await screen.findByRole("alert")).toHaveTextContent("Paid AI work stopped");
      await vi.advanceTimersByTimeAsync(7_000);
      await waitFor(() => expect(container).toBeEmptyDOMElement());
    } finally {
      vi.useRealTimers();
    }
  });
});
