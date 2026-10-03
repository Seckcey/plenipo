# ADR-211: Pro ends a set time after the paid period

- **Status:** Proposed. The builder's fix for security finding P-DESK-1, with the Development
  Coordinator's answers (2026-10-03). The owner accepts it at review.
- **Date:** 2026-10-03
- **Phase:** none (the security review's fixes)
- **Number:** ADR-210 to ADR-219 are the security work's block.
- **Amends:** [ADR-022 (subscription pricing and the weekly license check)](ADR-022-subscription-and-license-check.md)
  section 3, the rule for a PC with no internet, and the "what Plenipo keeps" note in
  [ADR-116 (the weekly answer is signed)](ADR-116-the-weekly-answer-is-signed.md), which said that
  deleting the record starts a new 30 days.

## In short

Pro keeps working for 30 days between checks, as before. Two things are added. First, without any
check that went through, Pro never runs more than 30 days past the paid-through date Plenipo can
prove: the one in the license key, or a later one in 8 West's signed answer after a renewal.
Second, the two things in the license record that only time or 8 West can give back are also kept
in the Vault next to the key: when the key was entered, and 8 West's newest signed answer. A
license record that is deleted or lost then starts nothing again.

## Context

- ADR-022 §3: Pro drops only when 8 West says the subscription ended, or after 30 days with no
  check that went through. An 8 West outage must never take Pro from a paying customer.
- The 30 days count from 8 West's newest signed answer, or from when the key was entered (ADR-116).
  Both are kept in the license record, a plain file in Plenipo's data folder.
- The security review (finding P-DESK-1, Medium) found that a record that is missing or cannot be
  read is treated as "the key was just entered", which starts a new 30 days. The key's own
  paid-through date was shown on screen but never used.
- A renewal does not send a new key. So a monthly key's own paid-through date goes out of date
  after its first month; the renewed date arrives only in 8 West's weekly answer.

## Decision

1. **The paid period is a ceiling.** Pro ends at the earlier of:
   - 30 days after 8 West's newest signed answer (or, before the first one, after the key was
     entered), as before; and
   - 30 days after the newest paid-through date Plenipo can prove: the key's own, or a later one in
     8 West's signed answer. A signed answer also proves it was paid on the day it was given.

   So once 8 West has answered, nothing changes for a paying customer: the 30 days end first. The
   ceiling matters only on a PC that has no answer at all.

2. **"Unknown" keeps today's behaviour.** When 8 West answers that it does not know the key, Pro
   stays on (ADR-022 §3), and the ceiling does not apply. Plenipo keeps that signed answer, so the
   same holds after a restart. An "unknown" answer is still not a check that went through: the 30
   days go on counting.
3. **A copy in the Vault.** The record's entry time and 8 West's newest signed answer are also kept
   in the Vault (Windows Credential Manager, the Mac's Keychain, or the Linux password store), under
   `plenipo-license-record`, next to the key. Never the key itself. On start, Plenipo uses the
   earlier entry time and the newer signed answer of the file and the copy. The copy only moves
   forward: a record that lost something never makes the copy lose it too. An answer whose
   signature does not check, and an entry time in the future, are ignored. "Delete my data" removes
   the copy with the key.
4. **This raises the bar; it is not a wall.** Someone who controls the PC can still remove things
   from the Vault and change the clock. Changing Plenipo's code is what the Elastic License forbids
   (ADR-021). The aim is that an ordinary file deletion gains nothing, and that a paying customer
   never loses Pro by mistake.
5. **Plain words.** Settings → License says: "Without a check, Pro never runs more than 30 days past
   your paid-through date." When Pro is off for this reason, it says Plenipo hasn't been able to
   confirm the subscription with 8 West in time, to connect to the internet and choose **Check
   now**, and that everything made is still here. After a check that failed, it also names the
   address Plenipo needs to reach, `account.getplenipo.com`, so a work network can allow it.

## Consequences

- A paying customer whose record is intact sees no change, online or offline.
- A paying customer whose record is lost keeps everything, because the Vault's copy brings it back.
- A customer on a **new PC, or with both the record and the copy gone, while offline**, gets Pro
  only if the key's own paid-through date is less than 30 days past. A monthly key from many months
  ago is on Free until the first check goes through. That check runs as soon as Plenipo starts, so
  online it takes a few seconds. Nothing is ever deleted (ADR-021).
- The ceiling does **not** depend on the account service filling the paid-through date in every
  answer. When an answer has none, its own "as of" day is used, and a test covers that. A filled
  date is a nicety, not a condition. (The service does fill it: seen in its code by the Development
  Coordinator, 2026-10-03; not tested here.)

## What this does not stop

- The Vault's copy does not keep the latest clock time Plenipo has seen; the record file does. So
  someone who both loses the record file and sets the PC's clock back can get back some of the
  30 days after 8 West's newest answer: never more than those 30 days, and never past an "ended"
  answer. A possible follow-up is to keep that clock time in the copy too, written at most once a
  day. It is left out here to keep this change small.
- A yearly key whose subscription ended early (a refund or a chargeback) still carries its own
  paid-through date. With the record file and the Vault copy both gone and no check able to go
  through, Pro comes back until that date plus 30 days. Online, the first check ends it within
  seconds.

## Alternatives considered

- **The ceiling alone, with no Vault copy.** Not chosen: a paying monthly customer who lost the
  record while offline would wait for the internet to get Pro back.
- **Keep the whole record in the Vault.** Not chosen: the record's clock times change every few
  minutes, and the Vault is not made for frequent writes. The entry time and the newest answer
  change only when a key is entered and once a week.
- **Apply the ceiling to an "unknown" answer too.** Not chosen: it would let 8 West's own mistake
  take Pro from a paying customer, which ADR-022 §3 forbids.

Plenipo is made by 8 West Ventures, LLC.
