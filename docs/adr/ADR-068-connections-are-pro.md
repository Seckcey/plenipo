# ADR-068: Connections and add-on tools are part of Pro

- **Status:** Accepted (by the owner, 2026-09-28: "Connecting tools is a Pro version feature",
  then every choice below as recommended)
- **Date:** 2026-09-28
- **Phase:** 20 (the decision), 11A (the lock)
- **Amends:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) — the split gains
  Connections and add-on tools on the Pro side; `docs/editions.md` and the plan's Phase 11A follow

> **On screen** (ADR-010, plain words and rank names): nothing changes until Phase 11A. Then
> **Connections** and **Add-on tools** say "**Part of Pro**" on a Free copy, with what Pro adds, as
> every Free limit does (Phase 11A).

## In short

Connections (Microsoft 365, Slack, Google, HubSpot, Stripe, WordPress and WooCommerce) and add-on
tools are **Pro** features. Plenipo cannot tell Free from Pro yet: license keys come with Phase 11A,
later in the order of work. So, as with the other Pro features today, **every copy can use
Connections until Phase 11A** turns the lock on. When a Pro subscription ends, Connections
**pause**: nothing is deleted, running tasks finish, new work cannot use them until Pro is back, and
**Disconnect** always works. GitHub's tools stay Free: they are part of the Development department.
Accepting this record means building Phase 20 with no lock, and Phase 11A adding it as written
here.

## Context

The owner's direction (2026-09-28), while approving the Phase 20 design: "Connecting tools is a Pro
version feature." The owner then chose, each as recommended:

1. **Before license keys exist:** Connections work for everyone until Phase 11A.
2. **When Pro ends:** Connections pause.
3. **Add-on tools:** Pro too.

What exists today (read at `066e9af`, `main`, after the Phase 20 design merged):

- **No license check.** `docs/editions.md` says: "Releases up to v1.8.0 have no license check and
  no limits — everything is unlocked." No licensing code exists (`crates/licensing` is Phase 11A's).
  Pro features already built, such as workers that learn from their work, work in every copy.
- **Phase 11A** (eighth in the order of work, after Phase 21) builds the license key and one place
  that decides: `Entitlements::check(limit)`, returning Allowed or Blocked with a plain reason. Its
  rules: lapsing never deletes or hides anything; "safety is never gated".
- **The edition table** (`docs/editions.md`, ADR-021) does not list Connections.
- **GitHub's tools** (pull requests, issues, checks) are part of the Development department, which is
  Free.

## Decision

1. **Pro, not Free.** Connections and add-on tools join the Pro column of the edition table. Free
   has neither. GitHub's tools stay Free.
2. **No lock before Phase 11A.** Phase 20 builds Connections with no edition check, like every Pro
   feature built so far. Every copy can use them until Phase 11A.
3. **Where Phase 11A locks them** (added to Phase 11A's deliverables and tests):
   - **Connect** (and **Add a program** for add-on tools) on a Free copy: refused through
     `Entitlements::check`, with "Connections are part of Pro" and what Pro adds.
   - **Offering the tools** to a worker's step: none on a Free copy.
   - Nothing else: Guard, the Vault, approvals, and the record are never behind Pro.
4. **When Pro ends, Connections pause:**
   - Nothing is deleted: each connection, its sign-in, who may use it, and its lists stay.
   - Tasks already running keep the tools their step was given, and finish.
   - New work gets no Connection or add-on tools, and the worker's note says why.
   - **Disconnect** always works, and removes the sign-in from the Vault, on Free or Pro.
   - When Pro comes back, the paused connections work again, with no new sign-in unless the
     service asks for one.
5. **Settings** shows Connections on every copy, so an owner on Free can see what Pro adds and can
   always disconnect.

## Consequences

- The owner and early users get Connections before selling starts, as with the other Pro features.
- Phase 11A has two more checks to build and test (Connect, and offering the tools), plus the pause.
- Disconnecting is never locked, so an owner can always take back a sign-in, paid or not.

## Alternatives considered

- **Build the license key (Phase 11A) first.** Not chosen by the owner: much more work before
  Connections, and Phase 11A waits on the 8 West account service's contract.
- **When Pro ends, keep existing connections working and block only new ones** (the rule written for
  departments). Not chosen: a connection is an ongoing service, not something the owner made; the
  pause loses nothing and resumes when Pro returns.
- **Add-on tools on Free.** Not chosen: they are another way of connecting outside tools.

## As built (v1.13.0)

As decided: no lock yet. Settings → Connections says "Connections are part of Plenipo Pro. Every
copy can use them for now; disconnecting always works." Phase 11A adds the lock.

## As built (v1.14.2, part 20C)

As decided: HubSpot, Stripe, the website, and add-on tools are under the same "part of Plenipo Pro"
line on Settings → Connections, with no lock yet. Phase 11A adds the lock; **Disconnect** and
**Remove** (an add-on) always work.
