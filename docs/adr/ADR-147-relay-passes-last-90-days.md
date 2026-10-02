# ADR-147: Relay passes last 90 days, renewed at every sign-in

- **Status:** Accepted (2026-10-01, by the builder, under the owner's standing order for Phase 14:
  "free to code, commit, push, merge and release"; a deviation from an accepted record needs its own
  record, so here it is). The owner may reverse it.
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Amends:** [ADR-143 (the relay and the lock)](ADR-143-the-relay-and-the-lock.md) §4: "an end
  date 7 days away" becomes 90 days. Everything else in §4 stands.

> **On screen** (ADR-010, plain words and rank names): nothing. A phone that has not been used for
> a long time still connects.

## In short

A paired phone shows the relay a **pass** its PC signed. ADR-143 said a pass lasts 7 days, and the
PC renews it over the sealed line. But a phone can only reach its PC with a good pass, so a phone
not opened for a week could never renew it: the owner would have to pair it again. A pass now lasts
**90 days**, and the PC still renews it at every sign-in.

## Context

- The pass only keeps strangers from reaching the PC through the relay. **The lock is what keeps
  them out** (ADR-143 §4): a stranger with a pass but not the phone's own key fails the very first
  message, and a removed phone is refused by the PC whether or not its pass still works at the
  relay.
- Removing a phone tells the relay to refuse its pass until the pass would have ended (the relay
  keeps a short list of dropped passes).
- Owners go on holiday, or only use the phone when something needs them.

## Decision

1. **A pass lasts 90 days** from when the PC issues it.
2. **The PC issues a fresh pass at every sign-in** (and at pairing), so a phone in use never comes
   near the end.
3. **The relay keeps a dropped pass's phone on its list for up to 90 days** (until the pass would
   have ended), as the change request says.

## Consequences

- A phone not used for up to 90 days connects as before, and signs in with its passkey.
- After 90 days unused, the owner pairs it again.
- The relay's list of dropped passes can hold entries for up to 90 days. It is small: one line per
  removed phone.

## Alternatives considered

- **7 days, as ADR-143 first said.** A quiet week would mean pairing again.
- **No end date.** A copied pass would work at the relay forever. Still harmless to the PC, but the
  relay's list of dropped passes would grow forever.
- **Let a phone with an expired pass ask for a new one.** The relay would have to pass messages for
  phones it cannot vouch for, which is what the pass is there to stop.
