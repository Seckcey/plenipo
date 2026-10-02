# Change request for the relay's own repository — passing Plenipo's sealed messages

**From:** Plenipo, Phase 14 (Plenipo on your phone), made by 8 West Ventures, LLC.
**To:** the repository of the relay 8 West runs for Milepost.
**Status:** Draft, 2026-10-01. **The owner approves this change in the relay's own repository.**
Plenipo's builder does not touch that repository.
**Depends on:** [ADR-143 (the relay and the lock)](../adr/ADR-143-the-relay-and-the-lock.md),
[ADR-147 (relay passes last 90 days)](../adr/ADR-147-relay-passes-last-90-days.md),
[ADR-146 (where the phone's page lives)](../adr/ADR-146-where-the-phone-page-lives.md), and, for
notices, [ADR-144](../adr/ADR-144-notices-on-your-phone.md). If the owner changes those records,
this request changes with them.

> This file holds **no address and no sign-in** for the relay, and must never hold one. Plenipo
> reaches the relay as `relay.getplenipo.com`, a name the owner points at the relay in Cloudflare.

## In short

Plenipo needs the relay to **pass sealed messages** between a customer's PC and that customer's
phones, and do nothing else with them. The relay cannot read them, and must not try. **Milepost keeps
working exactly as before**: Plenipo gets its own paths, its own limits, and its own off switch.

## What the relay gets, and what it never gets

- **Gets:** a PC's relay public key and its proof of holding it; 8 West's signed weekly answer for
  that PC's license (to check it is Pro); passes the PC signs for its phones; and sealed messages,
  each at most 64 KB.
- **Never gets:** any Plenipo secret, license key, password, or account sign-in; the pairing code;
  anything readable from inside a sealed message; and, if ADR-144 is accepted as recommended, nothing
  about notices.

## The change, item by item

1. **Plenipo's own paths, under `relay.getplenipo.com`** (WebSockets over HTTPS, through Cloudflare):
   one for PCs and one for phones. Milepost's paths, sign-in, and behavior do not change.
2. **A PC connects** and proves who it is:
   - the relay sends a fresh random challenge;
   - the PC answers with its Ed25519 relay public key, a signature over the challenge (with a fixed
     label, so the signature means nothing anywhere else), and its newest **signed weekly answer**;
   - the relay checks the signature, and checks the weekly answer with **8 West's two public license
     keys** (the same ones every copy of Plenipo carries; public, not secret): the signature is
     right, the state is not "ended" or "unknown", a cancelled subscription has not reached its end
     date, and the answer's time is less than 30 days ago;
   - the relay then knows the PC by its key's fingerprint. It must not keep the license key ID
     after the check;
   - one live connection per PC key: a new one replaces the old.
3. **A phone connects** with a **relay pass** the PC signed: which PC (the fingerprint), which phone (a
   random ID), and an end date at most 90 days away (ADR-147). The relay checks the pass against the PC's key,
   and then passes messages only between that phone and that PC.
4. **Pairing mailboxes.** A PC may open one mailbox at a time, named by an ID it chooses (made from
   the pairing code, never the code itself), for at most 10 minutes. One phone may connect to a
   mailbox **without a pass**, and at most 3 phone connections in all. The mailbox closes after a
   pairing, after 10 minutes, or when the PC closes it.
5. **Passing messages.** Binary messages of at most 64 KB, passed in order, as they are. Nothing is
   stored or queued. If the PC is not connected, the relay tells the phone at once ("PC offline"), and
   drops the message.
6. **Dropping a pass.** The PC may tell the relay to drop a phone's pass. The relay closes that
   phone's connections at once, and refuses that pass until its end date.
7. **Limits**, so Plenipo cannot crowd out Milepost:
   - connections and tries from each internet address (IPv6 counted by its /64), and per PC;
   - messages and bytes per PC and per phone, per minute;
   - a cap on Plenipo's open connections and memory in all;
   - a switch that turns Plenipo's part off without touching Milepost.
8. **Logs:** counts and errors only. Never a message's contents, a pass, a weekly answer, a key ID,
   or a challenge. Logs kept as short a time as Milepost keeps its own.
9. **Errors** are short codes the PC and phone understand (for example `pc_offline`, `bad_pass`,
   `not_pro`, `too_many_tries`, `mailbox_closed`, `too_big`), written in the contract.

## The contract both sides test

The exact messages, codes, and limits are written once, in Plenipo's repository, under
[`contracts/phone-relay/v1/`](../../contracts/phone-relay/v1/README.md), as the weekly license check's contract is
(`contracts/license-check/v1`). The relay pins the contract's version it was tested with. Plenipo's
own tests run against a stand-in relay that follows the same contract, including a **bad relay**
mode, so Plenipo never needs the real relay to test.

## Tests for the relay's repository

- Milepost's own tests all still pass, unchanged.
- A PC with a good weekly answer connects; one with an ended, unknown, too old, or badly signed answer
  is refused (`not_pro`); one that cannot sign the challenge is refused.
- A phone with a good pass reaches its own PC and no other; a pass for another PC, an expired pass,
  a dropped pass, or a changed pass is refused.
- Messages pass in order, unchanged, and are never stored; a message over 64 KB is refused.
- A phone gets `pc_offline` at once when its PC is not connected.
- A mailbox takes one pairing, closes after 10 minutes, and refuses a fourth phone.
- The limits slow, then refuse, too many connections or tries, and Milepost keeps working while
  Plenipo's part is flooded.
- Turning Plenipo's part off closes Plenipo's connections and leaves Milepost's alone.
- The logs hold none of the things item 8 lists.

## What the owner does

1. Read this request, and approve it (or ask for changes) in the relay's own repository.
2. Have the change built there, with its own review and tests.
3. In Cloudflare, point `relay.getplenipo.com` at the relay, with WebSockets on.
4. Tell Plenipo's builder when it answers, so part 14A can be checked against it once, by hand, before
   phone access is turned on in a release.
