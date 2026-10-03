# ADR-164: Private messages, sealed end to end

- **Status:** Accepted, with the owner's change (2026-10-02): question 1 (sealed end to end) "as
  recommended"; question 8 (what a message may hold) "**Everything that's on a phone's keyboard**",
  instead of plain text only (§4); and question 15 "**Allow gifs and stickers**", in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-143 (the relay and the lock)](ADR-143-the-relay-and-the-lock.md), which seals
  everything between your PC and your phone, and [ADR-162 (your account in Plenipo)](ADR-162-your-account-in-plenipo.md)
  for each PC's Community key

> **On screen** (ADR-010, plain words and rank names): **Messages**, **New message**, **Requests**
> (from people you have not talked with), **Accept**, **Block**, **Report**, **Sealed: only you and
> Pat can read this**, **Check the safety code**, **Pat's computers changed**, **Delete for me**,
> **Waiting to be delivered**, **Delivered**, **GIF**, **Stickers**, **Photos can't be sent in
> Community**.

## In short

A private message is **sealed on your PC for the other person's PCs**, so only the two of you can
read it. 8 West carries it and cannot read it. It waits on 8 West's server only until the other
person's PCs have picked it up, then it is deleted there (at most 30 days). A message holds
**everything a phone's keyboard types**: letters, numbers, symbols, every emoji, in any language,
**GIFs** from a GIF library, and Plenipo's own **stickers**. No photos, and no files. Someone you have never talked with lands in **Requests**, and can
send one message until you **Accept**. If you **Report** a message, your PC sends that message to
8 West with a proof that it is real, so a report cannot be made up and only what you report is seen.

## Context

- Phase 24's plan: "private messages between people", and "this phase's ADR decides whether
  private messages are end-to-end encrypted".
- Phase 14 already seals everything between your PC and your phone end to end (ADR-143), and the
  relay keeps nothing. Messages are different: the other person's PC may be off for days, so
  something must hold a message until it is picked up. The account service can, if it holds only
  sealed messages.
- Sealing end to end means 8 West cannot look at messages to stop abuse. Reports have to bring the
  message with them, and must not be fakeable.
- A person may run Plenipo on more than one PC (ADR-110), each with its own Community key
  (ADR-162 §3).
- Other people's words are untrusted input for workers (plan §3.1).

## Decision

1. **Sealed end to end** (question 1). Each message is sealed on the sender's PC, once for each of
   the other person's PCs and once for each of the sender's own other PCs (so your other PCs show
   what you sent).
   - **How:** HPKE (RFC 9180) with X25519, HKDF-SHA256, and AES-256-GCM, to each PC's Community
     key. Inside the seal, the message is signed with the sender's Ed25519 key, so the receiver
     knows who wrote it and that nothing changed.
   - **Each side's tests pass the published HPKE test answers**, then talk to each other, as the
     Noise tests did for Phase 14.
   - The account service sees only the **envelope**: who to, who from, when, the size, and the
     report proof's tag (item 6). Never the words.
