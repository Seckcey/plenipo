# Code signing

Every Plenipo release is signed as **8 West Ventures, LLC**, so Windows shows that name instead of
"Unknown publisher". Signing happens only in the Release workflow — never in CI, and never in a
local `pnpm build`. The secrets that sign live in one GitHub Environment, `release`, which only
`main` and release tags may use and which waits for the owner's approval before each release run
(ADR-052, signing runs only for main and release tags, behind the owner's approval).

## What signs it

| Piece               | Value                                                                          |
| ------------------- | ------------------------------------------------------------------------------ |
| Service             | Azure Artifact Signing (formerly Trusted Signing)                              |
| Account             | `eightwest-signing` (resource group `rg-milepost-signing`)                     |
| Endpoint            | `https://eus.codesigning.azure.net`                                            |
| Certificate profile | `eightwest-public` — Public Trust                                              |
| Identity            | 8 West Ventures, LLC, verified; valid to **10/21/2028**                        |
| App registration    | `plenipo-github-signing`, role **Artifact Signing Certificate Profile Signer** |
| Tool                | `artifact-signing-cli` 0.11.0, run by Tauri's `signCommand`                    |
| Config              | `apps/desktop/src-tauri/tauri.signing.conf.json`                               |

Milepost signs with the same account and profile. The Basic plan includes one Public Trust profile
and 5,000 signatures a month, shared by both apps; a Plenipo release uses a handful.

The certificate behind the profile lasts only a few days and renews itself, so the expiry date on
the profile moves forward on its own. Every signature carries a timestamp from Microsoft, which
keeps it valid after that certificate expires; the Release workflow fails if one is missing.

## Where the secrets live: the `release` Environment

The five signing secrets are **Environment secrets**, not repository secrets. A job can read them
only when it names the Environment `release`, only from `main` or a `v*` tag, and only after you
have approved the run. A repository secret, by contrast, can be read by a workflow run on any
branch — the workflow file on that branch decides what happens to it — with nobody asked.

| Secret                               | Where it comes from                                                                           |
| ------------------------------------ | --------------------------------------------------------------------------------------------- |
| `AZURE_TENANT_ID`                    | App registration → Overview → Directory (tenant) ID                                           |
| `AZURE_CLIENT_ID`                    | App registration → Overview → Application (client) ID                                         |
| `AZURE_CLIENT_SECRET`                | App registration → Certificates & secrets → the secret's Value                                |
| `TAURI_SIGNING_PRIVATE_KEY`          | The updater key's private half ([the updater key](#updates-the-updater-key-phase-13-adr-038)) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Its password                                                                                  |

If any is missing, the Release workflow stops at the step **Signing secrets are set** and says
which one (`Environment secret … is not set`). Until the Environment is set up as below and holds
all five, every release run stops there as soon as the repository-level copies are gone.

### Set it up once (about 15 minutes)

1. **Make the `release` Environment.** On GitHub, go to **Seckcey/plenipo** → **Settings** (the
   tab with the gear, top right of the repository) → in the left list, **Environments** → **New
   environment**. Name: `release` → **Configure environment**.
2. **Required reviewers.** Tick **Required reviewers** and add yourself. Leave **Prevent
   self-review** unticked: you approve the runs you start yourself. **Save protection rules**.
3. **Only `main` and release tags.** Under **Deployment branches and tags** ("deploy" is GitHub's
   word for using the Environment), choose **Selected branches and tags**, then **Add deployment
   branch or tag rule** twice: **Ref type** `Branch` with the name pattern `main`, and **Ref
   type** `Tag` with the name pattern `v*`.
4. **The secrets.** Lower on the same page, under **Environment secrets**, **Add environment
   secret** five times: the five names above, each with its value. Copy the updater key's two
   values the way [its steps](#step-by-step-once-about-10-minutes) show.
5. **Make the `dry-run` Environment.** **New environment**, name `dry-run`, **Configure
   environment**, and change nothing: no reviewers, no rules, no secrets. (If a dry run happens
   first, GitHub makes it on its own, empty, which is the same thing.)
6. **Delete the repository-level copies.** **Settings → Secrets and variables → Actions**, tab
   **Secrets**: for each of the five names, the delete (trash) icon next to it, then confirm.
   Until they are gone, every branch can still read them, and the Environment protects nothing.
   The variable `PLENIPO_UPDATER_PUBLIC_KEY` on the **Variables** tab stays: it is not secret.
7. **Protect the release tags** ([below](#protect-the-release-tags-once)).
8. **Try it.** A dry run needs no approval: **Actions → Release → Run workflow**, tick **Dry
   run**. A green run means the build still works. The next real release then waits for your
   approval ([below](#approving-a-release-run)) and signs with the Environment's secrets.

## Approving a release run

Every release run (Run workflow on `main`, or a pushed `vX.Y.Z` tag) waits before its job starts;
GitHub emails you. Open the run (**Actions → Release** → the run) and click **Review
deployments**, tick `release`, and click **Approve and deploy**: only then does the job read the
secrets, build, sign, and publish. **Reject** ends it. Nobody else can approve, and a run that is
not approved never sees a secret.

Approve only a run you started yourself and expect: a Run workflow release on `main`, or a tag
you pushed. Reject anything else, and look at who started it.

## Protect the release tags (once)

A tag ruleset stops anyone but a repository admin (you) from moving or deleting a release tag, so
a release always points at the commit it was built from. **Settings → Rules → Rulesets → New
ruleset → New tag ruleset**:

- **Ruleset name:** `release-tags`. **Enforcement status:** `Active`.
- **Bypass list → Add bypass:** `Repository admin`, **Always allow**.
- **Target tags → Add a target → Include by pattern:** `v*`.
- **Rules:** tick **Restrict updates** and **Restrict deletions**. Leave **Restrict creations**
  unticked: on a Run workflow release, the workflow itself creates the tag with its own GitHub
  token, which is not an admin and which GitHub gives no way past that rule, so ticking it fails
  every Run workflow release at its last step. A tag anyone else pushes only starts a run that
  waits for your approval and goes nowhere without it. (If you only ever release by pushing the
  tag yourself, you can tick it too.)
- **Create**.

## Dry run: check the build, not the signing

**Actions → Release → Run workflow**, pick any branch, tick **Dry run**. It needs no approval and
touches no secret: it builds the app and the NSIS installer **unsigned**, checks nothing about
signing, and keeps the installer as the download `plenipo-unsigned-installer-dry-run` on the run
page for 7 days. It creates no tag and no release. Use it to see that a branch still builds an
installer.

Only a real release signs. To check signing after a change here (a renewed client secret, a new
updater key) without shipping a full release, release a pre-release version such as `X.Y.Z-rc.1`
([versioning → releasing](versioning.md#releasing)): it is published as a GitHub pre-release,
marked as such, and installed copies never offer it as an update.

## Dates that stop signing

| When                                                 | What expires                 | Do                                                                                     |
| ---------------------------------------------------- | ---------------------------- | -------------------------------------------------------------------------------------- |
| The client secret's expiry (24 months from creation) | `AZURE_CLIENT_SECRET`        | New client secret, update the Environment secret, then release (a pre-release will do) |
| 10/21/2028                                           | 8 West's identity validation | Renew in Azure (**Identity validations → Renew**); it signs Milepost too               |

Microsoft emails reminders 60 days before the identity validation expires. The client secret sends
none — keep it in a calendar.

## After each release: SmartScreen

A signature replaces "Unknown publisher" at once. Windows' blue "Windows protected your PC" screen
can still appear for a new release until enough people have downloaded it; no certificate skips
that. To speed it up, submit the signed installer at
[microsoft.com/wdsi](https://www.microsoft.com/wdsi) as a software developer.

## Updates: the updater key (Phase 13, ADR-038)

Plenipo installs an update only when it carries a second signature: the **updater key**'s, over
the installer and the version it claims. This key is separate from the Azure signing above, and
only the owner holds it. How to make it once is in
[ADR-038 (updates), section 6](../adr/ADR-038-updates.md#6-what-the-owner-does-once-the-updater-key).
Its two secret values go in the `release` Environment with the Azure ones (ADR-052; ADR-038 said
repository secrets, which every branch could read). Its public half is not secret and stays a
repository variable:

| Name                                 | Kind                | What                                                      |
| ------------------------------------ | ------------------- | --------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | Environment secret  | Everything in `plenipo-updater.key` (the private half)    |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Environment secret  | Its password                                              |
| `PLENIPO_UPDATER_PUBLIC_KEY`         | Repository variable | Everything in `plenipo-updater.key.pub` (the public half) |

### Step by step (once, about 10 minutes)

Never paste the key or its password into a chat, an issue, or a file in the repository.

1. **Make the key.** Open PowerShell (Start → type "PowerShell" → Enter) and run:

   ```powershell
   npx --yes @tauri-apps/cli@2.11.5 signer generate -w "$env:USERPROFILE\.tauri\plenipo-updater.key"
   ```

   It needs Node.js (the same one used to build Plenipo). It asks for a password twice: make a
   strong one and save it in your password manager now. You get two files in
   `C:\Users\<you>\.tauri\`: `plenipo-updater.key` (secret) and `plenipo-updater.key.pub`
   (not secret). (In the `plenipo` folder after `pnpm install`,
   `pnpm --filter @plenipo/desktop tauri signer generate -w "$env:USERPROFILE\.tauri\plenipo-updater.key"`
   does the same.)

2. **Open the `release` Environment.** On GitHub, go to **Seckcey/plenipo** → **Settings** (the
   tab with the gear, top right of the repository) → in the left list, **Environments** →
   `release` (make it first, as [above](#set-it-up-once-about-15-minutes), if it is not there).
3. **The private key.** Under **Environment secrets**, choose **Add environment secret**. Name:
   `TAURI_SIGNING_PRIVATE_KEY`. For the value, copy the key without showing it on screen:

   ```powershell
   (Get-Content "$env:USERPROFILE\.tauri\plenipo-updater.key" -Raw).Trim() | Set-Clipboard
   ```

   Click in the **Value** box, press Ctrl+V, then **Add secret**.

4. **Its password.** **Add environment secret** again. Name: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
   Value: the password from step 1. **Add secret**.
5. **The public key.** It is not secret, so it is a repository variable: **Settings → Secrets and
   variables → Actions** → the **Variables** tab → **New repository variable**. Name:
   `PLENIPO_UPDATER_PUBLIC_KEY`. Copy the value:

   ```powershell
   (Get-Content "$env:USERPROFILE\.tauri\plenipo-updater.key.pub" -Raw).Trim() | Set-Clipboard
   ```

   Paste it, then **Add variable**.

6. **Back it up.** Save `plenipo-updater.key` (the file, or its text) in your password manager
   next to the password. Losing either means one manual install for everyone.
7. **Check it.** A dry run does not sign (ADR-052), so the first release after this is the check.
   To check without shipping a full release, release a pre-release version (`X.Y.Z-rc.1`,
   [versioning → releasing](versioning.md#releasing)) and approve its run: a green run means the
   key signs and the check accepts it. Installed copies never offer a pre-release as an update.

The Release workflow builds the public half into Plenipo, signs the installer with the private
half (`tauri signer sign --app-version`, after the 8 West signature), checks that signature and the
version in it before publishing, and attaches `Plenipo_<version>_x64-setup.exe.sig` and
`latest.json` to the release. Each copy of Plenipo reads `latest.json` from the newest release.
If any of the three is missing, the Release workflow stops before building and says which.

- **Losing the private key or its password:** copies already installed refuse updates signed
  with a new key, so everyone installs the next version by hand once. Keep a backup in your
  password manager.
- **Leaking it:** someone could make an update Plenipo accepts (it would still have to be served
  from Plenipo's GitHub Releases). Make a new key, replace the three values, and release at once;
  tell users to install that release by hand.
- **CI and the E2E tests** make a throwaway key in each run and build a copy that trusts it and
  looks for updates on `127.0.0.1` only, so they can install a test update. That copy is never a
  release; its installer (a CI download) is for testing only.
