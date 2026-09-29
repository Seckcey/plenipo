# Plenipo marketing website

The public product site is built from `apps/website` for <https://plenipo.8westit.com>.
It serves static HTML, CSS, and JavaScript. The homepage includes an optional, self-hosted
React Flow sample. Repository website code adds no forms, accounts, tracking scripts, or billing
integrations. The public edge may insert the project's existing Cloudflare analytics script.
Plenipo's desktop app and its release workflow are unchanged.

## Interactive sample

The interactive homepage automatically opens its sample from approved source
`0a018ad3bb61b461de76928e1aef377dc98839a7`, deployed September 28, 2026. See the
[automatic-entry release receipt](website-releases/2026-09-28-autostart.md) for exact source/image
identity, public acceptance, and cleanup. The previous [click-entry release](website-releases/2026-09-28-interactive.md)
remains the immediate rollback. Installer metadata stays v1.6.0 through an explicit build argument;
this correction does not update the advertised installer.

The hero and download links render without JavaScript. A small loader in
`main.js` starts the React/React Flow island immediately on page entry, without a click, scroll,
or idle gate. The loader's
content-hashed JS and CSS are built with esbuild and served from the same origin. A readable
HTML example remains available without JavaScript, after a load/render failure, and through
**Return to the static example**. No desktop modules, commands, provider calls, storage, or
customer data are used. The permanent disclosure identifies it as a sample with no real AI.
Automatic mounting does not focus the sample or scroll the page. Retry/reactivation focuses it
only if the visitor is still on its button. Pending/ready loads reject duplicate starts; returning
to static stays static until explicitly reopened. Network transfer still takes time, so the
readable example and loading status remain visible until the sample is ready.

`apps/website/src/sample.ts` owns the fictional team, conversations, and activity. `Demo.tsx`
owns the map/list view, selected-person panel, story tabs, session switch, sample approval,
light/dark preview, and Pip guidance. Dragging changes positions only. Reporting lines cannot
be edited, and sample approval buttons cannot execute or publish anything. Decisions survive
story changes until reset or reload; nothing is persisted. The diagram refits on canvas size
changes so opening details keeps the team visible; a drag or pan does not trigger a refit.

Phone/coarse-pointer visitors start in the list view and can select the map. Keyboard users can
use the list, arrow/Home/End story tabs, and React Flow's focused-card controls. Page scrolling
is not captured by wheel zoom. Pinch, zoom buttons, minimap, Fit View, and Reset remain available.
There is no continuous animation or polling; reduced-motion CSS disables transitions. The
dark toggle applies to the sample window only.

The source imports React Flow's stylesheet before scoped demo CSS. Preserve the isolated
palette and the explicit SVG background override: the marketing page's global SVG stroke
otherwise darkens the dot pattern. Keep React Flow attribution visible.

### Reproducible builds

Workspace checks use the root `pnpm-lock.yaml`. The existing standalone Docker build context
is only `apps/website`, so it uses that folder's `package-lock.json` and `npm ci`. Both locks
are committed. Generate the npm lock in an empty temporary folder containing the website
`package.json`, not beside pnpm's installed symlinks; otherwise npm records workspace links.
Update both locks for dependency changes and validate both actual build contexts. The website
workflow installs the standalone dependencies before its Node tests and Docker build.

The build tests enforce local content-hashed demo assets, retained static disclosure, and gzip
budgets of 180,000 bytes of JS and 12,000 bytes of CSS. These are build-compression limits, not
network or speed guarantees. React Flow exceeds the earlier native-module proposal's 80 KB
target; the maintainers' explicit React Flow choice replaces that architecture. Measure actual
HTTP transfer sizes and interaction behavior as recorded in [acceptance](website-validation.md).
The corrected source requests demo JS/CSS on page entry; only the currently needed Pip pose loads.
Earlier click-activation measurements describe the initial implementation, not initial-page cost
after this correction. See the [automatic-entry acceptance record](../phases/website-autostart-acceptance.md)
for fresh-navigation measurements and fallback checks.

## Content and search

