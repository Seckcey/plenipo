# ADR-064: How each connection is built — into Plenipo, or the service's own MCP server

- **Status:** Accepted (by the owner, 2026-09-28), with the owner's choices in the
  [Phase 20 checklist](../phases/phase-20-checklist.md#owner-decisions-2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 20
- **Carries out:** ROLLOUT_PLAN.md Phase 20, "Each is either built into Plenipo or the service's
  **official** MCP server run as a supervised, approved program. Either way, every call passes
  through Plenipo's tool server and Guard. No unofficial servers by default. Chosen per connection
  in this phase's ADR."
- **Builds on:** ADR-062 (the rules for every connection), ADR-063 (signing in, and the Vault),
  ADR-066 (add-on tools)

> **On screen** (ADR-010, plain words and rank names): only the services' names and their parts.
> How a connection is built never shows. This record keeps the code's words (REST, MCP, OAuth,
> scope, API key).

## In short

All six named services — Microsoft 365, Slack, Google, HubSpot, Stripe, and WordPress with
WooCommerce — are **built into Plenipo**: Plenipo's own code calls each service's own official,
documented web interface. None of them uses an MCP server program. The reasons, for every one:
the sign-in token never leaves Plenipo; each tool's kind (reading, writing, sending, deleting,
paying) is fixed in Plenipo's code, where Guard can rely on it; nothing extra (such as Node.js) is
needed on your PC; and a service cannot add or change a tool without a Plenipo update that was
reviewed and tested. A service's own official MCP program can still be used, as an **add-on tool**
you set up (ADR-066). Accepting this record means building each connection this way.

## Context

What "the service's official MCP server" is for each service, checked on 2026-09-28 (sources in the
Phase 20 checklist):

| Service                   | Its own MCP server                                                                                                                                                               | Run as a program on the PC?                       |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| Microsoft 365             | Work IQ / Agent 365 servers: preview, hosted by Microsoft, a Microsoft 365 Copilot license per user; a local `@microsoft/workiq` (npm, preview, Copilot license)                 | Only the preview npm package                      |
| Slack                     | Slack's MCP server at `mcp.slack.com`, generally available since February 2026, hosted by Slack, needs a registered Slack app and the workspace admin's approval                 | No: hosted by Slack                               |
| Google                    | Google-managed servers for Gmail, Calendar, and Drive (`gmailmcp.googleapis.com`, …): **developer preview**, hosted by Google; Gmail's has no send tool                          | No: hosted by Google                              |
| HubSpot                   | `mcp.hubspot.com`, generally available since April 2026, hosted by HubSpot; its sign-in needs a client secret                                                                    | No: hosted by HubSpot                             |
| Stripe                    | `mcp.stripe.com`, **public preview**, hosted by Stripe; the npm package `@stripe/mcp` only relays to it; from 2026-10-31 it takes only its own sign-in or keys tagged for agents | Only as a relay to Stripe's hosted server         |
| WordPress and WooCommerce | The WordPress MCP Adapter, a plugin **inside the website** (WordPress 6.9+); WooCommerce's MCP is a **developer preview**; `@automattic/mcp-wordpress-remote` (npm) relays to it | No: a plugin on the website (an npm relay exists) |

Running an MCP server program for a connection would mean:

- **Handing it the token.** The program would need the service's sign-in in its environment, and
  could write it to its own logs or send it anywhere. Plenipo could no longer promise "only in the
  Vault" (ADR-063 §6).
- **Trusting its tool list.** Its tools, their descriptions, and which ones change things are the
  program's words, and change with its updates. Guard would have to be told, tool by tool, which
  is reading and which is sending (ADR-066 §2).
- **Node.js, Python, or another runtime** on the owner's PC, and a program that runs as the owner
  with the owner's files and network (ADR-034).
- Most services' own servers are **hosted** by the service, not programs at all: they would be a
  third kind of connection, with their own sign-in and tool lists that change on the service's
  side without notice.

## Decision

### 1. Built in, for all six

