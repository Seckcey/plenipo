import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import * as events from "../../api/events";
import { sampleRouting } from "../../test/routingFixtures";
import { ModelSettings } from "./ModelSettings";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRouting: vi.fn(),
    saveModel: vi.fn(),
    removeModel: vi.fn(),
    setRolePolicy: vi.fn(),
    setRoutingOptions: vi.fn(),
    clearUsageLimit: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
let emitLedger: (event: LedgerEvent) => void = () => undefined;

beforeEach(() => {
  api.getRouting.mockResolvedValue(sampleRouting());
  for (const f of [
    api.saveModel,
    api.removeModel,
    api.setRolePolicy,
    api.setRoutingOptions,
    api.clearUsageLimit,
  ]) {
    f.mockResolvedValue(sampleRouting());
  }
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((handler) => {
    emitLedger = handler;
    return Promise.resolve(() => undefined);
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const rowOf = (name: string) => screen.getByRole("row", { name: new RegExp(`^${name}`) });

describe("Settings → AI models", () => {
  it("shows where each role's next worker goes and why", async () => {
    render(<ModelSettings />);
    const dev = await screen.findByRole("row", { name: /^Senior Developer/ });
    expect(within(dev).getByText("Opus (Claude Code)")).toBeInTheDocument();
    expect(
      within(dev).getByText("Opus (Claude Code) is Senior Developer's first choice and is ready."),
    ).toBeInTheDocument();
    const designer = rowOf("Designer");
    expect(within(designer).getByText("None right now")).toBeInTheDocument();
    expect(within(designer).getByText(/not marked as able to make images/)).toBeInTheDocument();
    // The AI tools, with a usage limit and the way to try again.
    const codex = screen.getByRole("row", { name: /^Codex OpenAI/ });
    expect(within(codex).getByText("Not now")).toBeInTheDocument();
    await userEvent.setup().click(within(codex).getByRole("button", { name: "Try again now" }));
    expect(api.clearUsageLimit).toHaveBeenCalledWith("codex");
    expect(screen.getByText(/Pay-per-use API billing: Off/)).toBeInTheDocument();
  });

  it("changes a role's model choices: order, requirements, and companies", async () => {
    render(<ModelSettings />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Change Senior Developer's model choices" }),
    );
    const form = screen.getByRole("form", { name: "Model choices for Senior Developer" });
    expect(within(form).getByText("First choice")).toBeInTheDocument();
    await user.click(within(form).getByRole("button", { name: "Move Codex (default model) up" }));
    await user.selectOptions(
      within(form).getByRole("combobox", { name: "Add a model to the list" }),
      "Claude Code (default model)",
    );
    await user.click(within(form).getByRole("checkbox", { name: "Sees images" }));
    await user.type(
      within(form).getByRole("spinbutton", { name: /Context size, at least/ }),
      "100000",
    );
    await user.selectOptions(
      within(form).getByRole("combobox", { name: /^Reviews/ }),
      "Prefer a different AI company",
    );
    await user.click(within(form).getByRole("checkbox", { name: "OpenAI" }));
    await user.click(within(form).getByRole("button", { name: "Save model choices" }));
    expect(api.setRolePolicy).toHaveBeenCalledWith("r-dev", {
      models: ["m-codex", "m-opus", "m-claude"],
      needs: ["vision"],
      minContextTokens: 100000,
      neverCompanies: ["openai"],
      cost: "any",
      crossCompany: "prefer",
    });
    await waitFor(() =>
      expect(
        screen.queryByRole("form", { name: "Model choices for Senior Developer" }),
      ).not.toBeInTheDocument(),
    );
  });

  it("keeps the editor open with the refusal", async () => {
    api.setRolePolicy.mockRejectedValue({
      kind: "invalidInput",
      message: "a model is listed twice",
    });
    render(<ModelSettings />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Change Designer's model choices" }),
    );
    await user.click(screen.getByRole("button", { name: "Save model choices" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a model is listed twice");
    expect(screen.getByRole("form", { name: "Model choices for Designer" })).toBeInTheDocument();
  });

  it("adds, edits, and removes models, including one seen in use", async () => {
    render(<ModelSettings />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Add to your models" }));
    let dialog = screen.getByRole("dialog", { name: "Add a model" });
    expect(within(dialog).getByRole("textbox", { name: /Model name/ })).toHaveValue(
      "claude-opus-5-5",
    );
    const label = within(dialog).getByRole("textbox", { name: "Your name for it" });
    await user.clear(label);
    await user.type(label, "Opus 5.5");
    await user.click(within(dialog).getByRole("checkbox", { name: "Makes images" }));
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "Cost" }), "Premium");
    await user.click(within(dialog).getByRole("button", { name: "Add model" }));
    expect(api.saveModel).toHaveBeenCalledWith({
      runtimeId: "claude-code",
      name: "claude-opus-5-5",
      label: "Opus 5.5",
      features: ["imageGeneration"],
      cost: "premium",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    // A built-in entry keeps its AI tool and default model; only its details change.
    await user.click(screen.getByRole("button", { name: "Edit Codex (default model)" }));
    dialog = screen.getByRole("dialog", { name: "Edit Codex (default model)" });
    expect(within(dialog).getByRole("combobox", { name: "AI tool" })).toBeDisabled();
    await user.click(within(dialog).getByRole("button", { name: "Save model" }));
    expect(api.saveModel).toHaveBeenLastCalledWith({
      id: "m-codex",
      runtimeId: "codex",
      label: "Codex (default model)",
      features: [],
      cost: "standard",
    });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "Remove Codex (default model)" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Remove Opus" }));
    expect(api.removeModel).toHaveBeenCalledWith("m-opus");
  });

  it("chooses what a usage limit does, and follows the Ledger", async () => {
    render(<ModelSettings />);
    const user = userEvent.setup();
    const wait = await screen.findByRole("radio", { name: /Wait for the limit to reset/ });
    expect(wait).toBeChecked();
    await user.click(screen.getByRole("radio", { name: /Use the role's next choice/ }));
    expect(api.setRoutingOptions).toHaveBeenCalledWith({ onUsageLimit: "nextChoice" });
    // A turn result (a usage limit, a model seen) reloads the settings.
    api.getRouting.mockClear();
    emitLedger({
      seq: 1,
      id: "e1",
      taskId: null,
      executionId: null,
      source: "agent:codex",
      destination: null,
      eventType: "agent.result",
      payload: {},
      createdAt: 0,
    });
    await waitFor(() => expect(api.getRouting).toHaveBeenCalled());
  });
});
