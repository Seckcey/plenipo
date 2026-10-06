# Registering Plenipo's GitHub App — steps for 8 West

Written 2026-10-05 (Pacific time) for Phase 25 (ADR-204, a read-only GitHub connection to pick
repositories). Plenipo is made by 8 West Ventures, LLC. You do these steps yourself, in your own
browser, once. **Nothing here goes into a chat.** The only values you give are the App's
**Client ID** and its **short name**. Both are public (see "Safe to share, and secret" at the end).

GitHub renames buttons now and then. The names below are GitHub's own, checked in GitHub's docs on
2026-10-05. Sources are at the end.

## What this App can do, and what it never can

- **It can see:** your repositories' names and descriptions, their branch and tag names, and who
  collaborates on them. That is GitHub's **Metadata** permission, read only.
- **It never can:** read code, change anything, push, or see your email address. It has no other
  permission, and ADR-204 keeps it that way: any added permission needs a new decision record.
- **Your workers never use it.** They keep their own GitHub sign-in (the GitHub program `gh` and
  git), unchanged.

## Before you start

- **Who:** an owner of 8 West's GitHub organization, signed in with two-factor sign-in.
- **Where it lives:** under 8 West's GitHub organization, so it is 8 West's App, not a person's.

## Step 1 — Open the form (about 1 minute)

1. On GitHub, click your profile picture (upper right) → **Your organizations**.
2. Next to 8 West's organization, click **Settings**.
3. In the left menu, click **Developer settings** → **GitHub Apps**.
4. Click **New GitHub App**.

## Step 2 — Fill it in (about 5 minutes)

Fill in only these. Leave everything else as it is, except where this list says otherwise.

1. **GitHub App name:** `Plenipo by 8 West Ventures` (at most 34 characters). People see this name
   when they sign in. GitHub makes the **short name** from it, in small letters with dashes:
   `plenipo-by-8-west-ventures`. If GitHub says the name is taken, try `Plenipo by 8 West`.
2. **Description:** "Lists your GitHub repositories in Plenipo when you set up a project. Read
   only: names, descriptions, branch and tag names, and who collaborates; never code. Made by 8
   West Ventures, LLC."
3. **Homepage URL:** `https://getplenipo.com`
4. **Callback URL:** leave it **empty**. Plenipo signs in with a short code instead.
5. **Expire user authorization tokens:** leave it **ticked** (GitHub's default). A sign-in then
   lasts 8 hours and renews by itself for 6 months.
6. **Request user authorization (OAuth) during installation:** leave it **not ticked**.
7. **Enable Device Flow:** **tick it.** This is the short-code sign-in. Without it, Plenipo cannot
   sign in.
8. **Setup URL:** leave it empty. **Redirect on update:** not ticked.
9. **Webhook → Active:** **untick it.** Plenipo never receives anything from GitHub this way.
10. **Permissions:**
    - **Repository permissions:** leave every one at **No access**. **Metadata** shows
      **Read-only** by itself and cannot be changed: that is the one permission the App has.
    - **Organization permissions:** every one at **No access**.
    - **Account permissions:** every one at **No access**. In particular, **Email addresses** stays
      **No access**.
11. **Where can this GitHub App be installed?** Choose **Any account**, so each person can add it to
    their own account and their organizations.
12. Click **Create GitHub App**.

## Step 3 — Two values to note

You are on the App's **General** page.

1. **Client ID:** near the top. It looks like `Iv23li…` (an older App's looks like `Iv1.…`).
2. **Short name:** the last part of the App's public page, shown as **Public link**:
   `https://github.com/apps/<short name>`.

Do not click **Generate a new client secret**, and do not click **Generate a private key**. Plenipo
needs neither, and neither should exist.

## Step 4 — Put the two values where the Release workflow finds them

The Coordinator can do this step with your OK.

1. Open Plenipo's repository on GitHub → **Settings** → **Secrets and variables** → **Actions**.
2. Open the **Variables** tab (not **Secrets**: these values are public).
3. **New repository variable:** name `PLENIPO_GITHUB_CLIENT_ID`, value the Client ID. **Add
   variable.**
4. **New repository variable:** name `PLENIPO_GITHUB_APP_SLUG`, value the short name. **Add
   variable.**

The next release builds them in. Until then, the GitHub card says "This copy of Plenipo has no
GitHub app yet". The Release workflow refuses the tests' stand-in values (`Iv1.0123456789abcdef`
and `plenipo-test-app`), as it does for Microsoft's and Slack's.

## Step 5 — One real try (after the release)

1. In Plenipo: **Settings → Connections → GitHub → Sign in with GitHub.**
2. Plenipo shows a code and opens `github.com/login/device`. Type the code there, then
   **Authorize**.
3. The card says **Connected as** your name. Press **Choose on GitHub**, pick your account, choose
   **All repositories**, and **Install**. Do the same for 8 West's organization.
4. Back in Plenipo, press **Look again**. Your repositories show, private ones too.
5. Check: the accounts list shows no warning. A warning that an account's permissions are more
   than reading names means the App was given another permission: remove it on GitHub.

## What not to do

- Do not add any permission. Metadata (read only) is the only one, forever (ADR-204).
- Do not create a client secret or a private key.
- Do not turn on the webhook.
- Do not paste any password, token, or one-time code anywhere but GitHub's own pages.
- Only type a code into `github.com/login/device` that Plenipo just showed you.

## Safe to share, and secret

| Value                      | Secret?                     | Where it goes                             |
| -------------------------- | --------------------------- | ----------------------------------------- |
| Client ID (`Iv23li…`)      | **No** — public             | The repository variable; Plenipo's code   |
| Short name (`plenipo-by-…`) | **No** — public            | The repository variable; Plenipo's code   |
| App ID (a number)          | No, but not needed          | Nowhere                                   |
| A client secret            | **Yes** — do not create one | Nowhere. If one exists, delete it         |
| A private key (`.pem`)     | **Yes** — do not create one | Nowhere. If one exists, delete it         |
| Each person's sign-in      | **Yes**                     | Only in the Vault on that person's own PC |
| The short code             | **Yes, for 15 minutes**     | Only on GitHub's own page                 |

## Sources (checked 2026-10-05)

- [Registering a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/registering-a-github-app)
- [Generating a user access token for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
  (the device flow, and renewing without a secret)
- [Refreshing user access tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)
- [Choosing permissions for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app)