Each service gets a module in `crates/capabilities/src/connections/`, calling the service's own
official, documented web interface through Guard's gate for Plenipo's own requests (ADR-062 §8),
with a fixed table of tools, each with its kind. The module is compiled in, reviewed, and tested
against a stand-in of the service (ADR-063 §8).

### 2. Microsoft 365 — built in, on Microsoft Graph

Sign-in: 8 West's multitenant app, PKCE, no secret (ADR-065). Microsoft's own MCP servers need a
Copilot license for every user and are in preview. Details, tools, and permissions: ADR-065.

### 3. Slack — built in, on Slack's Web API

- **Sign-in:** Slack's OAuth with **PKCE**, generally available since 2026-03-30: an app with PKCE
  turned on is a public client, needs no client secret, and may send the browser back to
  `http://localhost`. Such an app gets **user tokens only** (no bot), which is what Plenipo needs:
  workers act as the person who connected. Refresh tokens of PKCE apps expire after **30 days**,
  so a Slack connection unused for a month needs the owner to sign in again.
- **Any workspace, and more than one** (the owner's choice 9): each person can connect any Slack
  workspace they belong to, and add as many as they want; each is its own card, with its own
  sign-in, parts, **Who may use it**, and "send without asking" list.
- **Whose Slack app:**
  - **8 West's Slack app** (its client ID built into Plenipo, like Microsoft's; not a secret):
    **Connect** works for any workspace whose admin allows it. While 8 West's app is outside the
    Slack Marketplace, Slack limits it to **1 request per minute and 15 messages** when reading a
    channel's or thread's history (since 2025-05-29). Reading still works, slowly; the card says
    so.
  - **The workspace's own Slack app** (an **Advanced** field, like Microsoft's): Plenipo shows a
    ready-made app description to paste at `api.slack.com/apps` (**Create New App** → **From a
    manifest**), with PKCE on and only the user permissions below. Apps a workspace makes for
    itself keep Slack's normal speed (50+ requests a minute). Its client ID is not a secret.
  - **Slack's terms still need a lawyer** before Plenipo Pro is sold with Slack. Slack's API terms
    count "a free App that connects to a paid product" as commercial distribution, which needs the
    Marketplace or a partner agreement, "even when you provide customers with a custom Application
    or Application template", unless it was made "for use only by a single third party". Slack's
    Marketplace lists apps that "do not include functionality in Slack" or "replicate Slack client
    functionality" as unsuitable, and asks for 10 active workspaces. The owner chose to let every
    user add any workspace; the Phase 20 checklist keeps this risk in its list.
- **Parts and permissions** (user scopes), each asked only when its part is on: **Channels**
  (`channels:read`, `channels:history`, `groups:read`, `groups:history`), **Direct messages**
  (`im:read`, `im:history`, `mpim:read`, `mpim:history`), **Search** (`search:read`), **Posting**
  (`chat:write`), and always `users:read` (names instead of IDs).
- **Tools:** list channels, read a channel's recent messages or a thread, search messages (Read);
  post a message or a thread reply (Send, asks unless the channel or every person is on the "send
  without asking" list). No deleting or editing messages in this phase.
- **Slack's MCP server** (`mcp.slack.com`) is not used: it is hosted by Slack, not a program on the
  PC, and needs the same registered app and admin approval.

### 4. Google — built in, on the Gmail, Google Calendar, and Google Drive APIs

- **Sign-in:** Google's method for desktop apps: a **Desktop app** client, the system browser,
  PKCE, and a loopback address (`http://127.0.0.1:<port>`). Google says a desktop app's client
  secret "is obviously not treated as a secret"; Plenipo still keeps it in the Vault.
- **Whose Google app** (choice 10):
  - **Recommended: your own Google app, in your own Google Cloud project**, set to **Internal**
    when you use Google Workspace. Reading Gmail and Drive needs Google's **restricted**
    permissions. For an app offered to the public, those need Google's verification and, because
    mail and files go on to an AI service, very likely a yearly paid security assessment (CASA,
    by an outside lab: about $675 to $6,000 or more a year). An **Internal** app, used only inside
    its own organization, needs neither. The owner types the app's client ID and secret into the
    Google card (they go to the Vault).
  - Later: 8 West's own verified app, if clients want it and 8 West takes on the assessment.
  - Not recommended: "Testing" mode (sign-ins expire every 7 days; 100 people at most).
- **Parts and permissions** (each asked only when its part is on): **Gmail** (`gmail.readonly` to
  read, `gmail.compose` to draft and send drafts — both restricted), **Calendar**
  (`calendar.events` — sensitive), **Drive** (`drive.readonly` to read — restricted — and
  `drive.file` to add files — not sensitive).
- **Tools:** search and read mail, draft a reply or a new message, send a draft (Send); today's
  events, add an event (Write, or Send with guests); search, list, and read files, add a new file
  (Write). The same kinds and limits as Microsoft 365.
- **Google's own MCP servers** are a developer preview, hosted by Google, and Gmail's has no way to
  send.
- **Google's user data policy** asks apps to keep sign-ins encrypted at rest (the Vault is Windows
  Credential Manager) and to protect against planted instructions (ADR-062 §5–§6), and forbids using
  the data to train AI models (Plenipo trains none).

