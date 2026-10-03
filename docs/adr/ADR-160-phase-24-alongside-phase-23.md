# ADR-160: Building Phase 24 (Community) alongside Phase 23 (Mac and Linux)

- **Status:** Accepted (2026-10-02). The owner asked "can we build the community phase while you
  make the mac and linux versions?", heard the builder's answer (yes, in its own session, records
  and the attorney's drafts first, and Guard's request path after Phase 23's Wave 1 Guard work
  merges), and then asked: "write that decision record and a ready-to-paste prompt for the
  Community session."
- **Date:** 2026-10-02
- **Phase:** plan change, during Phase 23 (Wave 0 delivered; Wave 1 in pull requests #136 and #138)
- **Amends:** [ADR-132 (the final push)](ADR-132-the-final-push.md) §1: Phase 24 no longer waits
  for Phase 23 to be delivered; it is built beside it.
- **Number:** Phase 23 uses ADR-150 to ADR-159
  ([ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)). So that the two sessions never pick
  the same number, **Phase 24 uses ADR-160 to ADR-169**, starting with this one.
- **Amended by:** [ADR-171 (switching Community on part by part)](ADR-171-switching-on-part-by-part.md):
  a security review at the highest effort before **each** part of Community is switched on (§7).

> **On screen:** nothing. This record only changes when Phase 24 is built.

## In short

The owner told Plenipo's builder to start Phase 24 (**Community**: profiles, private messages,
linked organizations, collaborators, and block, report, and leave) now, while another session
builds Phase 23 (**Mac and Linux**). Accepting this record means the order-of-work table shows
Phase 24 as **in progress, beside Phase 23**, and the rule "work only on the earliest incomplete
phase" allows it, because the owner said so. Phase 24 starts with its own records and the drafts
the attorney must review, and builds the part that reaches Guard last.

## Context

- **The order of work** (ADR-132, the final push) is: Phase 14 (delivered), then **Phase 23**
  (in progress), then **Phase 24**. Rule §8.3 of `ROLLOUT_PLAN.md` says: "Work only on the
  earliest incomplete phase **unless explicitly instructed otherwise**." The owner's question and
  request above are that instruction.
- **Phase 24's dependencies are met:** the account service (Phase 22, live), the signed-in
  connection from Plenipo's own web interface (Phase 14, delivered), and profiles (Phase 18,
  delivered). It does not need Phase 23.
- **Phase 24's slowest part is not code.** The plan requires terms of service, an age
  requirement, a moderation process, and the privacy policy, reviewed by an attorney before
  launch. Writing those drafts now starts that clock.
- **The two phases mostly touch different code:**
  - **Phase 23 touches** each system's own parts: `crates/runtime` (program trees, the keeper),
    `crates/capabilities` (the Mac's ticket check, the terminal, the browser, the Vault),
    `crates/guard` (program names, the risky-program lists), the desktop shell
    (`apps/desktop/src-tauri`), CI, and the release workflow.
  - **Phase 24 touches** the account service (its own private repository, `plenipo-account`,
    ADR-101) and its contract in this repository (`contracts/`), the phone line and relay
    (`crates/remote`, `crates/relay`, `crates/relay-contract`), new screens in
    `apps/desktop/src`, the Ledger's events, and, for objectives from a linked organization and
    a collaborator's actions, **Guard's request path**.
  - **Both touch** a few shared files: `crates/guard`, `apps/desktop/src-tauri/src/lib.rs`, the
    generated types, `ROLLOUT_PLAN.md`, the roadmap, `docs/design/vocabulary.md`, the ADR index,
    the package versions, and `Cargo.lock`.
- **The owner's PC cannot build two copies of Plenipo at once.** On 2026-10-02 the compiler
  crashed twice while two full builds ran side by side (crates "required to be available in
  rlib format", "only metadata stub found"), and two worktrees sharing one build folder gave one
  of them the other's copy of a crate.
- **Phase 24 is the riskiest phase left:** other people's words and requests reach the owner's
  workers. The plan treats them as untrusted input (plan §3.1), like email in Phase 20.

## Decision

1. **Phase 24 is built now, beside Phase 23,** in its own session, from its own branches
   (`claude/phase-24-…`), with its own checklist (`docs/phases/phase-24-checklist.md`) and
   acceptance report, using ADR-160 to ADR-169.
2. **Records and drafts first, before any code.** The Community session first writes Phase 24's
   starting record (what the check found, its parts, and the owner's questions: whether private
   messages are sealed end to end, how long anything is kept, and how reports are handled and by
   whom), as ADR-140 did for Phase 14 and ADR-150 for Phase 23. With it come the drafts for the
   attorney: the terms of service, the privacy policy's changes, the age requirement, and the
   moderation process. The drafts live in `docs/legal/phase-24/` until the attorney approves
   them; only then do they change the website's published pages (`apps/website/legal/`), which go
   live with the next website update. Coding starts after the owner answers.
3. **Never two builds at once on the owner's PC.** The Community session builds in the cloud
   (Claude Code on the web) or on another computer. If it must build on the owner's PC, it builds
   only while Phase 23's session is not building, in its own worktree with its own build folder
   (`target\wt-<name>`).
4. **Guard's request path waits for Phase 23's Wave 1.** Phase 24's code that brings a linked
   organization's objective or a collaborator's action through Guard starts after pull requests
   #136 (Guard on a Mac and Linux) and #138 (the keeper) are merged. Profiles, private messages,
   block, report, and leave, the screens, and the account service's side can start before.
5. **Shared files: whichever merges second merges `main` and keeps both sides,** as the owner's
   standing rule says; generated types are made again with `pnpm bindings`, never by hand. Each
   phase updates only its own row in the order of work and the roadmap. Each release takes the
   next free version number when it is made.
6. **The account service's side** is built in `plenipo-account`, under its own rules (ADR-101,
   ADR-102). Anything both sides rely on is written first as a contract in this repository
   (`contracts/`), as the license check and the phone relay were.
7. **A security review at the highest effort** checks Phase 24 before anything reaches real
   people, and again after the attorney's changes.

## Consequences

- Community is ready sooner, and the attorney's review runs while Mac and Linux are built.
- Two sessions spend about twice as much while both run.
- More merging of shared files; each phase keeps to its own rows and numbers.
- Phase 23's Wave 1 Guard changes land first, so Guard's most sensitive code never takes two
  sets of changes at once.

## Alternatives considered

- **Wait for Phase 23 to be delivered.** Slower, and the attorney's review would wait too.
- **One session for both.** Slower, and it mixes two risky kinds of work in one review.
- **Build Phase 24's Guard path now.** It would collide with Phase 23's Wave 1 in Guard's rule
  matching and the broker, the most security-sensitive files.
