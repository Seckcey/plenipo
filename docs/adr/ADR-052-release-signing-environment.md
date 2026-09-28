# ADR-052: Signing runs only for main and release tags, behind the owner's approval

- **Status:** Accepted (by the owner, 2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 13 (follow-up)
- **Builds on:** [ADR-038 (updates)](ADR-038-updates.md), whose section 6 put the updater key in
  repository secrets and had a dry run prove that it signs; both change here

## Context

The Release workflow signs the installer and the app as 8 West Ventures, LLC with three Azure
secrets, and signs the installer again with the updater key (two more secrets) so that installed
copies accept it as an update. All five were repository secrets. A repository secret can be read
by a workflow run on any branch, and the workflow file on that branch says what happens to it. A
run on any `v*` tag signed with them, a dry run on any branch signed with them and kept the signed
installer as a download for 7 days, nobody had to approve, and a pushed tag published a release on
its own. So anyone or anything that can push to the repository — a collaborator, an AI tool
working in a session with push rights, a stolen token — could get code signed in 8 West's name
without the owner knowing, and could publish a release.

GitHub Environments are made for this. An Environment holds its own secrets, can be limited to
named branches and tags, and can name required reviewers: a job that uses it then waits for their
approval before it starts, and reads a secret only after. A tag ruleset keeps anyone but an admin
from moving or deleting a tag.

## Decision

1. **The signing secrets live in the Environment `release`.** All five: `AZURE_TENANT_ID`,
   `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `TAURI_SIGNING_PRIVATE_KEY`, and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The Environment lets only `main` and tags `v*` use it,
   and has the owner as its required reviewer, so every release run waits for the owner's
   approval before its job starts. The repository-level copies are deleted; until they are, every
   branch can still read them and the Environment protects nothing. The updater key's public half
   stays a repository variable: it is not secret. (The finding named the three Azure secrets; the
   updater key goes too, because it signs the installer as well, and a copy of it that any branch
   can read is the same weakness.)
2. **The Release job names its Environment.** `release` for a release, `dry-run` for a dry run:
   `environment: ${{ inputs.dry_run == true && 'dry-run' || 'release' }}`. `dry-run` holds
   nothing and asks nobody.
3. **A dry run builds an unsigned installer.** From any branch, with no approval, it skips the
   secrets check, the signing tool, the signed build, the updater signature, the updater check,
   and the signature check; it runs `tauri build` without the signing config and keeps the
   installer as the download `plenipo-unsigned-installer-dry-run` for 7 days. It publishes
   nothing. So a dry run proves that a branch builds an installer, not that signing works: the
   first real release proves that, and a pre-release version (`X.Y.Z-rc.1`, published as a GitHub
   pre-release, which installed copies never offer as an update) proves it without shipping a
   full release. This amends ADR-038, section 6, step 4 ("Check it"): a dry run no longer signs.
4. **Release tags are protected.** A tag ruleset on `v*` restricts updates and deletions to
   repository admins, so only the owner can move or delete a release tag, and a release always
   points at the commit it was built from. Creating a `v*` tag stays open: on a Run workflow
   release, the workflow itself creates the tag with its own GitHub token, which is not an admin
   and which GitHub gives no way past that rule, so restricting creations would fail every Run
   workflow release at its last step. A tag anyone else pushes starts a run that waits for the
   owner's approval and goes nowhere without it. The owner may restrict creations too, if
   releases are only ever made by pushing the tag by hand.
5. **The step "Signing secrets are set" names Environment secrets.** Until the Environment holds
   the five secrets, a release run stops at that step and says which one is missing. The step
   "Run workflow releases `main` only" stays as the plain-words reason when a run is started on
   another branch.

## Consequences

- No installer is signed, and no release is published, without the owner clicking **Approve and
  deploy** on the run page. The owner must judge each request: approve only a run they started
  themselves (Run workflow on `main`, or a tag they pushed), and reject anything else.
- A branch cannot reach a signing secret, whatever its workflow file says; nor can a dry run.
  Contributors and AI tools with push rights can build an installer, never sign one.
- A dry run no longer checks signing. After renewing the Azure client secret or making a new
  updater key, the check is the next release, or a pre-release.
- A release has one more click. GitHub cancels a run that waits too long for approval; start it
  again.
- The Environment set-up is the owner's to do on GitHub
  ([code signing → where the secrets live](../development/code-signing.md#where-the-secrets-live-the-release-environment)).
  A release run before that: while the repository-level secrets are still there (GitHub makes
  `release` on its own, empty, the first time a run names it), it signs as before, with nobody
  asked; once they are gone, it stops at "Signing secrets are set".
- A tag anyone else pushes leaves a junk tag for the owner to delete (an admin may).

## Alternatives considered

- **Keep repository secrets and only refuse other branches inside the workflow.** The workflow
  file on a branch can be changed, so a check inside it protects nothing; the Environment's rule
  is GitHub's, outside the run.
- **Restrict creating `v*` tags to admins as well.** It would stop a stranger's tag before any run
  starts, but a Run workflow release creates its tag with the workflow's own token, which no
  bypass covers; releases would then need the owner's personal token in the workflow (a second
  long-lived credential, one that lets an approved run act as the owner) or the owner tagging by
  hand every time. The approval already covers a stranger's tag; the option stays open to the
  owner.
- **A signed dry run behind the same approval.** The owner would approve every test build, and a
  signed installer would again sit on a run page from any branch. An unsigned dry run needs no
  approval and no secret.
- **A separate workflow for dry runs.** Two files to keep in step; one job with two Environments
  says it in one place.
