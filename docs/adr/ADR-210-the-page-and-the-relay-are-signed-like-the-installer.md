# ADR-210: The page and the relay are signed like the installer

- **Status:** Accepted (2026-10-04). Proposed on 2026-10-03 by the builder. It fixes finding
  **P-SRV-1** (High) of the [October 2 security report](../../PLENIPO-SECURITY-REPORT.MD) and
  **P-WEB-3** (Info) from the same file. The owner's steps are in
  [code signing → the server key](../development/code-signing.md#the-server-key-adr-210).
- **Date:** 2026-10-03
- **Phase:** security follow-up (after Phase 14 and Phase 23)
- **Amends:**
  - [ADR-146 (where the phone's page lives)](ADR-146-where-the-phone-page-lives.md) §2: the page's
    files are not only built by the release and updated from it; the release **signs** them, and
    the page's server serves only what the signature proves is 8 West's.
  - [ADR-149 (Plenipo runs its own relay)](ADR-149-plenipo-runs-its-own-relay.md) §6 and §7: the
    release attaches a signature next to the relay's program; the updater checks it first, and
    the downloaded program runs as the relay's own user, never as root.
  - [ADR-038 (updates)](ADR-038-updates.md) §2: a third signature joins the two, made with a key
    of its own, the **server key**, for the files the servers install.
  - [ADR-052 (release signing behind the owner's approval)](ADR-052-release-signing-environment.md)
    §1 and §5: the Environment `release` holds two more secrets, the server key and its password,
    and the step "Signing secrets are set" names them.
- **Keeps:** everything else in those four decisions; ADR-040 §5 and ADR-146 §4 (the servers'
  addresses stay out of this repository); the desktop installer's own signing, unchanged.

> **On screen** (ADR-010, plain words and rank names): nothing changes for a person using
> Plenipo. The server's logs say "is signed with 8 West's server key" or "STOPPED: … is not
> signed with 8 West's server key".

## In short

The relay and the phone's page were installed from each GitHub release with a **checksum**,
which proves only that a download was not damaged, not who made it. Now each release also
carries a **signature** for both, made with a key only the owner holds (the **server key**), and
each server installs a file only when the signature proves 8 West made it, for exactly that file
and that version. The relay's updater no longer runs a download as root. Neither updater takes a
step back to an older version on its own.

## Context

- Finding P-SRV-1: anyone who could publish a release on this repository (a stolen token, a
  compromised build, a collaborator's account) got, within 15 minutes, a program of their choice
  running as root on 8 West's shared server, and a page of their choice at
  `remote.getplenipo.com`. The page is what every paired phone trusts (ADR-146: "the same way you
  trust Plenipo's installer"), and the installer _is_ signed (ADR-038), while the page and the
  relay were not.
- The desktop updater already checks a minisign signature with a key built into Plenipo, and the
  version written inside it (ADR-038 §2). The same tool, the same format, and the same kind of
  key fit the servers, which can check them with `minisign`, a small standard program.
- Finding P-WEB-3: the Release workflow gave every job leave to write to the repository, when
  only the publishing job needs it.
- The website (`apps/website`) is built on its server from `main` once the checks passed at that
  commit. There is no release file to sign; its safety is branch protection on `main` (finding
  G-1) and pinned base images (finding P-WEB-2), which are their own work.

## Decision

1. **A key of its own: the server key.** The owner makes a second updater-style key
   (`tauri signer generate`, a minisign key with a password) and keeps it as they keep the
   updater key: the private half and its password as Environment secrets in `release`
   (`PLENIPO_SERVER_SIGNING_KEY`, `PLENIPO_SERVER_SIGNING_KEY_PASSWORD`), the public half as the
   repository variable `PLENIPO_SERVER_SIGNING_PUBLIC_KEY`, and a backup in their password
   manager. **Never the updater key:** with one key for both, a validly signed installer could
   be uploaded under the relay's file name and a server would accept it. The workflow refuses to
   run when the two keys are the same.
2. **The release signs both files.** After the owner's approval (ADR-052), the Windows job signs
   `plenipo-relay-<version>-linux-x86_64` and `plenipo-phone-page-<version>.zip` with the server
   key, over their final bytes, writing the file's name and the version inside the signature
   (`tauri signer sign --app-version`), checks each signature against the public half before
   publishing, and attaches `….sig` next to each file. The `.sha256` stays, as a check against a
   damaged download.
3. **The servers trust the public half, installed once by hand.** Each server keeps it in a
   folder root owns and only root may write: `/etc/plenipo-relay/trusted-keys.d/*.pub` on the
   relay's server, `/etc/plenipo-phone-page/trusted-keys.d/*.pub` on the page's. A key is the
   line `tauri signer generate` printed, or a minisign public key file. No key is ever fetched
   from the network, and with no key in the folder, or a folder anyone else could write, the
   updater installs nothing.
4. **The signature is the gate.** Each updater downloads the file, its `.sig`, and its
   `.sha256`, and before anything else checks that the signature was made with a trusted key over
   that very file, and that the name and version written inside it are this file's name and the
   release's version. Only then the checksum, the content checks, and the install. A failed check
   stops the run with a plain reason and leaves nothing behind.
5. **Never as root.** The relay's updater runs the downloaded program's `--version` check as the
   relay's own user (`runuser -u plenipo-relay`), in a folder only root and that user may enter,
   with a time limit. The page's updater already never runs as root.
6. **Never a step back on its own.** Both updaters remember the version installed and refuse a
   "latest" release that is older. The operator steps back on purpose with `--version`.
7. **The Release workflow's permissions** (P-WEB-3): every job reads; only the publishing job
   writes.
8. **Replacing the key.** The folders hold more than one key. To change keys, the owner puts the
   new public half next to the old one, then signs the next release with the new key, and removes
   the old `.pub` when no older release needs it. A leaked key is removed at once and a release
   signed with a new key follows; a version already installed stays until the next good release
   replaces it, because the updaters only move forward.
9. **Tested.** `scripts/updater-signing-check.sh` runs both updaters on Linux against a stand-in
   for GitHub with a throwaway key: signed → installed and served; no signature, another key,
   another file's name, another version, a damaged download → refused before the file is used at
   all; an older release → refused unless `--version`; the relay's program never runs as root;
   no key, or a loose key folder → nothing installed. CI runs it on every change.

## Consequences

- Publishing a release is no longer enough to change what the servers run: it takes the server
  key too, which lives only behind the owner's approval. A stolen token, a run nobody approved, or
  a collaborator's account cannot reach the servers.
- What it does not stop: whoever controls an approved release run (the workflow file on `main`
  at that commit, and the job's dependencies) can sign. Branch protection on `main` and the
  owner's approval of each run are what guard that.
- **The order of the owner's steps matters.** First the key and its secrets, then a release that
  carries `.sig` files, then the new updater scripts and the public key on each server. New
  scripts on a server before a signed release exists refuse every release (safe: the current
  version keeps running); a release before the key exists stops at "Signing secrets are set".
- Both servers need `minisign` installed (`apt install minisign`), an operator step written in
  each README. The updater says so if it is missing.
- The website's updater is unchanged. It builds `main` after the checks pass; what keeps `main`
  honest is branch protection (G-1), not this decision.

## Alternatives considered

- **Reuse the updater key.** One key fewer to keep. Rejected: a signed installer could then be
  passed off as a relay program or a page, and the version check alone does not tell them apart.
  Checking the file name inside the signature helps, but a key of its own makes the meaning of
  the signature plain: "this is a file for 8 West's servers".
- **GitHub's build attestations** (`actions/attest-build-provenance`, checked with
  `gh attestation verify`). Strong, and they name the exact workflow run. They need a signed-in
  `gh` on each server and more permissions in the workflow, and they trust GitHub's own identity
  service rather than a key the owner holds. Possible later, next to the signature, not instead
  of it.
- **Sign by hand on the owner's PC** and upload the `.sig`. Keeps the key off GitHub entirely,
  but every release would wait for a manual step that is easy to get wrong. Not recommended; the
  Environment behind the owner's approval already keeps the key out of any run nobody approved.
- **Fetch the public key from the repository** on each server run. Convenient, but then whoever
  changes the repository changes what the server trusts, which is the weakness being fixed.
