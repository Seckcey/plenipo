# ADR-070: Slack and Google — the owner's choices, and what their sign-ins need

- **Status:** Accepted (by the owner, 2026-09-29: "Allow a Slack channel by its ID", "Add the
  permission", "Both. This app needs to be able to be used by other people and orgs. Not just me
  and 8 West.", and "Go with whatever you recommend")
- **Date:** 2026-09-29
- **Phase:** 20, part 20B (ADR-067)
- **Amends:** [ADR-062 (one set of rules for every connection)](ADR-062-connections-one-set-of-rules.md)
  as built — a Slack channel can be on **Send without asking to** by its ID;
  [ADR-063 (signing in in your own browser)](ADR-063-signing-in-to-a-connection.md) §2 — Slack's
  sign-in comes back to one of three fixed ports;
  [ADR-064 (how each connection is built)](ADR-064-how-each-connection-is-built.md) §3–§4 — Slack's
  parts, its permission to see email addresses, both kinds of Slack app, Slack's sign-in
  addresses, Google's own app, and Google Calendar at Read only

> **On screen** (ADR-010, plain words and rank names): **Add another Slack workspace**, **Remove
> this workspace**, **Your Google app**, **Client ID**, **Client secret** (a box that hides what
> you type), **Use your workspace's own Slack app**, and a channel ID shown as `C0123ABCD`. This
> record keeps the code's words (OAuth, PKCE, scope, client ID, manifest, redirect).

## In short

Before building Slack and Google, the approved design was checked against what part 20A built and
against Slack's and Google's own pages. The owner answered four questions:

1. **A Slack channel can be on "Send without asking to" by its channel ID** (like `C0123ABCD`).
   Slack channel IDs never change and are never reused, unlike Teams channel names.
2. **Plenipo asks Slack for permission to see people's email addresses**, so a Slack message to
   people on the list can go ahead without asking, like an email.
3. **Both kinds of Slack app:** 8 West's app (one click for anyone, any workspace, any
   organization) and each workspace's own app under **Advanced**.
4. **Google: your own Google app**, as recommended: its client ID and secret are typed into the
   Google card; the secret goes only to the Vault.

The check also found small things the design had to fit (section 5). Accepting this record means
building part 20B with these choices.

## Context

The approved design (Phase 20 checklist, design §6; ADR-064 §3–§4) and what 20A built
(Phase 20 acceptance report) were compared on 2026-09-29. Four places needed the owner:

- Part 20A made channels **never** listable, because a Teams channel is known only by names anyone
  can reuse. Slack channels have fixed IDs.
- Matching a Slack person against an email address needs Slack's `users:read.email` permission,
  which the design did not ask for.
- ADR-064 §3 described both 8 West's Slack app and each workspace's own; the checklist's choice 9
  had recommended the workspace's own only.
- Google needs a client ID and a secret for a desktop app; the design proposed a new command,
  `save_connection_app`, to take them.

Slack's and Google's own pages, read on 2026-09-29:

- **Slack, "Using PKCE":** the sign-in is `https://slack.com/oauth/v2/authorize` with the PKCE
  fields, and the code is traded at `oauth.v2.access` "without `client_secret`"; renewals use
  `oauth.v2.access` with `grant_type=refresh_token` and the client ID only. "Redirects to localhost
  … are treated as desktop redirects if the app has opted into PKCE", and "desktop redirects are not
  allowed to request bot scopes". With PKCE on, "all refresh tokens issued to your app will expire
  in 30 days". Rotating tokens come for a `localhost` redirect only when the app's **token
  rotation** setting is on.
- **Slack, redirect addresses:** the address a sign-in comes back to must match one written into
  the app, **port included** (other desktop apps using Slack found the same, and fixed their port).
- **Google, "OAuth 2.0 for iOS & Desktop Apps":** a **Desktop app** client, the system browser,
  PKCE, and a loopback address `http://127.0.0.1:<any port>`; the token request includes the
  client secret, which "is obviously not treated as a secret".

