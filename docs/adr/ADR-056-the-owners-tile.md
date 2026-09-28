# ADR-056: The owner's tile — your picture, status, mood, and message

- **Status:** Accepted (by the owner, 2026-09-28, as recommended)
- **Date:** 2026-09-28
- **Phase:** 18
- **Carries out:** ADR-039 (the owner's notes) §3 (Phase 18: "the owner's own tile (avatar,
  status light, mood, short message)")

> **On screen** (ADR-010, plain words and rank names): **Your picture**, your **status**
> (**Available**, **Busy**, **Away**, **Do not disturb**), your **mood**, and your **message**.
> This record keeps the plan's words (avatar, profile).

## In short

Your tile on the canvas, and a small button in the top bar, show your picture, a status light
(Available, Busy, Away, Do not disturb), a mood, and a short message such as "Feeling great!".
You choose the picture with Windows' own "Open" box; Plenipo keeps a small copy on this PC.
Nothing leaves your PC — sharing a profile with other people is Phase 24. While you are on **Do
not disturb**, Windows pop-up notices wait, and the bell still counts them. Accepting this
record means building it as written below.

## Context

Phase 18 of `ROLLOUT_PLAN.md`: "the owner's tile: an avatar (a picture kept on this PC), a
status light (available, busy, away, do not disturb), a mood picker, and a short message such as
'Feeling great!' — shown on the canvas and in the top bar. Local only until Phase 24." Its test:
"the owner's avatar, status, mood, and message are saved and shown". Out of scope: "other
people's profiles, and showing the owner's profile to anyone (Phase 24)".

Today (v1.10.0) your tile shows a glyph, "You", and your rank. Nothing about you is stored
except the organization's name and the rank names you chose (the Ledger's `organization`
setting). Windows notices (Phase 12) have their own settings; nothing pauses them.

## Decision

1. **What you set:**
   - **Your picture:** optional; without one, your tile shows the owner glyph.
   - **Status:** Available (the default), Busy, Away, or Do not disturb — a light **and** a word.
   - **Mood:** none, or one of Great, Good, Okay, Tired, Stressed, Focused, Celebrating — a small
     face **and** a word.
   - **Message:** up to 80 characters, one line, plain text.
2. **Where it shows:** your tile on the canvas (picture, name, status, mood, message) and a
   button in the top bar (picture and status light) that opens a small panel to change them.
3. **Choosing the picture:** **Choose a picture…** opens Windows' own "Open" box in Plenipo's
   window. The window reads the file you picked, cuts it square, and shrinks it to 256 × 256
   pixels; Plenipo receives only that small picture. **Plenipo never opens a file path itself**,
   no worker can reach the picture, and it is never sent anywhere. Plenipo checks what it
   receives is a real picture (a PNG it can read, at most 256 × 256 and 256 KB) before keeping
   it. **Remove picture** takes it away.
4. **Guard is for workers, not for you.** Your own choice of picture, like your own terminal
   (ADR-031 §3), is yours: Guard checks what workers do. This adds no path for any worker to
   files, programs, the network, the browser, or the screen.
5. **Where it is kept:** in the Ledger's settings (`owner`), so it is in your backups and exports,
   and follows a restore. Changing it is recorded (`owner.profile_changed`: which parts changed,
   and your new status and mood; never the picture). Like your Workforce (ADR-045 §11), it lives
   in this organization's Ledger until Phase 21 adds more organizations, which moves it where
   every organization shares it.
6. **Do not disturb** holds Windows pop-up notices while it is on; each still waits in the bell,
   and approvals still wait for you. The other statuses change nothing but the light.
7. **Not in the diagnostics file,** like everything else of yours (Phase 13): not the picture, not
   the message.
8. **New desktop commands, the main window's alone:** `get_owner_profile` and
   `set_owner_profile` (status, mood, message, and the picture, or none), refused from the sign
   window and from any web page, with IPC tests; each input is checked (the status and mood from
   their lists, the message's length, and the picture as in §3).

## Your choices (recommended first)

- **Do not disturb holds Windows notices** (the bell still counts them). _Or:_ it only changes
  the light.
- **Keep your profile in the Ledger** (backed up with your organization). _Or:_ a file in
  Plenipo's own folder — shared by every organization from the start, but not in your backups.

## Consequences

- Your tile says who you are and whether you are around, at a glance, on your own PC.
- A small picture (tens of kilobytes) is kept in the Ledger.
- Phase 24 can share the same fields through your 8 West account, only if you turn it on.

## Alternatives considered

- **Let Plenipo open the picture from a path.** Rejected: the window's own "Open" box gives
  Plenipo only what you picked, and Plenipo never needs file access for it.
- **Status set automatically from the mouse and keyboard.** Not chosen: the plan asks for a light
  you set, and watching your mouse and keyboard is not needed for it.
- **Emoji only for mood.** Rejected: a face alone can be misread; each mood also has a word.

## As built (v1.11.0)

Built as written. The status light has its own shape for each status as well as its color, and
its word is always there. The window shrinks the picture to at most 256 × 256 and, when the
PNG would be over 256 KB, tries smaller sizes before giving up with a plain reason. Your tile on
the canvas is named for screen readers with your status, mood, and message ("You, President:
Busy, feeling Great, “Feeling great!”").
