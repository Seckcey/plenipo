# Setting up your Slack and Google apps — steps for 8 West, and for anyone who uses Plenipo

Written 2026-09-29 (Pacific time) for Phase 20, part 20B
([ADR-069 (Slack and Google: the owner's choices)](../adr/ADR-069-slack-and-google-choices.md)).
You do these steps yourself, in your own browser. **Nothing here goes into a chat.**

- **Slack** needs no secret at all. The only value is a Slack app's **Client ID**, which is public.
- **Google** needs your own Google app's **Client ID** (public) and **Client secret**. You type
  both into Plenipo's Google card (**Settings → Connections → Google → Your Google app**). The
  secret goes only to the Vault (Windows Credential Manager) and is never shown again. Never paste
  it into a chat, an email, or GitHub.

Slack and Google rename buttons often. Where a button has a newer and an older name, both are
given. Sources are at the end.

## Which steps you need

| You want to…                                                                              | Do                            |
| ----------------------------------------------------------------------------------------- | ----------------------------- |
| Let anyone who uses Plenipo connect any Slack workspace with one click (8 West's app)     | Part A (once, for 8 West)     |
| Connect a Slack workspace with its own app (its admin allows only its own apps, or speed) | Part B (once per workspace)   |
| Connect Google (Gmail, Calendar, Drive)                                                   | Part C (once per Google user) |

## Part A — 8 West's Slack app (once, about 15 minutes)

This is the app Plenipo's **Connect** button uses for Slack. Its client ID goes into Plenipo's
releases as a GitHub **variable** (not a secret).

**Before you start:** sign in to Slack in your browser with an account in 8 West's own workspace
that may create apps (usually an owner or admin).

### A1 — Create the app from Plenipo's app description

1. Open Plenipo → **Settings → Connections** → the **Slack** card → **Advanced**. Click **Copy the
   app description**. (It lists Plenipo's sign-in addresses and the permissions it may ask for; it
   holds no secret.)
2. Go to **https://api.slack.com/apps** and click **Create New App** (or **Create an App**).
3. Choose **From a manifest** (older name: **From an app manifest**).
4. Pick **8 West's workspace** and click **Next**.
5. Choose the **JSON** tab, delete what is there, paste Plenipo's app description, and click
   **Next**.
6. Check the summary (user permissions only, no bot), then click **Create**.

### A2 — Check the sign-in settings

1. In the app's left menu: **OAuth & Permissions**.
2. Under **Redirect URLs** you should see exactly these three:
   - `http://localhost:47211`
   - `http://localhost:47212`
   - `http://localhost:47213`

   Slack only sends the sign-in back to an address written here, port included, so Plenipo uses
   the first of these that is free on your PC.

3. **PKCE** should be **on** (it makes sign-in work with no secret). Slack says turning PKCE on
   cannot be undone; that is fine for Plenipo.
4. **Token rotation** (under **Advanced token security via token rotation**) should be **on**:
   Plenipo's sign-in then renews itself every 12 hours, and a sign-in not used for 30 days ends.
5. Under **Scopes → User Token Scopes** you should see the permissions from the app description,
   and **no Bot Token Scopes**.
6. In the left menu: **Basic Information** → **App Credentials**. Copy the **Client ID** (it looks
   like `1234567890.9876543210`). Leave the **Client Secret** alone: Plenipo never uses it.

### A3 — Let other workspaces install it

1. In the left menu: **Manage Distribution**.
2. Under **Share Your App with Other Workspaces**, complete the checklist and click **Activate
   Public Distribution**.
3. **Check this:** Slack's checklist has asked for `https` addresses for distributed apps. I could
   not confirm from Slack's pages whether it accepts Plenipo's `http://localhost` sign-in
   addresses for an app with PKCE on. **If Slack refuses**, stop here and tell me: 8 West's app
   then works only in 8 West's own workspace, and every other workspace uses its own app (Part B),
   which already works in Plenipo.

### A4 — Put the client ID into Plenipo's releases

1. On GitHub, open the repository **Seckcey/plenipo** → **Settings** → **Secrets and variables** →
   **Actions** → the **Variables** tab (not Secrets).
2. Click **New repository variable**. **Name:** `PLENIPO_SLACK_CLIENT_ID`. **Value:** the Client
   ID from A2. Click **Add variable**.
3. The next release builds it into Plenipo. Until then, the Slack card says "This copy of Plenipo
   has no Slack app yet" and a workspace can still use its own app (Part B).

### Before offering it to clients — read this

- **Slack reads slowly for 8 West's app.** While the app is outside the Slack Marketplace, Slack
  lets it read a channel's or thread's history once a minute, 15 messages at a time. The card says
  so. A workspace's own app (Part B) is not slowed.
- **Slack's terms need a lawyer before Plenipo Pro is sold with Slack.** Slack's API terms count a
  free app that connects to a paid product as commercial distribution, which needs the Slack
  Marketplace or a partner agreement. Plenipo Pro is paid. Have a lawyer read Slack's API terms
  (link at the end) before selling Pro with Slack, and before listing the app.

## Part B — A workspace's own Slack app (about 10 minutes)

For a workspace whose admin allows only apps it made itself, or that wants Slack's normal speed.
The workspace's own admin can do this; nothing goes to 8 West.

1. In Plenipo: **Settings → Connections** → that workspace's **Slack** card → **Advanced** →
   **Copy the app description**.
2. Do steps **A1** and **A2** above in **that** workspace (skip A3 and A4: the app stays in its
   own workspace).
3. Back in Plenipo, on the same card, under **Advanced** → **Your Slack app's client ID**: paste
   the Client ID from A2 and click **Use this app**.
4. Turn on the parts you want, pick **Who may use it**, and click **Connect**.

**More than one workspace:** click **Add another Slack workspace** at the end of the Slack
section. Each workspace gets its own card, sign-in, parts, and lists. A card that is not connected
can be removed with **Remove this workspace**.

## Part C — Your own Google app (about 20 minutes)

Google connects through **your own** Google app, in your own Google Cloud project. There is no
8 West Google app: an app for everyone would need Google's review and a yearly paid security
check (see "Risks" at the end).

**Before you start:** which Google account will you connect?

- **Google Workspace** (an address at your own domain, managed in Google Admin): you will make the
  app **Internal**. No Google review, and sign-ins do not expire every week.
- **A personal Gmail account** (@gmail.com): you will make the app **External** and put it **In
  production**. Google shows "Google hasn't verified this app" when you connect; that is expected
  for an app only you use. Do **not** leave it in **Testing**: its sign-ins end every 7 days.

### C1 — Make a project

1. Go to **https://console.cloud.google.com** and sign in with the Google account you will
   connect (for Workspace, an account in that organization).
2. At the top, click the project picker → **New project**. **Name:** `Plenipo`. Click **Create**,
   then make sure the new project is selected at the top.

### C2 — Turn on the three APIs

1. In the search bar at the top, search **Gmail API** → open it → **Enable**.
2. Search **Google Calendar API** → **Enable**.
3. Search **Google Drive API** → **Enable**.

### C3 — Set up the sign-in screen

1. In the left menu (☰): **APIs & Services** → **OAuth consent screen**. Google now calls this
   **Google Auth Platform**; click **Get started** if you see it.
2. **App information:** **App name** `Plenipo`; **User support email**: your address. **Next**.
3. **Audience:** **Internal** (Google Workspace) or **External** (personal Gmail). **Next**.
4. **Contact information:** your address. **Next**. Agree to Google's user data policy, then
   **Continue** and **Create**.
5. **External only:** in the left menu **Audience** → **Publishing status** → click **Publish
   app** (to **In production**) and confirm.
6. **Data access** (older name: **Scopes**): you may leave it empty. Plenipo asks at sign-in only
   for what the parts you turn on need (listed in the table below).

### C4 — Make the Desktop app client

1. In the left menu: **Clients** (older: **APIs & Services → Credentials** → **+ Create
   credentials** → **OAuth client ID**).
2. Click **+ Create client**. **Application type:** **Desktop app**. **Name:** `Plenipo`. Click
   **Create**.
3. A box shows the **Client ID** (it ends in `.apps.googleusercontent.com`) and the **Client
   secret** (it often starts `GOCSPX-`). Keep this box open.

### C5 — Put them into Plenipo (never into a chat)

1. In Plenipo: **Settings → Connections** → **Google** → **Your Google app**.
2. Copy the **Client ID** from Google's box into **Client ID**.
3. Copy the **Client secret** from Google's box into **Client secret** (the box hides what you
   type).
4. Click **Save**. The card now says "Client ID: … Its secret is kept in Windows Credential
   Manager". The secret is never shown again.
5. Close Google's box.
6. Turn on the parts you want (Gmail, Calendar, Drive), pick **Who may use it**, and click
   **Connect**. Google's sign-in page opens in your browser. For a personal account, click
   **Advanced** → **Go to Plenipo (unsafe)** on the "Google hasn't verified this app" page; it is
   your own app.

**To change the app later:** Disconnect first, then **Remove this app**, then save the new one.

### What Plenipo asks Google for

Each only when its part is on, at that level:

| Permission                   | What it lets Plenipo do, as you        | Part and level        | Google calls it |
| ---------------------------- | -------------------------------------- | --------------------- | --------------- |
| `openid`, `email`, `profile` | Sign you in, see your name and address | always                | —               |
| `gmail.readonly`             | Read your Gmail                        | Gmail, Read only      | Restricted      |
| `gmail.compose`              | Save drafts and send them              | Gmail, Full access    | Restricted      |
| `calendar.events.readonly`   | Read your calendar                     | Calendar, Read only   | Sensitive       |
| `calendar.events`            | Read your calendar and add events      | Calendar, Full access | Sensitive       |
| `drive.readonly`             | Read your Drive                        | Drive, Read only      | Restricted      |
| `drive.file`                 | Add new files                          | Drive, Full access    | Not sensitive   |

Sending a draft, and inviting guests, still ask you first unless everyone is on that
connection's **Send without asking to** list and the switch is on.

## Safe to share, and secret

| Value                            | Safe to share?                                                                          | Where it goes                                 |
| -------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------- |
| A Slack app's **Client ID**      | Yes (public)                                                                            | GitHub variable, or the Slack card's Advanced |
| A Slack app's **Client Secret**  | Not needed; never use it                                                                | Nowhere                                       |
| A Google app's **Client ID**     | Yes (public)                                                                            | The Google card                               |
| A Google app's **Client secret** | Keep it private (Google says a desktop app's is not really secret, but treat it as one) | The Google card only (it goes to the Vault)   |

## Risks to know about

- **Client data goes to your AI company.** What a worker reads (a Slack message, an email, a
  file) is sent to the AI tool it runs on. Check your agreements with clients first.
- **Slack's terms:** a lawyer reads them before Plenipo Pro is sold with Slack (Part A).
- **Slack's sharing checklist** may refuse `http://localhost` sign-in addresses (A3). Then each
  workspace uses its own app.
- **Google's restricted permissions:** Gmail and Drive reading are "restricted". An app offered to
  the public needs Google's verification and, very likely, a yearly paid security assessment
  (CASA; about $675 to $6,000 or more a year). Your own Internal app, or an External app only you
  use, needs neither.
- **Google's user data policy:** never train AI models on the data (Plenipo trains none).

## Sources (checked 2026-09-29)

Slack: [using PKCE](https://docs.slack.dev/authentication/using-pkce) ·
[token rotation](https://docs.slack.dev/authentication/using-token-rotation) ·
[app manifests](https://docs.slack.dev/app-manifests/configuring-apps-with-app-manifests) ·
[distribution](https://docs.slack.dev/app-management/distribution/) ·
[auth.revoke](https://docs.slack.dev/reference/methods/auth.revoke) ·
[rate limits outside the Marketplace](https://docs.slack.dev/changelog/2025/05/29/rate-limit-changes-for-non-marketplace-apps/) ·
[API terms](https://slack.com/terms-of-service/api)

Google: [OAuth for desktop apps](https://developers.google.com/identity/protocols/oauth2/native-app) ·
[Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes) ·
[Calendar scopes](https://developers.google.com/workspace/calendar/api/auth) ·
[Drive scopes](https://developers.google.com/workspace/drive/api/guides/api-specific-auth) ·
[revoking a sign-in](https://developers.google.com/identity/protocols/oauth2/native-app#tokenrevoke) ·
[unverified apps](https://support.google.com/cloud/answer/7454865) ·
[testing status](https://support.google.com/cloud/answer/15549945) ·
[Workspace user data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)
