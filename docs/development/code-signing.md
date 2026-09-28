# Code signing

Every Plenipo release is signed as **8 West Ventures, LLC**, so Windows shows that name instead of
"Unknown publisher". Signing happens only in the Release workflow — never in CI, and never in a
local `pnpm build`.

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

## GitHub secrets

Repository **Settings → Secrets and variables → Actions**:

| Secret                | Where it comes from                                            |
| --------------------- | -------------------------------------------------------------- |
| `AZURE_TENANT_ID`     | App registration → Overview → Directory (tenant) ID            |
| `AZURE_CLIENT_ID`     | App registration → Overview → Application (client) ID          |
| `AZURE_CLIENT_SECRET` | App registration → Certificates & secrets → the secret's Value |

If any is missing, the Release workflow stops before building and says which one.

## Check signing without releasing

**Actions → Release → Run workflow**, pick any branch, tick **Dry run**. It builds, signs, checks
that the installer and `plenipo-desktop.exe` are validly signed by 8 West Ventures, LLC with a
timestamp, and keeps the installer as a download on the run page for 7 days. It creates no tag and
no release.

Run a dry run after changing anything here, and after renewing the client secret.

## Dates that stop signing

| When                                                 | What expires                 | Do                                                                       |
| ---------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------------ |
| The client secret's expiry (24 months from creation) | `AZURE_CLIENT_SECRET`        | New client secret, update the GitHub secret, dry run                     |
| 10/21/2028                                           | 8 West's identity validation | Renew in Azure (**Identity validations → Renew**); it signs Milepost too |

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
It goes in **Settings → Secrets and variables → Actions**:

| Name                                 | Kind     | What                                                      |
| ------------------------------------ | -------- | --------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | Secret   | Everything in `plenipo-updater.key` (the private half)    |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret   | Its password                                              |
| `PLENIPO_UPDATER_PUBLIC_KEY`         | Variable | Everything in `plenipo-updater.key.pub` (the public half) |

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
