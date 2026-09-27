# ADR-018: Phase 9 postponed — no Paperclip; a new Sales department later, on HubSpot

- **Status:** Proposed
- **Date:** 2026-09-27
- **Phase:** 9 (postponed; recorded at the start of Phase 10)

## Context

The rollout plan's Phase 9 was to bring the existing Paperclip-managed Sales department (Westy,
Scout, Quinn, Harper, Riley, Morgan, and Avery) into Plenipo through an integration adapter, with
Paperclip staying authoritative for the sales workflow it owns. Its dependency was "Development
MVP stable and Paperclip interface documented".

On 2026-09-27 the owner reported that Paperclip's Sales department was never working. An
adapter, source-of-truth rules, and task synchronization for a department that does not work
would cost a phase and deliver nothing, and the dependency can never be met. The owner wants a
Sales department built inside Plenipo instead, using the HubSpot account the business already
has as its CRM, and wants Phase 10 (browser automation and computer use) first.

## Decision

1. **Phase 9 is postponed.** Plenipo does not integrate Paperclip: no adapter, no import, no
   synchronization, and no Paperclip data in the Ledger.
2. **A new Sales department, built from scratch in Plenipo**, when the owner schedules it. It is
   data over the same engine as every department (Workforce, Router, Guard, Liaison, Ledger,
   capabilities), like the Development department (ADR-016). Paperclip's agent names are not
   carried over; the owner names the new positions.
3. **HubSpot is the CRM and the system of record** for contacts, companies, deals, and sales
   activity. Plenipo keeps its own records (tasks, approvals, the audit trail) and refers to
   HubSpot records by ID and link. It keeps no competing sales database.
4. **Official API first** (the plan's capability order, Phase 10): HubSpot's API, with a HubSpot
   private app access token kept in the Vault (Windows Credential Manager) and never shown to
   workers. The scopes are chosen when Phase 9 is planned. The Phase 10 browser is a fallback
   for HubSpot screens the API does not cover, never the default.
5. **No autonomous outbound.** Emails, sequences, and messages to prospects always wait for the
   owner's approval (Guard's "Sending or publishing outside this computer" kind).
6. **The plan is updated in place.** `ROLLOUT_PLAN.md` gets a rewritten Phase 9 section (a
   sketch, to be detailed in its own checklist when started), its dependencies, and every
   mention of Paperclip. Phase numbers stay, so earlier references hold.

## Consequences

- **Order of work:** Phase 8 → Phase 10 → Phase 9 when the owner schedules it → Phase 11 and on.
  The plan's rule "work only on the earliest incomplete phase" (§8.3) skips Phase 9 while it is
  postponed.
- **Versions:** Phase 10 is released as v1.3.0 (v1.1.0 and v1.2.0 went to the Grok and Ollama AI tools); Phase 9 will be a later minor version.
- **History stays as written.** Earlier checklists and acceptance reports that mention Paperclip
  describe what was planned then; only the plan and the living documents (architecture
  overview, README) change.
- **To watch when Phase 9 starts:** HubSpot's API rate limits and scopes; HubSpot's terms for
  API use; rotating the private app token; copying no customer or prospect data into the Ledger
  beyond record IDs and what a task needs (the plan's Phase 9 rule stays).

## Alternatives considered

- **Integrate Paperclip anyway.** Rejected: its Sales department never worked, so there is
  nothing to integrate.
- **Repair Paperclip first, then integrate it.** Rejected: a second system to run and maintain
  outside Plenipo, for a department the owner would rather have in Plenipo.
- **Build the new Sales department now, as the next phase.** Rejected for now: the owner wants
  Phase 10 first.
- **Use HubSpot through the browser instead of its API.** Rejected as the default: the plan puts
  official APIs first; the browser is slower, breaks when screens change, and is harder to
  audit.
