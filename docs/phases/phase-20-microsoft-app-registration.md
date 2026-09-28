# Registering Plenipo with Microsoft — steps for 8 West, and a page for clients' admins

Written 2026-09-28 (Pacific time) for Phase 20 (ADR-065, the Microsoft 365 connection). You do
these steps yourself, in your own browser. **Nothing here goes into a chat.** The only value you
give me is the **Application (client) ID**, which is public (see "Safe to share, and secret" at
the end).

Microsoft renames buttons often. Where a button has a newer and an older name, both are given.
Sources are at the end.

## Before you start

- **Who:** an account in 8 West's Microsoft 365 organization that is a **Global Administrator**,
  or both an **Application Administrator** (or **Cloud Application Administrator**) and able to
  approve permissions. Sign in with multi-factor sign-in (MFA).
- **Your domain:** check that 8 West's own web domain (for example `8westventures.com`) is
  verified in Entra: **Entra ID** → **Domain names**. It must say **Verified**. The publisher
  domain cannot be the `…onmicrosoft.com` one.
- **Two web pages on 8 West's website,** before clients use it: a **privacy statement** for
  Plenipo and **terms of use**. Microsoft shows an alert on the sign-in page without them, and
  Microsoft's API terms require a privacy statement. (Plain pages are fine: what Plenipo reads,
  that it keeps sign-ins only on the user's own PC, that it keeps IDs and links but not copies of
  mail or files, and that uninstalling with "delete my data" removes everything.)

## Step 1 — Register the app (about 5 minutes)

1. Go to **https://entra.microsoft.com** and sign in.
2. In the left menu: **Entra ID** → **App registrations**.
3. Click **+ New registration**.
4. **Name:** `Plenipo` (people see this name when they sign in).
5. **Supported account types:** choose **Any Entra ID Tenant + Personal Microsoft accounts**.
   (Older name: "Accounts in any organizational directory (Any Microsoft Entra ID tenant -
   Multitenant) and personal Microsoft accounts (e.g. Skype, Xbox)".) This lets both work or
   school accounts and personal accounts (outlook.com, hotmail.com) connect.
6. **Redirect URI:** in the first box choose **Public client/native (mobile & desktop)**. In the
   second box type exactly: `http://localhost`
7. Click **Register**.
   (If the app was first registered for organizations only and you are changing it to include
   personal accounts: **Manifest** → set `"requestedAccessTokenVersion": 2` → **Save**, then change
   the account types under **Authentication**. Microsoft requires it for personal accounts.)
8. You are on the app's **Overview**. Copy the **Application (client) ID** (it looks like
   `12345678-abcd-…`). Keep it; you will give it to me.

## Step 2 — Check the sign-in settings

1. In the app's menu: **Authentication** (it may say **Authentication (Preview)**).
2. Under **Mobile and desktop applications** you should see `http://localhost`. If not: **+ Add a
   platform** (or **Add Redirect URI**) → **Mobile and desktop applications** → in **Custom redirect
   URIs** type `http://localhost` → **Configure**.
3. Find **Allow public client flows** (under **Advanced settings**, or the **Settings** tab). Leave
   it **No / off**. Plenipo does not need it.
4. Click **Save** if you changed anything.
5. In the app's menu: **Certificates & secrets**. **Leave it empty.** Do not create a client
   secret. Plenipo is a desktop app and uses a one-time secret made fresh at each sign-in (PKCE)
   instead.

## Step 3 — Add the permissions (the fewest that work)

