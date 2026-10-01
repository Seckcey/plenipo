import type { ReactNode } from "react";
import type { LedgerEvent, LicenseView } from "@plenipo/types";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { PlenipoCommandError } from "../api/commands";
import { describeEvent } from "../ledger/format";
import { a11yProblems } from "../test/a11y";
import { LicenseSettings } from "./LicenseSettings";
import { PartOfPro } from "./PartOfPro";
import { describeLicenseEvent, freeLine, reasonWords } from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getLicense: vi.fn(),
    enterLicenseKey: vi.fn(),
    removeLicenseKey: vi.fn(),
    checkLicenseNow: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLicense: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

/** Where the section sits: under Settings' heading and its own. */
function inPage(part: ReactNode) {
  return render(
    <main>
      <h1>Settings</h1>
      <h2>License</h2>
      {part}
    </main>,
  );
}
const DAY = 86_400_000;
const NOW = Date.UTC(2026, 9, 1, 17);
const KEY_ID = "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T";

function free(patch: Partial<LicenseView> = {}): LicenseView {
  return {
    edition: "free",
    reason: "noKey",
    keyId: null,
    keyEdition: null,
    organizationsCovered: 1,
    organizationsInUse: 1,
    holder: null,
    plan: null,
    paidThrough: null,
    endsAt: null,
    lastChecked: null,
    lastTried: null,
    nextCheck: null,
    graceEnds: null,
    problem: null,
    freeLimits: { organizations: 1, departments: 1, projects: 1, workersAtOnce: 3 },
    testBuild: false,
    clockAheadDays: null,
    ...patch,
  };
}

