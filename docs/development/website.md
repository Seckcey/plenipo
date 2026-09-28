# Plenipo marketing website

The public product site is built from `apps/website` for <https://plenipo.8westit.com>.
It is a static HTML, CSS, and JavaScript application with no production JavaScript dependencies,
forms, account system, tracking scripts, or billing integration. Plenipo's desktop app and its
release workflow are unchanged.

## Content and search

The page links to the published Windows installer, explains the current free release, and labels
the proposed Free/Pro limits as planned. Nobody types the version into the page: both download
buttons, both version labels, the structured data, and `release.json` come from the version the
build is given (the root `package.json`, or `PLENIPO_VERSION`). A merge containing a desktop
version bump does not mean that an installer has been published, so a deploy passes the release
it checked on GitHub Releases (below). Still check GitHub Releases before changing availability
or edition claims.

The Pip branding update aligns the page with the published
[v1.6.0 Windows release](https://github.com/Seckcey/plenipo/releases/tag/v1.6.0)
(published September 27, 2026, 10:10:51 UTC). Its tagged
[Kimi runtime](https://github.com/Seckcey/plenipo/blob/v1.6.0/crates/runtime/src/agent/kimi.rs) and
[owner acceptance record](https://github.com/Seckcey/plenipo/blob/v1.6.0/docs/phases/ai-tools-kimi-acceptance-report.md)
establish the Kimi/Moonshot entry; Ollama cloud support remains in adjacent text.
Both download buttons, both visible version labels, the structured data, and `release.json`
always show the same version: the one the build was given. Existing screenshot captions continue
to identify earlier app previews.

The initial content is present in HTML without JavaScript. The title, description, canonical URL,
Open Graph/Twitter metadata, SoftwareApplication structured data, `robots.txt`, sitemap, and
favicon set use the public hostname. There are no fabricated ratings or reviews. Local storage
and permissions do not imply offline AI: requests go to the selected AI provider, using the
person's existing sign-in and subject to that provider's plan limits.

Publish a crawlable HTTPS site through the owner's Cloudflare Tunnel before submitting the
sitemap in Google Search Console. Search Console ownership verification and indexing submission
are separate account actions. Metadata helps discovery; it does not guarantee indexing or rank.
The GitHub About description and topics were improved in the owner's browser. The homepage field
should point to the final hostname when its origin is ready. Do not change repository visibility,
license terms, the owner's profile avatar, or authentication settings as an SEO shortcut.

## Build and check

From the repository root:

```sh
pnpm --filter @plenipo/website test
SOURCE_REVISION="$(git rev-parse HEAD)" pnpm --filter @plenipo/website build
```

The build reads the version from the root `package.json` (or `PLENIPO_VERSION`), copies public
files to `apps/website/dist`, and writes `release.json`, which names its source and the version
it shows. The website workflow also builds and starts the production image with the root version,
checks health, the home page, sitemap, `release.json`'s version, and a genuine HTTP 404
response. Existing repository checks still cover formatting,
linting, the workspace, and the desktop application.

On Frankie's machines, container builds/tests/previews run on `ssh coastline`, never Docker
Desktop. Use a separate task source directory and bounded CPU/RAM. Browser checks cover desktop,
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
HTTP; Cloudflare provides public HTTPS. Frankie configures the Tunnel hostname. An origin health
check does not establish public routing or Google indexing.

`compose.yaml` requires explicit `PLENIPO_IMAGE` and `PLENIPO_PORT`. The service runs as an
unprivileged user with a read-only filesystem, a 16 MiB temporary directory, dropped capabilities,
and a 64 MiB memory cap. Its restart policy is `unless-stopped`; it has no database or persistent
application data. Logs rotate. Nginx serves the static files with compression, security headers,
short asset caching, revalidated HTML, and a health check.

Coastline's automatic Docker network pools were exhausted during setup. Use the additional
`compose.coastline.yaml` with a freshly verified `PLENIPO_SUBNET`; the permanent service's chosen
subnet is `10.204.229.0/28`. The temporary preview uses `10.204.230.0/28`. Check all Docker networks
(including unused networks) and every host route table, including VPN routes, for overlap before
creating either. Record the app subnet under the same allocation lock. Do not remove another
project's network or change Docker's daemon pools to make room.

## Reserve, deploy, and roll back

Use the owner's `coastline` skill. The agreed shared allocation directory is
`/srv/8west/port-allocations`; hold exclusive `flock` on its persistent `allocations.lock` while
checking, reserving, and starting a new binding. Inspect IPv4/IPv6 listeners, every Docker
container including stopped allocations, existing reservation records, active release locks,
and host backup/build activity. Never reuse stopped Logbook or another project's ports/data.

Create an atomic task-owned JSON record containing the host/bind/ports, app and Compose project,
task owner, purpose, timestamp, and state before starting. Retain the lock through the actual
Docker bind and update the record with the healthy container/image identity. This serializes
participating deployers only; Docker's successful bind decides collisions with other processes.
Never remove the shared lock inode. Temporary preview 14381 uses a separate project and directory;
remove only its reservation after its containers and reclaimable configuration are removed.

After the coordinator merges the reviewed PR, export that exact commit into an immutable release
directory, build the image, and retain the archive hash, image ID, image revision label, and
`release.json` response together. For example, on Coastline with verified values:

The image has no `package.json` to read, so its build stops without `PLENIPO_VERSION`. Pass the
latest published release, and check that its installer is there before building:

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

See [brand assets](../brand/website-assets.md) for provenance and downloadable variants.
