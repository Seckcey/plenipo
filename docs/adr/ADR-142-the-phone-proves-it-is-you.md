# ADR-142: The phone proves it is you — a passkey, with your face, fingerprint, or passcode, checked by your PC

- **Status:** Accepted (by the owner, 2026-10-01), with one change from the builder's
  recommendation: the face, fingerprint, or passcode check is asked **only at sign-in** (the owner's
  answer to question 2: "Only at sign in"), not again for each approval. The notice buttons are as
  recommended (question 3), and so is how long you stay signed in (question 4).
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Answers:** the plan's "The phone confirms it is the owner first (its passcode, face, or
  fingerprint); the phase's ADR settles how that works from a notice"

> **On screen** (ADR-010, plain words and rank names): **Check it's you** (the button that brings up
> the phone's face, fingerprint, or passcode check), **Signed in** / **Sign out**, **Set a screen lock
> on your phone first**, **Approve on your PC** (ADR-145).

## In short

When you pair a phone, it makes a **passkey** for Plenipo: a key that lives in the phone's own
keychain and only works after your face, fingerprint, or phone passcode. Your **PC** checks it, not
the relay and not 8 West. You use it to **sign in**. While you are signed in (30 minutes after your
last tap, 12 hours at most), the phone can answer approvals without asking again.

From a notice: on **Android**, **Refuse** works right on the notice, and **Approve** opens Plenipo on
that approval (the phone unlocks first, and Plenipo asks you to sign in if you are not). On an
**iPhone**, a web notice has no buttons, so one tap opens that approval.

## Context

What phones allow today (checked against the browsers' published support, 2026-10-01; part 14A and
14C check each on a real phone):

- **Passkeys (WebAuthn) work in the phone's browser.** Android's Chrome uses the screen lock: face,
  fingerprint, pattern, or PIN. An iPhone's Safari, and a page saved to its Home Screen, use Face ID,
  Touch ID, or the passcode. A website can ask that the person be checked (`userVerification:
"required"`), and the answer says whether they were.
- **A passkey may sync** to the owner's other devices through Apple's or Google's keychain. That is
  safe here, because signing in also needs the phone's own key, which never leaves that browser
  (ADR-143): a synced passkey alone opens nothing.
- **A notice cannot run the check.** A notice's button runs in the page's background part (its
  service worker), which has no passkey support. The check needs the page itself, on screen.
- **Android's web notices can have two buttons.** **An iPhone's cannot**, as far as Apple documents
  today; tapping the notice opens the page.
- **Some phones need one tap before the check.** Safari may refuse to start a passkey check that
  the person did not start with a tap. Part 14C checks this on real phones.
- **A phone with no screen lock cannot make a passkey** that checks the person.

## Decision

1. **A passkey for each phone**, made right after pairing (ADR-141), with
   `authenticatorAttachment: "platform"`, `userVerification: "required"`, and Plenipo's page address
   as its site (ADR-146). Only its public half goes to the PC. A phone with no screen lock is told
   "**Set a screen lock on your phone first**".
2. **Your PC checks every passkey answer itself:** the address it came from, the site, the
   challenge, the signature, and the flag that says the person was checked. An answer without that
   flag is refused. A signature counter that goes backwards is refused; a counter that is always 0
   (common for synced passkeys) is allowed.
3. **Signing in needs the check, and nothing else does** (the owner's choice). Opening the page
   starts a sealed meeting (ADR-143), and the PC asks for a passkey answer over that meeting's own
   fingerprint, so a sign-in cannot be lifted into another meeting. **You stay signed in for 30
   minutes after your last request, and 12 hours at most.** A dropped connection inside that time
   picks up again without a new check, because it needs the phone's own key too.
4. **While signed in, the phone may make any request on the list** (ADR-145): read, **Approve**,
   **Refuse**, **Allow again**, **Run again**, **Leave stopped**, **Keep** or **Discard** a lesson,
   stop work, **Stop all**, and send an objective. Each still goes through Guard on the PC.
5. **From a notice (part 14C):**
   - **Android:** an approval's notice has **Approve** and **Refuse**. **Refuse** is sent from the
     notice by the page's background part, sealed with the phone's own key (ADR-143); it needs no
     sign-in, because it only says no. **Approve** opens Plenipo on that approval. Opening it makes
     the phone unlock first (the phone's own check), Plenipo asks you to sign in if you are not, and
     one tap on **Approve** answers it. A link that opens the page never approves anything by itself.
   - **iPhone:** one tap on the notice opens that approval, with **Approve** and **Refuse**.
   - **Allow again** after Stop all, and a lesson's **Keep**, work the same way as **Approve**; a
     lesson's **Discard** works like **Refuse**.
6. **Too many failed checks pause the phone.** Three passkey answers the PC refuses within 10
   minutes pause that phone until you un-pause it on the PC (Settings → Devices), with a notice on the
   PC. The phone's own face and fingerprint retries are the phone's business.
7. **Recorded:** `remote.signed_in`, `remote.signed_out` (with why: you, idle, 12 hours, removed,
   switched off, Pro ended), `remote.check_refused`, and `remote.device_paused`, with the device.
   Never a passkey answer or a challenge.

## Consequences

- A stolen, locked phone can do nothing: the page needs a sign-in with your face, fingerprint, or
  passcode, and a notice's **Approve** needs the phone unlocked.
- **The risk the owner accepted:** someone holding your **unlocked** phone within 30 minutes of your
  last use could approve what is waiting. Lock your phone when you put it down, and use **Sign out**
  or **Remove** on the PC if it is lost.
- The owner sees the face or fingerprint prompt once per sign-in, not for every approval.
- Approving from an Android notice is two taps (the notice, then **Approve**), not one. A link alone
  never approves, so a trick link cannot.
- The PC needs code to check a passkey answer. It uses the signature libraries already in the build
  (`p256` for most phones, `ed25519-dalek` for the rest), with test answers made by a stand-in
  authenticator.

## Alternatives considered

- **A fresh check for each approval, made for that one answer** (the builder's recommendation). It
  would stop someone holding your unlocked phone, at the cost of a prompt for every approval. Not
  chosen by the owner.
- **Trust the phone's own lock screen** (a notice's **Approve** answers at once). One step, but the
  PC could not tell who tapped it, and some phones run notice buttons from the lock screen. Not
  chosen.
- **A PIN typed into Plenipo's page.** It works on any phone, but it can be watched over a shoulder,
  and the page would have to keep the PIN safe itself. The phone's own check is stronger and already
  set up.
- **The 8 West account's sign-in.** It would put 8 West between you and your PC.
