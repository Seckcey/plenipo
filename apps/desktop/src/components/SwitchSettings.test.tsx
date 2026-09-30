import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { aiPage, aiTool } from "../test/aiToolFixtures";
import { samplePermissions } from "../test/permissionFixtures";
import { SwitchSettings } from "./SwitchSettings";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getPermissions: vi.fn(),
    setSwitches: vi.fn(),
    getAiTools: vi.fn(),
    setAiToolsAutoUpdate: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

beforeEach(() => {
  api.getPermissions.mockResolvedValue(samplePermissions());
  api.getAiTools.mockResolvedValue(aiPage([aiTool("grok")]));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Switches", () => {
  it("shows each switch's state and flips one", async () => {
    const on = samplePermissions();
    on.settings.switches.sendWithoutAsking = true;
    api.setSwitches.mockResolvedValue(on);
    render(<SwitchSettings />);
    const browser = await screen.findByRole("switch", { name: "Plenipo's browser" });
    expect(browser).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("switch", { name: "Screen, mouse, and keyboard" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    const send = screen.getByRole("switch", { name: "Sending forms and messages" });
    expect(send).toHaveAttribute("aria-checked", "false");
    expect(screen.getByRole("note")).toHaveTextContent("Off means workers ask you first.");
    expect(screen.getByText(/Always on, with no switch/)).toBeInTheDocument();
    await userEvent.setup().click(send);
    expect(api.setSwitches).toHaveBeenCalledWith({
      ...samplePermissions().settings.switches,
      sendWithoutAsking: true,
    });
    expect(
      await screen.findByRole("switch", { name: "Sending forms and messages" }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("keeps remote computers (SSH) off until you turn them on", async () => {
    const on = samplePermissions();
    on.settings.switches.servers = true;
    api.setSwitches.mockResolvedValue(on);
    render(<SwitchSettings />);
    const ssh = await screen.findByRole("switch", { name: "Remote computers (SSH)" });
    expect(ssh).toHaveAttribute("aria-checked", "false");
    await userEvent.setup().click(ssh);
    expect(api.setSwitches).toHaveBeenCalledWith({
      ...samplePermissions().settings.switches,
      servers: true,
    });
    expect(await screen.findByRole("switch", { name: "Remote computers (SSH)" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("keeps paid AI keys off until you turn them on (Phase 16 Wave 3)", async () => {
    const on = samplePermissions();
    on.settings.switches.paidAiKeys = true;
    api.setSwitches.mockResolvedValue(on);
    render(<SwitchSettings />);
    const paid = await screen.findByRole("switch", { name: "Let workers use paid AI keys" });
    expect(paid).toHaveAttribute("aria-checked", "false");
    expect(paid).toHaveAccessibleDescription(/only within your spending caps/);
    await userEvent.setup().click(paid);
    expect(api.setSwitches).toHaveBeenCalledWith({
      ...samplePermissions().settings.switches,
      paidAiKeys: true,
    });
    expect(
      await screen.findByRole("switch", { name: "Let workers use paid AI keys" }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("turns on Update AI tools by themselves, the same setting as the AI tools page's (Phase 19)", async () => {
    api.setAiToolsAutoUpdate.mockResolvedValue(aiPage([aiTool("grok")], { autoUpdate: true }));
    render(<SwitchSettings />);
    const auto = await screen.findByRole("switch", { name: "Update AI tools by themselves" });
    // Off to start with: Plenipo asks first.
    expect(auto).toHaveAttribute("aria-checked", "false");
    expect(auto).toHaveAccessibleDescription(
      "Off (the default): Plenipo tells you when a new version is ready and updates only when you press Update. On: it updates each AI tool by itself, only when no task is using it.",
    );
    await userEvent.setup().click(auto);
    expect(api.setAiToolsAutoUpdate).toHaveBeenCalledWith(true);
    expect(api.setSwitches).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("switch", { name: "Update AI tools by themselves" }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("shows a refusal", async () => {
    api.setSwitches.mockRejectedValue(new commands.PlenipoCommandError("internal", "Ledger busy"));
    render(<SwitchSettings />);
    await userEvent
      .setup()
      .click(await screen.findByRole("switch", { name: "Screenshots in the Activity trail" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Ledger busy");
  });
});
