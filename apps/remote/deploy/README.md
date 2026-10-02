# The phone's page on its own server (`remote.getplenipo.com`)

Plenipo on your phone (Phase 14) is a page your phone opens at `remote.getplenipo.com`. Each
release carries it, already built and checked: `plenipo-phone-page-<version>.zip` and its `.sha256`
(release 1.19.2 and later). It is served from **a small AWS server of its own**
([ADR-148](../../../docs/adr/ADR-148-the-phone-page-on-its-own-server.md), the phone's page on its
own server; [ADR-146](../../../docs/adr/ADR-146-where-the-phone-page-lives.md), where the phone's
page lives), through a Cloudflare Tunnel. Made by 8 West Ventures, LLC.

**In short:** a small web server on that server serves the page on the server itself only
(`127.0.0.1`); every 15 minutes, `update-page.sh` checks GitHub for a new release, checks the
page's checksum and that it names only 8 West's relay, switches to it, checks it, and goes back if
anything is wrong. The Tunnel brings `remote.getplenipo.com` to it. Nothing else runs there, and it
holds no secrets but the Tunnel's token.

The page works for real phones only when 8 West's relay answers at `relay.getplenipo.com` too
(see the [relay change request](../../../docs/phases/phase-14-relay-change-request.md)).

Keep the server's address out of this repository (as ADR-100 does for the account service's
server); these steps call it "the page's server".

## 1. Make the server (once, in AWS)

In AWS, in the same region as 8 West's other servers, launch one EC2 server:

- **Name** `plenipo-remote`; **image** Ubuntu Server 24.04 LTS, 64-bit (Arm); **type** `t4g.nano`.
- **Disk:** 8 GB gp3, encrypted. **Termination protection** on. **Metadata:** IMDSv2 required.
- **Security group** of its own: only SSH, only from 8 West's office address. No web ports: the
  Tunnel needs none.
- **A public IPv4 address**: the updater reaches GitHub over it.
- **Key pair:** 8 West's usual one.

It costs about $7 to $8 a month (ADR-148). Optionally, add a status-check alarm that tells the same
alerts topic as the account service's server.

## 2. Set it up (once, on the server)

Sign in to the page's server as `ubuntu` (never run the updater as root).

1. **Swap and Docker:**

   ```sh
   sudo fallocate -l 1G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile
   echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
   sudo apt-get update && sudo apt-get install -y docker.io docker-compose-v2 curl jq unzip git
   sudo usermod -aG docker ubuntu
   ```

   Then sign out and in again, so `docker` works without `sudo`.

2. **The Tunnel**, with nothing secret typed or pasted anywhere:
   - Install `cloudflared` from Cloudflare's own package source (`pkg.cloudflare.com`, as
     Cloudflare's instructions for Ubuntu say).
   - Run `cloudflared tunnel login` on the server. It prints a link: the owner opens it while
     signed in to Cloudflare, picks **getplenipo.com** only, and clicks **Authorize**.
   - Then, on the server:

     ```sh
     cloudflared tunnel create plenipo-remote
     cloudflared tunnel route dns plenipo-remote remote.getplenipo.com
     ```

   - Move the Tunnel's own key (`~/.cloudflared/<id>.json`) to `/etc/cloudflared/` (root only,
     `chmod 600`), and write `/etc/cloudflared/config.yml`: the Tunnel's ID, that key, and one rule,
     `remote.getplenipo.com` → `http://127.0.0.1:8080`, with everything else answered 404.
   - `sudo cloudflared service install`, then **delete `~/.cloudflared/cert.pem`**: it could make
     tunnels and DNS records for the whole zone, and the server needs only its own Tunnel's key.

3. **The folder and the files** (from this repository's `main` branch):

   ```sh
   sudo mkdir -p /srv/8west/apps/plenipo-phone-page
   sudo chown ubuntu: /srv/8west/apps/plenipo-phone-page
   git clone --depth 1 https://github.com/Seckcey/plenipo /tmp/plenipo
   mkdir -p /srv/8west/apps/plenipo-phone-page/deploy
   cp /tmp/plenipo/apps/remote/deploy/* /srv/8west/apps/plenipo-phone-page/deploy/
   chmod +x /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh
   echo 'PORT=8080' > /srv/8west/apps/plenipo-phone-page/update-page.env
   rm -rf /tmp/plenipo
   ```

4. **Try it, then run it once:**

   ```sh
   /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh --check
   /srv/8west/apps/plenipo-phone-page/deploy/update-page.sh
   curl -sI http://127.0.0.1:8080/ | head
   ```

5. **The timer:** copy `plenipo-phone-page-update.service` and `.timer` to `/etc/systemd/system/`,
   put `ubuntu` in place of `DEPLOY_USER`, then:

   ```sh
   sudo systemctl daemon-reload && sudo systemctl enable --now plenipo-phone-page-update.timer
   ```

6. **Check from outside:** `https://remote.getplenipo.com/healthz` answers `ok`, and
   `https://remote.getplenipo.com/` shows **Pair this phone**.

## On a shared machine

On a machine that runs other services, pick an unused loopback port for `PORT`. If its automatic
Docker address pool is full, set `SUBNET` to a reserved network that overlaps nothing (the updater
then adds `compose.subnet.yaml`), and if the machine has a lock file that its services take before
they pick ports and networks, set `ALLOCATION_LOCK` to it.

## Going back

`update-page.sh --version 1.19.2` serves an earlier release (1.19.2 is the first with the page).
Each release stays in its own folder under `releases/`; nothing is deleted.

## What it never does

It never stops other services, and never touches the firewall, DNS, or the Tunnel. It serves only
files from a release that GitHub published, whose checksum matches, and which names 8 West's relay
and no test address.
