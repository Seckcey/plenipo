import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../../api/commands";
import { a11yProblems } from "../../test/a11y";
import {
  SAMPLE_MANIFEST,
  connectedCard,
  googleCard,
  keyedCard,
  sampleAddOn,
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
    saveConnectionKey: vi.fn(),
    addAddOn: vi.fn(),
    changeAddOn: vi.fn(),
    removeAddOn: vi.fn(),
    checkAddOnTools: vi.fn(),
    setAddOnTools: vi.fn(),
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
  it("lists every service, each built, and add-on tools last", async () => {
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
    for (const built of ["Slack", "Google", "HubSpot", "Stripe", "WordPress and WooCommerce"]) {
      expect(screen.getByRole("listitem", { name: built })).toHaveTextContent("Not connected");
    }
    expect(screen.queryByText("Coming in a later update")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Add-on tools" })).toBeInTheDocument();
    expect(screen.getByText(/Money always asks you/)).toBeInTheDocument();
    expect(screen.getByText(/other people's words/)).toBeInTheDocument();
    expect(screen.getByText(/kept in Windows Credential Manager/)).toBeInTheDocument();
    // Plain words; every secret box hides what you type: Google's app secret, HubSpot's and
    // Stripe's keys, and the website's Application Password and WooCommerce key.
    const secretBoxes = container.querySelectorAll("input[type=password]");
    expect(secretBoxes).toHaveLength(6);
    const google = screen.getByRole("listitem", { name: "Google" });
    expect(within(google).getByLabelText("Client secret")).toBe(secretBoxes[0]);
    const hubspot = screen.getByRole("listitem", { name: "HubSpot" });
    expect(within(hubspot).getByLabelText("Service key")).toHaveAttribute("type", "password");
    const stripe = screen.getByRole("listitem", { name: "Stripe" });
    expect(within(stripe).getByLabelText("Restricted key")).toHaveAttribute("type", "password");
    // (The app description to paste into Slack is Slack's own format, copied as it is. "MCP" is
    // named once, in the add-on tools' help.)
    const words = container.cloneNode(true) as HTMLElement;
    words.querySelectorAll(".connection__manifest").forEach((m) => m.remove());
    expect(words.textContent?.match(/MCP/g)).toHaveLength(1);
    expect(words).not.toHaveTextContent(/password:|token|OAuth|MCP server|plugin/i);
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
      "Connected as alex@8westit.com (a work or school account, 8 West IT).",
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
            name: "Alex Rivera",
            address: "alex@8westit.com",
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
    expect(eight).toHaveTextContent("Connected as alex@8westit.com (8 West IT).");
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
    // Refused: the reason shows, and the secret box is emptied all the same.
    api.saveConnectionApp.mockRejectedValueOnce({
      kind: "invalidInput",
      message: "That is not a Google client ID.",
    });
    await user.type(within(app).getByLabelText("Client ID"), "not-an-id");
    await user.type(within(app).getByLabelText("Client secret"), "GOCSPX-first-try");
    await user.click(within(app).getByRole("button", { name: "Save" }));
    expect(await within(google).findByText("That is not a Google client ID.")).toBeInTheDocument();
    expect(within(app).getByLabelText("Client secret")).toHaveValue("");
    await user.clear(within(app).getByLabelText("Client ID"));
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
    // The form is back, and its secret box is empty.
    const again = await screen.findByRole("listitem", { name: "Google" });
    expect(await within(again).findByLabelText("Client secret")).toHaveValue("");
  });

  it("says it is cancelling the sign-in at Slack while Disconnect waits for Slack", async () => {
    const connected = slackCard(
      "slack",
      {},
      {
        connection: {
          ...slackCard().connection,
          state: "connected",
          account: { name: "Alex", address: "alex@8westit.com", organization: "8 West IT" },
        },
      },
    );
    api.getConnections.mockResolvedValue(samplePage(sampleCard(), {}, { slack: [connected] }));
    let done: (page: ReturnType<typeof samplePage>) => void = () => {};
    api.disconnectConnection.mockReturnValue(new Promise((resolve) => (done = resolve)));
    render(<ConnectionsSettings go={go} />);
    const slack = await screen.findByRole("listitem", { name: "Slack — 8 West IT" });
    const user = userEvent.setup();
    await user.click(within(slack).getByRole("button", { name: "Disconnect" }));
    expect(slack).toHaveTextContent("and cancelled at Slack");
    await user.click(within(slack).getByRole("button", { name: "Yes, disconnect" }));
    expect(within(slack).getByRole("status")).toHaveTextContent("Cancelling the sign-in at Slack…");
    done(samplePage());
    await waitFor(() =>
      expect(screen.queryByText("Cancelling the sign-in at Slack…")).not.toBeInTheDocument(),
    );
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

  it("saves a key once into a box that hides it, and the card says what the key needs", async () => {
    api.saveConnectionKey.mockResolvedValue(
      samplePage(
        sampleCard(),
        {},
        {
          hubspot: keyedCard(
            "hubspot",
            {},
            {
              connection: {
                id: "hubspot",
                service: "hubspot",
                parts: { contacts: "readOnly", companies: "readOnly", deals: "readOnly" },
                granted: ["crm.objects.contacts.read"],
                access: [],
                sendList: [],
                state: "connected",
                account: { name: "HubSpot account 24681357", address: "" },
              },
            },
          ),
        },
      ),
    );
    render(<ConnectionsSettings go={go} />);
    const hubspot = await screen.findByRole("listitem", { name: "HubSpot" });
    // No sign-in page, no Connect: a key.
    expect(within(hubspot).queryByRole("button", { name: /^Connect/ })).not.toBeInTheDocument();
    expect(hubspot).toHaveTextContent("crm.objects.contacts.read");
    expect(hubspot).toHaveTextContent("never shown again");
    const box = within(hubspot).getByLabelText("Service key");
    const save = within(hubspot).getByRole("button", { name: "Save and check" });
    expect(save).toBeDisabled();
    const user = userEvent.setup();
    await user.type(box, "plenipo-test-hubspot-typed-key");
    await user.click(save);
    expect(api.saveConnectionKey).toHaveBeenCalledWith("hubspot", { key: "plenipo-test-hubspot-typed-key" });
    expect(await within(hubspot).findByText(/Connected to/)).toHaveTextContent(
      "Connected to HubSpot account 24681357.",
    );
    // The box is empty again: the key is never shown back.
    expect(within(hubspot).getByLabelText("Service key")).toHaveValue("");
    expect(within(hubspot).getByRole("button", { name: "Replace the key" })).toBeInTheDocument();
    // HubSpot has nothing that sends: no Send without asking to list.
    expect(within(hubspot).queryByText("Send without asking to")).not.toBeInTheDocument();
  });

  it("will not save a key while no part is on, or while the Vault is missing", async () => {
    const user = userEvent.setup();
    api.getConnections.mockResolvedValue(
      samplePage(
        sampleCard(),
        {},
        { hubspot: keyedCard("hubspot", { contacts: "off", companies: "off", deals: "off" }) },
      ),
    );
    render(<ConnectionsSettings go={go} />);
    const hubspot = await screen.findByRole("listitem", { name: "HubSpot" });
    await user.type(within(hubspot).getByLabelText("Service key"), "plenipo-test-hubspot-typed-key");
    expect(within(hubspot).getByRole("button", { name: "Save and check" })).toBeDisabled();
    cleanup();
    api.getConnections.mockResolvedValue(samplePage(sampleCard(), { vaultAvailable: false }));
    render(<ConnectionsSettings go={go} />);
    const again = await screen.findByRole("listitem", { name: "HubSpot" });
    await user.type(within(again).getByLabelText("Service key"), "plenipo-test-hubspot-typed-key");
    expect(within(again).getByRole("button", { name: "Save and check" })).toBeDisabled();
  });

  it("warns that a live Stripe key moves real money, and a key refused needs a new one", async () => {
    const live = keyedCard(
      "stripe",
      {},
      {
        connection: {
          id: "stripe",
          service: "stripe",
          parts: { payments: "fullAccess", customers: "readOnly", invoices: "readOnly" },
          granted: ["live mode"],
          access: [],
          sendList: [],
          state: "connected",
          account: { name: "8 West IT", address: "", organization: "Live mode" },
        },
        granted: [{ name: "live mode", words: "Live mode: moves real money" }],
      },
    );
    const gone = keyedCard(
      "hubspot",
      {},
      {
        connection: {
          id: "hubspot",
          service: "hubspot",
          parts: { contacts: "readOnly" },
          granted: [],
          access: [],
          sendList: [],
          state: "needsSignIn",
        },
      },
    );
    api.getConnections.mockResolvedValue(
      samplePage(sampleCard(), {}, { stripe: live, hubspot: gone }),
    );
    const { container } = render(
      <main>
        <h1>Settings</h1>
        <h2>Connections</h2>
        <ConnectionsSettings go={go} />
      </main>,
    );
    const stripe = await screen.findByRole("listitem", { name: "Stripe" });
    expect(stripe).toHaveTextContent("Connected to 8 West IT (Live mode).");
    expect(within(stripe).getByRole("alert")).toHaveTextContent(
      "Live mode: this key moves real money.",
    );
    const hubspot = screen.getByRole("listitem", { name: "HubSpot" });
    expect(within(hubspot).getByText("Needs a new key")).toBeInTheDocument();
    expect(hubspot).toHaveTextContent("HubSpot needs a new key.");
    // Disconnect says where to delete the key in Stripe too.
    const user = userEvent.setup();
    await user.click(within(stripe).getByRole("button", { name: "Disconnect" }));
    expect(stripe).toHaveTextContent("delete it in Stripe too (Developers → API keys)");
    expect(a11yProblems(container)).toEqual([]);
  });

  it("saves the website's address, user, and Application Password, and a WooCommerce key", async () => {
    api.saveConnectionKey.mockResolvedValue(samplePage());
    render(<ConnectionsSettings go={go} />);
    const site = await screen.findByRole("listitem", { name: "WordPress and WooCommerce" });
    const user = userEvent.setup();
    await user.type(within(site).getByLabelText("Your site's address"), "https://shop.example.com");
    await user.type(within(site).getByLabelText("WordPress user name"), "plenipo");
    await user.type(
      within(site).getByLabelText("Application Password"),
      "abcd EFGH 1234 ijkl MNOP 5678",
    );
    await user.click(within(site).getByText("WooCommerce key (optional)"));
    await user.type(within(site).getByLabelText("Consumer key (ck_…)"), "ck_1");
    await user.type(within(site).getByLabelText("Consumer secret (cs_…)"), "cs_2");
    await user.click(within(site).getByRole("button", { name: "Save and check" }));
    expect(api.saveConnectionKey).toHaveBeenCalledWith("wordpress", {
      site: "https://shop.example.com",
      user: "plenipo",
      password: "abcd EFGH 1234 ijkl MNOP 5678",
      storeKey: "ck_1",
      storeSecret: "cs_2",
    });
    // The website's list is for customers; publishing and refunds always ask.
    expect(site).toHaveTextContent("publishing on your site and refunds always ask you");
  });

  it("adds a program off, marks its tools, switches it on, and picks who may use it", async () => {
    api.addAddOn.mockResolvedValue(samplePage(sampleCard(), { addOns: [sampleAddOn()] }));
    render(<ConnectionsSettings go={go} />);
    const form = await screen.findByRole("form", { name: "Add a program" });
    const user = userEvent.setup();
    await user.type(within(form).getByLabelText("Name"), "Tickets");
    await user.type(within(form).getByLabelText("Program"), "tickets-mcp");
    await user.type(within(form).getByLabelText("Arguments, one a line"), "--stdio{enter}--quiet");
    await user.click(within(form).getByLabelText("Notion key"));
    await user.click(within(form).getByRole("button", { name: "Add a program" }));
    expect(api.addAddOn).toHaveBeenCalledWith({
      name: "Tickets",
      program: "tickets-mcp",
      args: ["--stdio", "--quiet"],
      secrets: ["Notion key"],
    });
    const card = await screen.findByRole("listitem", { name: "Tickets" });
    expect(card.querySelector(".connection__header")).toHaveTextContent("Off");
    expect(card).toHaveTextContent("The program's words: Looks up an order by its number.");
    expect(card).toHaveTextContent("The program says it only reads.");
    // Each tool starts Off; the owner marks it.
    api.setAddOnTools.mockResolvedValue(samplePage(sampleCard(), { addOns: [sampleAddOn()] }));
    const mark = within(card).getByRole("group", { name: "create_ticket: what it may do" });
    expect(within(mark).getByRole("button", { name: "Off" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(mark).getByRole("button", { name: "Changing" }));
    expect(api.setAddOnTools).toHaveBeenCalledWith("tickets", { create_ticket: "changing" });
    api.changeAddOn.mockResolvedValue(
      samplePage(sampleCard(), { addOns: [sampleAddOn({ on: true })] }),
    );
    await user.click(within(card).getByRole("button", { name: "Switch on" }));
    expect(api.changeAddOn).toHaveBeenCalledWith("tickets", { on: true });
    // Who may use it: nobody to start; added lines start at Read only.
    expect(card).toHaveTextContent("Nobody yet");
    await user.selectOptions(
      within(card).getByLabelText("Add a role or an agent"),
      "role:role-sup",
    );
    await user.click(within(card).getByRole("button", { name: "Add" }));
    expect(api.changeAddOn).toHaveBeenLastCalledWith("tickets", {
      access: [{ who: { kind: "role", id: "role-sup" }, level: "readOnly" }],
    });
  });

  it("says when a program's tool changed and must be looked at again", async () => {
    const [lookup] = sampleAddOn().tools;
    if (!lookup) throw new Error("the sample add-on has tools");
    const changed = sampleAddOn({
      on: true,
      tools: [{ ...lookup, changed: true, mark: "off" }],
    });
    api.getConnections.mockResolvedValue(samplePage(sampleCard(), { addOns: [changed] }));
    const { container } = render(
      <main>
        <h1>Settings</h1>
        <h2>Connections</h2>
        <ConnectionsSettings go={go} />
      </main>,
    );
    const card = await screen.findByRole("listitem", { name: "Tickets" });
    expect(within(card).getByRole("alert")).toHaveTextContent("1 tool changed — look again.");
    expect(within(card).getByText("Changed — look again")).toBeInTheDocument();
    api.checkAddOnTools.mockResolvedValue(samplePage(sampleCard(), { addOns: [changed] }));
    await userEvent.setup().click(within(card).getByRole("button", { name: "Look at its tools" }));
    expect(api.checkAddOnTools).toHaveBeenCalledWith("tickets");
    expect(a11yProblems(container)).toEqual([]);
  });
});
