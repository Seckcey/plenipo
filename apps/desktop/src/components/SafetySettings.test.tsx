import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { samplePermissions } from "../test/permissionFixtures";
import { SAFETY_CHOICES, safetyWarning } from "./safety";
import { SafetySettings } from "./SafetySettings";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getPermissions: vi.fn(), setSafety: vi.fn() };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

beforeEach(() => {
  api.getPermissions.mockResolvedValue(samplePermissions());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Safety", () => {
  it("starts on Light and says what that means, with the warning in plain words", async () => {
    render(<SafetySettings />);
    const light = await screen.findByRole("button", { name: "Light" });
    expect(light).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Careful" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "Strict" })).toHaveAttribute("aria-pressed", "false");
    const note = screen.getByRole("note");
    expect(note).toHaveTextContent("Read this once.");
    expect(note).toHaveTextContent("without asking you");
    expect(note).toHaveTextContent("Never run list");
    expect(note).toHaveTextContent("Choose Careful if you want to be asked first.");
    // Every choice is explained, and the one that is on says so.
    for (const c of SAFETY_CHOICES) expect(screen.getByText(c.says)).toBeInTheDocument();
    expect(screen.getByText("(on now)")).toBeInTheDocument();
  });

  it("changes the choice and shows the warning for the new one", async () => {
    const careful = samplePermissions();
    careful.settings.safety = "careful";
    api.setSafety.mockResolvedValue(careful);
    render(<SafetySettings />);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Careful" }));
    expect(api.setSafety).toHaveBeenCalledWith("careful");
    expect(await screen.findByRole("button", { name: "Careful" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("note")).toHaveTextContent("On Careful, agents still create");
  });

  it("does not save when you choose what is on already", async () => {
    render(<SafetySettings />);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Light" }));
    expect(api.setSafety).not.toHaveBeenCalled();
  });

  it("shows a refusal instead of changing the screen", async () => {
    api.setSafety.mockRejectedValue(
      new commands.PlenipoCommandError("internal", "That did not save."),
    );
    render(<SafetySettings />);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Strict" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That did not save.");
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
  });

  it("lists what always asks you and what is never allowed", async () => {
    render(<SafetySettings />);
    const asks = await screen.findByRole("heading", {
      name: "What still asks you, whichever you choose",
    });
    const list = asks.parentElement as HTMLElement;
    expect(
      within(list).getByText(/Sending or publishing outside this computer/),
    ).toBeInTheDocument();
    expect(within(list).getByText(/Changing DNS/)).toBeInTheDocument();
    expect(within(list).getByText("(blocked)")).toBeInTheDocument();
    expect(screen.getByText(/outside the agent's own folder/)).toBeInTheDocument();
  });

  it("warns less on the other choices and uses everyday words", () => {
    for (const choice of ["light", "careful", "strict"] as const) {
      const words = safetyWarning(choice);
      expect(words).not.toMatch(/capabilit|runtime|sandbox|privilege|ACL/i);
    }
    expect(safetyWarning("strict")).toContain("only read");
  });
});
