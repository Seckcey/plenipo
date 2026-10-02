# ADR-141: Pairing a phone — a picture code or a typed code, shown on your PC

- **Status:** Accepted (by the owner, 2026-10-01, as recommended)
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Answers:** the plan's "pairing a device from the PC: a one-time code (or QR code) the PC shows, or
  the owner's 8 West account (Phase 22); the phase's ADR chooses"

> **On screen** (ADR-010, plain words and rank names): **Settings → Devices**, **Add a phone**,
> **picture code** (said once as "picture code (QR code)"), **typed code**, **Is this your phone?**,
> **Add** / **Cancel**, **Rename**, **Remove**. On the phone: **Pair this phone**, **Scan the code on
> your PC**, **Type the code instead**.

## In short

You add a phone **at your PC**. Plenipo shows a picture code (a QR code) and a 16-letter typed code.
Your phone scans the picture, or you type the code. Your PC then asks **"Is this your phone?"**, and
you click **Add**. The code works once, for 10 minutes, and dies after 3 wrong tries. Your 8 West
account is not used, so no 8 West server can ever add a phone to your PC.

## Context

- The plan lets this record choose between a code the PC shows and the owner's 8 West account
  (Phase 22). Phase 22 is live, so either can work.
- Adding a device widens who may connect, so it is the PC's alone (ADR-040 §4, the plan's list of
  what stays on the PC).
- The relay passes every message along. It must never learn the code, and it must not be able to
  guess it, even with a fast computer and the messages in hand.
- **On an iPhone, a page saved to the Home Screen keeps its own storage**, apart from Safari. iPhone
  web notices only work from the Home Screen page. So an iPhone must be paired from inside the Home
  Screen page; a link opened in Safari would pair Safari instead. On Android, the page installed from
  Chrome shares Chrome's storage, so either works.

## Decision

1. **Pairing starts on the PC:** Settings → Devices → **Add a phone** (main window only). It needs
   phone access switched on, and Pro (ADR-145).
2. **The PC shows two things:**
   - a **picture code (QR code)** holding the phone page's pairing address, with the code after a
     `#` (the part of an address a browser never sends to any server);
   - the same code to type: **16 letters and digits**, in four groups (for example
     `7K3Q-M9TX-2HFD-R8WB`), 80 bits of chance, with no letters that look alike.
3. **The phone pairs from its page:** **Pair this phone**, then **Scan the code on your PC** (the
   page's own camera button, so an iPhone can pair from its Home Screen page) or **Type the code
   instead**. On Android, the phone's own camera app may open the picture code too.
4. **The code is the key to a sealed first meeting.** Both sides run the Noise protocol's
   `XXpsk3` pattern (ADR-143), with the code turned into a shared key. Each side shows the other its
   own long-term key inside that meeting, and only a side that knows the code can finish it. A wrong
   code fails the meeting; the relay never sees the code, and 80 bits is far too many to guess even
   from the messages. The relay finds the waiting PC by a mailbox name made from the code, never the
   code itself.
5. **The code works once, for 10 minutes, and dies after 3 wrong tries.** After 3 dead codes in an
   hour, **Add a phone** pauses for 15 minutes, and the PC says so: "Someone tried a wrong code."
6. **Your PC asks before it adds anything:** "**Is this your phone?** Frank's iPhone · Safari on
   iPhone · just now", with **Add** and **Cancel**. Nothing is added until you click **Add**. This
   stops someone who photographed your screen from adding their phone first.
7. **Then the phone sets up its passkey** (ADR-142): one tap, and your face, fingerprint, or passcode.
8. **What the PC keeps for each phone** (in the first organization's Vault under its own name, like
   the license, ADR-110; never in a log): its name, its long-term public key, its passkey's public key
   and ID, when it was added, when it was last seen, and, after part 14C, where to send its notices.
   The phone keeps its own long-term key in the browser, made so it can never be copied out
   (ADR-143).
9. **Each phone has a name, shows in Settings → Devices, and can be renamed or removed.** **Remove**
   cuts the phone off at once (ADR-143 §7). A phone can also remove itself (**Remove this phone**, on
   the phone), because that only takes access away.
10. **Recorded:** `remote.device_added`, `remote.device_renamed`, `remote.device_removed`, and
    `remote.pairing_refused` (with why: expired, wrong, used, or cancelled), with source `owner` and
    the device's name, never the code.

## Consequences

- No 8 West server is part of pairing. A fault or an attack at 8 West cannot add a phone to anyone's
  PC, and pairing works the same if the account service is down.
- Nothing changes in the account service's repository.
- The owner types 16 characters when the camera cannot be used. The picture code is the usual way.
- A phone that is wiped, or a browser whose site data is cleared, must be paired again.

## Alternatives considered

- **Your 8 West account.** You would sign in on your phone, and the account service would tell your
  PC about it. Not recommended: the account service would become able to add a phone to a customer's
  PC, the account service's own code would have to change, and the PC still has to confirm each new
  phone, so it saves no steps.
- **Both ways.** Twice the code and twice the tests, for no gain over the picture code.
- **A short 6-digit code.** It needs an extra kind of lock (a "PAKE") so the relay cannot try every
  code; that is more special code on both sides. Not chosen: the picture code makes a long code easy,
  and a typed 16-letter code stays safe with standard parts only.
- **No question on the PC.** One step fewer, but someone who saw the screen could add their phone in
  the 10 minutes.
