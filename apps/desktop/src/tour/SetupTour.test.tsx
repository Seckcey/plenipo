import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { OrgListing } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { emptyOrganization, sampleOrganization } from "../test/orgFixtures";
import { SetupTour } from "./SetupTour";
import { SetupTourButton } from "./SetupTourButton";
import { BY_ITSELF_KEY, SETUP_TOUR_KEY, endSetupTour, progressOf, saveProgress } from "./store";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getOrganization: vi.fn(), getOrganizations: vi.fn() };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeShared: vi.fn(() => Promise.resolve(() => undefined)),
  subscribeOrganizations: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

const listing = (createdAt: number): OrgListing => ({
  current: "org-1",
  organizations: [
    {
      id: "org-1",
      name: "Acme",
      first: true,
      archived: false,
      here: true,
      inWindow: true,
      working: 0,
      createdAt,
    },
  ],
  templates: [],
});

/** The popover's words, once Driver.js shows them. */
const popoverTitle = () => document.querySelector(".driver-popover-title")?.textContent ?? null;

beforeEach(() => {
  localStorage.clear();
  api.getOrganization.mockResolvedValue(emptyOrganization());
  api.getOrganizations.mockResolvedValue(listing(Date.now()));
});

afterEach(() => {
  cleanup();
  act(() => endSetupTour());
  vi.clearAllMocks();
});

describe("the setup tour (Phase 25, item 2.9)", () => {
  it("starts by itself in a new organization, skips what's done, and remembers where you stopped", async () => {
    const go = vi.fn();
    render(
      <>
        <SetupTour go={go} snapshot={emptyOrganization()} />
        <SetupTourButton />
      </>,
    );
    await waitFor(() => expect(popoverTitle()).toBe("Welcome to Plenipo"), { timeout: 3000 });
    // The page stays usable: nothing blocks clicks, and Tab isn't kept inside the tour.
    expect(document.body).not.toHaveClass("driver-active");
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    window.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(false);
    expect(document.querySelector(".driver-popover-progress-text")).toHaveTextContent(
      "Step 1 of 9",
    );
    expect(go).toHaveBeenCalledWith({ view: "home", id: null });
    // The button waits while the tour shows.
    expect(screen.getByRole("button", { name: "Take the setup tour again" })).toBeDisabled();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Next" }));
    // Claude Code is signed in already: on to naming the organization in Settings.
    await waitFor(() => expect(popoverTitle()).toBe("Name your organization, or pick a template"), {
      timeout: 3000,
    });
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "organization" });
    expect(go).not.toHaveBeenCalledWith({ view: "runtimes", id: null });

    await user.click(screen.getByRole("button", { name: "Stop the tour for now" }));
    await waitFor(() => expect(popoverTitle()).toBeNull());
    expect(progressOf("org-1")).toEqual({ step: 2, status: "stopped" });
    expect(screen.getByRole("button", { name: "Pick up the setup tour" })).toBeEnabled();

    // Picking it up starts where you stopped.
    go.mockClear();
    await user.click(screen.getByRole("button", { name: "Pick up the setup tour" }));
    await waitFor(() => expect(popoverTitle()).toBe("Name your organization, or pick a template"), {
      timeout: 3000,
    });
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "organization" });
  });

  it("waits on a step until it's really done, and offers to skip it", async () => {
    saveProgress("org-1", { step: 3, status: "going" });
    render(<SetupTour go={vi.fn()} snapshot={emptyOrganization()} />);
    // The app closed in the middle of the tour: it picks up at "Add a department".
    await waitFor(() => expect(popoverTitle()).toBe("Add a department"), { timeout: 3000 });
    expect(screen.getByRole("button", { name: "Skip this step" })).toBeInTheDocument();
  });

  it("doesn't start by itself in an organization already set up, or once stopped", async () => {
    const { unmount } = render(<SetupTour go={vi.fn()} snapshot={sampleOrganization()} />);
    await waitFor(() => expect(api.getOrganizations).toHaveBeenCalled());
    await act(() => new Promise((r) => setTimeout(r, 50)));
    expect(popoverTitle()).toBeNull();
    unmount();

    saveProgress("org-1", { step: 1, status: "stopped" });
    const again = render(<SetupTour go={vi.fn()} snapshot={emptyOrganization()} />);
    await act(() => new Promise((r) => setTimeout(r, 1300)));
    expect(popoverTitle()).toBeNull();
    expect(localStorage.getItem(SETUP_TOUR_KEY)).toContain("stopped");
    again.unmount();

    // Turned off on this PC: never by itself.
    localStorage.clear();
    localStorage.setItem(BY_ITSELF_KEY, "off");
    render(<SetupTour go={vi.fn()} snapshot={emptyOrganization()} />);
    await act(() => new Promise((r) => setTimeout(r, 1300)));
    expect(popoverTitle()).toBeNull();
  });
});
