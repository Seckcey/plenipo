# ADR-149: Plenipo runs its own relay, from this repository, on 8 West's server

- **Status:** Accepted (by the owner, 2026-10-02: "Plenipo runs its own relay. All its code lives
  in this repository, and it is deployed over SSH to the Akamai/Linode server … Do not touch
  Milepost's repository.").
- **Date:** 2026-10-02
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Amends:** [ADR-143 (the relay and the lock)](ADR-143-the-relay-and-the-lock.md): "8 West's
  relay, the one Milepost uses" becomes **Plenipo's own relay**, built in this repository. Everything
  ADR-143 says about what the relay does and never does (§1 to §3, §10 to §14) stands; it is now
  Plenipo's code that does it. [ADR-146 (where the phone's page lives)](ADR-146-where-the-phone-page-lives.md)
  §4: `relay.getplenipo.com` points at Plenipo's own relay on 8 West's server, not at Milepost's.
- **Replaces:** the [change request for Milepost's relay](../phases/phase-14-relay-change-request.md).
  Nothing is asked of Milepost's repository any more.
- **Keeps:** [ADR-040](ADR-040-phone-web-interface.md) §5 and ADR-146: the relay's server address
  and sign-in stay out of this repository; the relay never serves the phone's page.

> **On screen** (ADR-010, plain words and rank names): nothing changes. **Use Plenipo from another
> device** says **Coming soon** until the relay is live, as before (ADR-140 §4).

## In short

Plenipo's phone needs a **relay**: the small server in the middle that passes sealed messages
between your PC and your phones. Instead of asking Milepost's relay (another repository, another
review, another release) to learn Plenipo's contract, **Plenipo gets its own relay.** Its code is in
this repository (`crates/relay`), every release carries it as one small Linux program, and it runs
on the server 8 West already has, behind that server's proxy, at `relay.getplenipo.com`. It does
exactly what the written contract says and nothing more, and it can still read nothing.

## Context

- Parts 14A to 14C are built and released (v1.19.0 to v1.19.2), and the phone's page is live at
  `remote.getplenipo.com` (ADR-148). The one missing piece is a relay that answers at
  `relay.getplenipo.com`.
- The plan was a change to Milepost's relay, written as a request for its repository
  (`docs/phases/phase-14-relay-change-request.md`). That means two repositories to keep in step, a
  second review and release path, and a relay whose tests Plenipo cannot run.
- The contract is small (`contracts/phone-relay/v1`: two paths, a dozen messages, nine codes), and
  Plenipo already has a stand-in relay that speaks it for the tests. A real relay is the stand-in
  made safe for the internet.
- 8 West's server (Akamai/Linode) already runs Milepost's realtime service, RustDesk, Nginx Proxy
  Manager (ports 80, 81, 443), NocoDB with Postgres, and more. It is **shared and short on memory**
  (about 1 GB, swap full). Anything added must be small, must stop nothing, and must change nothing
  else.
- The relay must never be able to change the phone's page (ADR-146), so it does not go on the page's
  server either.

## Decision

1. **Plenipo runs its own relay**, a new crate `crates/relay` (`plenipo-relay`): one static Linux
   program (x86_64, musl), about 2 MB, with two worker threads.
2. **One definition of the contract.** The relay's messages, codes, passes, fingerprints, and
   base64url move from `plenipo-remote` into a shared crate, `crates/relay-contract`, which both
   the PC and the relay use. The PC's code keeps the same paths. The relay's tests pin the written
   contract: every example message reads and writes back the same, and the example pass and proof
   check.
3. **It implements `contracts/phone-relay/v1` exactly:** the PC's challenge and Ed25519 proof; 8
   West's signed weekly answer checked with the public license keys in `crates/licensing`
   (`active`, or `cancelled` and not yet ended; less than 30 days old); phone passes of 90 days
   (ADR-147); pairing mailboxes (10 minutes, 3 phones); dropped passes; sealed messages of at most
   65,535 bytes, passed in order as they are; one live connection per PC key; `pc_offline` to a
   PC's phones when it leaves. Nothing is stored or queued, ever.
4. **Hardened for the internet:** limits per internet address (IPv6 by its /64) on open
   connections, new connections, and refusals per minute; a cap on connections in all; phones per
   PC; messages and bytes per connection per minute; a deadline for the first message; idle
   timeouts and pings; an **off switch** (a file the operator makes: every connection closes, new
   ones get `503`); `GET /healthz`; a clean stop on SIGTERM; and logs that hold counts, codes, and
   addresses only: never a message, a key, a pass, a weekly answer, a mailbox name, or a challenge.
