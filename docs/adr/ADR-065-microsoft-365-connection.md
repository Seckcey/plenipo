# ADR-065: The Microsoft 365 connection — 8 West's app, the fewest permissions, and what admins approve

- **Status:** Proposed (2026-09-28). Becomes Accepted when the owner approves the Phase 20 design,
  with the choices the owner makes in the
  [Phase 20 checklist](../phases/phase-20-checklist.md#choices-for-you).
- **Date:** 2026-09-28
- **Phase:** 20 (part 20A if the owner splits the phase; ADR-067)
- **Carries out:** ROLLOUT_PLAN.md Phase 20, "**Microsoft 365:** Outlook mail, Outlook calendar,
  OneDrive, SharePoint, Teams", and "Microsoft 365 needs 8 West to register an app with Microsoft
  (Microsoft Entra), for 8 West's own tenant and its clients'. Asks for the fewest permissions that
  work. Publisher verification and client admin consent are part of the phase."
- **Builds on:** ADR-062 (the rules for every connection), ADR-063 (signing in, and the Vault),
  ADR-064 (built in, calling Microsoft Graph)

> **On screen** (ADR-010, plain words and rank names): **Microsoft 365**, its parts **Mail**,
> **Calendar**, **OneDrive**, **SharePoint**, and **Teams**, "**Your organization's admin needs to
> approve Plenipo first**", **Copy the approval link for your admin**, and **What Plenipo was
> allowed**. Microsoft's own permission names appear only in that list, each with plain words
> beside it. This record keeps Microsoft's words (Entra, app registration, delegated permission,
> admin consent, tenant).

## In short

8 West registers one app, "Plenipo", with Microsoft. Every copy of Plenipo — at 8 West and at its
clients — signs in through that app, in the owner's own browser, as the person signing in, with
**no client secret**. Plenipo asks only for the permissions of the parts you turn on, and it can
only ever do what the signed-in person could do themselves: it never reads anyone else's mailbox.
Because Microsoft's default setting does not let ordinary users approve an app that reads mail,
calendars, or chats, **an admin approves Plenipo once per organization** — you, for 8 West; each
client's IT admin for theirs. Publisher verification (free) puts a blue "verified" badge on the
sign-in page. Accepting this record means building the connection with the tools and permissions
below.

## Context

Checked on Microsoft's own pages on 2026-09-28 (the Phase 20 checklist lists every source):

- **A desktop app signs in with PKCE and no secret.** "Public clients, which include native
  applications … must not use secrets or certificates when redeeming an authorization code." The
  platform is **Mobile and desktop applications**, and "for apps using system browsers, use the
  exact value: `http://localhost`"; "the port component … is ignored for the purposes of matching a
  localhost redirect URI". **Allow public client flows** is only for device code, username and
  password, and Windows sign-in, so it stays **off**.
- **Multitenant, work and school accounts only:** `signInAudience` `AzureADMultipleOrgs`, sign-in
  at `login.microsoftonline.com/organizations`. Personal accounts cannot use Teams' chat and
  channel messages or Microsoft Search.
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
code (ADR-064 §1). Sign-in is `https://login.microsoftonline.com/organizations/oauth2/v2.0/…`, then
the person's own organization's address once known. Guard's gate for Plenipo's own requests gets a
purpose, "Microsoft 365", allowing only `login.microsoftonline.com` and `graph.microsoft.com`.

### 2. The parts, their tools, and their permissions

Every tool below is in Plenipo's fixed table, with its kind (ADR-062 §5). "Always" permissions are
asked at every sign-in: `openid`, `profile`, `offline_access` (a refresh token), and `User.Read`
(who signed in).