## Decision

### 1. A Slack channel on the list, by its ID (the owner's answer 1)

- **Send without asking to** on a Slack card takes a channel ID: `C` or `G` and 8 to 12 capital
  letters and digits (Slack shows it in the channel's details: click the channel's name, then look
  at the bottom of **About**). It is kept in capitals. `#general` is refused, with where to find the
  ID: a name can change and be reused, an ID cannot.
- A post, or a thread reply, in a channel goes ahead without asking only when the switch "Sending
  forms and messages (without asking)" is on and **that channel's ID** is on that workspace's list.
  Everyone in the channel sees it, guests from other organizations in a shared channel too; the
  warning above the list says so.
- A direct message or a group message is decided by **its people**, never by an ID (§2).
- Microsoft 365 and Google keep what 20A built: only addresses and `@domains`; posting in a Teams
  channel always asks.

### 2. People's email addresses in Slack (the owner's answer 2)

- Plenipo asks for `users:read.email` **only while Channels or Direct messages is at Full access**
  (the only time the list matters), so Read only connections ask for nothing more.
- A direct or group message goes ahead without asking only when the switch is on and every other
  person in it has an email address in Slack that is on the list (an address or its `@domain`). A
  person Slack gives no address for (some guests, apps) is shown by name, "(no email address in
  Slack)", and always asks, as in Teams.
- The people are read again just before sending; a change stops the message (as Teams).

### 3. Both kinds of Slack app (the owner's answer 3)

- **8 West's Slack app:** its client ID (public, not a secret) is built into Plenipo from the
  repository variable `PLENIPO_SLACK_CLIENT_ID`, like Microsoft's. **Connect** works for any
  workspace whose admin allows it, for 8 West and for anyone else who uses Plenipo. 8 West turns on
  **public distribution** so other workspaces can install it.
  - **Slower outside the Slack Marketplace:** Slack lets such an app read a channel's or a thread's
    history once a minute, 15 messages at a time. The Slack card says so, and a worker that is
    slowed down is told to try again in a minute.
- **The workspace's own Slack app** (**Advanced** → **Use your workspace's own Slack app**): its
  client ID (not a secret). The card shows a ready-made app description (a Slack "manifest") to
  paste at `api.slack.com/apps` → **Create New App** → **From a manifest**: user permissions only,
  PKCE on, token rotation on, and the three sign-in addresses (§5.2). Apps a workspace makes for
  itself keep Slack's normal speed.
- **Legal (unchanged, and now more pressing):** Slack's API terms count a free app that connects
  to a paid product as commercial distribution, which needs the Slack Marketplace or a partner
  agreement. **A lawyer reads Slack's terms before Plenipo Pro is sold with Slack.** This record
  builds the connection; it does not settle that question.

### 4. Your own Google app (the owner's answer 4: as recommended)

- **Where:** the Google card gets **Your Google app**: a **Client ID** box, a **Client secret** box
  that hides what you type, and **Save**. Until it is saved, **Connect** is off and the card names
  the setup steps (Plenipo opens no web pages of its own; the release notes link to them). After saving, the card shows the client ID and "Its secret is kept in Windows
  Credential Manager"; the secret is never shown again and never leaves the Vault except to Google's
  token address. **Remove this app** erases both.