### 5. HubSpot — built in, on HubSpot's CRM API

- **Sign-in:** a **HubSpot service key** the owner creates in HubSpot (**Settings** →
  **Integrations** → **Service Keys**) with only the permissions below, and types into the HubSpot
  card (it goes to the Vault). HubSpot's apps with a browser sign-in need a client secret and a web
  server, which a desktop app cannot keep; its **legacy private apps**, which ADR-018 named, can no
  longer be created from 2026-09-28 (new HubSpot accounts) and 2026-10-26 (all accounts). Service
  keys are their successor (public beta since February 2026). (Choice 11.)
- **Permissions:** `crm.objects.contacts.read`, `crm.objects.companies.read`,
  `crm.objects.deals.read`, and, only if workers may change records, the matching `.write` ones.
- **Tools:** search and read contacts, companies, and deals (Read); create or update one, and add a
  note (Write). No emails or sequences from HubSpot in this phase; the Sales department (Phase 9)
  decides those, and ADR-018 §5 keeps every outbound message waiting for the owner.
- **HubSpot's MCP server** (`mcp.hubspot.com`) is hosted by HubSpot and its sign-in needs a client
  secret.

### 6. Stripe — built in, on Stripe's API, with a restricted key

- **Sign-in:** a **restricted key** (`rk_…`) the owner creates in Stripe's Dashboard
  (**Developers** → **API keys** → **Create restricted key**), **tagged for an agent** where
  Stripe offers it, with only the permissions below, typed into the Stripe card (it goes to the
  Vault). Stripe applies its own approval rules to agent-tagged keys (payouts, refunds, account
  changes) on top of Plenipo's. A test-mode key first is recommended. Stripe's app sign-ins need a
  secret kept on a server. (Choice 11.)
- **Permissions:** read on balance, charges and payment intents, customers, invoices, products and
  prices, subscriptions, and payouts; write on invoices and refunds only if the owner wants those
  tools.
- **Tools:** read the balance, payments, customers, invoices, subscriptions, and payouts (Read);
  draft an invoice (Write); **finalize and send an invoice, and refund a payment (Pay: always
  ask)**. No charges, payouts, or payment-link creation in this phase.
- **Stripe's MCP server** is a public preview, hosted by Stripe; its npm package only relays to it.

### 7. WordPress and WooCommerce — built in, on their REST APIs

- **Sign-in:** the site's address (`https` only) and a WordPress **Application Password** for a
  **user made for Plenipo, with the smallest role that does the job** (for example **Editor** for
  posts, **Shop Manager** for the store), typed into the card (it goes to the Vault). An
  Application Password acts with that user's full permissions, so the user's role is the limit.
  For the store, a **WooCommerce REST key** with **Read** permission (or **Read/Write** if workers
  may change orders or products) is an option. (Choice 11.)