5. **It listens on this machine only:** `127.0.0.1` and a free port, or the machine's own private
   address on the Docker bridge when the proxy runs in a container and cannot reach the host's
   loopback. It refuses any address the internet could reach. Nginx Proxy Manager, already on the
   server, ends TLS and brings `relay.getplenipo.com` to it with WebSockets on. The relay carries
   no certificate and no secret.
6. **Releases carry it.** The Release workflow builds it on Linux, checks it is static, says its
   version, and carries no test hooks, and attaches `plenipo-relay-<version>-linux-x86_64` and its
   `.sha256` to the GitHub release, next to the phone's page. CI builds it the same way and runs
   it.
7. **On the server** (`crates/relay/deploy`): a systemd service as its own unprivileged user,
   sandboxed (`NoNewPrivileges`, `ProtectSystem=strict`, a read-only system, no devices, only
   internet sockets, 128 MB at most); an installer that **looks first and writes down what it saw**
   (ports, memory, services, containers, how the proxy reaches the host) and refuses a port in use;
   and an updater like the page's (`update-relay.sh`): the latest release, its checksum, the
   program's own version, one atomic switch, a restart, `/healthz`, and a step back if anything is
   wrong, every 15 minutes from a timer. It never stops or changes another service, and never
   touches the firewall, DNS, or the proxy's other hosts.
8. **Tested with Plenipo's own PC and phone, through the real relay:** `crates/relay/tests` runs
   `plenipo-remote`'s PC side and its stand-in phone through the real relay (pairing, sign-in,
   requests, a removed phone, a PC that leaves, a PC that is not Pro), checks the hardening by
   hand (bad hellos and proofs, messages over the limit, mailbox tries, replacement, the limits,
   timeouts, the off switch, the door's plain HTTP answers, the stop), and proves the logs hold
   none of the things above. The **real-app tests** (`tests/e2e/specs/remote.e2e.mjs`) run against
   the real relay, built with its test hooks (`--features test-hooks`: it also trusts the
   contract's test key and prints what the test reads). A release never carries the test hooks;
   CI and the Release workflow check for them. The stand-in relay stays for the Rust tests' bad
   relay modes.
9. **What stays out of this repository:** the server's address, how to sign in to it, and anything
   from Nginx Proxy Manager or Cloudflare. Plenipo knows only the name `relay.getplenipo.com`
   (ADR-146 §4).
10. **The owner's steps** (written in `crates/relay/deploy/README.md`, in plain words): copy the
    deploy folder to the server; `install-relay.sh --check`, then `install-relay.sh`; a proxy host
    for `relay.getplenipo.com` in Nginx Proxy Manager (WebSockets on, a certificate, Force SSL);
    the `A` record in Cloudflare; a check from outside with `cargo run -p plenipo-relay --example
probe`; then the repository variable `PLENIPO_RELAY_LIVE=true`, so the next release turns phone
    access on (ADR-140 §4).

## Consequences

- Phase 14 reaches real phones without a change to any other repository. One review, one release
  path, one set of tests.
- The relay's whole behaviour is tested here, against Plenipo's own PC code, before any phone uses
  it.
- 8 West runs one more small service on a shared machine: about 2 MB on disk and tens of megabytes
  of memory, capped at 128 MB, as its own user, in a sandbox. If it misbehaves, systemd restarts it;
  if it is wrong, the updater steps back by itself; if it must stop, one file turns it off.
- A release that carries a new relay restarts it on the server within 15 minutes: PCs reconnect by
  themselves within seconds; a phone reconnects when its page is opened.
- `PLENIPO_RELAY_LIVE` is set only after the relay is seen answering from outside, so the first
  release that carries the relay (v1.19.3) still says **Coming soon**; the one after turns the
  switch on.
- The change request for Milepost's relay is withdrawn, and its "what the owner does" is replaced
  by §10.

## Alternatives considered

- **Milepost's relay, with the change request** (the plan until now). Fewer programs, but two
  repositories, another release path, and tests Plenipo cannot run. Rejected by the owner.
- **A separate repository for the relay.** It would need its own copy of the contract, or a
  published crate, and its own checks. The contract crate in this repository keeps one definition
  and one CI.
- **The phone page's own AWS server.** It could run both, but it has 512 MB, and ADR-146 keeps the
  relay and the page apart: a relay gone bad must not be able to change the page that holds the
  phone's keys. Separate machines keep that true.
- **A managed WebSocket service in the cloud.** It would hold connection data about customers at a
  third party and cost by the message; the relay's whole job is small enough to run on a machine 8
  West already pays for.
- **Listening on a public port with the relay's own certificate.** Then the relay would hold a
  private key and face the internet directly. Behind the proxy it holds nothing and is reached only
  through the one name.
