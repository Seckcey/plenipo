import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { a11yProblems } from "../../test/a11y";
import {
  SAMPLE_MANIFEST,
  connectedCard,
  googleCard,
  sampleCard,
  samplePage,
  slackCard,
} from "../../test/connectionFixtures";
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
    saveConnectionApp: vi.fn(),
    addConnection: vi.fn(),
    removeConnection: vi.fn(),
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
  it("lists every service: Microsoft 365, Slack, and Google, then the rest as coming later", async () => {
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
    for (const built of ["Slack", "Google"]) {
      expect(screen.getByRole("listitem", { name: built })).toHaveTextContent("Not connected");
    }
    for (const later of ["HubSpot", "Stripe", "WordPress and WooCommerce"]) {
      const item = screen.getByRole("listitem", { name: `${later}, coming in a later update` });
      expect(item).toHaveTextContent("Coming in a later update");
    }
    expect(screen.getByText(/other people's words/)).toBeInTheDocument();
    expect(screen.getByText(/kept in Windows Credential Manager/)).toBeInTheDocument();
    // Plain words; the only secret box is Google's app secret, on Google's card, hiding what you
    // type; never a place for a password.
    const secretBoxes = container.querySelectorAll("input[type=password]");
    expect(secretBoxes).toHaveLength(1);
    const google = screen.getByRole("listitem", { name: "Google" });
    expect(within(google).getByLabelText("Client secret")).toBe(secretBoxes[0]);
    // (The app description to paste into Slack is Slack's own format, copied as it is.)
    const words = container.cloneNode(true) as HTMLElement;
    words.querySelectorAll(".connection__manifest").forEach((m) => m.remove());
    expect(words).not.toHaveTextContent(/password:|token|OAuth|MCP|plugin/i);
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
    expect(allowed).toHaveTextContent(
      "Send mail as you (asks you first, unless everyone is on your list) Mail.Send",
    );
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
    expect(parts).toHaveTextContent(
      "Full access: Save drafts. Sending one asks you, unless everyone is on your Send without asking to list.",
    );
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
    // The agent is offered with its role; an archived one is not offered.
    expect(within(who).queryByRole("option", { name: /Old Scout/ })).toBeNull();
    expect(
      within(who).getByRole("option", { name: "Backend Developer (Senior Developer)" }),
    ).toBeInTheDocument();
  });

  it("names an archived agent on the list, and keeps a removed one removable", async () => {
    const lines = sampleCard(
      {},
      {
        connection: {
          ...sampleCard().connection,
          access: [
            { who: { kind: "agent", id: "pos-old" }, level: "readOnly" },
            { who: { kind: "agent", id: "pos-gone" }, level: "readOnly" },
          ],
        },
      },
    );
    api.getConnections.mockResolvedValue(samplePage(lines));
    api.setConnectionAccess.mockResolvedValue(samplePage());
    const m365 = await card();
    const who = within(m365).getByRole("region", { name: "Who may use it" });
    expect(who).toHaveTextContent("Old Scout (archived)");
    await userEvent
      .setup()
      .click(within(who).getByRole("button", { name: "Take A removed agent off the list" }));
    expect(api.setConnectionAccess).toHaveBeenCalledWith("microsoft365", [
      { who: { kind: "agent", id: "pos-old" }, level: "readOnly" },
    ]);
  });

  it("locks the organization's own app while a sign-in waits", async () => {
    api.getConnections.mockResolvedValue(samplePage(sampleCard({}, { signingIn: true })));
    const m365 = await card();
    await userEvent.setup().click(within(m365).getByText("Advanced"));
    expect(within(m365).queryByLabelText("Microsoft app ID")).toBeNull();
    expect(m365).toHaveTextContent("Finish or cancel the sign-in first");
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
    expect(send).toHaveTextContent("Posting in a Teams channel always asks you.");
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
      secretKept: false,
    });
    expect(within(m365).queryByLabelText(/secret/i)).toBeNull();
  });

  it("connects Slack with one button, names the workspace, and says 8 West's app reads slowly", async () => {
    const connected = slackCard(
      "slack",
      { channels: "fullAccess" },
      {
        connection: {
          ...slackCard().connection,
          state: "connected",
          account: {
            name: "Frankie Gonzalez",
            address: "frankie@8westit.com",
            organization: "8 West IT",
          },
        },
      },
    );
    const both = samplePage(sampleCard(), {}, { slack: [connected, slackCard("slack-2")] });
    api.getConnections.mockResolvedValue(both);
    api.connectConnection.mockResolvedValue(both);
    render(<ConnectionsSettings go={go} />);
    const eight = await screen.findByRole("listitem", { name: "Slack — 8 West IT" });
    expect(eight).toHaveTextContent("Connected as frankie@8westit.com (8 West IT).");
    expect(eight).toHaveTextContent("Slack lets Plenipo read one channel or thread a minute");
    // A connected workspace cannot be removed; the second, not connected, can.
    expect(within(eight).queryByRole("button", { name: "Remove this workspace" })).toBeNull();
    const second = screen.getByRole("listitem", { name: "Slack — workspace 2" });
    expect(within(second).queryByRole("button", { name: /personal/ })).toBeNull();
    const user = userEvent.setup();
    await user.click(within(second).getByRole("button", { name: "Connect" }));
    expect(api.connectConnection).toHaveBeenCalledWith("slack-2", "work");
    api.removeConnection.mockResolvedValue(both);
    await user.click(within(second).getByRole("button", { name: "Remove this workspace" }));
    expect(api.removeConnection).toHaveBeenCalledWith("slack-2");
    // Search only reads: Off or Read only.
    const search = within(eight).getByRole("group", { name: "Search: what workers may do" });
    expect(within(search).queryByRole("button", { name: "Full access" })).toBeNull();
    expect(within(search).getByRole("button", { name: "Read only" })).toBeInTheDocument();
    // Disconnect says Slack cancels the sign-in too.
    await user.click(within(eight).getByRole("button", { name: "Disconnect" }));
    expect(eight).toHaveTextContent(
      "removed from Windows Credential Manager and cancelled at Slack",
    );
  });

  it("adds another Slack workspace on its own card", async () => {
    api.addConnection.mockResolvedValue(
      samplePage(sampleCard(), {}, { slack: [slackCard(), slackCard("slack-2")] }),
    );
    render(<ConnectionsSettings go={go} />);
    const add = await screen.findByRole("listitem", { name: "Add another Slack workspace" });
    await userEvent
      .setup()
      .click(within(add).getByRole("button", { name: "Add another Slack workspace" }));
    expect(api.addConnection).toHaveBeenCalledWith("slack");
    expect(
      await screen.findByRole("listitem", { name: "Slack — workspace 2" }),
    ).toBeInTheDocument();
  });

  it("puts a Slack channel on the list by its ID, and says a post reaches everyone in it", async () => {
    api.setConnectionSendList.mockResolvedValue(samplePage());
    render(<ConnectionsSettings go={go} />);
    const slack = await screen.findByRole("listitem", { name: "Slack" });
    const send = within(slack).getByRole("region", { name: "Send without asking to" });
    expect(send).toHaveTextContent("its ID is at the bottom of About");
    expect(send).toHaveTextContent("guests from other organizations too");
    expect(send).not.toHaveTextContent("Teams");
    const user = userEvent.setup();
    await user.type(
      within(send).getByLabelText("An address, an @domain, or a channel's ID"),
      "C0100000001",
    );
    await user.click(within(send).getByRole("button", { name: "Add to the list" }));
    expect(api.setConnectionSendList).toHaveBeenCalledWith("slack", ["C0100000001"]);
  });

  it("uses a workspace's own Slack app from Plenipo's app description, by its client ID", async () => {
    api.saveConnectionApp.mockResolvedValue(samplePage());
    const user = userEvent.setup();
    render(<ConnectionsSettings go={go} />);
    const slack = await screen.findByRole("listitem", { name: "Slack" });
    await user.click(within(slack).getByText("Advanced"));
    expect(within(slack).getByLabelText("Plenipo's app description for Slack")).toHaveTextContent(
      "http://localhost:47211",
    );
    await user.click(within(slack).getByRole("button", { name: "Copy the app description" }));
    expect(await navigator.clipboard.readText()).toBe(SAMPLE_MANIFEST);
    await user.type(
      within(slack).getByLabelText("Your Slack app's client ID"),
      " 1234567890.9876543210 ",
    );
    await user.click(within(slack).getByRole("button", { name: "Use this app" }));
    expect(api.saveConnectionApp).toHaveBeenCalledWith("slack", {
      clientId: "1234567890.9876543210",
    });
    expect(within(slack).queryByLabelText(/secret/i)).toBeNull();
  });

  it("asks for your Google app first; its secret box hides what you type and is emptied", async () => {
    const saved = googleCard(
      {},
      {
        hasApp: true,
        connection: {
          ...googleCard().connection,
          ownApp: { appId: "123456789012-abc.apps.googleusercontent.com", secretKept: true },
        },
      },
    );
    api.saveConnectionApp.mockResolvedValue(samplePage(sampleCard(), {}, { google: saved }));
    render(<ConnectionsSettings go={go} />);
    const google = await screen.findByRole("listitem", { name: "Google" });
    expect(within(google).getByRole("button", { name: "Connect" })).toBeDisabled();
    const app = within(google).getByRole("region", { name: "Your Google app" });
    expect(app).toHaveTextContent("Setting up your Slack and Google apps");
    const user = userEvent.setup();
    await user.type(
      within(app).getByLabelText("Client ID"),
      "123456789012-abc.apps.googleusercontent.com",
    );
    const secret = within(app).getByLabelText("Client secret");
    expect(secret).toHaveAttribute("type", "password");
    expect(secret).toHaveAttribute("autocomplete", "off");
    await user.type(secret, "GOCSPX-typed-secret");
    await user.click(within(app).getByRole("button", { name: "Save" }));
    expect(api.saveConnectionApp).toHaveBeenCalledWith("google", {
      clientId: "123456789012-abc.apps.googleusercontent.com",
      secret: "GOCSPX-typed-secret",
    });
    // Saved: the client ID and where the secret is — never the secret.
    const after = await screen.findByRole("listitem", { name: "Google" });
    expect(after).toHaveTextContent(
      "Client ID: 123456789012-abc.apps.googleusercontent.com. Its secret is kept in Windows Credential Manager.",
    );
    expect(after).not.toHaveTextContent("GOCSPX-typed-secret");
    expect(within(after).getByRole("button", { name: "Connect" })).toBeEnabled();
    api.saveConnectionApp.mockResolvedValue(samplePage());
    await user.click(within(after).getByRole("button", { name: "Remove this app" }));
    expect(api.saveConnectionApp).toHaveBeenLastCalledWith("google", null);
  });

  it("looks again after connection events", () => {
    for (const e of [
      "connection.connected",
      "connection.sign_in_needed",
      "connection.removed",
      "guard.switches_changed",
    ]) {
      expect(affectsConnections(e)).toBe(true);
    }
    expect(affectsConnections("agent.message")).toBe(false);
  });
});