- **The command:** `save_connection_app(connectionId, app)`, the main window's alone. `app` is
  `{ clientId, secret }` (Google), `{ clientId }` (a Slack workspace's own app), or `null` (remove).
  The client ID is kept in the connection's settings; the secret goes straight to the Vault
  (`connection-google-app-secret`), is read back to check it, and is never returned. Changing or
  removing an app is refused while the connection is connected or a sign-in waits ("Disconnect
  first"): a sign-in belongs to the app it was made with.
- The secret is among the values hidden in every text Plenipo records, and uninstalling with
  "delete my data" removes it.
- **Which Google account:** with Google Workspace, set the app to **Internal** (no Google review).
  With a personal Gmail account, **External** and **In production**: Google shows "Google hasn't
  verified this app" until you choose **Continue**; Google does not ask for a review of an app used
  only by its own maker and a few people they know. Not **Testing**: its sign-ins end every 7 days.
  The setup steps cover both.
- **If the app is ever offered to the public** (one 8 West app for everyone), Gmail and Drive
  reading are "restricted" permissions and need Google's review and, very likely, a yearly paid
  security assessment. Not part of 20B.

### 5. What the design had to fit

1. **Slack's sign-in addresses:** `https://slack.com/oauth/v2/authorize` with `user_scope` (no bot
   permissions) and PKCE; the code is traded, and the sign-in renewed, at
   `https://slack.com/api/oauth.v2.access` with no secret. (The design had named
   `oauth/v2_user/authorize` and `oauth.v2.user.access`.)
2. **Slack comes back to a fixed port** (amends ADR-063 §2): Slack needs the exact address, port
   included, so Slack's sign-in listens on `http://localhost:47211`, or `47212`, or `47213` — the
   first one free on this computer — and all three are written into the app. Microsoft and Google
   keep a port Windows picks. The rest is unchanged: a one-time start page, one answer with the
   right `state`, and a stolen code is useless without the PKCE secret.
3. **Slack's parts** as **Off**, **Read only**, or **Full access** (the owner's rule for every part;
   ADR-064 §3 had listed "Posting" as a part of its own):
   - **Channels:** Read only lists and reads the channels you are in, and their threads
     (`channels:read`, `channels:history`, `groups:read`, `groups:history`); Full access adds
     posting and thread replies (`chat:write`).
   - **Direct messages:** Read only reads your direct and group messages (`im:read`, `im:history`,
     `mpim:read`, `mpim:history`); Full access adds sending in them (`chat:write`).
   - **Search:** Off or Read only (`search:read`). Results come only from the parts that are on: a
     search never shows a direct message while Direct messages is off.
   - Always `users:read` (names instead of IDs); `users:read.email` as §2 says.
4. **Slack's sign-in lasts while it is used.** Plenipo's app description turns on token rotation;
   with PKCE, a Slack sign-in not used for 30 days needs the owner to sign in again (the card says
   so). A workspace app without rotation gives a sign-in that does not expire; the Vault keeps it
   the same way.
5. **Google Calendar at Read only** asks `calendar.events.readonly`, fewer than `calendar.events`
   (which Full access asks). Gmail: `gmail.readonly`, and `gmail.compose` at Full access. Drive:
   `drive.readonly`, and `drive.file` at Full access. Always: `openid`, `email`, `profile`.
6. **Google's tools** follow ADR-064 §4 with Microsoft 365's kinds and limits: search and read mail,
   draft a new message or a reply, send a draft (Send); events between two times, add an event
   (Write, or Send with guests); search, list, and read files (text files, Google Docs, and Word
   documents), and add a new text file (Write). Nothing is forwarded or deleted. With `drive.file`,
   a new file goes to the top of My Drive, or into a folder Google lets Plenipo use.
