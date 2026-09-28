import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { a11yProblems } from "../../test/a11y";
import { connectedCard, sampleCard, samplePage } from "../../test/connectionFixtures";
import { ConnectionsSettings } from "./ConnectionsSettings";
import { affectsConnections } from "./useConnections";

vi.mock("../../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getConnections: vi.fn(),
    connectConnection: vi.fn(),
    cancelConnectionSignIn: vi.fn(),
    disconnectConnection: vi.fn(),
    setConnectionParts: vi.fn(),
    setConnectionAccess: vi.fn(),
    setConnectionSendList: vi.fn(),
    setConnectionOwnApp: vi.fn(),
  };
});
vi.mock("../../api/events", () => ({
  subscribeLedgerEvents: vi.fn(() => Promise.resolve(() => undefined)),
}));

const api = vi.mocked(commands);
const go = vi.fn();

beforeEach(() => {
  api.getConnections.mockResolvedValue(samplePage());
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

async function card() {
  render(<ConnectionsSettings go={go} />);
  return screen.findByRole("listitem", { name: "Microsoft 365" });
}

describe("Settings → Connections", () => {
  it("lists every service, Microsoft 365 first, and the rest as coming later", async () => {
    // As Settings shows it: under the page's heading and the section's.
    const { container } = render(
      <main>
        <h1>Settings</h1>
        <h2>Connections</h2>
        <ConnectionsSettings go={go} />
      </main>,
    );
    const m365 = await screen.findByRole("listitem", { name: "Microsoft 365" });
    expect(within(m365).getByText("Not connected")).toBeInTheDocument();
    for (const later of ["Slack", "Google", "HubSpot", "Stripe", "WordPress and WooCommerce"]) {
      const item = screen.getByRole("listitem", { name: `${later}, coming in a later update` });
      expect(item).toHaveTextContent("Coming in a later update");
    }
    expect(screen.getByText(/other people's words/)).toBeInTheDocument();
    expect(screen.getByText(/kept in Windows Credential Manager/)).toBeInTheDocument();
    // Plain words, and never a place to type a password or a key.
    expect(container.querySelector("input[type=password]")).toBeNull();
    expect(container).not.toHaveTextContent(/password:|client secret|token|OAuth|MCP|plugin/i);
    expect(a11yProblems(container)).toEqual([]);
  });

  it("connects in your browser, says so while it waits, and can cancel", async () => {
    const waiting = sampleCard({}, { signingIn: true });
    api.connectConnection.mockResolvedValue(samplePage(waiting));
    api.cancelConnectionSignIn.mockResolvedValue(samplePage());
    const m365 = await card();
    const user = userEvent.setup();
    await user.click(
      within(m365).getByRole("button", { name: "Connect a work or school account" }),
    );
    expect(api.connectConnection).toHaveBeenCalledWith("microsoft365", "work");
    expect(await within(m365).findByText("Finish signing in in your browser.")).toBeInTheDocument();
    expect(within(m365).getByText("Waiting for you in your browser")).toBeInTheDocument();
    expect(
      within(m365).getByRole("button", { name: "Connect a work or school account" }),
    ).toBeDisabled();
    await user.click(within(m365).getByRole("button", { name: "Cancel" }));
    expect(api.cancelConnectionSignIn).toHaveBeenCalledWith("microsoft365");
    // A personal account signs in the same way.
    await user.click(
      await within(m365).findByRole("button", { name: "Connect a personal account" }),
    );
    expect(api.connectConnection).toHaveBeenLastCalledWith("microsoft365", "personal");
  });

  it("shows who it is connected as, what Plenipo was allowed, and disconnects after asking", async () => {
    api.getConnections.mockResolvedValue(samplePage(connectedCard()));
    api.disconnectConnection.mockResolvedValue(samplePage());
    const m365 = await card();
    expect(m365).toHaveTextContent(
      "Connected as frankie@8westit.com (a work or school account, 8 West IT).",
    );
    expect(within(m365).getByText("Connected")).toBeInTheDocument();
    const allowed = within(m365).getByRole("region", { name: "What Plenipo was allowed" });
    expect(allowed).toHaveTextContent("Send mail as you (each send asks you first) Mail.Send");
    expect(within(m365).getByRole("button", { name: "Reconnect" })).toBeEnabled();
    const user = userEvent.setup();
    await user.click(within(m365).getByRole("button", { name: "Disconnect" }));
    expect(api.disconnectConnection).not.toHaveBeenCalled();
    expect(m365).toHaveTextContent("its sign-in is removed from Windows Credential Manager");
    await user.click(within(m365).getByRole("button", { name: "Yes, disconnect" }));
    expect(api.disconnectConnection).toHaveBeenCalledWith("microsoft365");
  });

  it("sets each part Off, Read only, or Full access, and says what workers can do", async () => {
    api.setConnectionParts.mockResolvedValue(samplePage(sampleCard({ mail: "fullAccess" })));
    const m365 = await card();
    const parts = within(m365).getByRole("region", { name: "What it can do" });
    const mail = within(parts).getByRole("group", { name: "Mail: what workers may do" });
    expect(within(mail).getByRole("button", { name: "Read only" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(parts).toHaveTextContent("Full access: Save drafts; sending a draft asks you.");
    expect(parts).toHaveTextContent("Your organization's admin approves it once.");
    await userEvent.setup().click(within(mail).getByRole("button", { name: "Full access" }));
    expect(api.setConnectionParts).toHaveBeenCalledWith("microsoft365", { mail: "fullAccess" });
  });

  it("shows only Mail, Calendar, and OneDrive for a personal account", async () => {
    const personal = sampleCard({}, {}, true);
    api.getConnections.mockResolvedValue(samplePage(personal));
    const m365 = await card();
    const parts = within(m365).getByRole("region", { name: "What it can do" });
    expect(within(parts).getAllByText("Not in personal accounts")).toHaveLength(2);
    expect(within(parts).queryByRole("group", { name: "Teams: what workers may do" })).toBeNull();
    expect(
      within(parts).getByRole("group", { name: "OneDrive: what workers may do" }),
    ).toBeInTheDocument();
  });

  it("asks to reconnect for parts turned up after connecting", async () => {
    api.getConnections.mockResolvedValue(samplePage(connectedCard({ reconnectFor: ["Teams"] })));
    const m365 = await card();
    expect(m365).toHaveTextContent("Reconnect to allow Teams.");
  });

  it("adds roles and agents to Who may use it at Read only, changes, and removes them", async () => {
    const withLine = sampleCard(
      {},
      {
        connection: {
          ...sampleCard().connection,
          access: [{ who: { kind: "role", id: "role-sup" }, level: "readOnly" }],
        },
      },
    );
    api.setConnectionAccess.mockResolvedValue(samplePage(withLine));
    const m365 = await card();
    const who = within(m365).getByRole("region", { name: "Who may use it" });
    expect(who).toHaveTextContent("Nobody yet, so no worker can use it.");
    const user = userEvent.setup();
    await user.selectOptions(within(who).getByLabelText("Add a role or an agent"), "role:role-sup");
    await user.click(within(who).getByRole("button", { name: "Add" }));
    expect(api.setConnectionAccess).toHaveBeenCalledWith("microsoft365", [
      { who: { kind: "role", id: "role-sup" }, level: "readOnly" },
    ]);
    expect(
      await within(who).findByText("Supervisor (every agent in this role)"),
    ).toBeInTheDocument();
    await user.click(within(who).getByRole("button", { name: "Read and write" }));
    expect(api.setConnectionAccess).toHaveBeenLastCalledWith("microsoft365", [
      { who: { kind: "role", id: "role-sup" }, level: "readWrite" },
    ]);
    await user.click(
      within(who).getByRole("button", {
        name: "Take Supervisor (every agent in this role) off the list",
      }),
    );
    expect(api.setConnectionAccess).toHaveBeenLastCalledWith("microsoft365", []);
    // The agent is offered with its role.
    expect(
      within(who).getByRole("option", { name: "Backend Developer (Senior Developer)" }),
    ).toBeInTheDocument();
  });

  it("warns above the send list, says whether the switch is on, and edits the list", async () => {
    const listed = sampleCard(
      {},
      { connection: { ...sampleCard().connection, sendList: ["@clientco.com"] } },
    );
    api.setConnectionSendList.mockResolvedValue(samplePage(listed));
    const m365 = await card();
    const send = within(m365).getByRole("region", { name: "Send without asking to" });
    expect(send).toHaveTextContent(
      "An email could trick a worker into writing to anyone on this list without asking you.",
    );
    expect(send).toHaveTextContent("Not used now: the switch Sending forms and messages");
    expect(send).toHaveTextContent("Nobody on the list: every send asks you.");
    const user = userEvent.setup();
    await user.type(within(send).getByRole("textbox"), "  @clientco.com ");
    await user.click(within(send).getByRole("button", { name: "Add to the list" }));
    expect(api.setConnectionSendList).toHaveBeenCalledWith("microsoft365", ["@clientco.com"]);
    await user.click(
      await within(send).findByRole("button", { name: "Take @clientco.com off the list" }),
    );
    expect(api.setConnectionSendList).toHaveBeenLastCalledWith("microsoft365", []);
    await user.click(within(send).getByRole("button", { name: "Open Settings → Switches" }));
    expect(go).toHaveBeenCalledWith({ view: "settings", id: "switches" });
  });

  it("gives the link for your admin when Microsoft says an admin must approve first", async () => {
    const link =
      "https://login.microsoftonline.com/organizations/adminconsent?client_id=0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0";
    api.getConnections.mockResolvedValue(
      samplePage(
        sampleCard(
          {},
          {
            adminLink: link,
            problem: "Your organization's admin needs to approve Plenipo first.",
          },
        ),
      ),
    );
    const m365 = await card();
    const alert = within(m365).getByRole("alert");
    expect(alert).toHaveTextContent("Your organization's admin needs to approve Plenipo first.");
    expect(alert).toHaveTextContent(link);
    expect(
      within(alert).getByRole("button", { name: "Copy the approval link for your admin" }),
    ).toBeInTheDocument();
  });

  it("says when Microsoft needs you to sign in again", async () => {
    const again = connectedCard();
    again.connection = { ...again.connection, state: "needsSignIn" };
    api.getConnections.mockResolvedValue(samplePage(again));
    const m365 = await card();
    expect(within(m365).getAllByText(/needs you to sign in again/i).length).toBeGreaterThan(0);
    expect(within(m365).getByRole("button", { name: "Sign in again" })).toBeEnabled();
    expect(within(m365).getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
  });

  it("explains a copy with no app ID, and a Vault that cannot be used", async () => {
    api.getConnections.mockResolvedValue(
      samplePage(sampleCard({}, { hasApp: false }), { vaultAvailable: false }),
    );
    const m365 = await card();
    expect(m365).toHaveTextContent("This copy of Plenipo has no Microsoft app ID yet");
    expect(
      within(m365).getByRole("button", { name: "Connect a work or school account" }),
    ).toBeDisabled();
    expect(screen.getByText(/Windows Credential Manager is not available/)).toBeInTheDocument();
  });

  it("takes the organization's own app ID under Advanced, never a secret", async () => {
    api.setConnectionOwnApp.mockResolvedValue(samplePage());
    const m365 = await card();
    const user = userEvent.setup();
    await user.click(within(m365).getByText("Advanced"));
    await user.type(
      within(m365).getByLabelText("Microsoft app ID"),
      " 0F1E2D3C-4B5A-6978-8796-A5B4C3D2E1F0 ",
    );
    await user.type(
      within(m365).getByLabelText("Your organization's domain or ID"),
      "clientco.com",
    );
    await user.click(within(m365).getByRole("button", { name: "Use this app" }));
    expect(api.setConnectionOwnApp).toHaveBeenCalledWith("microsoft365", {
      appId: "0F1E2D3C-4B5A-6978-8796-A5B4C3D2E1F0",
      tenant: "clientco.com",
    });
    expect(within(m365).queryByLabelText(/secret/i)).toBeNull();
  });

  it("looks again after connection events", () => {
    for (const e of [
      "connection.connected",
      "connection.sign_in_needed",
      "guard.switches_changed",
    ]) {
      expect(affectsConnections(e)).toBe(true);
    }
    expect(affectsConnections("agent.message")).toBe(false);
  });
});
