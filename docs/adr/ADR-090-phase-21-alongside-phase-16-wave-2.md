# ADR-090: Building Phase 21 alongside Phase 16's second wave

- **Status:** Accepted (2026-09-30). The owner's own instruction: "phase 16 wave 2 is already in
  work in another session. write the ADR first as a small docs-only pull request and then give me
  a prompt to start phase 21 in a new session."
- **Date:** 2026-09-30
- **Phase:** plan change, after Phase 20 (delivered in v1.14.2, pull request #104)
- **Amends:** [ADR-039 (the owner's notes and the order of work)](ADR-039-owners-notes-order-of-work.md)
  §1, the order-of-work table, as [ADR-061 (doing Connections before new AI models)](ADR-061-connections-before-new-ai-models.md)
  left it: Phase 21 starts now, beside Phase 16, instead of after it.
- **Number:** Phase 16 uses ADR-080 and up
  ([ADR-080 (Phase 16's first wave alongside Phase 20)](ADR-080-phase-16-wave-1-alongside-phase-20.md)),
  and Wave 2 adds one decision record per AI tool. So that the two sessions never pick the same
  number, Phase 21 uses **ADR-090 to ADR-099**, starting with this one, and Phase 16 keeps ADR-083
  to ADR-089.

> **On screen:** nothing. This record only changes when Phase 21 is built.

## In short

The owner told Plenipo's builder to start Phase 21 (**the workspace: panels, windows, files, and
more than one organization**) now, while another session builds Phase 16's **second wave**
(Cursor's agent, and GitHub Copilot's second try). Accepting this record means the order-of-work
table shows Phase 21 as **in progress, beside Phase 16**, and the rule "work only on the earliest
incomplete phase" allows it, because the owner said so. Phase 21 builds its safe parts first and
**more than one organization** last.

## Context

The order of work (ADR-039, changed by ADR-061 and ADR-080) is: 13, 17, 18, 19, 20 (delivered),
**16 (Wave 1 delivered in v1.14.0; Wave 2 in progress)**, then **21**, then 11A and 22 together.

Rule §8.3 of `ROLLOUT_PLAN.md` says: "Work only on the earliest incomplete phase **unless explicitly
instructed otherwise**." The owner's instruction above is that instruction.

Phase 21 and Wave 2 touch mostly different code:

- **Wave 2 touches** each AI tool's program and models (`crates/runtime/src/agent/`), the Router
  (`crates/router/`), Settings → AI models, and the AI tools page.
- **Phase 21 touches** the app's layout and panels (`apps/desktop/src/`), each window type's
  permission file (`apps/desktop/src-tauri/capabilities/` and `permissions/`), a new command for
  the owner's own file reads and edits, and, for more than one organization, the Ledger
  (`crates/ledger/`), backups, and the Vault.
- **Both touch** a few shared files: `apps/desktop/src-tauri/src/lib.rs` (where commands are
  listed), the generated types in `packages/types/src/generated`, `ROLLOUT_PLAN.md`,
  `docs/development/versioning.md`, `docs/design/vocabulary.md`, the ADR index, the package
  versions, and `Cargo.lock`. Where both change one, the merge keeps both sides, and generated
  files are made again with `pnpm bindings`, never by hand.

Phase 21's dependencies are met: Phase 13 (backups per Ledger) and Phase 8 (working copies,
ADR-016) are delivered. Its one tie to a later phase is the Free edition's limit on organizations,
which needs Phase 11A's editions.

Wave 2's first step checks Cursor and Copilot on the owner's PC with the owner's own sign-ins, so
that session will sometimes wait on the owner. Phase 21 keeps the work moving meanwhile.

## Decision

1. **Phase 21 is built now, beside Phase 16's second wave.** It is built in its own session, from
   its own branch, with its own checklist (`docs/phases/phase-21-checklist.md`), acceptance report,
   and release notes. Before building, that session checks the plan against the code and asks the
   owner its open questions, recorded in Phase 21's own decision record (ADR-091).
2. **Phase 21 is exactly the plan's list** (`ROLLOUT_PLAN.md`, Phase 21, Deliverables): panels
   that resize, dock, and pop out; a file view; a built-in editor; one writer at a time; watching a
   worker's edits in the editor; and more than one organization.
3. **Safe parts first, organizations last.** Panels, windows, the file view, and the editor come
   first. **More than one organization** comes last, because it changes how the Ledger, backups,
   and the Vault are kept, which is where a clash with other work would cost the most. If Phase 21
   is split into parts with their own pull requests (as Phase 20 was, ADR-067), ADR-091 records
   the split.
4. **The Free edition's organization limit** is decided with the owner in ADR-091, as the plan
   says. Plenipo enforces it once Phase 11A's editions exist. Until then, no limit is enforced.
5. **Staying out of Wave 2's way:** Phase 21 does not change the AI tool programs, the Router,
   Settings → AI models, or the AI tools page. If it ever has to, the builder stops and asks the
   owner first. Each branch merges `main` whenever `main` moves.
6. **Version numbers.** Whichever merges first takes the next minor version (`1.15.0`); the other
   takes the one after. `docs/development/versioning.md` lists releases in the order they merge.
   The builder tells the owner the number before merging.
7. **The order-of-work table** in `ROLLOUT_PLAN.md` shows Phase 16 as "Wave 2 in progress" and
   Phase 21 as "In progress beside Phase 16's Wave 2 (ADR-090)".

## Consequences

- The owner gets the workspace sooner, without waiting for Cursor and Copilot.
- Two pull requests are open at once. Each merges `main` when the other lands; the shared files
  above can conflict, and each merge keeps both sides.
- Release numbers may not follow the order of work: Phase 21 could ship as `1.15.0` before Wave 2,
  or after it.
- Waves 3 and 4 of Phase 16 (spending caps, paid keys, OpenRouter, Hermes) are unchanged by this
  record and still follow Wave 2.
- Rule §8.3 is unchanged. This record is the owner's explicit instruction for Phase 21, not a new
  rule.

## Alternatives considered

- **Wait for Phase 16 to finish** (all four waves). Not chosen: the owner's instruction.
- **Build Phase 21's organizations first.** Rejected: it touches the Ledger, backups, and the
  Vault, which carry the most risk of clashing with other work, so it waits until the safer parts
  are merged.
- **Share one ADR number range for both sessions.** Rejected: two sessions could pick the same
  number. Phase 21 gets its own range (090 to 099), as Phase 20 did (069 to 079, ADR-080).
