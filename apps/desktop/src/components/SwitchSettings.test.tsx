import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import { samplePermissions } from "../test/permissionFixtures";
import { SwitchSettings } from "./SwitchSettings";

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return { ...actual, getPermissions: vi.fn(), setSwitches: vi.fn() };
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

  it("shows a refusal", async () => {
    api.setSwitches.mockRejectedValue(new commands.PlenipoCommandError("internal", "Ledger busy"));
    render(<SwitchSettings />);
    await userEvent
      .setup()
      .click(await screen.findByRole("switch", { name: "Screenshots in the Activity trail" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Ledger busy");
  });
});
