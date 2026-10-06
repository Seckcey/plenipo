# ADR-204: A read-only GitHub connection to pick repositories

- **Status:** Proposed (2026-10-05). The owner's request (I1, 2026-10-05): "When selecting a github
  repo on project setup, there needs to be a list of all the github repos for the users account.
  We may have to setup some kind of connection to github that we configure in settings >>
  connections." The owner approved the design's four recommendations the same day, and the
  reviewer's required changes G1 to G4 are folded in.
- **Date:** 2026-10-05
- **Phase:** 25 (fixes and a simpler Plenipo before launch)
- **Amends:** [ADR-062 (connections, one set of rules)](ADR-062-connections-one-set-of-rules.md)
  (a connection with no parts that no worker may use),
  [ADR-063 (signing in to a connection)](ADR-063-signing-in-to-a-connection.md) (the short-code
  sign-in, for GitHub only), and [ADR-068 (connections are Pro)](ADR-068-connections-are-pro.md)
  (the GitHub card is free)
- **Leaves alone:** [ADR-016 (the Development department)](ADR-016-development-department.md):
  workers keep the GitHub program `gh` and git with their own sign-in, and its turned-down
  "Plenipo's own GitHub token for the workers' tools" stays turned down.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): **Sign in with GitHub**, "Type this code on
> GitHub's page", **Copy the code**, **Open GitHub's page**, **Choose on GitHub**, **Add an account
> or organization**, **Look again**, "Yours alone", "Free". Never "OAuth", "device flow",
> "installation", "token", or "scope". This record keeps the code's words (`Service::Github`,
> `owner_only`, `SignInCode`).

## In short

Settings → Connections gets a **GitHub** card. You press **Sign in with GitHub**; Plenipo shows a
short code and opens GitHub's own page, you type the code there, and that's it. No password or key
ever goes into Plenipo. Then you choose, on GitHub, which accounts Plenipo may list (your own, and
each organization).

The sign-in is **read only**: Plenipo sees your repositories' **names, descriptions, branch and tag
names, and who collaborates; never code**. It is **yours alone** (no worker ever uses it) and
**free** with every plan. The list is for project setup: the repository box becomes a search box
(built next, in the second part), and Plenipo can make the copy on this PC (the third part).

Accepting this record means keeping 8 West's GitHub App at **Metadata: read** forever, signing in
to it with a short code, and the rules below for what Plenipo sends to GitHub and keeps.

## Context

- A project's repository is a typed address today (`projects.repository_url`), checked only for
  shape. There is no list, and no copy is made.
- Workers reach GitHub through the GitHub program `gh` and git, signed in on the PC (ADR-016 §6).
- GitHub's classic apps see private repositories only with `repo`, which reads and writes all code.
  A **GitHub App** with only **Metadata: read** sees private repositories' names and nothing of
  their code.
- GitHub's browser sign-in needs the app's secret, even with PKCE; a secret built into an installer
  anyone can download is not a secret. GitHub's **short-code sign-in** (the device flow) needs no
  secret, and its sign-in renews without one. GitHub recommends it for desktop tools.
- ADR-063 turned the short code down for Microsoft, whose admins block it as a phishing route.
  GitHub's case is different, and the risk is kept small (§2).

## Decision

### 1. 8 West's GitHub App: Metadata, read only, forever

1. **8 West Ventures, LLC registers one GitHub App** under its GitHub organization (the owner's
   steps: [docs/phases/phase-25-github-app.md](../phases/phase-25-github-app.md)). It has:
   - **Repository → Metadata: Read-only** and **nothing else**: no other repository permission, no
     organization permission, and **no account permission, so never Email addresses**;
   - the **short-code sign-in** (Device Flow) on, no callback address, the webhook off;
   - sign-ins that expire (8 hours) and renew (6 months), GitHub's default;
   - installable on any account; no client secret and no private key, ever.
2. **Pinned.** Any added permission (Contents, Email addresses, anything) is a new ADR, never a
   settings change on GitHub. Plenipo checks it too: each account's record on GitHub
   (`GET /user/installations`) carries its permissions, and an account whose permissions show
   more than `metadata: read` is **not listed**. The card names it and says why.
3. **What Metadata exposes,** said on the card word for word: "names, descriptions, branch and tag
   names, and who collaborates; never code".
4. **Its Client ID and short name are public** and built in like Microsoft's and Slack's app IDs:
   `option_env!("PLENIPO_GITHUB_CLIENT_ID")` and `option_env!("PLENIPO_GITHUB_APP_SLUG")`, from
   repository variables. CI builds the end-to-end copy with stand-in values; the Release workflow
   refuses them. A copy built without them says "This copy of Plenipo has no GitHub app yet".

### 2. Signing in with a short code (GitHub only; an exception to ADR-063)

