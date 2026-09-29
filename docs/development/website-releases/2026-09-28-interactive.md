# Interactive homepage production release — September 28, 2026

The maintainers authorized deployment of the approved PR #87 homepage. It is live at
<https://plenipo.8westit.com/>. This release uses the exact merged source below; later main-branch
installer-version templating and unrelated app/security changes are excluded. It does not deploy
the desktop app. Existing v1.6.0 download metadata was deliberately retained within this scope.
GitHub's official non-prerelease v1.10.0 installer was separately verified as published on
September 28, 2026, at 09:03:19 UTC, with its installer, signature, and updater manifest. The
deployed v1.6.0 link is older; this release does not claim it is the latest official installer.
Updating that metadata remains a separate work item.

| Identity                 | Verified value                                                            |
| ------------------------ | ------------------------------------------------------------------------- |
| Source                   | `155ad42cd7830976400bad4535beda5297de6435`                                |
| Source archive SHA-256   | `1c318f3da97c80914d7ffb1ae1c49ad4af3161d4e716bd65f0fc8343da9e649c`        |
| Image                    | `plenipo-website:155ad42cd7830976400bad4535beda5297de6435`                |
| Image ID                 | `sha256:ec9fa84e16458beda272619df3b0af3e727fe5a1a6db58b881926ceb85bc81c9` |
| Container                | `5bfe088e40aaecc354305593256bb3e37893d59727ab764d3ec627b4cec41da2`        |
| Host / Compose project   | `coastline` / `plenipo-website`                                           |
| Origin                   | `http://127.0.0.1:14380`                                                  |
| Release HTTP acceptance  | September 28, 2026, 13:14:42 UTC                                          |
| Final runtime acceptance | September 28, 2026, 13:18:33 UTC                                          |

Both post-merge workflows passed for the exact source: full CI run `36407180028` and Website run
`36407180108`. The archived source was transferred with matching SHA-256; all 1,575 extracted
files matched archive bytes before and after the bounded Coastline image build. The image label,
image ID, built `release.json`, actual runtime, origin response, and public response agree.

The existing host `cloudflared` service, loopback binding, network `10.204.229.0/28`, security
headers, unprivileged user `10001:10001`, read-only filesystem, 64 MiB limit, and `unless-stopped`
policy are retained. There are no application data volumes or migrations. Deployment held the
persistent allocation mutex and rechecked the prior container/image/record and public source before
replacement. No other service, DNS/Tunnel setting, desktop Docker, or unrelated checkout changed.

## Acceptance

- Origin and public HTTPS report source `155ad42...`, health `ok`, the intended page, and a real
  404 for an unknown route. All 30 checked asset URLs match the built files byte for byte,
  including the hashed demo bundle/style, Pip images, brand ZIP, metadata, and fonts.
- Origin HTML exactly matches the build. Public HTML differs only by the existing Cloudflare
  beacon script and whitespace; removing that known insertion gives the same page. No analytics
  configuration was changed.
- Live Chromium at 1440 px and 390 px exercised activation, six cards, supervisor details,
  conversation switching, sample approval/activity/reset, themes, keyboard movement, phone list
  selection, and static return. Phone page width remained within its viewport. The coordinator
  separately verified the public conversation flow and all six cards.
- A browser-only blocked module produced readable fallback and an enabled retry. Restoring the
  resource and retrying mounted successfully. A subsequent ordinary load passed. Normal browser
  runs had no warning/error entries; deliberately induced blocked-resource errors are separate.
- No demo resource was requested before activation. Resource inspection found no provider or
  application fetches; the only external resource origin observed was the existing Cloudflare
  analytics host. Sample approvals send and publish nothing.
- Final runtime was healthy with zero restarts. All 95 inspected container log lines had zero
  warning/error/critical/alert/emergency entries. Only sanitized counts are retained in the receipt.

Actual public-page captures:
[desktop](../../brand/website-evidence/interactive-live-desktop.png) and
[phone](../../brand/website-evidence/interactive-live-phone.png).
Earlier [performance measurements](../website-validation.md) remain explicit limitations;
deployment does not turn them into broader performance or cross-browser guarantees.

## Rollback and durable evidence

The previous `16cc6f3aef736a35ee67b1b56c2c41b24128a304` source, image, and configuration remain
available. Its image ID is `sha256:df63afe8210bfacae9c3ff97953c56b5ed978f9868f4fdcb89b3dbaecd7594ee`.
No rollback was needed. Under the same allocation/release ownership gates, restore that release
with its own Compose files and the retained `rollback.env`, then repeat origin/public acceptance
and update the active pointer, release record, and allocation record to the actual restored state.
Do not apply newer main-branch build arguments to this older release.

Permanent release directory:
`/srv/8west/apps/plenipo-website/releases/155ad42cd7830976400bad4535beda5297de6435`.
It contains `source.tar`, `source-manifest.json`, `build.log`, `build-receipt.json`, `built-site`,
`deploy.env`, `rollback.env`, `previous-release-record.json`, `previous-allocation.json`,
`release-record.json`, `runtime-final-check.json`, and `evidence/browser-live.json` plus captures.
The application-level `current` symlink, `current.env`, `release-record.json`, and permanent
14380 allocation record point to this release. Environment files retain restricted permissions.

The build/extraction container was removed. No new preview or SSH forward was created; the earlier
14382 review preview remains retired. The worker's QA tab was closed and temporary browser settings
were restored; the coordinator retains its public-page deliverable tab. Source, images, logs,
screenshots, rollback material, and permanent mutex files are retained. Allocation/heavy mutex
inodes remain `6161580` / `5785265`, with no held task lock after completion.

Release executor: worker `01a0e6d4-bf94-7dc1-b9b3-6c51e826b44e`. The coordinator owns this
documentation PR's review and merge; a later docs merge is not a later production release.
