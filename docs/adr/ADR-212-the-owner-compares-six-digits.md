# ADR-212: The owner compares six digits — both screens show the same check digits when a phone is added

- **Status:** Proposed. Built from the security review's findings P-SRV-2 [High] and P-SRV-3
  [Medium] (`PLENIPO-SECURITY-REPORT.MD`, 2026-10-02), for the owner to accept at review.
- **Date:** 2026-10-03
- **Phase:** none (security hardening of Phase 14, Plenipo on your phone)
- **Number:** ADR-210 to ADR-219 are the security work's block, so no session picks the same
  number as another.
- **Amends:** [ADR-141 (pairing a phone)](ADR-141-pairing-a-phone.md) §6 (the question "Is this
  your phone?") and [ADR-142 (the phone proves it is you)](ADR-142-the-phone-proves-it-is-you.md)
  §3 (how long you stay signed in).
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): on the PC, under **Is this your phone?**,
> "The phone in your hand shows the same six digits: **222 130**" and "Add it only if the digits
> match. A name proves nothing." On the phone: "Your PC shows the same six digits as this phone:
> **222 130**", and, for a phone that lost, "Another phone already used this code. If that was not
> you, click Cancel on your PC and start again." In Activity: "iPhone tried a pairing code another
> phone had already used: it was turned away." This record keeps the code's words (`check`,
> `check_digits`, the `used` reason of `remote.pairing_refused`, and `again`).

## In short

When you add a phone, your PC shows a code for 10 minutes. The security review found a hole: if
someone can see your screen (a screen share, a photo, a video call) and scans the code **before
you do**, your PC asks "Is this your phone?" about **their** phone, and shows only the name their
phone chose, such as "iPhone". Your own phone just says the code did not work. Nothing on the PC
lets you tell the two phones apart, so you click **Add**, and a stranger's phone can approve work
and send objectives.

Now both screens show the **same six digits**, made from the sealed meeting itself. Your phone and
your PC agree on the digits only when they met each other. If a stranger's phone got in first, your
PC shows the stranger's digits, your phone does not, and your phone also says in plain words that
another phone used your code. You click **Cancel** and start again.

The review also found that a phone stays signed in as long as the PC keeps changing (the page
re-reads by itself every time), so the 30-minute limit never came. That is fixed in the same
record: only what you do on the phone keeps you signed in.

## Context

- ADR-141 §6 said the question "Is this your phone?" "stops someone who photographed your screen
  from adding their phone first". It did not: the question shows what the phone said about itself
  (`PairHello`: a name and a browser), and a stranger can say "iPhone" too. The second phone with
  the right code was cut off with no message (`service.rs`, the third message of
  `pairing_meeting`), so the owner's own phone, if it lost the race, showed only "That didn't work:
  the code may be wrong or used."
- The sealed first meeting is the Noise protocol's `XXpsk3` pattern (ADR-143). After its third
  message, both sides hold the same **handshake hash**: a 32-byte number that covers the pairing
  code, both sides' long-term keys, and both sides' one-time keys for that meeting. Two meetings
  never share it, and the relay cannot make it (it knows neither the code nor the keys).
- ADR-142 §3 promises "30 minutes after your last request, and 12 hours at most". The page
  re-reads a page with `again: true` every time the PC says something changed, and the PC counted
  those re-reads as "your last request". While workers run, the PC says so every few seconds.

## Decision

1. **Six digits from the meeting's own hash.** Right after the first meeting's third message,
   each side takes the handshake hash's first four bytes as a number and keeps its last six
   digits, with zeros in front when it is small (`crates/remote/src/noise.rs::check_digits`;
   `apps/remote/src/lock/noise.ts::checkDigits`). Nothing is sent: each side makes the digits
   itself, and both tests check the written contract's pairing meeting
   (`contracts/phone-relay/v1/noise-vectors.json`) against the same answer, `222130`.
2. **Both screens show them, as two groups of three.** The PC's "Is this your phone?" shows
   "The phone in your hand shows the same six digits: **222 130**" and tells the owner to add the
   phone only if the digits match, because a name proves nothing. The phone's page shows "Your PC
   shows the same six digits as this phone: **222 130**", with what to do if the PC shows other
   digits. `PairingView::Asking` carries `check` to the PC's screen.