1. **Sign in with GitHub** asks `POST https://github.com/login/device/code` with the Client ID
   only. The card shows the code, "Type this code on GitHub's page", and, word for word, "**Only
   type a code Plenipo just showed you here.**" Plenipo opens `https://github.com/login/device`
   itself (Guard checks it); **Open GitHub's page** opens it again, **Copy the code** copies it,
   and **Cancel** stops.
2. Plenipo asks `POST https://github.com/login/oauth/access_token` at GitHub's pace. It slows down
   when GitHub says so, and stops when the code runs out (15 minutes), when the owner says no,
   and on Cancel or Disconnect. Each sign-in takes a turn, as today, so a late answer keeps
   nothing.
3. **Only the page GitHub gives** is accepted: an answer that names any other page is refused.
4. **From the main window's card only, one at a time.** The command is the organization window's
   alone (IPC-tested against another window, the sign, and a web page). A second press while a
   code waits shows the same code. Another organization's window gets "Another GitHub sign-in is
   waiting in another organization's window. Finish or cancel it first." The code is shown only
   on the card: never in a notice, the Ledger, a log, or on the phone.
5. On success, `GET https://api.github.com/user` gives the account. The card says **Connected as**
   its name and user name.
6. **Renewing** needs no secret. The 8-hour sign-in is kept in memory only and renewed from the
   long-lived one in the Vault, only when the owner next asks for the list. A renewal GitHub
   refuses forgets the sign-in, and the card says GitHub needs you to sign in again.
7. **Disconnect** erases the Vault entry and the list. GitHub lets an app cancel a sign-in only
   with the app's secret, which Plenipo never has, so the card says where to finish: **Open
   Authorized GitHub Apps** (`github.com/settings/apps/authorizations`) and **Open Installed
   GitHub Apps** (`github.com/settings/installations`).

### 3. Yours alone: no parts, no worker tools, no lists