7. **More than one Slack workspace** (ADR-064 §3): **Add another Slack workspace** makes a new card
   (`slack-2`, `slack-3`, …) and **Remove this workspace** removes a card that is not connected — two
   more commands, `add_connection` and `remove_connection`, the main window's alone. One workspace
   per card: connecting a workspace that is already on another card is refused. Slack's tools take a
   `workspace` (the card's ID) when a worker may use more than one; Plenipo's note to the worker names
   each.
8. **Disconnect cancels the sign-in at the service** (ADR-063 §5): Plenipo stops the tools, removes
   the sign-in from the Vault, then cancels it at Slack (`auth.revoke`) or Google (its revoke
   address). If the service cannot be reached, the card says the sign-in is gone from this computer
   and how to remove Plenipo in Slack or Google.

## Consequences

- An owner can let workers post in chosen Slack channels without asking. A planted instruction
  could make a worker post in a listed channel; the warning above the list says so, as for
  addresses.
- 8 West must create and distribute its Slack app, and set `PLENIPO_SLACK_CLIENT_ID`, before
  **Connect** works with 8 West's app; until then, a workspace's own app works.
- The Google card is the first place in Settings → Connections where a secret is typed. It goes to
  the Vault only, and the box hides it.
- Slack's fixed ports can be taken by another program; then the card says so, and the owner closes
  that program or tries again.

## Alternatives considered

- **Slack channels always ask** (as Teams). Not chosen by the owner.
- **Every Slack message to a person asks** (no `users:read.email`). Not chosen by the owner.
- **Only 8 West's Slack app, or only each workspace's own.** Not chosen by the owner: both.
- **Both Google values in the Vault** (the owner's first words). Chosen instead: the client ID in
  the settings, so the card can show which app is used without reading the Vault; only the secret
  in the Vault. A client ID is not a secret.
- **One fixed port for Slack.** Three, so another program holding one does not stop Slack.
- **Slack's "Posting" as a part of its own.** Replaced by Full access on Channels and Direct
  messages, the owner's rule for every part.

## As built (v1.13.1)

Built as decided. Where each choice lives:

- **§1, a Slack channel by its ID:** `guard::connections::send_entry` takes a Slack channel ID
  (`C…` or `G…`, uppercased) on a Slack card's list, and refuses `#general` with "its ID is at the
  bottom of About". `listed_for` matches a post's channel ID; a person in Slack is matched by the
  email address Slack gives (§2). A group message goes ahead without asking only when every person
  in it has a listed address; a person Slack gives no address for (a guest, or a Slack app) is
  never listed.
- **§3, both kinds of Slack app:** 8 West's client ID comes from `PLENIPO_SLACK_CLIENT_ID` at build
  time (a GitHub variable; the release refuses the tests' ID). A workspace's own app is saved with
  `save_connection_app` (client ID only; Slack takes no secret). The card shows Plenipo's app
  description (`slack::manifest`) to paste into Slack.
- **§4, your own Google app:** `save_connection_app` keeps the client ID in the settings and the
  secret only in the Vault (`connection-google-app-secret`), read back before the card says it is
  kept; it is refused while connected, and never returned. The Vault's list of Plenipo's names
  includes it, and the secret filter hides it in logs and the diagnostics file.
- **§5, the fits:** Slack's fixed ports 47211–47213 (`Listener::open_on`); Slack's
  `oauth/v2/authorize` and `oauth.v2.access` with PKCE and token rotation (a lasting token is kept
  as is when Slack gives no refresh value); `add_connection` / `remove_connection` (11 Connections
  commands); Disconnect cancels at the service (`auth.revoke` for the renewal itself and the
  short-lived sign-in, since Slack cancels only the token it is given; Google's revoke address)
  after the Vault is cleared, with a 15-second limit and a note on the card if it fails.
- **Found in the review, and fixed:** a Slack channel's ID is taken only in capitals, as Slack
  shows it ("companynews" is a name); Slack's lists are read page by page and say when there are
  more; a channel tool refuses a direct message's ID (and the other way round) before asking
  Slack; a Slack app deleted in Slack needs the owner again; Reconnect stays with the card's
  workspace or account; Gmail reads Western European mail and encoded subjects, searches spam and
  the bin when asked, and folds long subjects and thread IDs; the card's words say "Slack's
  words" where Plenipo quotes Slack's own button or page names.
- **Tests:** `crates/capabilities/tests/connections.rs` (the Slack and Google tests), Guard's
  `a_slack_channel_is_on_a_slack_list_by_its_id_only` and `slack_workspaces_and_the_owners_own_apps`,
  the desktop IPC tests, and `tests/e2e/specs/connections.e2e.mjs` part 20B. See the
  [part 20B acceptance report](../phases/phase-20b-acceptance-report.md).
