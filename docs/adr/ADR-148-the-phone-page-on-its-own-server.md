# ADR-148: The phone's page on its own small server

- **Status:** Accepted (by the owner, 2026-10-02: "lets put it on its own small instance for a few
  dollars a month").
- **Date:** 2026-10-02
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Amends:** [ADR-146 (where the phone's page lives)](ADR-146-where-the-phone-page-lives.md) §2 and
  §6: the page is served from **its own small AWS server**, not from Coastline next to the website.
  Everything else in ADR-146 stands: the address `remote.getplenipo.com`, the page built from this
  repository with each release, its rules, and the relay's name `relay.getplenipo.com`.
- **Keeps:** [ADR-103 (hosting the account service)](ADR-103-account-service-hosting.md) §1: the
  account service's server still runs only the account service.

> **On screen** (ADR-010, plain words and rank names): nothing changes. Your phone opens
> `remote.getplenipo.com`, wherever it is served from.

## In short

Your phone's page gets **a small server of its own on AWS**, for a few dollars a month. It serves
only the page's files, and updates itself from each release, the way ADR-146 planned for Coastline.
Coastline stays for test copies and previews. The account service's server stays for accounts and
licenses alone.

## Context

- ADR-146 put the page next to Plenipo's website on Coastline. Coastline is 8 West's machine for test
  copies and previews; production services run on AWS (Logbook moved there, and the account service
  was set up there in Phase 22).
- The owner asked whether the account service's server could serve the page. It could, but that
  server signs license keys and holds customer accounts and payment keys, and ADR-103 keeps it to
  that one job. A second public service on it would be a small risk, but not none.
- The page is only read-only files. Serving it needs very little: a small web server and the updater
  (`apps/remote/deploy`), which checks each release's checksum and that the page names only 8 West's
  relay.

## Decision

1. **Its own server.** One small AWS EC2 server serves only the phone's page: ARM (`t4g.nano`, 512 MB
   of memory, with swap), Ubuntu's long-term release, a small encrypted disk, in the same region as
   8 West's other servers. It holds no secrets but the Cloudflare Tunnel's token.
2. **Nothing reaches it from outside but the Tunnel.** No open ports for the web: Cloudflare's Tunnel
   brings `remote.getplenipo.com` to the small web server on the server itself (`127.0.0.1`). The
   firewall lets in only 8 West's own sign-in for upkeep, as on the account service's server.
3. **The same updater.** `apps/remote/deploy/update-page.sh` runs every 15 minutes from a timer, as
   ADR-146 planned: it serves a new release's page only when its checksum matches and it names only
   8 West's relay, checks it, and goes back if anything is wrong.
4. **Its address stays out of this repository**, like the account service's server (ADR-100). The
   steps here name it only as "the page's server".
5. **Nothing to back up.** Every release carries the page; a new server can be set up from the steps
   in [apps/remote/deploy](../../apps/remote/deploy/README.md) and serve it again within minutes.

## Consequences

- About $7 to $8 a month: the server (about $3), the public internet address AWS charges for (about
  $3.65; the updater needs it to reach GitHub), and its disk (under $1).
- If the page's server is down, phones cannot open the page; the PC keeps working, and approvals wait
  on the PC. If the alarm on it fires, the owner hears of it like the account service's.
- The account service's server keeps its one job (ADR-103), and the website keeps Coastline.

## Alternatives considered

- **Coastline, next to the website (ADR-146 as first decided).** It works, but Coastline is for test
  copies and previews.
- **The account service's server.** Fewer servers, but it would break ADR-103's one-job rule on the
  server that signs license keys.
- **Cloudflare Pages.** No server at all, but GitHub would hold a Cloudflare key to publish
  (ADR-146's own reason against it).
