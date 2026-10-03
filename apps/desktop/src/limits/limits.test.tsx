import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LimitWait } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { a11yProblems } from "../test/a11y";
import { LimitBanners } from "./LimitBanners";
import { limitTitle, otherToolWords, resetWords, waitWords, waitingLead } from "./words";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getLimitWaits: vi.fn(),
    pickUpWorkNow: vi.fn(),
    leaveWorkStopped: vi.fn(),
    openResetPage: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();

const NOW = new Date(2026, 9, 3, 12, 0).getTime();
const THREE_PM = new Date(2026, 9, 3, 15, 0).getTime();
const at3 = new Date(THREE_PM).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });

function wait(over: Partial<LimitWait> = {}): LimitWait {
  return {
    runtimeId: "claude-code",
    label: "Claude Code",
    resetCompany: "Anthropic",
    since: NOW - 60_000,
    until: THREE_PM,
    reported: true,
    work: [
      { taskId: "t-1", objective: "Plan the release.", who: "Website Supervisor" },
      { taskId: "t-2", objective: "Say hi", who: null },
    ],
    ...over,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  api.getLimitWaits.mockResolvedValue([wait()]);
  api.pickUpWorkNow.mockResolvedValue([]);
  api.leaveWorkStopped.mockResolvedValue([]);
  api.openResetPage.mockResolvedValue(undefined);
});

afterEach(() => vi.useRealTimers());

describe("the words when a plan runs out (Phase 25, item 4.2)", () => {
  it("says when the work picks back up, and what you can do", () => {
    expect(limitTitle(wait(), NOW)).toBe(`Claude Code is out until ${at3}.`);
    expect(waitWords(wait(), NOW)).toBe(`Plenipo picks the work back up at ${at3}.`);
    expect(waitingLead(wait())).toBe("2 objectives wait for it:");
    expect(resetWords(wait())).toBe(
      "If Anthropic gave you a usage reset, you can use it now in Claude. Then press Pick it up now.",
    );
    expect(otherToolWords(wait())).toBe(
      "Choose your Anthropic key or another AI tool for this work in Settings → AI models.",
    );
    // Codex: OpenAI's reset is used in ChatGPT.
    const codex = wait({ runtimeId: "codex", label: "Codex", resetCompany: "OpenAI" });
    expect(resetWords(codex)).toContain("you can use it now in ChatGPT");
    // No reset time reported: Plenipo's own next try, said plainly, never guessed as a reset.
    const guessed = wait({ reported: false });
    expect(limitTitle(guessed, NOW)).toBe("Claude Code reached its usage limit.");
    expect(waitWords(guessed, NOW)).toBe(
      `It didn't say when it resets. Plenipo tries again at ${at3} and picks the work back up then.`,
    );
    // A company with no resets gets no reset choice.
    const grok = wait({ runtimeId: "grok", label: "Grok", resetCompany: null });
    expect(resetWords(grok)).toBeNull();
    expect(otherToolWords(grok)).toBe(
      "Choose another AI tool for this work in Settings → AI models.",
    );
    expect(limitTitle(wait({ until: null }), NOW)).toBe("Claude Code's usage limit is over.");
  });
});

describe("the notice when a plan runs out (Phase 25, item 4.2)", () => {
  it("lists the waiting work and offers each choice as a button", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <main>
        <h1>Page</h1>
        <LimitBanners go={go} />
      </main>,
    );
    const notice = await screen.findByRole("status", { name: "Claude Code's usage limit" });
    expect(within(notice).getByText(/Claude Code is out until/)).toBeInTheDocument();
    expect(within(notice).getByText("· Website Supervisor")).toBeInTheDocument();
    const choices = within(notice).getByRole("list", {
      name: "What you can do about Claude Code",
    });
    expect(
      within(choices)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Wait", "Use a reset", "Pick it up now", "Use another AI tool", "Leave stopped"]);
    expect(a11yProblems(container)).toEqual([]);

    // A waiting objective opens its task.
    await user.click(within(notice).getByRole("button", { name: "Plan the release." }));
    expect(go).toHaveBeenCalledWith({ view: "task", id: "t-1" });
    // Use a reset opens the company's own page; Plenipo uses nothing itself.
    await user.click(within(choices).getByRole("button", { name: "Use a reset" }));
    expect(api.openResetPage).toHaveBeenCalledWith("claude-code");
    // Another AI tool: Settings → AI models.
    await user.click(within(choices).getByRole("button", { name: "Use another AI tool" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "aiModels" });
    // Leave stopped: all its waiting work, and the notice goes.
    await user.click(within(choices).getByRole("button", { name: "Leave stopped" }));
    expect(api.leaveWorkStopped).toHaveBeenCalledWith(["t-1", "t-2"]);
    await waitFor(() =>
      expect(screen.queryByRole("status", { name: "Claude Code's usage limit" })).toBeNull(),
    );
  });

  it("picks the work up now after a reset, and Wait hides the notice for this limit", async () => {
    const user = userEvent.setup();
    api.getLimitWaits.mockResolvedValue([
      wait(),
      wait({ runtimeId: "grok", label: "Grok", resetCompany: null }),
    ]);
    render(<LimitBanners go={go} />);
    const claude = await screen.findByRole("status", { name: "Claude Code's usage limit" });
    const grok = screen.getByRole("status", { name: "Grok's usage limit" });
    expect(within(grok).queryByRole("button", { name: "Use a reset" })).toBeNull();
    await user.click(within(claude).getByRole("button", { name: "Pick it up now" }));
    expect(api.pickUpWorkNow).toHaveBeenCalledWith("claude-code");
    await user.click(within(grok).getByRole("button", { name: "Wait" }));
    expect(screen.queryByRole("status", { name: "Grok's usage limit" })).toBeNull();
  });

  it("says why a choice didn't work", async () => {
    const user = userEvent.setup();
    api.pickUpWorkNow.mockRejectedValue(new Error("All work is stopped."));
    render(<LimitBanners go={go} />);
    const notice = await screen.findByRole("status", { name: "Claude Code's usage limit" });
    await user.click(within(notice).getByRole("button", { name: "Pick it up now" }));
    expect(await within(notice).findByRole("alert")).toHaveTextContent("All work is stopped.");
  });

  it("shows nothing when no work waits", async () => {
    api.getLimitWaits.mockResolvedValue([]);
    const { container } = render(<LimitBanners go={go} />);
    await waitFor(() => expect(api.getLimitWaits).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});
