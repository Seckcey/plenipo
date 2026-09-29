# ADR-065: The Microsoft 365 connection — 8 West's app, the fewest permissions, and what admins approve

- **Status:** Accepted (by the owner, 2026-09-28), with the owner's choices: every part can be
  **Read only** or **Full access** (choices 5 and 8), and both work or school accounts and personal
  Microsoft accounts (choice 6); the rest as recommended
- **Date:** 2026-09-28
- **Phase:** 20, part 20A (ADR-067)
- **Carries out:** ROLLOUT_PLAN.md Phase 20, "**Microsoft 365:** Outlook mail, Outlook calendar,
  OneDrive, SharePoint, Teams", and "Microsoft 365 needs 8 West to register an app with Microsoft
  (Microsoft Entra), for 8 West's own tenant and its clients'. Asks for the fewest permissions that
  work. Publisher verification and client admin consent are part of the phase."
- **Builds on:** ADR-062 (the rules for every connection), ADR-063 (signing in, and the Vault),
  ADR-064 (built in, calling Microsoft Graph)

> **On screen** (ADR-010, plain words and rank names): **Microsoft 365**, **Connect a work or
> school account**, **Connect a personal account**, its parts **Mail**, **Calendar**, **OneDrive**,
> **SharePoint**, and **Teams**, each **Read only** or **Full access**, "**Your organization's admin needs to
> approve Plenipo first**", **Copy the approval link for your admin**, and **What Plenipo was
> allowed**. Microsoft's own permission names appear only in that list, each with plain words
> beside it. This record keeps Microsoft's words (Entra, app registration, delegated permission,
> admin consent, tenant).

## In short

8 West registers one app, "Plenipo", with Microsoft. Every copy of Plenipo — at 8 West and at its
clients — signs in through that app, in the owner's own browser, as the person signing in, with
**no client secret**. Work or school accounts and personal Microsoft accounts (outlook.com,
hotmail.com) both work; Teams and SharePoint only exist for work and school accounts. Each part —
Mail, Calendar, OneDrive, SharePoint, Teams — is **off**, **Read only**, or **Full access**, and
Plenipo asks Microsoft only for the permissions of the parts and levels you pick. It can only ever
do what the signed-in person could do themselves: it never reads anyone else's mailbox. Because
Microsoft's default setting does not let ordinary users approve an app that reads mail, calendars,
or chats, **an admin approves Plenipo once per organization** — you, for 8 West; each client's IT
admin for theirs. Personal accounts approve for themselves. Publisher verification (free) puts a
blue "verified" badge on the sign-in page. Accepting this record means building the connection with
the tools and permissions below.

## Context

Checked on Microsoft's own pages on 2026-09-28 (the Phase 20 checklist lists every source):

- **A desktop app signs in with PKCE and no secret.** "Public clients, which include native
  applications … must not use secrets or certificates when redeeming an authorization code." The
  platform is **Mobile and desktop applications**, and "for apps using system browsers, use the
  exact value: `http://localhost`"; "the port component … is ignored for the purposes of matching a
  localhost redirect URI". **Allow public client flows** is only for device code, username and
  password, and Windows sign-in, so it stays **off**.
- **Accounts:** `signInAudience` `AzureADandPersonalMicrosoftAccount` lets both work or school
  accounts (any organization) and personal Microsoft accounts sign in. Sign-in at
  `login.microsoftonline.com/organizations` takes only work or school accounts, and
  `…/consumers` only personal ones. Personal accounts cannot use Teams' chats and channels,
  SharePoint, or Microsoft Search ("Delegated (personal Microsoft account): Not supported").
- **Microsoft's default for new organizations** ("Let Microsoft manage your consent settings")
  lets users approve apps except for mail (`Mail.Read`, `Mail.ReadWrite`, …), calendars
  (`Calendars.Read`, `Calendars.ReadWrite`, …), chats (`Chat.Read`, `Chat.ReadWrite`), and all
  files and sites (`Files.Read.All`, `Files.ReadWrite.All`, `Sites.Read.All`, …). Those need an
  admin, though Graph's own list says "admin consent: no".