The page links to the published Windows installer, explains the current free release, and labels
the proposed Free/Pro limits as planned. Build-time version templating is used. Since
[ADR-069 (the website follows new releases)](../adr/ADR-069-website-follows-releases.md), Coastline
builds each new published release on its own (see [Automatic updates](#automatic-updates)).
Nobody types the version into the page: both download
buttons, both version labels, the structured data, and `release.json` come from the version the
build is given (the root `package.json`, or `PLENIPO_VERSION`). A merge containing a desktop
version bump does not mean that an installer has been published, so a deploy passes the release
it checked on GitHub Releases (below). Still check GitHub Releases before changing availability
or edition claims.

The Pip branding update aligns the page with the published
[v1.6.0 Windows release](https://github.com/Seckcey/plenipo/releases/tag/v1.6.0)
(published September 27, 2026, 10:10:51 UTC). Its tagged
[Kimi runtime](https://github.com/Seckcey/plenipo/blob/v1.6.0/crates/runtime/src/agent/kimi.rs) and
[acceptance record](https://github.com/Seckcey/plenipo/blob/v1.6.0/docs/phases/ai-tools-kimi-acceptance-report.md)
establish the Kimi/Moonshot entry; Ollama cloud support remains in adjacent text.
Both download buttons, both visible version labels, the structured data, and `release.json`
always show the same version: the one the build was given. Existing screenshot captions continue
to identify earlier app previews.

The initial content is present in HTML without JavaScript. The title, description, canonical URL,
Open Graph/Twitter metadata, SoftwareApplication structured data, `robots.txt`, sitemap, and
favicon set use the public hostname. There are no fabricated ratings or reviews. Local storage
and permissions do not imply offline AI: requests go to the selected AI provider, using the
person's existing sign-in and subject to that provider's plan limits.

Publish a crawlable HTTPS site through the project's Cloudflare Tunnel before submitting the
sitemap in Google Search Console. Search Console ownership verification and indexing submission
are separate account actions. Metadata helps discovery; it does not guarantee indexing or rank.
The GitHub About description and topics were improved by the maintainers. The homepage field
should point to the final hostname when its origin is ready. Do not change repository visibility,
license terms, the maintainers' profile avatars, or authentication settings as an SEO shortcut.

## Build and check

From the repository root:

```sh
pnpm --filter @plenipo/website test
SOURCE_REVISION="$(git rev-parse HEAD)" pnpm --filter @plenipo/website build
```

The build reads the version from the root `package.json` (or `PLENIPO_VERSION`), copies public
files to `apps/website/dist`, and writes `release.json`, which names its source, the version it
shows, and whether the page has that version's notes (`releaseNotes`).

The "What's new" section shows the notes of the version the page shows. The build looks for them
in `PLENIPO_RELEASE_NOTES` (a file it must be able to read), then
`apps/website/release-notes/vX.Y.Z.md` (where the automatic update puts them, because a container
build sees only `apps/website`), then the repository's `docs/releases/vX.Y.Z.md`. With no notes,
the section links to the release on GitHub. `scripts/release-notes.mjs` renders them: every
character is escaped, and only headings, paragraphs, lists, code, bold, and http(s) links are
rendered. The website workflow also builds and starts the production image with the root version,
checks health, the home page, sitemap, `release.json`'s version, and a genuine HTTP 404
response. Existing repository checks still cover formatting,
linting, the workspace, and the desktop application.

Container builds, tests, and previews for this site run on the Coastline server (`ssh coastline`),
never Docker Desktop. Use a separate task source directory and bounded CPU/RAM. Browser checks cover desktop,
mobile, menu/tabs/FAQ/links, console errors, and screenshot comparison against the selected visual
concepts. Clean up only the task's temporary containers and SSH forwarding process afterward.

## Coastline origin

The intended permanent service is:

| Setting                  | Value                                                         |
| ------------------------ | ------------------------------------------------------------- |
| Host                     | `ssh coastline` (verified hostname `coastline`)               |
| Deployment directory     | `/srv/8west/apps/plenipo-website`                             |
| Compose project          | `plenipo-website`                                             |
| Internal HTTP port       | `8080`                                                        |
| Host binding             | `127.0.0.1:14380` after live reservation/startup verification |
| Cloudflare Tunnel origin | `http://127.0.0.1:14380`                                      |
| Public hostname          | `plenipo.8westit.com`                                         |

`cloudflared` was verified as a host systemd service during setup. Recheck that topology before
using localhost: a connector inside another container has a different localhost. The origin is
HTTP; Cloudflare provides public HTTPS. The Tunnel hostname is set in Cloudflare, outside this
repository. An origin health
check does not establish public routing or Google indexing.

`compose.yaml` requires explicit `PLENIPO_IMAGE` and `PLENIPO_PORT`. The service runs as an
unprivileged user with a read-only filesystem, a 16 MiB temporary directory, dropped capabilities,
and a 64 MiB memory cap. Its restart policy is `unless-stopped`; it has no database or persistent
application data. Logs rotate. Nginx serves the static files with compression, security headers,
short asset caching, revalidated HTML, and a health check.

Coastline's automatic Docker network pools were exhausted during setup. Use the additional
`compose.coastline.yaml` with a freshly verified `PLENIPO_SUBNET`; the permanent service's chosen
subnet is `10.204.229.0/28`. Earlier temporary previews used `10.204.230.0/28` and
`10.204.231.0/28`; both are stopped and their task networks/reservations removed. Check all Docker networks
(including unused networks) and every host route table, including VPN routes, for overlap before
creating either. Record the app subnet under the same allocation lock. Do not remove another
project's network or change Docker's daemon pools to make room.

## Reserve, deploy, and roll back

Use the maintainers' `coastline` skill. The agreed shared allocation directory is
`/srv/8west/port-allocations`; hold exclusive `flock` on its persistent `allocations.lock` while
checking, reserving, and starting a new binding. Inspect IPv4/IPv6 listeners, every Docker
container including stopped allocations, existing reservation records, active release locks,
and host backup/build activity. Never reuse stopped Logbook or another project's ports/data.

Create an atomic task-owned JSON record containing the host/bind/ports, app and Compose project,
task owner, purpose, timestamp, and state before starting. Retain the lock through the actual
Docker bind and update the record with the healthy container/image identity. This serializes
participating deployers only; Docker's successful bind decides collisions with other processes.
Never remove the shared lock inode. Temporary previews use separate projects and directories;
remove only their own reservations after their containers cannot reclaim those ports. Historical
preview ports are not evidence of current availability.

After the coordinator merges the reviewed PR, export that exact commit into an immutable release
directory, build the image, and retain the archive hash, image ID, image revision label, and
`release.json` response together. For example, on Coastline with verified values:

The image has no `package.json` to read, so its build stops without `PLENIPO_VERSION`. Pass the
authorized published release, and check that its installer is there before building. The example
below selects the latest release; a narrowly scoped correction can explicitly retain its previous
version instead, as the automatic-entry release did with v1.6.0:

```sh
version="$(gh release view --repo Seckcey/plenipo --json tagName -q .tagName)"
version="${version#v}"
curl -fsIL "https://github.com/Seckcey/plenipo/releases/download/v$version/Plenipo_${version}_x64-setup.exe" >/dev/null
docker build --build-arg SOURCE_REVISION="$revision" \
  --build-arg PLENIPO_VERSION="$version" \
  -t "plenipo-website:$revision" "$release_dir/apps/website"
PLENIPO_IMAGE="plenipo-website:$revision" PLENIPO_PORT=14380 PLENIPO_SUBNET=10.204.229.0/28 \
  docker compose -p plenipo-website -f "$release_dir/apps/website/compose.yaml" \
  -f "$release_dir/apps/website/compose.coastline.yaml" up -d --wait
```

Keep the active image, source and Compose environment references in the application directory's
release record; keep the prior image/release for rollback. Confirm the container image matches
the build, localhost health and pages/assets return the intended release (`release.json` shows
that version), an unknown path is 404,
the port is loopback only, logs are clean, and restart count is stable. Record public acceptance
separately after the Tunnel is configured. For a failed update, point only this Compose project
back to the retained prior image/configuration and repeat health checks. Do not prune images,
stop other services, modify firewall/DNS, or restart the Tunnel as part of a website release.

## Automatic updates

[ADR-069 (the website follows new releases)](../adr/ADR-069-website-follows-releases.md): a systemd
timer on Coastline runs [`apps/website/deploy/auto-release.sh`](../../apps/website/deploy/auto-release.sh)
every 15 minutes. When GitHub's latest published release (not a draft or pre-release, with its
installer attached) is newer than what `release.json` shows, or that release's notes changed on
`main` since they were shown, it:

1. exports the website from `main` and the release's `docs/releases/vX.Y.Z.md` (from `main`, so
   a correction made after the release shows; from the release's tag if `main` has none) into a
   new read-only folder, `releases/<commit>-v<version>`;
2. builds `plenipo-website:<commit>-v<version>` with that version;
3. swaps the container while holding `/srv/8west/port-allocations/allocations.lock`;
4. checks health, the home page and its "What's new" for this version, `release.json`'s version,
   source, and notes, a real 404, the image, the restart count, and the loopback-only port;
5. if anything fails, puts the previous image back (with the Compose files it was started from)
   and checks its health.

It keeps `auto-release/current.env` (what runs now and what ran before), `auto-release/release.json`
(the live answer after the last update), and `auto-release/history.jsonl` (one line per update,
including roll-backs). It reads GitHub without a login, never prunes images, and never touches
other services, the firewall, DNS, or the Tunnel. It needs `curl`, `jq`, `git`, `docker` with
Compose, `flock`, and `sha256sum`, and says which is missing.

Options: `--check` (say what would happen, change nothing), `--force` (rebuild even if up to date),
`--version X.Y.Z` (show an older published release, for example to go back), and
`--source-ref REF` (website code from another branch, tag, or commit). Settings such as `PORT`,
`SUBNET`, and `ALLOCATION_LOCK` default to the values in [Coastline origin](#coastline-origin) and
can be changed in `/srv/8west/apps/plenipo-website/auto-release.env`.

### Install it on Coastline (once)

After this is merged to `main`, on Coastline, as an admin (`root` or with `sudo`). The script and
the timer run as the account that owns the app folder, never as root: files made by root would
lock that account out, and the script refuses to run as root.

```sh
ssh coastline
command -v jq || sudo apt install -y jq
cd /srv/8west/apps/plenipo-website
deploy_user="$(stat -c %U .)"                  # the account that owns the app folder
id -nG "$deploy_user" | grep -qw docker || echo "Add $deploy_user to the docker group first"
raw=https://raw.githubusercontent.com/Seckcey/plenipo/main/apps/website/deploy
sudo -u "$deploy_user" mkdir -p bin
sudo -u "$deploy_user" curl -fsSL "$raw/auto-release.sh" -o bin/auto-release.sh
sudo -u "$deploy_user" chmod 755 bin/auto-release.sh
sudo -u "$deploy_user" bin/auto-release.sh --check   # says what it would do; changes nothing
sudo -u "$deploy_user" bin/auto-release.sh           # the first update, now
curl -fsSL "$raw/plenipo-website-update.service" | sed "s/DEPLOY_USER/$deploy_user/" \
  | sudo tee /etc/systemd/system/plenipo-website-update.service > /dev/null
sudo curl -fsSL "$raw/plenipo-website-update.timer" -o /etc/systemd/system/plenipo-website-update.timer
sudo systemctl daemon-reload
sudo systemctl enable --now plenipo-website-update.timer
systemctl list-timers plenipo-website-update.timer
```

The list should show a time under **NEXT**, about 15 minutes away. To change the script later,
download it again the same way; a merge alone never changes what runs on Coastline.

If the first update was run as root by mistake, give the files back to the deploy account (the
names are this app's own; nothing else is touched), then start one run to check:

```sh
cd /srv/8west/apps/plenipo-website
sudo chown -R "$(stat -c %U .)": auto-release bin releases
sudo systemctl start plenipo-website-update.service
systemctl status plenipo-website-update.service --no-pager   # ends with "Up to date" or "Done"
```

### Check on it, pause it, go back

```sh
systemctl status plenipo-website-update          # the last run: finished, or why it stopped
journalctl -u plenipo-website-update -n 100      # what each run did
tail -n 5 /srv/8west/apps/plenipo-website/auto-release/history.jsonl
sudo systemctl disable --now plenipo-website-update.timer    # pause automatic updates
sudo -u "$(stat -c %U /srv/8west/apps/plenipo-website)" \
  /srv/8west/apps/plenipo-website/bin/auto-release.sh --version 1.10.0   # show an older release
```

Release folders are made read-only so nothing edits them after they are built; to delete an old
one, run `chmod -R u+w` on it first. Keep the current and previous ones (`auto-release/current.env`).

Going back to an older release while the timer is on lasts only until the next run, which shows
the latest release again; pause the timer first to stay on it.

See [brand assets](../brand/website-assets.md) for provenance and downloadable variants.
