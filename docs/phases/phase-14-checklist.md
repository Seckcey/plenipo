# Phase 14 — Implementation Checklist

**Status: delivered: parts 14A (v1.19.0), 14B (v1.19.1), and 14C (v1.19.2) are built and tested, and Plenipo's own relay (ADR-149) is built for v1.19.3. It reaches real phones once the relay is set up on 8 West's server and seen answering at `relay.getplenipo.com` (ADR-140 §4).** The owner said Phase 22 is live on 2026-10-01, and Phase 14 started
on branch `claude/phase-14-phone` (ADR-132, the final push). The owner answered the questions the
same day, and the decision records below are **Accepted**. Below, "[x]" is done. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 14 — Plenipo on Your Phone: a Web Interface Built From Scratch, and
the records written for it:

- [ADR-140 (Phase 14 starts: its numbers, what the check found, and its three parts)](../adr/ADR-140-phase-14-starts.md)
- [ADR-141 (pairing a phone: a picture code or a typed code, shown on your PC)](../adr/ADR-141-pairing-a-phone.md)
- [ADR-142 (the phone proves it is you: a passkey, checked by your PC)](../adr/ADR-142-the-phone-proves-it-is-you.md)
- [ADR-143 (the relay and the lock: sealed end to end, no copies, sign-in, wrong tries)](../adr/ADR-143-the-relay-and-the-lock.md)
- [ADR-144 (notices on your phone when the page is closed, sealed for your phone)](../adr/ADR-144-notices-on-your-phone.md)
- [ADR-145 (the fixed list of what a phone may ask, and what stays on your PC)](../adr/ADR-145-what-a-phone-may-ask.md)
- [ADR-146 (where the phone's page lives: its own address, never on the relay)](../adr/ADR-146-where-the-phone-page-lives.md)
- [ADR-148 (the phone's page on its own small AWS server)](../adr/ADR-148-the-phone-page-on-its-own-server.md)
- [ADR-149 (Plenipo runs its own relay, from this repository, on 8 West's server)](../adr/ADR-149-plenipo-runs-its-own-relay.md),
  which replaces [the change request for Milepost's relay](phase-14-relay-change-request.md)

**Numbers:** ADR-140 to ADR-149 (ADR-140 sets them aside). ADR-133 was left for the Phase 22
session; ADR-134 to ADR-139 stay free.

Dates are Pacific time. The page uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md), and adds: **Use Plenipo from another
device**, **Settings → Devices**, **Add a phone**, **picture code (QR code)**, **typed code**, **Is this
your phone?**, **Check it's you**, **Signed in** / **Sign out**, **Approve on your PC**, **Your PC
can't be reached. Nothing was changed.**, **notice**, **Something needs you**.

**Goal (plan):** "Let the owner do as much as possible from a phone or another device, and at the
very least approve and allow from a notice and from the web interface, while the work, the
permissions, and the records stay on the owner's PC."

## In short, for the owner

- **Turn it on at your PC.** Settings → Switches → **Use Plenipo from another device**. It is part of
  Pro. Off is off: every phone is cut off at once.
- **Add a phone at your PC.** Settings → Devices → **Add a phone** shows a picture code and a typed
  code. Your phone scans it in Plenipo's page (`remote.getplenipo.com`), your PC asks **Is this your
  phone?**, and the phone sets up your face, fingerprint, or passcode for Plenipo.
- **Use it.** Sign in with your face or fingerprint. Then read every page, **Approve**, **Refuse**,
  **Stop all**, and stop work. Later parts add **Allow again**, **Run again**, lessons, objectives, and
  notices when the page is closed.
- **Your PC stays in charge.** The phone talks to it through 8 West's relay, sealed so the relay
  cannot read or change anything. Guard decides every request, and Activity shows each one with the
  phone that sent it.

## Owner decisions (answered, 2026-10-01)

The owner answered every question on 2026-10-01: "as recommended", except question 2 and the
address in question 8. Each record now says **Accepted**.

| #   | Question                                                 | The owner's answer                                                                                         | Record   |
| --- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------- |
| 1   | How a phone is paired                                    | As recommended: on the PC, a picture code or a 16-letter typed code; the PC asks "Is this your phone?"     | 141      |
| 2   | How the phone proves it is you                           | **Only at sign-in**: a passkey (face, fingerprint, or passcode), checked by your PC                        | 142      |
| 3   | From a notice                                            | As recommended: Android, **Refuse** on the notice and **Approve** opens the page; iPhone, one tap opens it | 142, 144 |
| 4   | How long you stay signed in                              | As recommended: 30 minutes after your last request, 12 hours at most                                       | 142, 143 |
| 5   | Does the relay check that the PC is Pro?                 | As recommended: yes, with 8 West's signed weekly answer                                                    | 143      |
| 6   | Notices: straight from the PC, or through the relay?     | As recommended: straight from the PC (amends ADR-040)                                                      | 144      |
| 7   | Approvals kept on the PC only, to begin with             | As recommended: none                                                                                       | 145      |
| 8   | The page's address, the relay's name, where it is served | **`remote.getplenipo.com`**, on its own small AWS server (ADR-148); `relay.getplenipo.com`                 | 146, 148 |
| 9   | Three parts (14A, 14B, 14C)                              | As recommended                                                                                             | 140      |

The owner also said: build it, commit, push, merge, and release, without stopping unless the owner
is truly needed.

## Before part 14A can reach a real phone (the owner's steps)

- [x] ~~Approve the relay change request in Milepost's repository~~ Replaced: Plenipo runs its own
      relay (ADR-149), built in this repository and carried by release v1.19.3.
- [ ] On 8 West's server, as root, from a copy of `crates/relay/deploy`: the installer's check
      (look only), then the installer ([the steps](../../crates/relay/deploy/README.md)). It looks
      first and keeps what it saw; it stops and changes nothing else.
- [ ] In Nginx Proxy Manager on that server: a proxy host for `relay.getplenipo.com` to the relay's
      address and port, WebSockets on, a certificate, Force SSL.
- [ ] In Cloudflare: an `A` record `relay` pointing at that server (DNS only to begin with).
      **Done for the page:** `remote.getplenipo.com` points at the page's own Tunnel (ADR-146,
      ADR-148).
- [x] The page's own small AWS server, and the page's home on it ([the steps](../../apps/remote/deploy/README.md)): set up 2026-10-02, serving v1.19.2 at `remote.getplenipo.com`.
- [ ] Check the relay from outside with Plenipo's own PC code (the `probe` example in
      `crates/relay`; the README says how): it answers `not_pro` and `mailbox_closed`.
- [ ] Set the repository variable `PLENIPO_RELAY_LIVE` to `true`; the next release turns the switch
      on.

## Deliverables

### Part 14A — the sealed line and the approvals (`1.19.0`)

- [x] `Limit::PhoneAccess` (Pro only), with its plain words; pauses when Pro ends (ADR-145 §1)
- [x] The switch **Use Plenipo from another device**, off to begin with; "Part of Pro" on Free;
      "Coming soon" until the relay is live (ADR-140 §4)
- [x] Guard's purpose **phone access**: only `relay.getplenipo.com`, only on Pro with the switch on;
      a stand-in only in test copies (ADR-143 §12)
- [x] The PC's relay key and its Noise key, in the Vault; the relay connection, built only on Pro
      (ADR-143 §2, §12)
- [x] `contracts/phone-relay/v1`: the messages, codes, and limits (ADR-143 §14)
- [x] A stand-in relay for the tests, with a **bad relay** mode (ADR-143 §14)
- [x] Noise on the PC (`snow`) and on the phone (Web Crypto), each passing Noise's test answers and
      each other (ADR-143 §5)
- [x] Settings → Devices: **Add a phone** (picture code, typed code, 10 minutes, 3 tries), **Is this
      your phone?**, the list, **Rename**, **Remove**, un-pause (ADR-141)
- [x] The passkey: made at pairing, checked by the PC at sign-in (ADR-142)
- [x] Sign-in and its end: 30 minutes idle, 12 hours, **Sign out**, **Remove**, switch off, Pro ends
      (ADR-143 §7)
- [x] Wrong tries: pairing codes, failed meetings, refused passkey answers (ADR-141 §5, ADR-142 §7,
      ADR-143 §8)
- [x] `guard::remote`: the fixed list as one `enum`, and Guard's checks in order (ADR-145 §2, §4)
- [x] The Ledger: `remote.*` events with the phone's ID and name; "Approved by you, from …" (ADR-145
      §6)
- [x] **Keep these approvals on my PC only**, none ticked; "Approve on your PC" on the phone (ADR-145
      §5)
- [x] `apps/remote`: the page, phone screen first, both themes, from the keyboard, design system and
      plain words; **Pair this phone**, sign-in, every read page, Approvals with **Approve** and
      **Refuse**, **Stop all**, **Sign out**, **Remove this phone**, more than one organization
      (ADR-145 §7)
- [x] The page's rules: no outside scripts, only its own files and the relay (ADR-146 §3)
- [x] PC offline, and a request lost part way (ADR-143 §9)
- [x] New desktop commands are the main window's alone, with refusal tests (ADR-145 §8)
- [x] `docs/editions.md`: what Plenipo sends to the relay, and phone access on Pro (ADR-143 §10)
- [x] Release notes, the plan's status line, the order of work, `docs/roadmap.md`, the vocabulary,
      and part 14A's section of the acceptance report with screenshots of the real app

### Part 14B — everything else that is safe from the page (`1.19.1`)

- [x] **Allow again** after Stop all
- [x] **Stop** one worker's task
- [x] **Run again** and **Leave stopped** after an unexpected stop
- [x] **Keep** and **Discard** a lesson, as written
- [x] **Send an objective** to a position that takes objectives, text only
- [x] Release notes and part 14B's section of the acceptance report

### Part 14C — notices when the page is closed (`1.19.2`; Phase 14 delivered)

- [x] The PC's notice key, in the Vault; signing up for notices inside the sealed line (ADR-144 §2)
- [x] Guard's purpose **phone notices**: only the four notice services, only to a phone's own address
      (ADR-144 §1)
- [x] Notices sealed for the phone (RFC 8291), signed by the PC's key (RFC 8292) (ADR-144 §4)
- [x] The short line, from Guard's cleaned approval card; the lock-screen choice on the phone
      (ADR-144 §3, §5)
- [x] Android: **Approve** / **Refuse**, **Allow again**, **Keep** / **Discard** on the notice; iPhone:
      one tap to the item (ADR-142 §6, ADR-144 §6)
- [x] An answered item shows "Already answered"; a repeated notice shows once (ADR-144 §8)
- [x] The Home Screen guide for iPhone (ADR-144 §9)
- [x] Release notes, the whole acceptance report, and the plan's status line: Phase 14 delivered

### The relay — Plenipo's own (`1.19.3`; ADR-149)

- [x] `crates/relay-contract`: the relay's messages, codes, passes, fingerprints, and base64url,
      one definition for the PC and the relay (ADR-149 §2)
- [x] `crates/relay`: the relay, implementing `contracts/phone-relay/v1` exactly (ADR-149 §3)
- [x] Hardened: limits per address and per PC, a cap in all, budgets per connection, a deadline for
      the first message, idle timeouts and pings, the off switch, `/healthz`, a clean stop, and logs
      with no contents, keys, passes, or codes (ADR-149 §4)
- [x] Listens on this machine only; refuses any address the internet could reach (ADR-149 §5)
- [x] The Release workflow builds it static (musl) and attaches it with its `.sha256`; CI builds
      and runs it (ADR-149 §6)
- [x] `crates/relay/deploy`: the sandboxed systemd service as its own user, the timer, an installer
      that looks first, an updater that checks and steps back, and a plain-words README (ADR-149 §7)
- [x] Tests: Plenipo's PC and phone through the real relay; the hardening by hand; the written
      contract pinned; the real-app tests on the real relay (ADR-149 §8)
- [x] ADR-149, the contract's README, this checklist, the acceptance report, the plan, `docs/editions.md`,
      `docs/roadmap.md`, and release notes for v1.19.3

## Tests (the plan's list)

Every test uses a relay on the test machine (Plenipo's own relay built for the tests, or the
stand-in with its bad-relay modes), stand-in notice services, a stand-in passkey (the browser's test
authenticator), and made-up data. None uses the relay at `relay.getplenipo.com`.

| Test (the plan's list)                                                                                                                                                                               | Part | Done |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- | ---- |
| Pairing a device, and a wrong or expired pairing code                                                                                                                                                | 14A  | [x]  |
| Signed-in connection                                                                                                                                                                                 | 14A  | [x]  |
| Unknown device (never paired)                                                                                                                                                                        | 14A  | [x]  |
| Removed device and ended session, refused at once                                                                                                                                                    | 14A  | [x]  |
| Replay protection (a copied request is refused)                                                                                                                                                      | 14A  | [x]  |
| Too many wrong tries (the connection slows down, then refuses)                                                                                                                                       | 14A  | [x]  |
| Approving and refusing from the web interface, after the phone confirms it is the owner                                                                                                              | 14A  | [x]  |
| An approval answered on the PC first, then on the phone, and the other way round: the first answer counts                                                                                            | 14A  | [x]  |
| An approval kept "on the PC only" cannot be answered from another device                                                                                                                             | 14A  | [x]  |
| Stop all from another device                                                                                                                                                                         | 14A  | [x]  |
| PC offline, and connection lost part way through                                                                                                                                                     | 14A  | [x]  |
| The relay cannot read a request, answer one, or make one up, and a request replayed through it is refused                                                                                            | 14A  | [x]  |
| Free edition: nothing connects to the relay, and the switch says it comes with Pro                                                                                                                   | 14A  | [x]  |
| The owner turns the switch off on the PC while a device is connected                                                                                                                                 | 14A  | [x]  |
| The web interface cannot start an AI tool, run a program, reach a shell, the terminal, files, the screen, the browser, or secrets, or change permissions, switches, Guard's rules, or paired devices | 14A  | [x]  |
| The web interface on a phone-sized screen, in both themes, from the keyboard, with no errors                                                                                                         | 14A  | [x]  |
| Allowing (Allow again) from the web interface, after the phone confirms it is the owner                                                                                                              | 14B  | [x]  |
| Stop a task, Allow again, Run again, and Leave stopped from another device                                                                                                                           | 14B  | [x]  |
| Sending an objective from another device                                                                                                                                                             | 14B  | [x]  |
| Approving, refusing, and allowing right from a notice (Android), and one tap to that approval (iPhone)                                                                                               | 14C  | [x]  |
| A notice's words can be read only on the owner's phone, and the lock-screen choice shows only "Something needs you"                                                                                  | 14C  | [x]  |

Also, from the rules of this phase: every request is recorded with the phone that sent it; new
desktop commands refuse a second window, the sign, and a web page; and no log or diagnostics file
holds a key, a code, a pass, a passkey answer, or a sealed message.

## Checks only the owner can do, on real phones

The tests prove the design with stand-ins. These need a real phone and the real relay, and are
listed again in the acceptance report:

- [ ] Pair an **iPhone** from its Home Screen page, and an **Android** phone from Chrome
- [ ] Face ID or Touch ID on the iPhone, and the fingerprint or screen lock on Android, at sign-in
- [ ] A notice on each phone while the page is closed; its words on the lock screen, and "Something
      needs you" when chosen
- [ ] On Android, **Refuse** from the notice, and **Approve** from the notice (unlock, then one tap)
- [ ] On the iPhone, one tap from the notice to that approval
- [ ] Turn the switch off on the PC while the phone is open, and see it cut off
- [ ] **Remove** a phone on the PC, and see it refused at once

## Before pushing

`pnpm check`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D
warnings`; `cargo test --workspace --locked` (run again until the whole suite finishes); `pnpm
bindings` with no diff in `packages/types/src/generated`. Docs-only changes: `pnpm docs:check`.