- **Publisher verification** needs a verified Microsoft AI Cloud Partner Program account (its
  Partner One ID), the app registered in a tenant tied to it, and a verified company domain as the
  publisher domain. Microsoft charges nothing. An unverified multitenant app shows "Unverified" on
  the consent screen, and where users may approve apps, an unverified app asking for more than
  sign-in is sent to an admin instead.
- **Microsoft's own MCP servers for mail, calendar, Teams, and files** (Work IQ, Agent 365) are
  preview, hosted by Microsoft, and need a Microsoft 365 Copilot license for every user. The
  Microsoft MCP Server for Enterprise covers directory data only.
- **Refresh tokens** last until unused for 90 days and come back new on every use; Microsoft
  estimates about 2 KB each. The Vault keeps up to 10,000 characters, in pieces (Phase 11).
- **Throttling:** 10,000 requests per 10 minutes per app and mailbox, 4 at once; Teams messages 1
  per second per chat or channel and per user.
- **Microsoft APIs Terms of Use:** ask only for the permissions needed; keep no database of copies
  "except as necessary"; delete data when the app is uninstalled; publish a privacy statement.

## Decision

### 1. Built in, on Microsoft Graph

The connection calls Microsoft Graph (`https://graph.microsoft.com/v1.0/…`) from Plenipo's own
code (ADR-064 §1). The card has two buttons: **Connect a work or school account** signs in at
`https://login.microsoftonline.com/organizations/oauth2/v2.0/…` (then the person's own
organization's address once known), and **Connect a personal account** at `…/consumers/…`. A
personal account offers only Mail, Calendar, and OneDrive. Guard's gate for Plenipo's own requests gets a
purpose, "Microsoft 365", allowing only `login.microsoftonline.com` and `graph.microsoft.com`.

### 2. The parts, their levels, their tools, and their permissions

Each part is **off**, **Read only**, or **Full access** (the owner's choices 5 and 8). Plenipo asks
Microsoft only for the permissions of the parts that are on, at their level. Reading tools need the
part on; writing, sending, and deleting tools need **Full access**. A worker's own line on **Who
may use it** (**Read only** or **Read and write**, ADR-062 §3) still decides what that worker gets.
Every tool is in Plenipo's fixed table, with its kind (ADR-062 §5). "Always" permissions are asked
at every sign-in: `openid`, `profile`, `offline_access` (a refresh token), and `User.Read` (who
signed in).

| Part           | Tool (worker's name)                                                     | Kind                            | What it does (Graph)                                                                                                           | Read only                                                                             | Full access adds                                         |
| -------------- | ------------------------------------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| **Mail**       | `m365_mail_search`                                                       | Read                            | Messages in a folder (Inbox by default), by sender, unread, date, or words; at most 25                                         | `Mail.Read`                                                                           | `Mail.ReadWrite`, `Mail.Send`                            |
|                | `m365_mail_read`                                                         | Read                            | One message as text, with attachment names                                                                                     |                                                                                       |                                                          |
|                | `m365_mail_draft`                                                        | Write                           | A new draft, or a reply, reply-all, or forward draft of one message (`createReply`, …), with the worker's words                |                                                                                       |                                                          |
|                | `m365_mail_send`                                                         | **Send**                        | Sends one draft (`/messages/{id}/send`); Plenipo reads the draft's recipients from Outlook just before, and Guard checks those |                                                                                       |                                                          |
| **Calendar**   | `m365_calendar_events`                                                   | Read                            | Events between two times (today by default, in this PC's time zone) (`calendarView`)                                           | `Calendars.Read`                                                                      | `Calendars.ReadWrite`                                    |
|                | `m365_calendar_add_event`                                                | Write, or **Send** with guests  | A new event; with guests, Outlook sends invitations, so it asks                                                                |                                                                                       |                                                          |
| **OneDrive**   | `m365_onedrive_search`, `m365_onedrive_list`, `m365_onedrive_read`       | Read                            | Files in the person's own OneDrive; a file's text (text files and the text of Word documents, up to 1 MB)                      | `Files.Read`                                                                          | `Files.ReadWrite`                                        |
|                | `m365_onedrive_upload`                                                   | Write, or **Delete** to replace | A new text file (up to 4 MB); replacing an existing file asks                                                                  |                                                                                       |                                                          |
| **SharePoint** | `m365_sharepoint_search`, `m365_sharepoint_list`, `m365_sharepoint_read` | Read                            | Sites and files the person can see                                                                                             | `Sites.Read.All`                                                                      | `Sites.ReadWrite.All`                                    |
|                | `m365_sharepoint_upload`                                                 | Write, or **Delete** to replace | A new text file in a site's library the person can edit                                                                        |                                                                                       |                                                          |
| **Teams**      | `m365_teams_chats`, `m365_teams_chat_messages`                           | Read                            | The person's chats and their recent messages                                                                                   | `Chat.Read`, `Team.ReadBasic.All`, `Channel.ReadBasic.All`, `ChannelMessage.Read.All` | `ChatMessage.Send`, `ChannelMessage.Send`, `Chat.Create` |
|                | `m365_teams_channels`, `m365_teams_channel_messages`                     | Read                            | The person's teams, their channels, and a channel's recent messages                                                            |                                                                                       |                                                          |
|                | `m365_teams_send_chat`                                                   | **Send**                        | A message in an existing chat                                                                                                  |                                                                                       |                                                          |
|                | `m365_teams_start_chat`                                                  | **Send**                        | A new chat with people, and its first message                                                                                  |                                                                                       |                                                          |
|                | `m365_teams_post`                                                        | **Send**                        | A post, or a reply to a post, in a channel                                                                                     |                                                                                       |                                                          |

- **Starting levels** when a part is turned on: **Read only** (the fewest permissions). The owner
  switches a part to **Full access** on the card, then presses **Reconnect** so Microsoft is asked
  for the added permissions.
- **Teams reading needs an admin** in every organization, because reading channel messages
  (`ChannelMessage.Read.All`) always does.
- **No application permissions.** Plenipo acts only as the person signed in, with what they can
  already see.
- **Not built:** deleting mail, events, messages, or files; moving mail; sharing links; reading
  attachments' contents; answering invitations; other people's mailboxes. Each can be added later on
  the same rules if the owner asks.
- **Sizes:** a message's text up to 20,000 characters, a file's up to 1 MB of text, 25 items per
  list; the worker is told when something was cut.

### 3. "Send without asking" for Microsoft 365

The list takes addresses and domains (`client@example.com`, `@8westit.com`) and Teams channels
(`Team name › Channel name`). A mail send goes ahead without asking only when every To, Cc, and Bcc
address of the draft **as Outlook has it** is on the list (ADR-062 §5). A chat message goes ahead
only when every member of the chat is. A guest invitation, only when every guest is.

### 4. Who signs in, and whose data

- The owner (or the person at a client who runs Plenipo) signs in as themselves, with a work or
  school account or a personal one. Workers read and write only that person's mail, calendar,
  OneDrive, and the SharePoint sites and Teams they belong to.
- One Microsoft 365 account per Plenipo organization (ADR-062 §1).

### 5. Admin approval, built into the card

- When Microsoft answers that an admin must approve, the card says "**Your organization's admin
  needs to approve Plenipo first**", with **Copy the approval link for your admin**:
  `https://login.microsoftonline.com/<the organization's domain>/adminconsent?client_id=<8 West's
app ID>`. The admin opens it, sees the list of permissions on Microsoft's page, and approves for
  everyone. Then the owner presses **Connect** again.
- A personal account has no admin: its owner approves on Microsoft's page when connecting.
- An admin who signs in with **Connect** themselves sees "Consent on behalf of your organization"
  on Microsoft's page and can approve there.
- The checklist gives the owner the steps for 8 West's own tenant and a page to send to clients'
  admins.

### 6. Publisher verification

8 West verifies the app as its publisher (free) before giving it to clients. Until then, the
consent page says "Unverified", which many admins refuse. Plenipo works the same either way.

### 7. The app ID

- **8 West's app ID** (the "Application (client) ID") is public and goes in Plenipo's code. Copies
  built without it show "This copy of Plenipo has no Microsoft app ID yet", and **Connect** is off.
- **An organization's own app ID** (choice 4): an **Advanced** field on the card lets a client whose
  admin allows only apps registered in their own tenant use their own registration (the same
  settings as 8 West's, single-tenant, work or school accounts only; sign-in at that
  organization's own address). Kept in the connection's settings; not a secret.

### 8. Tests

A stand-in for Microsoft's sign-in and Graph (ADR-063 §8) answers every address above, keeps a
small mailbox, calendar, drive, sites, chats, and channels, records what it was sent, and can
answer "admin approval needed", "sign in again", and "too many requests".

## Consequences

- 8 West must finish the Entra steps in the checklist (register, permissions, branding, publisher
  verification) and publish a privacy statement and terms before clients use the connection.
- Every client organization needs its admin's approval once. The card and the checklist make that
  a link to send.
- Microsoft can change Graph. Plenipo's updates carry the changes; the stand-in tests hold the
  shape Plenipo expects.

## Alternatives considered

- **Microsoft's Work IQ MCP servers.** Rejected for now: preview, hosted by Microsoft, and a
  Microsoft 365 Copilot license for every user.
- **One level for every part** (the first draft: full permissions for Mail and Calendar, reading
  only for SharePoint, channel reading as a separate part). Replaced by the owner's choice: each
  part **Read only** or **Full access**.
- **`Files.ReadWrite.All` for OneDrive.** Not needed: `Files.ReadWrite` covers the person's own
  OneDrive, and SharePoint has its own permissions.
- **Work or school accounts only.** Not chosen by the owner: personal accounts work too, for Mail,
  Calendar, and OneDrive.
- **A client secret, or a web app with a server.** Rejected: a desktop app cannot keep a secret,
  and Plenipo must work without an 8 West server.

## As built (v1.13.0)

Built as written: 21 tools — Mail (search, read, draft, send), Calendar (events, add an event),
OneDrive (search, list, read, save), SharePoint (search, list, read, save), and Teams (chats, chat
messages, teams and channels, channel messages, send in a chat, start a chat, post). The
permissions per part and level are the table above; a personal account has Mail, Calendar, and
OneDrive. Details as built:

- **8 West's app ID** comes from the repository variable `PLENIPO_MICROSOFT_APP_ID` (public, not a
  secret); until it is set, the card says "This copy of Plenipo has no Microsoft app ID yet", and
  an organization can use its own app under **Advanced** (locked while a sign-in waits).
- **Downloads** go only to Microsoft's storage: `*.sharepoint.com`, `*.files.1drv.com`, and, for
  personal accounts, `*.microsoftpersonalcontent.com`; the sign-in goes to Microsoft Graph only.
- **Reading a reply's own words:** Outlook's "unique body" may hold the earlier message too, so
  Plenipo cuts at Outlook's line above a quoted message.
- **Teams:** a chat message goes to the chat's members as Teams gives their addresses (the owner by
  account); posting in a channel always asks. **SharePoint:** adding a file asks (see ADR-062, as
  built).
- **"Too many requests":** waited on once, for up to 30 seconds as Microsoft asks.
- **Sending** uses Microsoft's documented request with no body.
