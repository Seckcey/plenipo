# ADR-143: The relay and the lock — sealed end to end, no copies, sign-in, and wrong tries

- **Status:** Accepted (by the owner, 2026-10-01, as recommended), including the relay's check that the PC is Pro
  (question 5). The face, fingerprint, or passcode check is asked only at sign-in (ADR-142).
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Carries out:** [ADR-040 (Phase 14 is Plenipo's own web interface for a phone)](ADR-040-phone-web-interface.md)
  §5: "the phone and the PC encrypt what they say end to end, so the relay cannot read the work,
  answer an approval, or make up a request"
- **Amended by:** [ADR-147 (relay passes last 90 days)](ADR-147-relay-passes-last-90-days.md): a
  pass lasts 90 days, not 7 (§4).
- **Goes with:** the change request for the relay's own repository,
  [`docs/phases/phase-14-relay-change-request.md`](../phases/phase-14-relay-change-request.md)

> **On screen** (ADR-010, plain words and rank names): **Your PC can't be reached. Nothing was
> changed.**, **We don't know if your PC got this. Check again when it's back.**, **Signed in** /
> **Sign out**, **This phone's browser is too old for Plenipo**.

## In short

Your PC and your phone talk through 8 West's relay, the one Milepost uses. Your PC calls out to it,
so nothing is opened on your PC or your router. Everything they say is **sealed end to end** with
the **Noise protocol**, the same kind of lock WhatsApp and WireGuard use. The relay only passes
sealed messages along. It cannot read them, answer them, make one up, or pass the same one twice.
If your PC is off, the phone says so and changes nothing. Wrong tries make the PC slow down, then
stop listening for a while.

## Context

- The relay belongs to Milepost, and now carries two products. A fault or an attack there must not
  reach Plenipo's work (ADR-040, Consequences).
- The plan's tests: a signed-in connection; an unknown device; a removed device and an ended
  session refused at once; a copied request refused; too many wrong tries (slow down, then refuse);
  the PC offline, or the connection lost part way; and the relay unable to read, answer, or make up
  a request, or to replay one.
- The phone's page runs in a browser. The browser can make keys that **can never be copied out**
  (Web Crypto's "non-extractable" keys), and it has the X25519 and AES-GCM locks that Noise uses,
  in Chrome, Safari, Firefox, and Edge since 2025.
- On the PC, the Rust library `snow` carries out Noise, and is widely used.
- Section 3.2 of the plan: "Unknown external outcomes must be reconciled before retry."

## Decision

1. **The relay passes sealed messages, and nothing else.**
   - The PC and each phone open a secure web connection (`wss://`, a WebSocket over HTTPS) to
     Plenipo's relay name (ADR-146). The PC calls out; nothing listens on the PC.
   - The relay keeps nothing: no queue, no copies, no message held for later. If the PC is not
     connected, the relay tells the phone so at once.
   - A message is at most 64 KB. Long pages come in pieces.
2. **How the relay knows a PC.** When phone access is switched on, the PC makes its own **relay
   key** (Ed25519), kept in the Vault. The relay knows the PC only by a fingerprint of that key, and
   the PC proves it holds the key each time it connects.
3. **The relay checks that the PC is Pro** (recommended): the PC shows the relay the **newest signed
   weekly answer** it already holds (ADR-116). The relay checks 8 West's signature with the same
   public keys every copy of Plenipo carries (ADR-104), that the subscription is not ended, and that
   the answer is less than 30 days old. Nothing new is made or sent to 8 West, and the relay never
   needs the account service. The relay then knows that key's ID, and must not keep it.
4. **How the relay knows a phone.** At pairing, the PC gives the phone a **relay pass** it signs with
   its relay key: which PC, which phone (a random ID), and an end date 7 days away. The PC renews it
   over the sealed line. The relay passes a phone's messages only with a valid pass, and drops a pass
   the moment the PC says so (ADR-141 **Remove**). The pass keeps strangers from even reaching the PC;
   the lock below is what keeps them out.
5. **The lock: Noise, end to end.**
   - **Pairing:** `Noise_XXpsk3_25519_AESGCM_SHA256`, with the pairing code (ADR-141) turned into the
     shared key by HKDF-SHA256. Each side learns the other's long-term key, and only with the right
     code.
   - **Every time after:** `Noise_KK_25519_AESGCM_SHA256`. Each side already knows the other's
     long-term key, so a stranger, or the relay, fails the very first message. Each meeting makes
     fresh keys, so a message from one meeting means nothing in another, and an old key found later
     does not open old messages.
   - **The phone's long-term key** is a Web Crypto X25519 key that can never be copied out, kept in
     the page's own storage. **The PC's** is kept in the Vault.
   - **Checked both ways:** the PC's side and the phone's side each pass Noise's published test
     answers, and then talk to each other in the tests.
   - A phone whose browser has no X25519 is told "**This phone's browser is too old for Plenipo**",
     with what to update.
6. **No copies, and nothing changed on the way.**
   - Every sealed message carries a counter. A message copied, changed, dropped, or out of order
     fails, and the PC ends that meeting at once.
   - Every request carries a random ID. The PC refuses an ID it has already seen in that sign-in, and
     keeps each request's outcome for that time.
   - Every request needs a signed-in meeting (ADR-142 §3, §4), except **Refuse** and **Discard**
     sent from a notice (ADR-142 §5).
7. **Sign-in, and ending it.** A meeting is signed in once the phone's passkey answer is checked
   (ADR-142 §3). It ends:
   - after **30 minutes** with no request, or **12 hours** in all;
   - when you tap **Sign out** on the phone;
   - **at once** when you **Remove** the phone on the PC: the PC closes its meeting, forgets its key,
     and tells the relay to drop its pass;
   - **at once, for every phone**, when you switch phone access off on the PC: the PC ends every
     meeting and leaves the relay. The phones stay listed, and sign in again when it is back on;
   - when Pro ends: phone access pauses like Connections (ADR-068). Nothing is deleted, the phones
     stay listed, and it comes back with Pro.
8. **Wrong tries slow the PC, then stop it.**
   - Pairing codes: ADR-141 §5.
   - Meetings that fail (an unknown phone, a bad message): after 10 in a minute, the PC stops
     answering new meetings for 1 minute, then 2, 4, and so on up to 15 minutes. A phone already
     signed in keeps working. If it keeps happening, the PC tells you: "**Someone keeps trying to
     reach your PC as a phone.**"
   - Passkey answers the PC refuses: ADR-142 §6.
   - The relay limits connections and tries from each internet address too (the change request).
9. **When the PC cannot be reached.** The phone shows "**Your PC can't be reached. Nothing was
   changed.**" Nothing waits at the relay to run later. If the connection drops after a request was
   sent, the phone shows "**We don't know if your PC got this. Check again when it's back.**" When it
   reconnects, it asks the PC what happened to that request ID. The phone never sends an action
   again by itself.
10. **What Plenipo sends to the relay, and nothing else** (written in `docs/editions.md` with the
    weekly check, and checked in a test): from the PC, its relay public key, its proof that it holds
    it, the signed weekly answer (§3), the passes it drops, and sealed messages; from a phone, its
    pass and sealed messages.
11. **What the relay can see:** that a PC and some phones are connected, when, how much they send,
    and their internet addresses. **It cannot see** what is said, which approvals, objectives,
    workers, or device names.
12. **Free connects to nothing.** On Free, Plenipo never builds its relay connection (as the license
    check is never built without a key, ADR-115). Guard's new purpose, **phone access**, allows only
    Plenipo's relay name, and only on Pro with the switch on.
13. **Recorded:** `remote.relay_connected`, `remote.relay_disconnected`, `remote.session_ended` (with
    why), `remote.meeting_refused` (counted, not one per try), and `guard.request_refused` for any
    other address. Never a key, a pass, or a sealed message.
14. **Tested without the real relay.** A stand-in relay in this repository speaks the same contract
    (`contracts/phone-relay/v1`, written in part 14A), and has a **bad relay** mode that reads,
    changes, copies, replays, drops, and makes up messages. Every one is refused.

## Consequences

- A broken or attacked relay can stop phone access, but cannot read your work, answer an approval,
  or start anything on your PC.
- A meeting dropped on a train picks up again within 30 minutes without your face or fingerprint;
  after that it asks again.
- The relay change is small and has no Plenipo secrets in it.
- Plenipo gains two libraries on the PC: `snow` (Noise) and the X25519 and AES-GCM parts it uses.
  Each is checked against published test answers.
- The phone's page needs a browser from 2025 or later.

## Alternatives considered

- **The relay checks nothing about Pro.** Simpler, and the license already forbids getting around
  Pro. Not recommended: anyone with a changed copy could use 8 West's relay for free, and the signed
  answer costs nothing to show.
- **Only HTTPS to the relay, no lock of our own.** The relay would see everything. Rejected by ADR-040.
- **A lock of Plenipo's own design.** Rejected: Noise is studied, documented, and has test answers.
- **TLS from the phone straight to the PC, through the relay.** The PC would need a web certificate
  and a public name of its own. Far more parts, for the same result as Noise.
- **Keeping requests at the relay until the PC is back.** An approval could then run hours later,
  after you changed your mind. The plan says the phone changes nothing when the PC is off.
