# Plenipo's relay on 8 West's server (`relay.getplenipo.com`)

Plenipo on your phone (Phase 14) needs a **relay**: the small server in the middle that passes
sealed messages between your PC and your phones. Plenipo runs its own
([ADR-149](../../../docs/adr/ADR-149-plenipo-runs-its-own-relay.md), Plenipo runs its own relay).
Its code is `crates/relay` in this repository, and each release carries it, already built,
checked, and signed: `plenipo-relay-<version>-linux-x86_64`, its `.sig` (the signature, made with
8 West's server key;
[ADR-210](../../../docs/adr/ADR-210-the-page-and-the-relay-are-signed-like-the-installer.md), the
page and the relay are signed like the installer), and its `.sha256` (release 1.19.3 and later
carry the relay; the `.sig` came later). It runs on the server 8 West already has (the one that
runs Milepost's realtime service and other apps), behind that server's Nginx Proxy Manager. Made
by 8 West Ventures, LLC.

**In short:** one small program, as its own unprivileged user, in a systemd sandbox, listening on
this machine only. The proxy brings `relay.getplenipo.com` to it with TLS and WebSockets. Every
15 minutes, `update-relay.sh` checks GitHub for a new release, **checks its signature first** (8
West's server key, this file, this version), then the checksum and the program's own version (as
the relay's own user, never as root), switches to it, checks `/healthz`, and goes back if anything
is wrong. It never steps back to an older release on its own. The relay keeps nothing on disk and
holds no secret: it checks a PC's signature and 8 West's public license keys, and passes sealed
messages it cannot read.

**What it never does:** it never stops or changes another service on the machine, never touches
the firewall, DNS, or the proxy's other hosts, and never writes a message, a key, a pass, a
weekly answer, a mailbox name, or a challenge to a log.

Keep the server's address and sign-in out of this repository (ADR-040, ADR-146): these steps call
it "the server". Plenipo knows only the name `relay.getplenipo.com`.

## Set up (once)

You need: SSH to the server as root, the Nginx Proxy Manager page (port 81 on the server), and
Cloudflare for `getplenipo.com`. You do **not** need a copy of this repository on your PC. A
published release that carries the relay (1.19.3 or later) must exist first.

### 1. Get the setup files onto the server

Sign in to the server, become root (`sudo -i` if your prompt does not say `root`), and paste:

```sh
mkdir -p /root/plenipo-relay-deploy && cd /root/plenipo-relay-deploy
base=https://raw.githubusercontent.com/Seckcey/plenipo/main/crates/relay/deploy
for f in install-relay.sh update-relay.sh plenipo-relay.service plenipo-relay-update.service plenipo-relay-update.timer relay.env.example; do
  curl -fsSLO "$base/$f" || echo "FAILED: $f"
done
chmod +x install-relay.sh update-relay.sh
ls -la
```

You should see six files and no `FAILED` line.

### 2. The server key

The updater installs only a release that 8 West's **server key** signed (ADR-210). The key's
public half goes on the server once, by hand, in a folder only root may write. Never fetch it
from GitHub: take it from the owner's own `plenipo-server.key.pub` (its text is the repository
variable `PLENIPO_SERVER_SIGNING_PUBLIC_KEY`; how the owner makes it is in
[code signing → the server key](../../../docs/development/code-signing.md#the-server-key-adr-210)).

```sh
apt install -y minisign
install -d -m 0755 -o root -g root /etc/plenipo-relay/trusted-keys.d
nano /etc/plenipo-relay/trusted-keys.d/8west-server-2026.pub
```

Paste the public key's one line (the owner reads it to you, or sends it: it is not secret), save,
then:

```sh
chmod 0644 /etc/plenipo-relay/trusted-keys.d/8west-server-2026.pub
ls -la /etc/plenipo-relay/trusted-keys.d
```

The file is root's, `-rw-r--r--`. The folder may hold more than one key (`*.pub`): to change
keys, add the new one next to the old, release, then remove the old one. A key is accepted as the
owner's tool prints it (one base64 line) or as a minisign public key file.

### 3. Look first (changes nothing)

```sh
./install-relay.sh --check
```

If it stops with `jq is not installed` (or another tool), run `apt install -y jq` and check again.
It says whether the server key is in place; without it, the install refuses to start.

Read two parts of what it prints:

- **Listening ports (TCP):** if `:8790` is **not** in the list, use port `8790`. If it is, pick
  another free port (for example `8791`) and use it everywhere below.
- **The proxy (Nginx Proxy Manager): how it reaches this machine:**
  - `network mode host`: your address is `127.0.0.1:8790`.
  - Anything else (`bridge`, or a name like `something_default`): the proxy runs in a Docker
    container, which cannot reach the server's `127.0.0.1`. Your address is the Docker bridge
    number on the next line, without the `/16`, for example `172.17.0.1:8790`. It is a private
    address: only this machine and its containers can reach it. (The relay refuses any address the
    internet could reach.)

### 4. Install

Use the address from step 3:

```sh
./install-relay.sh --listen 172.17.0.1:8790
```

It adds the system user `plenipo-relay`, `/opt/plenipo-relay`, `/etc/plenipo-relay/relay.env`, and
the services, downloads the newest release's relay, checks its signature and the rest, and starts
it. The last line starts with `Done. The relay runs as plenipo-relay on …`.

**Already installed before the signatures (ADR-210)?** Put the key in place (step 2) and fetch
the new files (step 1). Before copying them, make sure they are what was merged:
`sha256sum update-relay.sh plenipo-relay-update.service` on the server must print the same sums
as those two files at the merged commit on GitHub (in a checkout:
`git show <commit>:crates/relay/deploy/update-relay.sh | sha256sum`, and the same for the
service file). Then copy them over the old ones, reload, and run the updater once through its
service, so its new sandbox is tried right away:

```sh
cp update-relay.sh /opt/plenipo-relay/deploy/update-relay.sh && chmod 0755 /opt/plenipo-relay/deploy/update-relay.sh
cp plenipo-relay-update.service /etc/systemd/system/plenipo-relay-update.service
systemctl daemon-reload
systemctl start plenipo-relay-update.service && journalctl -u plenipo-relay-update -n 20
```

A good run ends with `Up to date and healthy. Nothing to do.` (the newest full release is the one
running), or with `… is signed with 8 West's server key (8west-server-2026.pub) for X.Y.Z.` and
`Running vX.Y.Z.` (it installed one; this is the run that proves `runuser` works inside the
sandbox). Until a full release carries the signatures (the updater skips pre-releases), it ends
with `STOPPED: vX.Y.Z has no … .sig attached`, and the relay running now keeps running. Anything
else (`Permission denied`, a `runuser` or `systemctl` error, `Failed`): stop and ask; nothing has
changed yet.

### 5. Check it, on the server and from the proxy

```sh
systemctl status plenipo-relay --no-pager      # active (running)
curl -s http://172.17.0.1:8790/healthz          # ok   (your address)
docker exec npm curl -s -m 5 http://172.17.0.1:8790/healthz   # ok: the proxy can reach it
```

`npm` is the proxy's container name (the check's **Containers** list shows it). If that container
has no `curl`, use `docker exec npm wget -qO- -T 5 http://172.17.0.1:8790/healthz`. If it waits and
prints nothing, something blocks the proxy from reaching the relay: stop and ask.

If `systemctl status` shows the relay stopped with `SIGSYS` (the sandbox was too strict for this
server), run:

```sh
sed -i '/^SystemCallFilter=/d' /etc/systemd/system/plenipo-relay.service
systemctl daemon-reload && systemctl restart plenipo-relay
```

### 6. The name, in Cloudflare

**getplenipo.com → DNS → Records → Add record:** type `A`, name `relay`, the server's public IPv4
address (`curl -4 -s ifconfig.me` on the server prints it). Save, and wait about a minute.

Either cloud works:

- **Orange cloud (Proxied), the way it runs now:** tell the relay to read each phone's own address
  from Cloudflare, so its limits count people and not Cloudflare. On the server:

  ```sh
  sed -i 's/^PLENIPO_RELAY_CLIENT_ADDRESS=.*/PLENIPO_RELAY_CLIENT_ADDRESS=cloudflare/' /etc/plenipo-relay/relay.env
  systemctl restart plenipo-relay
  ```

- **Gray cloud (DNS only):** nothing to change; the default (`proxy`) is right.

### 7. The proxy host, in Nginx Proxy Manager

**Hosts → Proxy Hosts → Add Proxy Host.**

- **Details:** Domain Names `relay.getplenipo.com` (press Enter after typing it); Scheme `http`;
  Forward Hostname / IP `172.17.0.1` (your address, without the port); Forward Port `8790`;
  **Websockets Support on**; Block Common Exploits on; Cache Assets off.
- **SSL:** Request a new SSL Certificate; **Force SSL**, **HTTP/2 Support**, and **HSTS Enabled**
  on. Leave **Trust Upstream Forwarded Proto Headers** off. (Newer versions of Nginx Proxy Manager
  ask no question about Let's Encrypt's terms.)
- **Custom settings** (optional): newer versions keep them behind the **gear icon** at the top
  right of the window, not an Advanced tab. You can skip them: the relay and the PC ping every 30
  seconds, which keeps connections open.
- **Save.** If the certificate fails, wait a minute for the Cloudflare record and save again.

### 8. Check from outside

- In a browser: `https://relay.getplenipo.com/healthz` says `ok`, with a padlock.
- With Plenipo's own PC code (needs this repository and Rust on the computer that runs it):

  ```sh
  cargo run -p plenipo-relay --example probe -- https://relay.getplenipo.com
  ```

  Both lines say `good`: the relay answers `not_pro` to a PC whose weekly answer is signed with the
  contract's test key (the path works, and the lock on Pro is on), and `mailbox_closed` to a phone
  at a mailbox nobody opened.

### 9. Turn phone access on for everyone

On GitHub: the repository's **Settings → Secrets and variables → Actions → Variables** tab → **New
repository variable**: name `PLENIPO_RELAY_LIVE`, value `true`. The next release's switch says
**Use Plenipo from another device** instead of **Coming soon**. (Done on 2026-10-02; v1.19.4 is
that release.)

## Every day

- **Updates:** the timer runs `update-relay.sh` every 15 minutes. It installs a release only
  when its signature passes (`… is signed with 8 West's server key`), and stops, leaving the
  running relay alone, when it does not (`STOPPED: … is not signed with 8 West's server key`, or
  a signature for another file or version). A new release restarts the relay for a second or
  two: PCs reconnect by themselves; a phone reconnects when its page is opened.
  `journalctl -u plenipo-relay-update -n 30` shows what it did.
- **Health:** `curl -s http://127.0.0.1:8790/healthz` (or the address you chose) says `ok`. The
  relay logs its counts every 10 minutes: connections, PCs, phones, refusals, and connections
  turned away.
- **The off switch:** `touch /etc/plenipo-relay/off` closes every connection within a second and
  turns new ones away with `503` until `rm /etc/plenipo-relay/off`. `/healthz` says `off`
  meanwhile. (A PC shows "Plenipo couldn't reach 8 West's relay" in Settings → Devices and tries
  again on its own.)
- **Stopping for good:** `systemctl disable --now plenipo-relay plenipo-relay-update.timer`.

## Going back

`/opt/plenipo-relay/deploy/update-relay.sh --version 1.19.3` installs that release's relay and
switches to it (1.19.3 is the first with the relay; a release with no `.sig` is refused by the
updater of ADR-210 and later). Each release stays in its own folder under
`/opt/plenipo-relay/releases/`; nothing is deleted. The updater itself goes back on its own when
a new release fails its health check, and never to an older release than the one installed
unless you ask with `--version`.

## Settings

`/etc/plenipo-relay/relay.env` holds the settings; `plenipo-relay --help` lists them all, with
their defaults: the listen address, where the internet address of a connection is read (`proxy`,
`cloudflare`, or `peer`), the off file, the log level, and the limits (connections in all and per
address, new connections and refusals per address per minute, phones per PC, messages and bytes
per connection per minute, and the idle time). The defaults suit a small shared server.

## Memory

The program is about 2 MB and uses two worker threads; with a few hundred connections it needs
tens of megabytes. Its unit caps it at 128 MB (`MemoryMax`), so it can never crowd out the other
services on the machine; if it ever reached the cap, systemd would restart it, and the log would
say so.