1. In the app's menu: **API permissions**.
2. Click **+ Add a permission** → **Microsoft Graph** → **Delegated permissions** ("the app acts as
   the signed-in person").
3. Use the search box to find and tick each of these, then click **Add permissions**. Plenipo
   asks for each one only when you turn its part on at that level ("Read only" or "Full access").

   | Permission                | What it lets Plenipo do, as the signed-in person                | Part and level          |
   | ------------------------- | --------------------------------------------------------------- | ----------------------- |
   | `openid`                  | Sign the person in                                              | always                  |
   | `profile`                 | See the person's name                                           | always                  |
   | `offline_access`          | Stay signed in (a refresh token)                                | always                  |
   | `User.Read`               | Read who signed in                                              | always                  |
   | `Mail.Read`               | Read the person's mail                                          | Mail, Read only         |
   | `Mail.ReadWrite`          | Read the person's mail, and save drafts (it cannot send)        | Mail, Full access       |
   | `Mail.Send`               | Send mail as the person (every send asks you first, in Plenipo) | Mail, Full access       |
   | `Calendars.Read`          | Read the person's calendar                                      | Calendar, Read only     |
   | `Calendars.ReadWrite`     | Read the person's calendar, and add events                      | Calendar, Full access   |
   | `Files.Read`              | Read the person's own OneDrive                                  | OneDrive, Read only     |
   | `Files.ReadWrite`         | Read and add files in the person's own OneDrive                 | OneDrive, Full access   |
   | `Sites.Read.All`          | Read SharePoint sites and files the person can already see      | SharePoint, Read only   |
   | `Sites.ReadWrite.All`     | Add files in SharePoint sites the person can already edit       | SharePoint, Full access |
   | `Chat.Read`               | Read the person's Teams chats                                   | Teams, Read only        |
   | `Team.ReadBasic.All`      | List the teams the person is in                                 | Teams, Read only        |
   | `Channel.ReadBasic.All`   | List those teams' channels                                      | Teams, Read only        |
   | `ChannelMessage.Read.All` | Read messages in those channels (always needs an admin)         | Teams, Read only        |
   | `ChatMessage.Send`        | Send a message in the person's chats (asks you first)           | Teams, Full access      |
   | `Chat.Create`             | Start a new chat (asks you first)                               | Teams, Full access      |
   | `ChannelMessage.Send`     | Post in those channels (asks you first)                         | Teams, Full access      |

   (`User.Read` is usually there already.)

4. **Do not add any "Application permissions".** Plenipo never acts on its own, only as the person
   signed in.
5. Still on **API permissions**, click **Grant admin consent for 8 West Ventures** → **Yes**. This
   approves Plenipo for everyone in 8 West's own organization. (Microsoft's default setting does
   not let ordinary users approve mail, calendar, chat, or site access themselves, even though the
   list says "Admin consent required: No".) Every line should now show a green check, "Granted for
   8 West Ventures".

Plenipo still asks, at each sign-in, only for the permissions of the parts you turned on in
Settings → Connections.

## Step 4 — Branding (what people see when they sign in)

