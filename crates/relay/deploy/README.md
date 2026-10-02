# Plenipo's relay on 8 West's server (`relay.getplenipo.com`)

Plenipo on your phone (Phase 14) needs a **relay**: the small server in the middle that passes
sealed messages between your PC and your phones. Plenipo runs its own
([ADR-149](../../../docs/adr/ADR-149-plenipo-runs-its-own-relay.md), Plenipo runs its own relay).
Its code is `crates/relay` in this repository, and each release carries it, already built and
checked: `plenipo-relay-<version>-linux-x86_64` and its `.sha256` (release 1.19.3 and later). It
runs on the server 8 West already has (the one that runs Milepost's realtime service and other
apps), behind that server's Nginx Proxy Manager. Made by 8 West Ventures, LLC.

**In short:** one small program, as its own unprivileged user, in a systemd sandbox, listening on
this machine only. The proxy brings `relay.getplenipo.com` to it with TLS and WebSockets. Every
15 minutes, `update-relay.sh` checks GitHub for a new release, checks the checksum and the
program's own version, switches to it, checks `/healthz`, and goes back if anything is wrong. The
relay keeps nothing on disk and holds no secret: it checks a PC's signature and 8 West's public
license keys, and passes sealed messages it cannot read.

**What it never does:** it never stops or changes another service on the machine, never touches
the firewall, DNS, or the proxy's other hosts, and never writes a message, a key, a pass, a
weekly answer, a mailbox name, or a challenge to a log.

Keep the server's address and sign-in out of this repository (ADR-040, ADR-146): these steps call
it "the server". Plenipo knows only the name `relay.getplenipo.com`.

## Before you start: look

The server is shared and short on memory. Look first, and keep what you saw. From a copy of this
folder on the server (as root):

```sh
cd /root/plenipo-relay-deploy   # wherever you copied crates/relay/deploy
./install-relay.sh --check
```

It prints, and changes nothing: memory and swap, disk, every listening port, every running
service, the containers, and **how Nginx Proxy Manager reaches this machine** (its network mode,
and this machine's address on the Docker bridge). The real run writes the same to
`/opt/plenipo-relay/looked-<time>.txt`.

**Pick the port.** The default is `8790`. If the list of listening ports shows `8790` in use, pick
another free one and pass it with `--listen`.

**Pick the address.** Nginx Proxy Manager usually runs in a Docker container. A container cannot
reach the host's `127.0.0.1`, so:

- If the proxy's network mode is `host`: use `127.0.0.1:8790`. The proxy forwards to
  `127.0.0.1`.
- If the proxy is on a Docker bridge (network mode `bridge`, or a named network): use this
  machine's address on that bridge, which the check prints (usually `172.17.0.1`), for example
  `--listen 172.17.0.1:8790`. The proxy forwards to that address. It is a private address: only
  this machine and its containers can reach it, never the internet. The relay refuses to listen
  on any address the internet could reach.

## Set up (once, as root)

1. **Copy this folder to the server**, for example from your PC:

   ```powershell
   scp -r crates\relay\deploy milepost-rt:/root/plenipo-relay-deploy
   ```

2. **Look, then install** (the install needs the first release that carries the relay, 1.19.3,
   to be published):

   ```sh
   cd /root/plenipo-relay-deploy
   chmod +x install-relay.sh update-relay.sh
   ./install-relay.sh --check
   ./install-relay.sh --listen 127.0.0.1:8790     # or --listen 172.17.0.1:8790, see above
   ```

   It adds the system user `plenipo-relay`, `/opt/plenipo-relay` (the program, one folder per
   release), `/etc/plenipo-relay/relay.env` (the settings), the units `plenipo-relay.service`,
   `plenipo-relay-update.service`, and `plenipo-relay-update.timer`, downloads the newest
   release's program, checks it, and starts the relay. Then:

   ```sh
   systemctl status plenipo-relay --no-pager
   curl -s http://127.0.0.1:8790/healthz      # ok   (use the address you chose)
   journalctl -u plenipo-relay -n 20 --no-pager
   ```

   If the status shows the program stopped with `SIGSYS` (the sandbox's system-call list was too
   strict for this kernel), remove the two `SystemCallFilter=` lines from
   `/etc/systemd/system/plenipo-relay.service`, then `systemctl daemon-reload && systemctl
   restart plenipo-relay`.

3. **The proxy host, in Nginx Proxy Manager** (the web page on port 81):
   - **Hosts → Proxy Hosts → Add Proxy Host.**
   - **Details:** Domain Names `relay.getplenipo.com`; Scheme `http`; Forward Hostname / IP
     `127.0.0.1` or `172.17.0.1` (the address you chose); Forward Port `8790`; **Websockets
     Support on**; Block Common Exploits on; Cache Assets off.
   - **SSL:** Request a new SSL Certificate (Let's Encrypt), **Force SSL** on, **HTTP/2** on,
     **HSTS** on. (The certificate request needs the Cloudflare record below to exist first and
     to point straight at the server, grey cloud; make the record, then come back and request the
     certificate.)
   - **Advanced** (optional, keeps idle connections open longer than the proxy's default 60
     seconds; the relay and the PC ping every 30 seconds either way):

     ```nginx
     proxy_read_timeout 300s;
     proxy_send_timeout 300s;
     ```

   - Save.

4. **The name, in Cloudflare** (the owner, signed in): **getplenipo.com → DNS → Add record**:
   type `A`, name `relay`, the server's public IPv4 address, proxy status **DNS only** (grey
   cloud) to begin with, so Let's Encrypt can check the name and WebSockets go straight to the
   proxy. (If the server has an IPv6 address the proxy listens on, add an `AAAA` record too.)
   Then request the certificate in step 3.

   If you later turn the Cloudflare proxy on (orange cloud): set
   `PLENIPO_RELAY_CLIENT_ADDRESS=cloudflare` in `/etc/plenipo-relay/relay.env` and
   `systemctl restart plenipo-relay`, so the limits count each phone's own address and not
   Cloudflare's.

5. **Check from outside**, from your PC, with Plenipo's own PC code:

   ```powershell
   cargo run -p plenipo-relay --example probe -- https://relay.getplenipo.com
   ```

   A healthy relay answers `not_pro` to a PC whose weekly answer is signed with the contract's
   test key (the path works, and the lock on Pro is on), and `mailbox_closed` to a phone at a
   mailbox nobody opened. `https://relay.getplenipo.com/healthz` says `ok` in a browser.

6. **Turn phone access on for everyone:** set the repository variable `PLENIPO_RELAY_LIVE` to
   `true` on GitHub (Settings → Secrets and variables → Actions → Variables). The next release's
   switch says **Use Plenipo from another device** instead of **Coming soon**.

## Every day

- **Updates:** the timer runs `update-relay.sh` every 15 minutes. A new release restarts the
  relay for a second or two: PCs reconnect by themselves; a phone reconnects when its page is
  opened. `journalctl -u plenipo-relay-update -n 30` shows what it did.
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
switches to it (1.19.3 is the first with the relay). Each release stays in its own folder under
`/opt/plenipo-relay/releases/`; nothing is deleted. The updater itself goes back on its own when
a new release fails its health check.

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