function pro(patch: Partial<LicenseView> = {}): LicenseView {
  return free({
    edition: "pro",
    reason: "active",
    keyId: KEY_ID,
    keyEdition: "pro",
    organizationsCovered: 3,
    organizationsInUse: 2,
    holder: "Frank's Garage",
    plan: "yearly",
    paidThrough: NOW + 300 * DAY,
    lastChecked: NOW - DAY,
    lastTried: NOW - DAY,
    nextCheck: NOW + 6 * DAY,
    graceEnds: NOW + 29 * DAY,
    ...patch,
  });
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("Settings → License", () => {
  it("says what Free includes, that Free never contacts 8 West, and takes a key", async () => {
    api.getLicense.mockResolvedValue(free());
    api.enterLicenseKey.mockResolvedValue(pro({ reason: "notCheckedYet", lastChecked: null }));
    const user = userEvent.setup();
    const { container } = inPage(<LicenseSettings />);
    expect(await screen.findByText(/You're on Free: 1 organization, 1 department/)).toBeTruthy();
    expect(screen.getByText(/A Free copy never contacts 8 West/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Check now" })).toBeNull();
    const box = screen.getByLabelText(/License key/);
    expect(box.getAttribute("type")).toBe("password");
    await user.type(box, "  plenipo1.abc  ");
    await user.click(screen.getByRole("button", { name: "Enter the key" }));
    expect(api.enterLicenseKey).toHaveBeenCalledWith("plenipo1.abc");
    expect(await screen.findByText(/Pro is on/)).toBeTruthy();
    expect((box as HTMLInputElement).value).toBe("");
    expect(screen.getByText(KEY_ID)).toBeTruthy();
    expect(a11yProblems(container)).toEqual([]);
  });

  it("shows a refused key's plain words and keeps what was typed", async () => {
    api.getLicense.mockResolvedValue(free());
    api.enterLicenseKey.mockRejectedValue(
      new PlenipoCommandError(
        "invalidInput",
        "That doesn't look like a Plenipo license key. Copy the whole key from the email 8 West sent you, and paste it again.",
      ),
    );
    const user = userEvent.setup();
    render(<LicenseSettings />);
    const box = await screen.findByLabelText(/License key/);
    await user.type(box, "nope");
    await user.click(screen.getByRole("button", { name: "Enter the key" }));
    expect((await screen.findByRole("alert")).textContent).toMatch(/doesn't look like/);
    expect((box as HTMLInputElement).value).toBe("nope");
  });

  it("shows the key's ID, never the key, and checks now", async () => {
    api.getLicense.mockResolvedValue(pro());
    api.checkLicenseNow.mockResolvedValue(pro({ lastChecked: NOW }));
    const user = userEvent.setup();
    render(<LicenseSettings />);
    expect(await screen.findByText(KEY_ID)).toBeTruthy();
    expect(screen.getByText("Frank's Garage")).toBeTruthy();
    expect(screen.getByText("Pro")).toBeTruthy();
    expect(screen.getByText("2 of 3")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(api.checkLicenseNow).toHaveBeenCalled();
  });

  it("asks before removing the key, and says nothing is deleted", async () => {
    api.getLicense.mockResolvedValue(pro());
    api.removeLicenseKey.mockResolvedValue(free());
    const user = userEvent.setup();
    render(<LicenseSettings />);
    await user.click(await screen.findByRole("button", { name: "Remove the key" }));
    expect(screen.getByText(/Nothing you made is deleted/)).toBeTruthy();
    expect(api.removeLicenseKey).not.toHaveBeenCalled();
    const buttons = screen.getAllByRole("button", { name: "Remove the key" });
    await user.click(buttons[buttons.length - 1]!);
    await waitFor(() => expect(api.removeLicenseKey).toHaveBeenCalled());
    expect(await screen.findByText(/You're on Free/)).toBeTruthy();
  });

  it("says when the PC's clock is ahead of 8 West's", async () => {
    api.getLicense.mockResolvedValue(
      free({ reason: "noCheck", keyId: KEY_ID, clockAheadDays: 40 }),
    );
    render(<LicenseSettings />);
    expect(await screen.findByText(/clock is 40 days ahead of 8 West/)).toBeTruthy();
  });

  it("names a Partner plan and an organization count with no limit (ADR-119)", async () => {
    api.getLicense.mockResolvedValue(
      pro({
        keyEdition: "partner",
        plan: "monthly",
        organizationsCovered: null,
        organizationsInUse: 14,
      }),
    );
    render(<LicenseSettings />);
    expect(await screen.findByText("Partner")).toBeTruthy();
    expect(screen.getByText("14 (no limit)")).toBeTruthy();
    expect(screen.getAllByText("Plenipo Partner").length).toBeGreaterThan(0);
  });

  it("says a test copy accepts test keys", async () => {
    api.getLicense.mockResolvedValue(free({ testBuild: true }));
    render(<LicenseSettings />);
    expect(await screen.findByText(/built for testing/)).toBeTruthy();
  });
});

describe("license words", () => {
  it("says why Plenipo is on Free or Pro", () => {
    const limits = free().freeLimits;
    expect(freeLine(limits)).toBe(
      "1 organization, 1 department, 1 project, and 3 workers on the job at a time",
    );
    expect(reasonWords(pro(), NOW)).toMatch(/^Pro is paid through /);
    expect(reasonWords(pro({ reason: "cancelling", endsAt: NOW + 5 * DAY }), NOW)).toMatch(
      /^Pro on this key stays on until .* If you changed plans, enter the new key/,
    );
    const ended = reasonWords(free({ reason: "ended", keyId: KEY_ID, keyEdition: "partner" }), NOW);
    expect(ended).toMatch(/^Partner on this key ended/);
    expect(ended).toMatch(/Everything you made is still here/);
    expect(ended).not.toMatch(/same key/);
    expect(reasonWords(free({ reason: "noCheck", keyId: KEY_ID }), NOW)).toMatch(
      /hasn't reached 8 West for 30 days/,
    );
  });

  it("puts the license's events on the Activity trail in plain words, never the key", () => {
    const event = (eventType: string, payload: Record<string, unknown>): LedgerEvent => ({
      seq: 1,
      id: "e",
      taskId: null,
      executionId: null,
      source: "plenipo",
      destination: null,
      eventType,
      payload,
      createdAt: NOW,
    });
    expect(describeLicenseEvent("license.key_entered", { keyId: KEY_ID })).toBe(
      `You entered a license key (${KEY_ID})`,
    );
    expect(
      describeLicenseEvent("license.key_refused", {
        reason: "This key wasn't signed by 8 West, so Plenipo can't use it.",
      }),
    ).toBe("A license key was refused: This key wasn't signed by 8 West, so Plenipo can't use it.");
    expect(describeLicenseEvent("license.checked", { keyId: KEY_ID, state: "active" })).toBe(
      "The weekly check with 8 West: Pro is paid",
    );
    expect(
      describeLicenseEvent("license.edition_changed", { to: "free", reason: "noCheck" }),
    ).toMatch(/^Plenipo is on Free now \(no check with 8 West for 30 days\)/);
    expect(
      describeEvent(
        event("liaison.waiting_for_free_slot", {
          reason:
            "Free runs 3 workers at a time. This one starts when one finishes. Plenipo Pro runs 4 at a time in each organization. Enter a license key in Settings → License.",
        }),
      ),
    ).toBe("Waiting its turn: Free runs 3 workers at a time");
  });
});

describe("Part of Pro", () => {
  it("opens Settings → License", async () => {
    const go = vi.fn();
    const user = userEvent.setup();
    render(<PartOfPro go={go}>Connections are paused on Free.</PartOfPro>);
    await user.click(screen.getByRole("button", { name: "Open Settings → License" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "license" });
  });
});