1. `Service::Github` has **no parts and no tools** (`tools_of` is empty; its tool prefix,
   `github_connection_`, never clashes with the workers' `github_` tools).
2. Guard refuses any line on its **Who may use it** and any entry on its **Send without asking
   to** ("GitHub is yours alone: no worker may use it"; "GitHub sends nothing: it only lists your
   repositories"). The card leaves those sections out.
3. Its requests start only from the owner's own commands in the main window: the card's sign-in,
   the card opening while connected, **Look again**, and (second part) the picker. It never asks
   GitHub in the background.

### 4. What Plenipo may send to GitHub (Guard)

1. **Hosts:** `github.com` and `api.github.com`, under `Purpose::Connection(Service::Github)`,
   https only, the default port, no name or password in the address.
2. **Paths, on top of the hosts** (`github_path_allowed`), so even a bug sends nothing else:
   - `github.com`: `/login/device/code` and `/login/oauth/access_token` (the sign-in), and the
     pages Plenipo opens in the browser: `/login/device`, `/apps/<short name>/installations/new`,
     `/settings/apps/authorizations`, `/settings/installations`;
   - `api.github.com`: `/user`, `/user/installations`,
     `/user/installations/<number>/repositories`, and `/user/repos`.
3. **No redirects at all for GitHub.** Other connections follow up to five, each checked; for
   GitHub any 3xx answer is refused and recorded as `guard.request_refused` with the host only.
4. **The sign-in goes only to `api.github.com`** (`api_hosts`). The two `github.com` sign-in
   addresses get the Client ID and the code in a form, asking for JSON, never a bearer.
5. **Pages Plenipo opens** (`open_github_page`) are four fixed names (`device`, `install`,
   `authorizations`, `installations`), never an address from the window.

### 5. What is kept

| What | Where | Never |
| --- | --- | --- |
| The long-lived sign-in | The Vault, as `connection-github-token`, under the organization's own Vault name (ADR-094 §9) | The Ledger, a prompt, a log, diagnostics, or a page |
| The 8-hour sign-in | Memory only | Saved anywhere |
| The connection's record (connected, who, when, `metadata:read`) | Guard's settings, like every connection | — |
| `connection.connected`, `connection.disconnected`, a failed or stopped sign-in | The Ledger, with no code and no sign-in | — |
| The list of accounts and repositories | Memory, **per organization**, for 10 minutes; another organization's window never sees it | Saved, logged, or given to a worker |

Both sign-ins feed the redactor, as every connection's do. **One GitHub sign-in per
organization**, like every connection (ADR-094): a client's organization can use another account.

The list keeps at most 100 accounts and 1,000 repositories, and only plain owner and repository
names (letters, digits, `-`, `_`, `.`; never `.` or `..` alone); a repository's address is always
built by Plenipo as `https://github.com/<owner>/<name>`, never copied from GitHub's text.

### 6. Free for everyone

The owner's decision, 2026-10-05. Listing repositories is part of setting up a project, and
ADR-068 already keeps GitHub's tools free. The GitHub card's Connect does not go through the Pro
check, and Settings → Connections says "except GitHub, which is free".

### 7. The picker and the copy (the second and third parts)

Decided now, built after this part:

1. **The picker** (second part): the repository box on New project, Edit project, and Set up a
   Development project searches the list, grouped by account, private ones marked. Typing or
   pasting any address still works, on any host. Not connected: the box stays as it is, with a
   link to the card. Project setup never waits on GitHub.
2. **Make a copy on this PC** (third part), only when the owner asks, with the address checked
   first and git run exactly as:

   ```
   git -c protocol.allow=never -c protocol.https.allow=always [-c protocol.ssh.allow=always]
       -c core.hooksPath= -c filter.lfs.smudge= -c filter.lfs.process= -c filter.lfs.required=false
       clone --no-recurse-submodules --progress -- <address> <folder>
   ```

   - The address is `https://` (or an `ssh` one only when the owner typed it), with a host, an
     owner, and a name and nothing more: no `http://`, `git://`, `file://`, `ext::`, local path,
     leading `-`, query, fragment, user name or password, or `https` port. `ext::` and `file::`
     stay refused by the check and by `protocol.allow=never`.
   - `GIT_LFS_SKIP_SMUDGE=1`; no submodules; no hooks; Plenipo's cleared environment and no
     terminal prompt.
   - The folder is the cleaned repository name (one folder name under `folder_name`'s rules,
     never GitHub's text), missing or empty, and passes ADR-205's folder checks.
   - One test for each refusal.

## As built

### Part 1 (A1): the connection and its card

- **Guard** (`crates/guard`): `Service::Github` (`owner_only`, `signs_in_with_a_code`, no parts);
  a waiting code may be shown again; `connection_hosts` and `github_path_allowed`;
  `set_connection_access` and `set_connection_send_list` refuse any line for an owner-only service;
  `Guard::record_refused_request` for the refused redirect.
- **Capabilities** (`crates/capabilities/src/connections/github.rs`, `mod.rs`, `http.rs`): the
  short-code sign-in, its renewal without a secret, Disconnect's note, the per-organization list
  (`github_repositories`), and `open_github_page`. One code at a time on the PC
  (`github::WAITING`). An account whose permissions show more than `metadata: read` is listed with
  the reason and its repositories are never asked for.
- **Desktop:** two new main-window commands, `list_github_repositories(fresh)` and
  `open_github_page(page)`, each IPC-tested (another window, the sign, and a web page are refused;
  an unknown page name is refused; nothing returns a sign-in). `connect_connection` and
  `cancel_connection_sign_in` serve GitHub as they serve the others. The card is
  `settings/connections/GithubCard.tsx`.
- **Workflows:** CI's end-to-end copy is built with `PLENIPO_GITHUB_CLIENT_ID=Iv1.0123456789abcdef`
  and `PLENIPO_GITHUB_APP_SLUG=plenipo-test-app`; the Release workflow passes the repository
  variables to every build and refuses those two values.
- **Tests:** a stand-in GitHub (`crates/capabilities/tests/support/github.rs`) answers like
  GitHub (200 with an `error` while it waits, a rotated sign-in on each renewal, no secret taken,
  pages of 100), and records whether a bearer ever reached `github.com`. Ten capability tests
  (`tests/github_connection.rs`) cover: the sign-in (a second press shows the same code, no bearer
  to `github.com`, JSON asked for, nothing in the record); the list (252 repositories across two
  accounts, an account with more permissions never asked about, kept ten minutes, **Look again**
  asks again); renewal with no secret, and a refused one; a sign-in that doesn't expire; a refused
  redirect, recorded; a **no**, a code that runs out, and a code for another page; slowing down,
  and a cancelled code; one sign-in at a time; Disconnect; and a copy with no GitHub App. Guard's
  tests cover the paths and the owner-only lists; the desktop's IPC test covers the commands;
  screen tests cover the card's states; and a real-app test signs in against the stand-in, lists
  the repositories, and disconnects.

Not in this part: **Your workers' GitHub sign-in** line on the card (the design's optional line,
`gh auth status`), the picker, and the copy.

## Consequences

- The owner signs in to GitHub once per organization, with no secret anywhere.
- A repository being in the list does not mean workers can push to it: workers keep their own
  sign-in, and opening a pull request still asks (ADR-016 §6).
- 8 West must register the App and set two repository variables before a release can sign in
  (the owner's step, with the Coordinator's help).
- A phishing message that sends the owner a GitHub code is answered by the card's words and by
  GitHub's own page, which names the App asking.

## Alternatives considered

- **A classic GitHub app with `repo`:** reads and writes all code. Turned down.
- **GitHub's browser sign-in:** needs the app's secret in the installer. Turned down.
- **A token the owner pastes:** one account per token, an organization may have to approve it, and
  the owner handles a secret. Turned down for the short code.
- **Reading the GitHub program's sign-in (`gh`)** as a fallback: reads and writes all code, and
  needs `gh` installed. The owner said no: one clear way.
