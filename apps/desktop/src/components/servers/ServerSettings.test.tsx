import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { sampleServer, sampleServers } from "../../test/serverFixtures";
import { ServerSettings } from "./ServerSettings";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getServers: vi.fn(),
    saveServer: vi.fn(),
    removeServer: vi.fn(),
    checkServerIdentity: vi.fn(),
    testServer: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);

beforeEach(() => {
  api.getServers.mockResolvedValue(sampleServers());
  api.saveServer.mockResolvedValue(sampleServers());
  api.removeServer.mockResolvedValue(sampleServers([]));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Servers", () => {
  it("marks a production server plainly, with its server ID, roles, and rules", async () => {
    const dev = sampleServer({
      id: "srv-dev",
      name: "Dev box",
      environment: "development",
      approval: "changes",
      signIn: "key",
    });
    api.getServers.mockResolvedValue(sampleServers([sampleServer(), dev]));
    render(<ServerSettings />);
    const shop = await screen.findByRole("listitem", { name: "Shop, production server" });
    expect(shop).toHaveClass("server--production");
    expect(within(shop).getByText("PRODUCTION")).toHaveClass("env--production");
    expect(shop).toHaveTextContent("SHA256:AAAAbbbbCCCCddddEEEEffffGGGGhhhhIIIIjjjjKKK");
    expect(shop).toHaveTextContent("Operations Engineer");
    expect(shop).toHaveTextContent("Before every command (production)");
    expect(shop).toHaveTextContent("stored in Windows Credential Manager");
    expect(shop).toHaveTextContent("Port forwarding" + "Off");
    const devCard = screen.getByRole("listitem", { name: "Dev box, test server" });
    expect(devCard).toHaveClass("server--development");
    expect(within(devCard).getByText("Test")).toHaveClass("env--development");
    expect(devCard).toHaveTextContent("Before anything that is not looking around");
  });

  it("says plainly when a server's ID changed", async () => {
    api.getServers.mockResolvedValue(
      sampleServers([
        {
          ...sampleServer(),
          identityChanged: {
            algorithm: "ssh-ed25519",
            fingerprint: "SHA256:NEWNEWNEW",
            at: Date.now(),
          },
          problem: "It showed a different server ID.",
        },
      ]),
    );
    render(<ServerSettings />);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("This server's ID changed");
    expect(alert).toHaveTextContent("SHA256:NEWNEWNEW");
    expect(alert).toHaveTextContent("Plenipo did not sign in");
  });

  it("adds a server: checks and pins its server ID, and sends the key once", async () => {
    api.getServers.mockResolvedValue(sampleServers([]));
    api.checkServerIdentity.mockResolvedValue({
      host: "dev.example.com",
      port: 22,
      algorithm: "ssh-ed25519",
      fingerprint: "SHA256:DEVDEVDEV",
    });
    render(<ServerSettings />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Add a server" }));
    const form = screen.getByRole("form", { name: "Add a server" });
    await user.type(within(form).getByLabelText("Name"), "Dev box");
    await user.type(within(form).getByLabelText("Address"), "dev.example.com");
    await user.type(within(form).getByLabelText("Sign in as"), "deploy");
    await user.type(
      within(form).getByLabelText(/^Private key/),
      "-----BEGIN OPENSSH PRIVATE KEY-----",
    );
    await user.click(within(form).getByRole("button", { name: "Check the server ID" }));
    expect(api.checkServerIdentity).toHaveBeenCalledWith("dev.example.com", 22);
    const identity = await within(form).findByLabelText("The server ID");
    expect(identity).toHaveTextContent("SHA256:DEVDEVDEV");
    // A fingerprint that does not match is never offered for pinning.
    const expected = within(identity).getByLabelText(/The fingerprint you expect/);
    await user.type(expected, "SHA256:OTHER");
    expect(within(identity).getByRole("alert")).toHaveTextContent("They are different");
    expect(within(identity).queryByRole("button", { name: /pin this ID/ })).toBeNull();
    await user.clear(expected);
    await user.click(
      within(identity).getByRole("button", { name: "This is my server: pin this ID" }),
    );
    await user.click(within(form).getByRole("checkbox", { name: /Operations Engineer/ }));
    await user.click(within(form).getByRole("button", { name: "Add the server" }));
    expect(api.saveServer).toHaveBeenCalledWith({
      name: "Dev box",
      host: "dev.example.com",
      port: 22,
      user: "deploy",
      environment: "development",
      signIn: "key",
      key: "-----BEGIN OPENSSH PRIVATE KEY-----",
      hostKey: { algorithm: "ssh-ed25519", fingerprint: "SHA256:DEVDEVDEV" },
      roles: ["role-ops"],
      classes: ["look", "services", "change"],
      approval: "changes",
      folders: [],
      forwards: [],
    });
  });

  it("asks before every command on production, whatever is chosen", async () => {
    render(<ServerSettings />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Change Shop" }));
    const form = screen.getByRole("form", { name: "Change Shop" });
    expect(within(form).getByRole("note")).toHaveTextContent("Production server.");
    const when = within(form).getByRole("combobox", { name: "When to ask you" });
    expect(when).toBeDisabled();
    expect(when).toHaveValue("every");
    // Turning on destroying commands on production warns that each still asks.
    await user.click(within(form).getByRole("checkbox", { name: /Delete, wipe, or shut down/ }));
    expect(form).toHaveTextContent("On a production server. Each one will still ask you.");
    await user.click(within(form).getByRole("button", { name: "Save the server" }));
    const sent = api.saveServer.mock.calls[0]?.[0];
    expect(sent?.approval).toBe("every");
    expect(sent?.classes).toContain("destroy");
    expect(sent?.id).toBe("srv-shop");
    // Nothing typed: the stored password is kept, not sent again.
    expect(sent?.password).toBeUndefined();
  });

  it("tests the connection and removes a server only when confirmed", async () => {
    api.testServer.mockResolvedValue({ ok: true, message: "Connected to Shop and signed in." });
    render(<ServerSettings />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Test the connection" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Connected to Shop");
    await user.click(screen.getByRole("button", { name: "Remove Shop" }));
    expect(api.removeServer).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Yes, remove it" }));
    expect(api.removeServer).toHaveBeenCalledWith("srv-shop");
  });
});
