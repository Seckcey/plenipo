# ADR-021: Free and Pro editions under the Elastic License 2.0

- **Status:** Accepted
- **Amended by:** [ADR-022](ADR-022-subscription-and-license-check.md) (subscription pricing
  and the weekly license check), which replaces **decision 5** below. Pro is a subscription,
  so the key cannot be verified offline forever; decision 5's "the check is local" is no
  longer how Plenipo works. Every other decision here stands.
- **Amended by:** [ADR-068 (Connections and add-on tools are part of Pro)](ADR-068-connections-are-pro.md):
  the Pro side of the split gains Connections and add-on tools (Phase 20), locked by Phase 11A.
- **Date:** 2026-09-27
- **Phase:** 10 (recorded after the browser work, alongside opening the repository up)

## Context

Plenipo's repository is public, and up to v1.3.0 it had no LICENSE file at all. With no license,
default copyright applies: people could read the code but had no right to use it. That is the worst
of both worlds — it gives visitors nothing, and it says nothing about what Plenipo is going to be.

Plenipo is a product of 8 West Ventures, LLC, and it is meant to earn money. Three things pull
against each other:

1. **Being seen.** A public repository with real source is how a tool like this gets found, tried,
   and talked about. A closed repository gets none of that.
2. **Not being resold.** Plenipo is a desktop app with no hosted component. Under MIT or
   Apache-2.0, anyone could take the whole thing, rename it, and sell it, and 8 West would have no
   recourse.
3. **A paid tier that means something.** Plenipo runs on the owner's own computer. There is no
   server to withhold. Any limit in the app can be compiled out by someone with the source, so a
   paid tier can only be defended by the license, not by the code.

The owner chose a feature-gated split: a Free edition that is genuinely useful, and a paid Pro
edition, from one public codebase.

## Decision

1. **Plenipo is licensed under the Elastic License 2.0** (`LICENSE`, verbatim; SPDX
   `Elastic-2.0`). It was chosen because it is the one well-known source-available license that
   does all three of these at once: it grants the right to read, build, run, change, and share the
   software; it forbids providing Plenipo to others as a hosted or managed service; and it forbids
   removing or working around license-key functionality. That last clause is what makes a paid
   tier defensible for a local desktop app.
2. **Copyright stays with 8 West Ventures, LLC**, recorded in `NOTICE` along with the trademark
   reservation and a plain-words summary that explicitly does not modify the license.
3. **Two editions, one codebase, one installer.** A license key entered in **Settings → License**
   unlocks Pro. The split is by scale and by business department
   ([`docs/editions.md`](../editions.md)):
   - **Free:** one department, one project, three workers on the job at a time, and the
     Development department.
   - **Pro:** unlimited departments, projects, and workers, plus the Sales department on HubSpot
     ([ADR-018](ADR-018-sales-on-hubspot-no-paperclip.md)) and the business departments that
     follow it, and **workers that learn from their work**
     ([ADR-024](ADR-024-workers-learn-from-work.md); the owner's decision, 2026-09-27).
4. **Nothing that keeps a worker in bounds is ever paid.** Permissions, Guard, folder limits,
   approvals, the Vault, the control center, the switches in Settings
   ([ADR-023](ADR-023-settings-switches.md)), the Ledger, and the Activity trail are in the Free
   edition and stay there. So do all four AI tools and the owner's own sign-ins. Selling safety
   would make the Free edition unsafe to run, which would be worse for Plenipo than any revenue it
   raised.
5. **The check is local.** The key is verified on the owner's own PC. No account, and no work
   leaves the computer to unlock a feature ([ADR-002](ADR-002-local-first-architecture.md),
   local-first architecture).
6. **Contributions are licensed to 8 West.** `CONTRIBUTING.md` states that a contribution is
   licensed under the Elastic License 2.0 and grants 8 West Ventures, LLC the right to use it,
   including in the Pro edition and in any later relicensing. Without that, 8 West could not ship
   contributed code in a paid edition.
7. **Not built yet.** No license check, no key, and no limits exist in the code as of v1.3.0.
   Everything is unlocked. `docs/editions.md` says so at the top. The check, the Settings screen,
   and the limits are a later phase, and Plenipo must never claim a limit it does not enforce.

## Consequences

- **The repository can stay open and stay Plenipo's.** Visitors get the whole source; a competitor
  cannot lift it and sell it as a service.
- **GitHub will not badge Plenipo as open source.** The Elastic License 2.0 is source-available,
  not OSI-approved. The sidebar will read as a custom license. Some developers will not contribute
  to a project under it, and Plenipo will not be accepted into distributions that require an OSI
  license. This is the accepted cost of items 1–3 above.
- **Enforcement is legal, not technical.** Someone can build Plenipo from source with the limits
  removed. Doing so breaches the license. The license is the boundary; the code only marks it.
- **A commercial license and an end-user agreement are still needed** before Plenipo can be sold.
  The Elastic License 2.0 covers the source; it is not a sales contract. Both should be reviewed
  by an attorney, along with the contribution terms in `CONTRIBUTING.md`.
- **The editions are Phase 11A** in `ROLLOUT_PLAN.md` (Free and Pro editions and the license key),
  which runs before Phase 11 and before the postponed Sales department, since Sales is a Pro
  department. It covers the key, the Settings screen, the limits, and what a Free owner sees when
  they reach one: a clear, plain-words message, never a silent failure.

## Alternatives considered

- **MIT or Apache-2.0.** Maximum reach and the friendliest to contributors, but either one lets
  anyone fork Plenipo, strip the paid tier, and sell it. Rejected because Plenipo is the product.
- **Business Source License 1.1.** Free for non-production use, paid for production, converts to
  open source on a change date. This gates by _who is using it_ rather than _what they get_, so
  every business user has to buy before trying Plenipo in anger. Rejected as too steep a first
  step for a new product; it also needs a change date decided now.
- **PolyForm Noncommercial.** Simplest of the user-gated licenses, but it puts every commercial
  user behind a sale and kills the free audience a new tool needs.
- **Open core: Apache-2.0 core plus a private Pro repository.** The most contributor-friendly
  answer, and the standard one. Rejected for now because it means maintaining a plugin boundary and
  two repositories for a one-person team, and because the Apache core could still be forked and
  sold. It stays available as a later move — ELv2 does not prevent opening the core up further.
- **Closing the repository.** Protects everything and gets Plenipo found by nobody.

## As built (v1.18.0, Phase 11A)

Built as decided. `crates/licensing` is the one place a Free limit is decided
(`Entitlements::check`), and every limit in `docs/editions.md` goes through it: one organization,
one department, one project, and three workers at a time, counted across the whole PC
([ADR-110](ADR-110-one-person-any-of-their-pcs.md)); business departments, lessons, Connections,
and add-on tools are Pro. Guard, permissions, approvals, the Vault, the Ledger, the Activity trail,
spending caps, and every AI tool never ask it: a test gives the same work to a Free copy and a Pro
copy and finds the same tools, the same refusals, and the same records. When Pro ends, nothing is
deleted or hidden. Making something new past a Free limit waits for Pro, and Connections, add-on
tools, and lessons pause.

Known limits, each kept on purpose:

- **Bringing back a department** on Free checks the department limit only. Its projects come back
  with it, even past Free's one project: they were made on Pro, and nothing is hidden.
- **A Vault that cannot be read** when Plenipo starts means Free until it answers again. Plenipo
  tries again every few minutes, Settings → License says so, and the record is left whole.
- **No pop-up notice when Pro ends.** Settings → License and the Activity trail say so. When Pro ends
  while Plenipo is closed, the next start shows Free without an Activity entry for the change.