3. **A phone that lost the race is told why**, in one of two ways, depending on when it arrived:
   - **Already at the mailbox** when the other phone finished (both scanned within the same
     moment): it finishes its own meeting and hears `PairStep::Refused` from the PC with "Another
     phone already used this code. If that was not you, click Cancel on your PC and start again."
     before its line is closed, instead of a silent close. The PC records
     `remote.pairing_refused` with reason `used` and that phone's name, so Activity shows that a
     second phone tried.
   - **Arriving later:** the PC closed the mailbox the moment the first phone finished, so the
     relay turns the late phone away (`mailbox_closed`) and the PC never hears from it. The page's
     own words for a closed mailbox now say to look at the PC: "This code was already used, or
     mistyped, or your PC stopped adding a phone. Look at your PC: if it is asking about a phone
     that is not yours, click Cancel and start again. Otherwise check the code, or press Add a
     phone on your PC for a new one. A code lasts 10 minutes and works once." Before, the page
     said "the code may be wrong or used. Press Add a phone on your PC again", which sent the
     owner the wrong way. Nothing is recorded on the PC in this case, because the relay never let
     the phone through.
4. **Why a stranger cannot show your digits.** The digits come from a hash that includes the
   stranger's own phone key and one-time keys, which differ from your phone's. The code alone, even
   the PC's key alone, does not fix the digits. One meeting in a million shows the same six digits
   as another by chance; that is the same odds as guessing a six-digit code once.
5. **Old and new.** The digits travel on no wire, so no message changed. An old phone page against
   a new PC shows no digits (the PC still shows them, so the owner simply cannot compare, as
   before); a losing old page that was already at the mailbox says "Your PC did not answer as
   expected.", and one arriving later keeps its old closed-mailbox words. A new page against an
   old PC shows digits the PC does not. The page is served from `remote.getplenipo.com`
   (ADR-146), so old pages are rare and short-lived.
6. **ADR-141 §6 now reads:** the PC asks "Is this your phone?" with the name, the browser, and the
   six digits; the owner compares the digits with the phone in hand; a name alone proves nothing.
7. **Only what you do keeps the phone signed in** (P-SRV-3). A request marked `again: true` (a
   page the phone reads again by itself after the PC said something changed) no longer moves the
   30-minute clock. Opening a page, an approval, an objective, or any other tap of yours still
   does. The page's **Try again** buttons also read with `again: true`, so a tap on one does not
   extend the sign-in either; the owner accepted that. **ADR-142 §3 now reads:** 30 minutes after
   your last tap, where a tap is a request you made, not a page the phone re-read on its own.

## How it is checked

- `crates/remote/src/noise.rs`: both sides of a first meeting make the same six digits; the
  contract's pairing hash gives `222130`; small numbers keep their leading zeros.
- `crates/remote/tests/remote.rs`
  (`two_phones_with_one_code_show_the_owner_which_one_got_in`): two stand-in phones, both named
  "iPhone", are at the mailbox before either says who it is; the PC's digits equal the first
  phone's and differ from the second's; the second phone hears the refusal; the owner cancels, and
  nothing is added.
- `apps/remote/src/lock/lock.test.ts` and `app.test.tsx`: the page makes the same digits as the
  PC from the same meeting, shows them while the PC asks, says why when another phone used the
  code first, and tells a phone the relay turned away to look at the PC.
- `apps/desktop/src/remote/remote.test.tsx`: "Is this your phone?" shows the digits and what to do
  when they differ, with no accessibility problems.
- `crates/remote/tests/remote.rs`
  (`a_page_the_phone_reads_again_by_itself_does_not_keep_it_signed_in`): a phone signed in for
  29 minutes reads a page again by itself, 2 more minutes pass, and it hears it was signed out
  for being idle; the same with a request of its own keeps it signed in.

## Consequences

- Adding a phone takes one more look: the owner compares six digits on two screens. The digits are
  large and in groups of three on both.
- A stranger who scans first gains nothing: the owner's phone says what happened, the PC shows
  digits the owner's phone does not, and **Cancel** ends it. Activity keeps the trace.
- `PairingView::Asking` gained a field, so the desktop app and the PC's bindings
  (`packages/types/src/generated/PairingView.ts`) change together.
- A phone left open on a desk while workers run now signs out after 30 minutes, as ADR-142
  promised, instead of staying signed in for 12 hours. The owner signs in again with one tap and
  the phone's own check.

## Alternatives considered

- **Send the digits from the PC to the phone.** Not chosen: a message on the wire is one more
  thing to get right, and both sides already hold the hash the digits come from.
- **A tap on the phone to confirm, as well as on the PC.** Not chosen: the owner's own phone is
  the one that lost the race in this attack, so a tap on the stranger's phone proves nothing; the
  comparison on the PC is what stops it.
- **Twelve digits, like the safety code the vocabulary lists for Community.** Not chosen: six is
  enough against a one-in-a-million chance, and easier to compare at a glance.
- **A longer code or a PAKE.** The code is already 80 bits and the meeting is sound (ADR-141 §4);
  the gap was what the owner could see, not the cryptography.
- **Count a Try again tap as a tap.** The buttons would need their own kind of request. Not
  chosen: the owner accepted that a **Try again** tap does not extend the sign-in; any other tap
  does, and signing in again is one tap.