1. In the app's menu: **Branding & properties**.
2. **Name:** `Plenipo`. **Logo:** optional (Plenipo's icon).
3. **Home page URL:** 8 West's Plenipo page.
4. **Terms of service URL** and **Privacy statement URL:** the two pages from "Before you start".
5. **Publisher domain:** choose 8 West's verified domain (not `…onmicrosoft.com`).
6. Click **Save**.

## Step 5 — Publisher verification (free; before clients use it)

This puts a blue **verified** badge and "8 West Ventures, LLC" on the sign-in page. Without it,
the page says **Unverified**, and many clients' admins will refuse the app.

1. **Join the Microsoft AI Cloud Partner Program** (free) in **Partner Center**
   (https://partner.microsoft.com) as 8 West Ventures, LLC, and finish its business
   verification. Use an email address at the same domain as the publisher domain.
2. In Partner Center, note the **Partner One ID** of the **partner global account** (PGA) — not a
   "location" ID.
3. Check that the person doing step 5 has **Application Administrator** or **Cloud Application
   Administrator** in Entra, and **Partner Admin** (or **Account Admin**) in Partner Center, with
   multi-factor sign-in.
4. In Entra: **App registrations** → **Plenipo** → **Branding & properties** → **Add Partner ID to
   verify publisher** (older name: "Add MPN ID to verify publisher") → type the Partner One ID →
   **Verify and save**.
5. If it refuses: the most common reasons are a domain mismatch (the Partner Center email domain
   must match the publisher domain) or the app's organization not being linked to the partner
   account (in Partner Center, add 8 West's Microsoft 365 organization to the partner account).

## Step 6 — Give me the app ID

Paste the **Application (client) ID** in the pull request or in chat. It is public: anyone who
signs in to Plenipo can see it. I put it in Plenipo's code. Nothing else is needed from you.

## What not to do

- Do not create a client secret or a certificate.
- Do not add application permissions.
- Do not turn on **Allow public client flows**.
- Do not paste any password, secret, token, or recovery code anywhere but Microsoft's own pages.

## Safe to share, and secret

| Value                           | Secret?                     | Where it goes                             |
| ------------------------------- | --------------------------- | ----------------------------------------- |
| Application (client) ID         | **No** — public             | Plenipo's code; the admin approval link   |
| Directory (tenant) ID           | No                          | Nowhere; Plenipo does not need it         |
| Redirect URI `http://localhost` | No                          | The app registration                      |
| Partner One ID                  | No, but no need to share    | Partner Center and step 5 only            |
| A client secret                 | **Yes** — do not create one | Nowhere. If one exists, delete it         |
| Each person's sign-in token     | **Yes**                     | Only in the Vault on that person's own PC |
| Any password or one-time code   | **Yes**                     | Only on Microsoft's own sign-in page      |

---

# For your clients' IT admins — approving Plenipo in your organization

_You can send this section to a client's admin as it is._

**What Plenipo is:** a desktop app by 8 West Ventures, LLC. It lets AI helpers on a person's own
PC read that person's mail, calendar, files, and Teams chats, and draft replies — only as that
person, only what that person can already see, and **every send, post, or delete waits for the
person to approve it** in Plenipo.

**Why you are asked:** Microsoft's default setting does not let ordinary users approve an app that
reads mail, calendars, or chats. An admin approves once for the whole organization.

**Who can approve:** a Global Administrator, Privileged Role Administrator, Cloud Application
Administrator, or Application Administrator.

**How to approve (either way):**

1. Open the link the Plenipo user sends you (Plenipo's **Copy the approval link for your admin**):
   `https://login.microsoftonline.com/<your domain>/adminconsent?client_id=<Plenipo's app ID>`.
   Sign in as an admin, read the permission list on Microsoft's page, and click **Accept**.
2. Or: sign in to Plenipo's **Connect** yourself and tick **Consent on behalf of your
   organization** before **Accept**.

**What you approve:** delegated permissions only (the list in step 3 above). No application
permissions: Plenipo can never read a mailbox, file, or chat the signed-in person cannot. Each
person's Plenipo asks only for the parts and levels they turn on.

**Limiting or removing it later:** **Entra admin center** → **Entra ID** → **Enterprise apps** →
**Plenipo**:

- **Permissions** (under **Security**): see or revoke what was granted.
- **Properties** → **Assignment required?** → **Yes**, then **Users and groups**: only the people
  you add can use Plenipo.
- **Properties** → **Enabled for users to sign in?** → **No**, or **Delete**: turns it off for
  everyone.

**Where sign-ins are kept:** only in Windows Credential Manager on each person's own PC. Plenipo
has no server that holds your organization's data or sign-ins.

---

## Sources (checked 2026-09-28)

- Desktop sign-in and redirect URIs:
  [desktop app configuration](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-configuration),
  [redirect URI rules](https://learn.microsoft.com/en-us/entra/identity-platform/reply-url),
  [public and confidential clients](https://learn.microsoft.com/en-us/entra/identity-platform/msal-client-applications),
  [authorization code flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow)
- Registering and account types:
  [register an app](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app),
  [multitenant apps](https://learn.microsoft.com/en-us/entra/identity-platform/howto-convert-app-to-be-multi-tenant)
- Permissions and consent:
  [Graph permissions reference](https://learn.microsoft.com/en-us/graph/permissions-reference),
  [Microsoft's managed consent policy](https://learn.microsoft.com/en-us/entra/identity/enterprise-apps/manage-app-consent-policies),
  [user consent settings](https://learn.microsoft.com/en-us/entra/identity/enterprise-apps/configure-user-consent),
  [grant admin consent](https://learn.microsoft.com/en-us/entra/identity/enterprise-apps/grant-admin-consent)
- Publisher verification:
  [overview](https://learn.microsoft.com/en-us/entra/identity-platform/publisher-verification-overview),
  [how to](https://learn.microsoft.com/en-us/entra/identity-platform/mark-app-as-publisher-verified),
  [terms and privacy links](https://learn.microsoft.com/en-us/entra/identity-platform/howto-add-terms-of-service-privacy-statement)
- Tokens and limits:
  [refresh tokens](https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens),
  [throttling limits](https://learn.microsoft.com/en-us/graph/throttling-limits)
- Terms: [Microsoft APIs Terms of Use](https://learn.microsoft.com/en-us/legal/microsoft-apis/terms-of-use)
- Microsoft's own MCP servers:
  [Agent 365 tooling servers (Work IQ)](https://learn.microsoft.com/en-us/microsoft-agent-365/tooling-servers-overview),
  [Microsoft MCP Server for Enterprise](https://learn.microsoft.com/en-us/graph/mcp-server/overview)
