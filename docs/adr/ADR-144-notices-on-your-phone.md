# ADR-144: Notices on your phone when the page is closed, sealed so only your phone can read them

- **Status:** Accepted (by the owner, 2026-10-01, as recommended): notices go **straight from the PC** (question 6),
  with the buttons of question 3.
- **Date:** 2026-10-01
- **Phase:** 14 (part 14C)
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Amends:** [ADR-040 (Phase 14 is Plenipo's own web interface for a
  phone)](ADR-040-phone-web-interface.md) §5, "by web push through the relay": your PC sends each
  sealed notice **straight to your phone's notice service** (Apple's, Google's, Mozilla's, or
  Microsoft's), and the relay never handles notices. Everything else in §5 stands.

> **On screen** (ADR-010, plain words and rank names): **notice** (never "push notification"),
> **Notices on this phone** (on / off), **On the lock screen: Show what it is / Show only "Something
> needs you"**, **Add Plenipo to your Home Screen** (iPhone), **Approve** / **Refuse**.

## In short

When something needs you, your PC sends your phone a **notice**, even when Plenipo's page is closed.
It says one short line, like "Approve: git push to Website". The line is **sealed for your phone
alone**: not 8 West, not the relay, not Apple, not Google can read it. On your phone you can choose
to show only "**Something needs you**" on the lock screen. On an iPhone, you add Plenipo's page to
your Home Screen first; the page shows you how.

## Context

- A web page can get notices while it is closed through **web push**: the phone's browser gives the
  page an address at its notice service (Apple's for Safari, Google's for Chrome, Mozilla's for
  Firefox, Microsoft's for Edge), plus two keys. Whoever holds those can send the phone a notice,
  sealed so the notice service cannot read it (the web's standard sealing, RFC 8291).
- A notice service only takes notices signed by the key the page named when it signed up for
  notices (VAPID, RFC 8292). So the PC holds that key, and nobody else can send your phone a
  Plenipo notice.
- Every notice must show something on the phone. Apple and Google stop notices for a page that
  receives them silently.
- **iPhone:** web notices work only for a page added to the Home Screen (iOS 16.4 and later), and
  have no buttons. **Android:** up to two buttons.
- Plenipo's PC already decides which events deserve a notice (`crates/ledger/src/notices.rs`):
  approvals waiting, a check that needs you, problems, finished work, lessons, Plenipo itself, and
  spending. Each kind can be turned off.
- ADR-040 said notices go "through the relay". The relay does not need to: the PC is online whenever
  it has something to tell you, and it can reach the notice services itself.

## Decision

1. **Your PC sends each notice straight to your phone's notice service** (recommended). Guard gains
   the purpose **phone notices**, which allows only the four notice services' fixed addresses, only
   on Pro, with phone access on, and only to an address one of your phones gave the PC.
2. **Signing up for notices.** After pairing, the phone page asks to send notices (**Notices on this
   phone**). The phone signs up with the PC's own **notice key** (made by the PC, kept in the Vault),
   and sends its notice address and keys to the PC **inside the sealed line** (ADR-143). The relay
   never sees them. Removing a phone, or switching phone access off, forgets them.
3. **What a notice holds:** what kind it is, its ID, which organization, and **one short line** in
   plain words (for example "Approve: git push to Website", "Waiting for you: 3 approvals", "Stopped:
   everything", "A lesson to keep or discard"). Never a secret, a file's contents, or a command's
   output. The line is made from the approval card, which Guard has already cleaned of secrets
   (ADR-013 §11).
4. **Sealed for your phone alone**, with the web's standard sealing (RFC 8291), using the keys only
   your phone and your PC hold. The notice service, and 8 West, see only a sealed blob.
5. **The lock screen is your phone's choice:** **Show what it is** (the line), or **Show only
   "Something needs you"**. The choice lives on the phone, and the phone applies it when the notice
   arrives, before anything shows.
6. **Buttons** (ADR-142 §5): on Android, an approval's notice has **Approve** and **Refuse**; Stop
   all's has **Allow again**; a lesson's has **Keep** and **Discard**. **Refuse** and **Discard** are
   sent from the notice; **Approve**, **Allow again**, and **Keep** open that item (the phone unlocks
   first, and Plenipo asks you to sign in if you are not), where one tap answers it. On an iPhone, a
   tap opens that item.
7. **Which notices go to the phone:** the same kinds your PC shows, with the same on and off switches
   in **Settings → Notifications**, plus a switch for the phone as a whole. Notices that come
   together are joined, as on the PC.
8. **An old notice cannot fool you.** Opening a notice always shows the item as it is now: an
   approval already answered shows "**Already answered on your PC**" (or on another phone). A notice
   sent again by mistake is shown once.
9. **iPhone.** The page shows how to **Add Plenipo to your Home Screen**, and that pairing and
   notices happen from there (ADR-141).
10. **Recorded:** `remote.notice_sent` and `remote.notice_failed` (the kind and the phone, never the
    line), and the phone's notice address is forgotten when its notice service says it is gone.

## Consequences

- The relay never handles notices, so the relay change is smaller (only passing sealed messages).
- Your PC talks to Apple's, Google's, Mozilla's, or Microsoft's notice service. They see your PC's
  internet address and when a notice is sent, and they cannot read it.
- A notice can only come while your PC is on, which is also the only time it could be answered.

## Alternatives considered

- **Through the relay, as ADR-040 first said.** Just as sealed, but the relay must keep each phone's
  notice address and call the notice services, so the relay change is bigger, and the shared server
  does more for Plenipo. Not recommended.
- **No line, only "Something needs you", always.** Simplest, but the plan wants the line, and the
  sealing keeps it private.
- **A phone app instead of web notices.** ADR-040 chose no phone app.
