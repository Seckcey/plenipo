import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { LedgerEvent, ModelInfo, RoutingSnapshot } from "@plenipo/types";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import * as events from "../../api/events";
import { groupModels, modelGroups } from "../../routing/format";
import { ANTHROPIC, OPENAI, sampleRouting, tool } from "../../test/routingFixtures";
import { ModelSettings } from "./ModelSettings";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getRouting: vi.fn(),
    saveModel: vi.fn(),
    removeModel: vi.fn(),
    setRolePolicy: vi.fn(),
    setModelRule: vi.fn(),
    setRoutingOptions: vi.fn(),
    clearUsageLimit: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeAgentUpdates: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();
let emitLedger: (event: LedgerEvent) => void = () => undefined;

beforeEach(() => {
  api.getRouting.mockResolvedValue(sampleRouting());
  for (const f of [
    api.saveModel,
    api.removeModel,
    api.setRolePolicy,
    api.setModelRule,
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
  it("sets model and effort rules for the organization and a department (ADR-041)", async () => {
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    expect(
      await screen.findByRole("heading", { name: "Model and effort rules" }),
    ).toBeInTheDocument();
    await user.click(
      await screen.findByRole("button", { name: "Change the rule for The whole organization" }),
    );
    const org = screen.getByRole("form", { name: "Rule for The whole organization" });
    // The organization's rule is emptied, never removed.
    expect(within(org).queryByRole("button", { name: "Remove rule" })).toBeNull();
    await user.selectOptions(
      within(org).getByRole("combobox", { name: "Effort for any other model" }),
      "High effort",
    );
    await user.click(within(org).getByRole("button", { name: "Save rule" }));
    expect(api.setModelRule).toHaveBeenCalledWith(
      { layer: "organization" },
      { models: [], efforts: {}, effort: "high", neverCompanies: [] },
    );
    await user.click(screen.getByRole("button", { name: "Change the rule for Engineering" }));
    const eng = screen.getByRole("form", { name: "Rule for Engineering" });
    await user.selectOptions(
      within(eng).getByRole("combobox", { name: "Add a model to the list" }),
      "Opus (Claude Code)",
    );
    // AI companies never to use add up across the rules.
    const openai = within(eng).getByRole("checkbox", { name: "OpenAI" });
    expect(openai).toHaveAccessibleDescription(
      "Never used for this work, even when another rule lists them: these add up.",
    );
    await user.click(openai);
    await user.click(within(eng).getByRole("button", { name: "Save rule" }));
    expect(api.setModelRule).toHaveBeenLastCalledWith(
      { layer: "department", id: "d-eng" },
      { models: ["m-opus"], efforts: {}, effort: null, neverCompanies: ["openai"] },
    );
  });

  it("shows where each role's next worker goes and why", async () => {
    render(<ModelSettings go={go} />);
    const dev = await screen.findByRole("row", { name: /^Senior Developer/ });
    expect(within(dev).getByText("Opus (Claude Code)")).toBeInTheDocument();
    expect(
      within(dev).getByText("Opus (Claude Code) is Senior Developer's first choice and is ready."),
    ).toBeInTheDocument();
    const designer = rowOf("Designer");
    expect(within(designer).getByText("None right now")).toBeInTheDocument();
    expect(within(designer).getByText(/not marked as able to make images/)).toBeInTheDocument();
    expect(screen.getByText(/Pay-per-use API billing: Off/)).toBeInTheDocument();
  });

  it("links to the AI tools page, where the usage limits moved (Phase 19)", async () => {
    render(<ModelSettings go={go} />);
    const section = (await screen.findByRole("heading", { name: "AI tools" })).closest("section")!;
    expect(
      within(section).getByText("Usage limits, sign-in, and updates are on the AI tools page."),
    ).toBeInTheDocument();
    // The old table of AI tools and their usage limits is gone.
    expect(within(section).queryByRole("table")).toBeNull();
    expect(screen.queryByRole("row", { name: /^Codex OpenAI/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Try again now" })).toBeNull();
    await userEvent.setup().click(screen.getByRole("button", { name: "Open the AI tools page" }));
    expect(go).toHaveBeenCalledWith({ view: "runtimes", id: null });
    // What a usage limit does stays here.
    expect(
      screen.getByRole("heading", { name: "When an AI tool reaches its usage limit" }),
    ).toBeInTheDocument();
  });

  it("changes a role's model choices: order, requirements, and companies", async () => {
    render(<ModelSettings go={go} />);
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
    // Codex's default model runs at high effort for this role; Opus keeps its own setting.
    const codexEffort = within(form).getByRole("combobox", {
      name: "Effort for Codex (default model)",
    });
    expect(
      within(codexEffort)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "Its effort (the AI tool's default)",
      "Low effort",
      "Medium effort",
      "High effort",
      "Extra high effort",
      "Max effort",
      "Ultra effort",
    ]);
    await user.selectOptions(codexEffort, "High effort");
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
      efforts: { "m-codex": "high" },
      effort: null,
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
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Change Designer's model choices" }),
    );
    await user.click(screen.getByRole("button", { name: "Save model choices" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("a model is listed twice");
    expect(screen.getByRole("form", { name: "Model choices for Designer" })).toBeInTheDocument();
  });

  it("adds, edits, and removes models, including one seen in use", async () => {
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Add to your models" }));
    let dialog = screen.getByRole("dialog", { name: "Add a model" });
    expect(within(dialog).getByRole("combobox", { name: "Model" })).toHaveValue("claude-opus-5-5");
    const label = within(dialog).getByRole("textbox", { name: "Your name for it" });
    await user.clear(label);
    await user.type(label, "Opus 5.5");
    await user.click(within(dialog).getByRole("checkbox", { name: "Makes images" }));
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "Cost" }), "Premium");
    await user.selectOptions(within(dialog).getByRole("combobox", { name: /^Effort/ }), "Max");
    await user.click(within(dialog).getByRole("button", { name: "Add model" }));
    expect(api.saveModel).toHaveBeenCalledWith({
      runtimeId: "claude-code",
      name: "claude-opus-5-5",
      label: "Opus 5.5",
      features: ["imageGeneration"],
      cost: "premium",
      effort: "max",
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

  it("adds a model from a menu of the AI tool's models, or by a typed name", async () => {
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Add a model" }));
    const dialog = screen.getByRole("dialog", { name: "Add a model" });
    const model = within(dialog).getByRole("combobox", { name: "Model" });
    const options = (select: HTMLElement) =>
      within(select)
        .getAllByRole("option")
        .map((o) => o.textContent);
    // The AI tool's default first, then Claude Code's own models and the models seen in use;
    // ones already in your list are shown but not offered.
    expect(options(model)).toEqual([
      "The AI tool's default (already in your list)",
      "fable",
      "opus (already in your list)",
      "sonnet",
      "haiku",
      "claude-opus-5-5",
      "Type another name…",
    ]);
    expect(
      within(model)
        .getAllByRole("group")
        .map((g) => g.getAttribute("label")),
    ).toEqual(["Claude Code's models", "Seen in use"]);
    expect(
      within(model).getByRole("option", { name: "opus (already in your list)" }),
    ).toBeDisabled();
    // Choosing one also names the model, until you name it yourself.
    await user.selectOptions(model, "fable");
    expect(within(dialog).getByRole("textbox", { name: "Your name for it" })).toHaveValue("Fable");
    expect(
      within(dialog).queryByRole("textbox", { name: /Model name the AI tool accepts/ }),
    ).toBeNull();
    // Effort offers the levels the chosen model takes; Haiku has no effort setting.
    const effort = within(dialog).getByRole("combobox", { name: /^Effort/ });
    await user.selectOptions(effort, "Max");
    await user.selectOptions(model, "haiku");
    expect(effort).toBeDisabled();
    expect(effort).toHaveDisplayValue("The AI tool's default");
    await user.selectOptions(model, "fable");
    await user.selectOptions(effort, "Max");
    await user.click(within(dialog).getByRole("button", { name: "Add model" }));
    expect(api.saveModel).toHaveBeenLastCalledWith(
      expect.objectContaining({
        runtimeId: "claude-code",
        name: "fable",
        label: "Fable",
        effort: "max",
      }),
    );

    // Codex's own models; typing a name remains as a last resort.
    await user.click(await screen.findByRole("button", { name: "Add a model" }));
    const next = screen.getByRole("dialog", { name: "Add a model" });
    await user.selectOptions(within(next).getByRole("combobox", { name: "AI tool" }), "codex");
    const codexModel = within(next).getByRole("combobox", { name: "Model" });
    expect(options(codexModel)).toEqual([
      "The AI tool's default (already in your list)",
      "gpt-6-sol",
      "gpt-6-luna",
      "Type another name…",
    ]);
    await user.selectOptions(codexModel, "gpt-6-luna");
    expect(within(next).getByRole("textbox", { name: "Your name for it" })).toHaveValue(
      "GPT-6-Luna",
    );
    // GPT-6-Luna goes up to max effort, not ultra.
    expect(options(within(next).getByRole("combobox", { name: /^Effort/ }))).toEqual([
      "The AI tool's default",
      "Low",
      "Medium",
      "High",
      "Extra high",
      "Max",
    ]);
    await user.selectOptions(codexModel, "Type another name…");
    await user.type(
      within(next).getByRole("textbox", { name: /Model name the AI tool accepts/ }),
      "gpt-x",
    );
    expect(within(next).getByRole("textbox", { name: "Your name for it" })).toHaveValue("gpt-x");
    await user.click(within(next).getByRole("button", { name: "Add model" }));
    expect(api.saveModel).toHaveBeenLastCalledWith(
      expect.objectContaining({ runtimeId: "codex", name: "gpt-x", label: "gpt-x" }),
    );
  });

  it("offers a model the AI tool reported as new, not checked yet; choosing it works like a typed name (Phase 19)", async () => {
    const routing = sampleRouting();
    routing.tools = routing.tools.map((t) =>
      t.runtimeId === "codex"
        ? { ...t, newModels: [{ name: "gpt-6-terra", label: "GPT-6-Terra", effortLevels: [] }] }
        : t,
    );
    // In the AI tool's own group, after the models Plenipo checked.
    expect(modelGroups(routing, "codex")).toEqual([
      {
        label: "Codex's models",
        options: [
          { name: "gpt-6-sol", label: "gpt-6-sol" },
          { name: "gpt-6-luna", label: "gpt-6-luna" },
          { name: "gpt-6-terra", label: "gpt-6-terra — new, not checked yet" },
        ],
      },
    ]);
    api.getRouting.mockResolvedValue(routing);
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Add a model" }));
    const dialog = screen.getByRole("dialog", { name: "Add a model" });
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "AI tool" }), "codex");
    const model = within(dialog).getByRole("combobox", { name: "Model" });
    expect(
      within(model)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual([
      "The AI tool's default (already in your list)",
      "gpt-6-sol",
      "gpt-6-luna",
      "gpt-6-terra — new, not checked yet",
      "Type another name…",
    ]);
    await user.selectOptions(model, "gpt-6-terra — new, not checked yet");
    expect(model).toHaveValue("gpt-6-terra");
    // Chosen from the menu, not typed.
    expect(
      within(dialog).queryByRole("textbox", { name: /Model name the AI tool accepts/ }),
    ).toBeNull();
    expect(within(dialog).getByRole("textbox", { name: "Your name for it" })).toHaveValue(
      "GPT-6-Terra",
    );
    await user.click(within(dialog).getByRole("button", { name: "Add model" }));
    expect(api.saveModel).toHaveBeenLastCalledWith(
      expect.objectContaining({ runtimeId: "codex", name: "gpt-6-terra", label: "GPT-6-Terra" }),
    );
  });

  it("groups your models by who made them or by AI tool, the same models both ways, and remembers the choice (ADR-081)", async () => {
    const routing = withOllama();
    api.getRouting.mockResolvedValue(routing);
    localStorage.clear();
    const view = render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    const table = await screen.findByRole("table", { name: "Your models" });
    // By AI tool at first: each AI tool by name, its models in the list's order.
    expect(groupsOf(table)).toEqual([
      ["Claude Code", ["Claude Code (default model)", "Opus"]],
      ["Codex", ["Codex (default model)"]],
      ["Ollama", ["DeepSeek V4 Pro", "Mystery", "gpt-oss"]],
    ]);
    const byMaker = screen.getByRole("button", { name: "Who made it" });
    expect(screen.getByRole("group", { name: "Group your models by" })).toContainElement(byMaker);
    expect(byMaker).toHaveAttribute("aria-pressed", "false");
    await user.click(byMaker);
    expect(byMaker).toHaveAttribute("aria-pressed", "true");
    // By who made them: each company by name, "Not known" last; OpenAI's gpt-oss on Ollama
    // sits with Codex's default model.
    expect(groupsOf(table)).toEqual([
      ["Anthropic", ["Claude Code (default model)", "Opus"]],
      ["DeepSeek", ["DeepSeek V4 Pro"]],
      ["OpenAI", ["Codex (default model)", "gpt-oss"]],
      ["Not known", ["Mystery"]],
    ]);
    // The same models, both ways.
    const names = (by: "maker" | "tool") =>
      groupModels(routing, by)
        .flatMap((g) => g.models.map((m) => m.id))
        .sort();
    expect(names("maker")).toEqual(names("tool"));
    expect(names("maker")).toEqual(routing.models.map((m) => m.id).sort());
    // Each row says who made it, in plain words.
    expect(within(rowOf("Mystery")).getByText("Not known")).toBeInTheDocument();
    expect(within(rowOf("gpt-oss")).getByText("OpenAI")).toBeInTheDocument();
    expect(within(rowOf("gpt-oss")).getByText("Ollama")).toBeInTheDocument();
    // Remembered on this PC: shown again, still by who made them.
    expect(localStorage.getItem("plenipo.models.groupBy")).toBe(JSON.stringify("maker"));
    view.unmount();
    render(<ModelSettings go={go} />);
    const again = await screen.findByRole("table", { name: "Your models" });
    expect(groupsOf(again).map(([label]) => label)).toEqual([
      "Anthropic",
      "DeepSeek",
      "OpenAI",
      "Not known",
    ]);
    await user.click(screen.getByRole("button", { name: "AI tool" }));
    expect(localStorage.getItem("plenipo.models.groupBy")).toBe(JSON.stringify("tool"));
    localStorage.clear();
  });

  it("names each model's exact version and, on an AI tool that runs several companies' models, who made it (ADR-081)", async () => {
    const routing = withOllama();
    routing.tools = routing.tools.map((t) =>
      t.runtimeId === "claude-code"
        ? {
            ...t,
            knownModels: [
              {
                name: "opus",
                label: "Opus",
                effortLevels: [],
                maker: ANTHROPIC,
                pointsTo: "claude-opus-5-5",
              },
              { name: "claude-opus-5-5", label: "Opus 5.5", effortLevels: [], maker: ANTHROPIC },
            ],
          }
        : t,
    );
    // Claude Code runs only Anthropic's models, so its menu does not repeat who made them.
    expect(modelGroups(routing, "claude-code", { yours: false })[0]).toEqual({
      label: "Claude Code's models",
      options: [
        { name: "opus", label: "opus — now Opus 5.5" },
        { name: "claude-opus-5-5", label: "claude-opus-5-5" },
      ],
    });
    // Ollama runs several companies' models: each one says who made it.
    expect(modelGroups(routing, "ollama", { yours: false })[0]).toEqual({
      label: "Ollama's models",
      options: [
        { name: "deepseek-v4-pro:cloud", label: "deepseek-v4-pro:cloud — made by DeepSeek" },
        { name: "gpt-oss:120b-cloud", label: "gpt-oss:120b-cloud — made by OpenAI" },
      ],
    });
    // "AI companies never to use" offers the companies that make models, not only the AI
    // tools' own.
    api.getRouting.mockResolvedValue(routing);
    render(<ModelSettings go={go} />);
    const user = userEvent.setup();
    await user.click(
      await screen.findByRole("button", { name: "Change Senior Developer's model choices" }),
    );
    const form = screen.getByRole("form", { name: "Model choices for Senior Developer" });
    await user.click(within(form).getByRole("checkbox", { name: "DeepSeek" }));
    await user.click(within(form).getByRole("button", { name: "Save model choices" }));
    expect(api.setRolePolicy).toHaveBeenCalledWith(
      "r-dev",
      expect.objectContaining({ neverCompanies: ["deepseek"] }),
    );
  });

  it("chooses what a usage limit does, and follows the Ledger", async () => {
    render(<ModelSettings go={go} />);
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

const DEEPSEEK = { id: "deepseek", label: "DeepSeek" };

/** An owner's model on Ollama, which runs several companies' models. */
const onOllama = (
  id: string,
  name: string,
  label: string,
  maker?: typeof ANTHROPIC,
): ModelInfo => ({
  id,
  runtimeId: "ollama",
  name,
  label,
  features: [],
  contextTokens: null,
  cost: "standard",
  effort: null,
  builtIn: false,
  ...(maker ? { maker } : {}),
});

/** The sample, with Ollama and three of its models: DeepSeek's, one not known, and OpenAI's. */
function withOllama(): RoutingSnapshot {
  const routing = sampleRouting();
  routing.tools.push(
    tool("ollama", {
      label: "Ollama",
      company: "ollama",
      companyLabel: "Ollama",
      runsOtherMakers: true,
      effortLevels: [],
      knownModels: [
        {
          name: "deepseek-v4-pro:cloud",
          label: "DeepSeek V4 Pro",
          effortLevels: [],
          maker: DEEPSEEK,
        },
        { name: "gpt-oss:120b-cloud", label: "gpt-oss 120B", effortLevels: [], maker: OPENAI },
      ],
    }),
  );
  routing.models.push(
    onOllama("m-deepseek", "deepseek-v4-pro:cloud", "DeepSeek V4 Pro", DEEPSEEK),
    onOllama("m-mystery", "mystery:cloud", "Mystery"),
    onOllama("m-gpt-oss", "gpt-oss:120b-cloud", "gpt-oss", OPENAI),
  );
  routing.companies = [ANTHROPIC, DEEPSEEK, { id: "ollama", label: "Ollama" }, OPENAI];
  return routing;
}

/** The model list's groups, in order: each heading and its models' names. */
function groupsOf(table: HTMLElement): [string, string[]][] {
  return within(table)
    .getAllByRole("rowgroup")
    .filter((g) => g.tagName === "TBODY")
    .map((g) => [
      g.getAttribute("aria-label") ?? "",
      within(g)
        .getAllByRole("rowheader")
        .map((h) => h.firstChild?.textContent ?? ""),
    ]);
}