2. **Whose keys.** The account service lists each person's PC keys. Because 8 West hands them out,
   each conversation has a **safety code** (12 digits made from both people's keys) that you can
   compare with the other person by phone or in person (**Check the safety code**). When their keys
   change, the conversation says "**Pat's computers changed**". A changed key never stops a
   message; it only tells you.
3. **Held only until picked up.** The account service keeps a sealed message until every PC it is
   sealed for has picked it up, or 30 days, whichever comes first, then deletes it (ADR-168). Your
   PC calls out to pick messages up while Community is on; nothing listens on your PC.
4. **What a message may hold** (question 8, the owner: "Everything that's on a phone's keyboard"):
   - **every letter, number, symbol, and emoji**, in any language and any writing direction, up to
     4,000 characters (an emoji counts as one). What people say into a keyboard's microphone arrives
     as these words too;
   - **emoji reactions** on a message (one of a short list, such as 👍 ❤️ 😂 😮 🙏), sealed like the
     message;
   - **GIFs** (question 15, the owner: "Allow gifs and stickers"), from a **GIF** button:
     - they come from one **GIF library** (a service such as GIPHY, chosen when this is built and
       named in the privacy notice). Plenipo's search goes through the account service, which holds
       the library's key, so no key is in Plenipo, and which asks only for GIFs rated **PG-13 or
       milder**, and **G or PG** for members under 18 (ADR-162 §4);
     - the sealed message carries only the GIF's ID in that library, never a picture or an address.
       The receiving PC fetches the GIF from the library's own picture address. Guard allows that
       one address and nothing else (a new purpose, **GIFs**, while Community is on), so a message
       can never make Plenipo fetch anything else;
     - the library sees which GIFs a PC fetches, and from what internet address, like any website;
       the privacy notice says so;
   - **stickers:** Plenipo's own sets, drawn for Plenipo and built into each release. A message
     carries the sticker's name; nothing is fetched or loaded at run time (ADR-014);
   - **not photos or pictures from your PC.** A pasted or dropped picture is refused with
     "**Photos can't be sent in Community**": a sealed picture is one 8 West could never check, and
     some members are 13. GIFs and stickers come only from the library and Plenipo's own sets;
   - **never:** files, voice recordings, or link previews;
   - Plenipo shows every character safely: hidden control characters that could disguise words
     (such as right-to-left overrides) are shown as visible marks, and a message is always shown as
     text, never as a web page. A web address shows as text; opening it asks
     "**Open this link in your web browser?**" and opens your own browser, never Plenipo's.
5. **One to one only** at launch. No group conversations.
6. **Reports carry a proof** ("message franking", as large sealed messengers use):
   - The sender puts a fresh random key inside the seal, and a tag made from that key and the words
     on the envelope. The account service stamps the envelope (who, who to, when, the tag) with its
     own signature when it accepts it, and the stamp travels with the message. The stamping key is
     its own Ed25519 key, never the license signing key (ADR-104).
   - When you report a message, your PC sends the words, the key, and the stamp. The account service
     checks that the words match the tag and its own stamp, so it needs to keep nothing after
     delivery. So a report cannot make up a message,
     and 8 West reads only the messages you chose to report (ADR-167).
7. **People you have not talked with land in Requests.** Their first message waits there with
   **Accept**, **Block**, and **Report**. Until you accept, they cannot send another. Only Pro can
   start a new conversation (ADR-162 §5).
8. **Limits against flooding** (the account service enforces them; numbers are a starting point):
   at most 20 new conversations a day per account, and 60 messages a minute per account.
9. **Messages are for people, not workers.** No worker can read your messages. **Give to a worker**
   on a message sends its text to a worker you choose, marked as **outside words** (like an email in
   Phase 20), so the worker treats it as information, never as orders.
10. **Kept on your PC** in the PC's shared record (with your tile and Workforce, ADR-094 §5), in the
    Ledger's backups and exports. Messages do not appear in Activity. **Delete for me** removes a
    message from your PCs. There is no "delete for everyone": the other person's copy is theirs.
11. **The Ledger records** that a conversation started, was accepted, blocked, or reported
    (`community.conversation_*`), never the words of a message.
12. **Logs never hold** a message, a key, a tag, or a stamp.
13. **New desktop commands are the main window's alone**, with IPC tests that refuse a second
    window, the sign, and a web page. The phone's fixed list (ADR-145) does not change (ADR-161).

## Consequences

- If your PCs are lost and you have no backup, your messages are gone: 8 West never had them.
- 8 West sees abuse only when someone reports it. The rules and the report process (ADR-167) say so
  plainly, and the attorney reviews that.
- No ratchet (a new key for every message, as Signal uses) at launch. If a PC's Community key were
  stolen, it could open messages still waiting for that PC (at most 30 days), as well as what is on
  that PC already. MLS (RFC 9420) could add this later, with groups.

## Alternatives considered

- **Encrypted on the way and on 8 West's server, but 8 West can read them.** Easier moderation, but
  8 West would hold every conversation, be a target for it, and have to answer requests for it.
- **The relay instead of the account service.** The relay keeps nothing by design (ADR-143), so a
  message to a PC that is off would be lost.
- **Any picture, including photos from your PC.** Much more risk (malware, illegal images, a duty to
  scan), and 8 West could never check a sealed one; GIFs from a rated library and Plenipo's own
  stickers give the fun without it.
- **GIFs uploaded inside the message** instead of a library's ID. The same risk as photos.
- **Plenipo calling the GIF library with its own key.** The key would be in every copy of Plenipo,
  and the rating for young members could not be enforced by the account service.
- **Reports without a proof.** Anyone could report a message nobody sent.
