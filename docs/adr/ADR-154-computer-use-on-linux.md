# ADR-154: Computer use on Linux — X11 in Wave 2, Wayland in Wave 4

- **Status:** Accepted (by the owner, 2026-10-02). The owner first chose "X11 now, Wayland later" as
  recommended. The builder then found that Ubuntu 26.04 LTS has no X11 desktop at all, so "later"
  would leave computer use off on the newest Ubuntu; the owner chose to build Wayland in Wave 4, as
  recommended.
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Keeps:** [ADR-020 (browser and computer use)](ADR-020-browser-and-computer-use.md) and
  [ADR-049 (computer use asks every step)](ADR-049-computer-use-asks-every-step.md)

> **On screen** (Wave 2, under Wayland): "Computer use doesn't work on this desktop yet." (Wave 4,
> Wayland): the desktop's own window asks the owner to share the screen and allow control, and
> Plenipo explains it first.

## In short

Computer use means a worker can see the screen and use the mouse and keyboard, asking the owner at
every step. On Linux it works today only on the older X11 desktop. Ubuntu's newest desktop uses
Wayland, which does not let one program watch or control the screen unless the desktop asks the
person first. Plenipo will support X11 in its first Linux version, and Wayland before Phase 23 is
done.

## Context

- Plenipo's Linux code takes screenshots and moves the mouse through X11 (`x11rb`, and `enigo` over
  XTest) in `crates/capabilities/src/desktop.rs`.
- **Wayland** is Ubuntu's default desktop since 2021. Under Wayland, X11 calls see only old X11
  windows, not the real screen. A program must ask through the desktop's **portals**: the desktop
  shows its own window asking the person to share the screen and allow control, once per session.
- **Ubuntu 26.04 LTS** (April 2026) runs GNOME on Wayland only; it has no X11 desktop to choose.
  Ubuntu 22.04 and 24.04 still offer "Ubuntu on Xorg" at the sign-in screen.
- ADR-049 already asks the owner before every step, so the desktop's own question is one more
  check, not a change of rule.

## Decision

1. **Wave 2 (Linux first look): X11.** Computer use works when the owner signs in to an X11 desktop.
2. **Under Wayland in Wave 2,** Plenipo refuses computer use and says plainly that it does not work
   on this desktop yet, and that on Ubuntu 22.04 and 24.04 "Ubuntu on Xorg" works. It never sends a
   worker a screenshot that shows only part of the screen.
3. **Wave 4: Wayland**, through the desktop's portals (screen sharing and remote control). Plenipo
   explains the desktop's question before it appears and never works around it; every step is still
   asked (ADR-049). Wave 4 is not done, and Phase 23 is not delivered, until computer use works on
   Ubuntu 26.04.
4. **Screen pixels and mouse points** are matched on each desktop, so a click lands where the
   screenshot showed (screens that scale up have more pixels than points).
5. **The Mac** is separate work in Wave 3 (ADR-150).

## Consequences

- The Linux first look ships without waiting for Wayland.
- Between Waves 2 and 4, computer use is off on Ubuntu 26.04 and on 24.04's default session.
- Wayland support adds a medium-size piece to Wave 4, with tests that need a Wayland desktop.

## Alternatives considered

- **Wayland in Wave 2.** The Linux first look would wait for it.
- **Wayland after Phase 23.** Computer use would stay off on the newest Ubuntu for a long time.
- **Working around Wayland's question** (for example, through accessibility tricks). Never: the
  owner must see and answer the system's own question.
