# ADR-140: Phase 14 starts — its numbers, what the check found, and its three parts

- **Status:** Proposed (2026-10-01). The owner said Phase 22 is live on 2026-10-01 and told the
  builder to begin Phase 14 in a new branch.
- **Date:** 2026-10-01
- **Phase:** 14 (Plenipo on your phone: a web interface built from scratch)
- **Carries out:** [ADR-132 (the final push)](ADR-132-the-final-push.md) §1 and §2: Phase 14 is
  first, and starts when the owner says Phase 22 is live;
  [ADR-040 (Phase 14 is Plenipo's own web interface for a phone)](ADR-040-phone-web-interface.md) §5:
  "Phase 14's own record settles the rest"
- **Number:** on 2026-10-01, `main` and every branch on GitHub stopped at ADR-131, and ADR-132 is on
  this branch. ADR-132 left **133** for the session that finished Phase 22. So that no two sessions
  pick the same number, **Phase 14 uses ADR-140 to ADR-149**, starting with this one. ADR-134 to
  ADR-139 stay free for other work.

> **On screen** (ADR-010, plain words and rank names): nothing yet. The words the phone uses are in
> each record below, and go into `docs/design/vocabulary.md` when they are built.

## In short

Phase 14 lets you use Plenipo from your phone. Your PC stays in charge, and Guard decides every
request. Before building, Plenipo's builder read the plan, the records it points at, and the code,
and wrote one record for each big choice. **Accepting this record means** Phase 14 uses ADR-140 to
ADR-149, is built in **three parts** (each its own pull request and release), and starts with the
records below:

| Record | What it settles                                                                     |
| ------ | ----------------------------------------------------------------------------------- |
| 141    | How a phone is paired: a picture code (QR code) or a typed code, shown on your PC   |
| 142    | How the phone proves it is you: a passkey, with your face, fingerprint, or passcode |
| 143    | The relay and the lock: sealed end to end, no copies, sign-in, and wrong tries      |
| 144    | Notices when the page is closed, sealed so only your phone can read them            |
| 145    | The fixed list of what a phone may ask, and what stays on your PC only              |
| 146    | Where the phone's page lives: its own address, never on the relay                   |

## Context

What the check found (the code read at `a9a2f59` on `main`, v1.18.1, 2026-10-01):

**What Phase 14 can build on**

1. **An approval is answered once.** `Broker::resolve_approval` refuses an answer to an approval that
   is no longer waiting, and the Ledger's `settle_approval` makes the same check inside its own
   transaction. So when the PC and a phone answer at the same moment, one answer wins and the other
   is told "that request was already approved". Phase 14 needs this, and it is already true.
2. **Every owner action already has one core function** under its desktop command: answering an
   approval (`Broker::resolve_approval`), a lesson (`Workforce::decide_lesson`), Stop all and Allow
   again (`stop_control_everywhere`, `Broker::allow_control`), Run again and Leave stopped
   (`recovery::run_again_plan`, `recovery::dismiss`), stopping a worker (`AgentRuntime::cancel_turn`),
   and giving an objective (`Workforce::give_objective`). The phone's connection calls these, never
   the desktop window's commands (the plan, Architecture).
3. **Desktop commands are the main window's alone** through Tauri's permission files
   (`capabilities/default.json`), with tests that a second window, the sign, and a web page are
   refused (`the_owners_control_over_workers_is_the_main_windows_alone`). New commands for Phase 14
   follow the same pattern.
4. **Guard already decides every request Plenipo makes by itself**, each for a named purpose with its
   own fixed addresses (`crates/guard/src/outbound.rs`), and records a refusal as
   `guard.request_refused`. Phase 14 adds purposes for the relay (and, if ADR-144 is accepted as
   recommended, for phone notices).
5. **One place decides Free and Pro:** `Entitlements::check(Limit)` in `crates/licensing`. There is
   no limit for phone access yet. Phase 14 adds `Limit::PhoneAccess`, Pro only, which pauses when
   Pro ends, the way Connections do (ADR-068).
6. **Libraries already in the build:** `ed25519-dalek` and `curve25519-dalek` (license keys, SSH),
   `p256` (through SSH), `sha2`, `reqwest` with Windows' own TLS, and `tokio-tungstenite` (for the
   browser; its TLS part is off today).

**What is missing, or differs from the plan**

7. **There is no "approve on your PC only" setting.** Production servers always ask, but nothing says
   where the answer may come from. Phase 14 adds it (ADR-145).
8. **The Ledger has no column for "which device".** An event's `source` names who acted ("owner",
   "plenipo", a worker). Phase 14 keeps `source` as `owner` for the owner's phone, and names the
   device in the event (ADR-145), so every check that looks for the owner still works.