- **Tools:** list and read posts, pages, orders, products, and customers (Read); draft a post or
  page, update a draft, add a private order note (Write); **publish, or change something already
  published** (Send: asks); change an order's status or add a note the customer sees (Send: asks);
  **refunds (Pay: always ask)**.
- **The WordPress MCP Adapter** runs inside the website as a plugin, and WooCommerce's is a
  developer preview; built in needs nothing installed on the site.

### 8. Everything else — add-on tools

Notion, Asana, Canva, Adobe, QuickBooks, and others "as the owner asks": until one is built in, the
owner can add that service's own official MCP program as an add-on tool (ADR-066), off by default,
each tool reviewed. Built-in connections are added one at a time, each with its own record.

### 9. No unofficial servers

Plenipo never ships, suggests, or starts a third party's MCP server for a service. An add-on is a
program the owner chose and installed (ADR-066), and Settings warns that it should come from the
service's own publisher.

## Consequences

- Each built-in connection is code Plenipo maintains. When a service changes its web interface,
  Plenipo needs an update; the stand-in tests hold the shape Plenipo expects.
- Tools are fewer than a service's own MCP server may offer. Each tool is one Plenipo can explain,
  classify, and test. More can be added on the same rules when the owner asks.
- The owner needs no Node.js or Python for any built-in connection.

## Alternatives considered

- **The service's own MCP server, for each service where one exists.** Not recommended, for the
  reasons in the Context: the token leaves the Vault, Guard must trust the program's tool list,
  most are hosted rather than programs, and some need extra licenses (Microsoft). Offered as a
  choice (the checklist, choice 2).
- **A hosted MCP server reached by Plenipo over the internet.** Deferred (ADR-066 §6).
- **Unofficial community servers.** Rejected by the plan: "No unofficial servers by default."

## As built (v1.13.0)

Part 20A built Microsoft 365 as [ADR-065](ADR-065-microsoft-365-connection.md) says. Slack and
Google come in part 20B; HubSpot, Stripe, and WordPress and WooCommerce in part 20C.

## As built (v1.13.1, part 20B: Slack and Google)

Part 20B built Slack and Google into Plenipo as §3 and §4 say, with the owner's answers and the
fits recorded in [ADR-070 (Slack and Google: the owner's choices)](ADR-070-slack-and-google-choices.md):

- **Slack's parts** are **Channels**, **Direct messages**, and **Search** (Search is Off or Read
  only). "Posting" is Full access on Channels and Direct messages (`chat:write`), and
  `users:read.email` is asked only while a part is at Full access. Both **8 West's Slack app**
  (`PLENIPO_SLACK_CLIENT_ID`, a public client ID built into releases) and **the workspace's own**
  (Advanced, with Plenipo's app description to paste into Slack) work.
- **7 Slack tools:** `slack_channels`, `slack_channel_messages` (a channel or one thread),
  `slack_post` (a post or thread reply, Send), `slack_direct_messages`, `slack_dm_messages`,
  `slack_send_dm` (Send), and `slack_search`. A channel tool refuses a direct message's ID, and the
  other way round, so each part's level holds.
- **10 Google tools:** `google_mail_search`, `google_mail_read`, `google_mail_draft` (new, reply,
  or reply to all; Write), `google_mail_send` (Send), `google_calendar_events`,
  `google_calendar_add_event` (Write, or Send with guests), `google_drive_search`,
  `google_drive_list`, `google_drive_read` (text files, Google Docs as text, Word documents), and
  `google_drive_upload` (a new text file; Write). Nothing is forwarded or deleted.
- **Google's permissions:** Gmail `gmail.readonly` (+ `gmail.compose`), Calendar
  `calendar.events.readonly` (Full access: `calendar.events`), Drive `drive.readonly`
  (+ `drive.file`), and always `openid`, `email`, `profile`.
- **Guard's gate:** Slack only `slack.com`; Google only `accounts.google.com`,
  `oauth2.googleapis.com`, `gmail.googleapis.com`, and `www.googleapis.com`; https only. The
  short-lived sign-in is sent to Slack's API and to Gmail, Calendar, and Drive only.