| Part           | Tool (worker's name)                                                     | Kind                            | What it does (Graph)                                                                                                           | Permissions asked when the part is on         |
| -------------- | ------------------------------------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| **Mail**       | `m365_mail_search`                                                       | Read                            | Messages in a folder (Inbox by default), by sender, unread, date, or words; at most 25                                         | `Mail.ReadWrite`, `Mail.Send`                 |
|                | `m365_mail_read`                                                         | Read                            | One message as text, with attachment names                                                                                     |                                               |
|                | `m365_mail_draft`                                                        | Write                           | A new draft, or a reply, reply-all, or forward draft of one message (`createReply`, …), with the worker's words                |                                               |
|                | `m365_mail_send`                                                         | **Send**                        | Sends one draft (`/messages/{id}/send`); Plenipo reads the draft's recipients from Outlook just before, and Guard checks those |                                               |
| **Calendar**   | `m365_calendar_events`                                                   | Read                            | Events between two times (today by default, in this PC's time zone) (`calendarView`)                                           | `Calendars.ReadWrite`                         |
|                | `m365_calendar_add_event`                                                | Write, or **Send** with guests  | A new event; with guests, Outlook sends invitations, so it asks                                                                |                                               |
| **OneDrive**   | `m365_onedrive_search`, `m365_onedrive_list`                             | Read                            | Files in the person's own OneDrive                                                                                             | `Files.ReadWrite`                             |
|                | `m365_onedrive_read`                                                     | Read                            | A file's text (text files and the text of Word documents, up to 1 MB)                                                          |                                               |
|                | `m365_onedrive_upload`                                                   | Write, or **Delete** to replace | A new text file (up to 4 MB); replacing an existing file asks                                                                  |                                               |
| **SharePoint** | `m365_sharepoint_search`, `m365_sharepoint_list`, `m365_sharepoint_read` | Read                            | Sites and files the person can see; reading only (choice 5)                                                                    | `Sites.Read.All`                              |
| **Teams**      | `m365_teams_chats`, `m365_teams_chat_messages`                           | Read                            | The person's chats and their recent messages                                                                                   | `Chat.ReadBasic`, `Chat.Read`                 |
|                | `m365_teams_send_chat`                                                   | **Send**                        | A message in an existing chat                                                                                                  | `ChatMessage.Send`                            |
|                | `m365_teams_channels`                                                    | Read                            | The person's teams and their channels                                                                                          | `Team.ReadBasic.All`, `Channel.ReadBasic.All` |
|                | `m365_teams_post`                                                        | **Send**                        | A post or reply in a channel                                                                                                   | `ChannelMessage.Send`                         |
|                | `m365_teams_channel_messages`                                            | Read                            | A channel's recent messages — **off by default** ("Teams: read channel messages"), because it always needs an admin            | `ChannelMessage.Read.All`                     |

- **No application permissions.** Plenipo acts only as the person signed in, with what they can
  already see.
- **Not built:** deleting mail or events, moving mail, sharing links, reading attachments' contents,
  accepting invitations, and other mailboxes. Each can be added later on the same rules if the
  owner asks.
- **Sizes:** a message's text up to 20,000 characters, a file's up to 1 MB of text, 25 items per
  list; the worker is told when something was cut.

### 3. "Send without asking" for Microsoft 365

The list takes addresses and domains (`client@example.com`, `@8westit.com`) and Teams channels
(`Team name › Channel name`). A mail send goes ahead without asking only when every To, Cc, and Bcc
address of the draft **as Outlook has it** is on the list (ADR-062 §5). A chat message goes ahead
only when every member of the chat is. A guest invitation, only when every guest is.

### 4. Who signs in, and whose data

- The owner (or the person at a client who runs Plenipo) signs in as themselves. Workers read and
  write only that person's mail, calendar, OneDrive, the SharePoint sites and Teams they belong to.
- One Microsoft 365 account per Plenipo organization (ADR-062 §1).

### 5. Admin approval, built into the card

- When Microsoft answers that an admin must approve, the card says "**Your organization's admin
  needs to approve Plenipo first**", with **Copy the approval link for your admin**:
  `https://login.microsoftonline.com/<the organization's domain>/adminconsent?client_id=<8 West's
app ID>`. The admin opens it, sees the list of permissions on Microsoft's page, and approves for
  everyone. Then the owner presses **Connect** again.
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
  settings as 8 West's, single-tenant). Kept in the connection's settings; not a secret.

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
- **`Mail.Read` and `Calendars.Read` only, no drafts.** Rejected: the acceptance test drafts a
  reply. Both are admin-approved anyway.
- **`Files.ReadWrite.All` for OneDrive and SharePoint.** Not recommended: broader than needed;
  `Files.ReadWrite` covers the person's own OneDrive without an admin, and `Sites.Read.All` covers
  reading SharePoint.
- **`/common` with personal Microsoft accounts.** Not recommended: Teams and search do not work
  with them, and 8 West's use is business accounts (choice 6).
- **A client secret, or a web app with a server.** Rejected: a desktop app cannot keep a secret,
  and Plenipo must work without an 8 West server.