9. **Stop all is kept in memory, for the whole PC.** That matches the phone: Stop all and Allow again
   from a phone act on every organization, like the tray's Stop all.
10. **Nothing loads code at run time.** ADR-014 says this for AI tool adapters. Phase 14 keeps the
    same rule for the phone's page: its code is built with each release, and it loads no code from
    anywhere else (ADR-146).
11. **The plan says notices go "through the relay".** A PC can send a sealed notice straight to
    Apple's or Google's notice service, with the relay never seeing it. ADR-144 asks the owner which.
12. **The relay belongs to Milepost.** Its address and sign-in never go in this repository. Plenipo
    reaches it under a name of its own (ADR-146), and the change it needs is written as a request for
    the relay's own repository (`docs/phases/phase-14-relay-change-request.md`), which the owner
    approves there.

## Decision

1. **Numbers.** Phase 14 uses ADR-140 to ADR-149.
2. **Three parts, each its own pull request and release**, as Phase 20 was built (ADR-067):

   | Part    | What                                                                                                                                                                                                                                          | Version  |
   | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
   | **14A** | The sealed line and the approvals: the switch (Pro only), adding and removing a phone, the lock and sign-in, the passkey, every page to read, **Approve** and **Refuse** in the page, **Stop all**, and approvals kept on the PC only         | `1.19.0` |
   | **14B** | Everything else that is safe from the page: **Allow again**, stopping one worker, **Run again** and **Leave stopped**, **Keep** and **Discard** a lesson, and **sending an objective**                                                        | `1.19.1` |
   | **14C** | Notices when the page is closed: sealed for your phone, **Approve** and **Refuse** from the notice on Android, one tap to the approval on an iPhone, the lock-screen choice, and the Home Screen guide for iPhone. Phase 14 is then delivered | `1.19.2` |

3. **Each part is tested against a stand-in relay** built in this repository for the tests, with
   made-up data. No test touches the real relay.
4. **Phone access reaches real people only after the relay change is live.** The owner approves the
   relay's change in its own repository during part 14A. Until the relay answers at Plenipo's relay
   name (ADR-146), a released copy shows **Use Plenipo from another device** as "Coming soon", and
   connects to nothing.
5. **Every part follows the plan's rules:** every request from a phone goes through Guard and is
   recorded in the Ledger with the device that sent it; the phone never gets a shell, the terminal,
   files, the screen, the browser, or secrets; and new desktop commands are the main window's alone,
   with tests that refuse the sign and web pages.
6. **Order of work.** Phase 14 shows as **In progress** until part 14C is merged. Phase 23 (Mac and
   Linux) starts after, as ADR-132 says.

## Consequences

- You can approve from the page on your phone after part 14A, before notices exist.
- The riskiest new thing, a door into the PC from outside, is built first and tested alone, before
  the phone can start work.
- Three rounds of release notes instead of one.
- Branding: the phone's page uses the design system in `packages/ui`. Branding work on its own
  branch (`codex/branding`) reaches the phone's page when it is merged; Phase 14 does not change the
  brand.

## Alternatives considered

- **One pull request for the whole phase.** One large, slow review; nothing usable until notices are
  done.
- **Notices first.** A notice is only useful when the page can answer it, so the page comes first.
- **Look only in the first part, with no approving.** Safer still, but approving from the phone is
  the point of the phase, and its own lock (ADR-142) is built in part 14A either way.
