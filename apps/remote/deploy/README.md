# The phone's page on Coastline (`remote.getplenipo.com`)

Plenipo on your phone (Phase 14, [ADR-146](../../../docs/adr/ADR-146-where-the-phone-page-lives.md),
where the phone's page lives) is a page your phone opens at `remote.getplenipo.com`. Each release
carries it, already built and checked: `plenipo-phone-page-<version>.zip` and its `.sha256`
(release 1.19.2 and later). Coastline serves it next to the website, through the same Cloudflare
Tunnel. Made by 8 West Ventures, LLC.

**In short:** a small web server on Coastline serves the page on this computer only
(`127.0.0.1`); every 15 minutes, `update-page.sh` checks GitHub for a new release, checks the
page's checksum and that it names only 8 West's relay, switches to it, checks it, and goes back if
anything is wrong. The Tunnel brings `remote.getplenipo.com` to it.

The page works for real phones only when 8 West's relay answers at `relay.getplenipo.com` too
(see the [relay change request](../../../docs/phases/phase-14-relay-change-request.md)).

## Set up (once)

Everything runs as the account that owns `/srv/8west/apps` and may use Docker (never as root),
except installing the timer. Containers and tests on Coastline follow its own rules: a separate
folder, a uniquely named Compose project (`plenipo-phone-page`), and an unused loopback port.

1. **Pick a port and a network**, as the website did: an unused loopback port (for example
   `14381`; check `ss -ltn` and Coastline's port allocations) and a reserved subnet that overlaps
   no Docker network or host route (`docker network inspect`, `ip route`, the VPN's routes).
2. **Make the folder and copy the files** (from a checkout of this repository at a release tag):

   ```sh
   mkdir -p /srv/8west/apps/plenipo-phone-page/deploy
   cp apps/remote/deploy/* /srv/8west/apps/plenipo-phone-page/deploy/
   chmod +x /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh
   ```

3. **Its settings**, in `/srv/8west/apps/plenipo-phone-page/update-page.env`:

   ```sh
   PORT=14381
   SUBNET=10.204.229.16/28
   ```

4. **Try it, then run it once:**

   ```sh
   /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh --check
   /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh
   curl -sI http://127.0.0.1:14381/ | head
   ```

5. **The timer** (as root): copy `plenipo-phone-page-update.service` and `.timer` to
   `/etc/systemd/system/`, put the account's name in place of `DEPLOY_USER`, then
   `systemctl daemon-reload && systemctl enable --now plenipo-phone-page-update.timer`.
6. **The name, in Cloudflare** (the owner): in the Tunnel that serves the website, add the public
   hostname `remote.getplenipo.com` to `http://127.0.0.1:14381` (the port from step 1).
7. **Check from outside:** `https://remote.getplenipo.com/healthz` answers `ok`, and
   `https://remote.getplenipo.com/` shows **Pair this phone**.

## Going back

`update-page.sh --version 1.19.2` serves an earlier release (1.19.2 is the first with the page).
Each release stays in its own folder under `releases/`; nothing is deleted.

## What it never does

It never stops other services, and never touches the firewall, DNS, or the Tunnel. It serves only
files from a release that GitHub published, whose checksum matches, and which names 8 West's relay
and no test address.
