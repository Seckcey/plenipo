# ADR-156: When Plenipo cannot tell which program sent a tool call, it refuses

- **Status:** Accepted (by the owner, 2026-10-02, as recommended)
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Amends:** [ADR-034 (approved programs run as the owner)](ADR-034-approved-programs-run-as-the-owner.md)
  §1, its last sentence

> **On screen:** where the check cannot run, once: "Plenipo can't check which program is asking on
> this computer, so workers can't use Plenipo's tools here yet." The Activity trail says the same.

## In short

A worker's AI tool gets a ticket to use Plenipo's tools. Plenipo checks that each call really comes
from that AI tool, so another program that copied the ticket cannot use it. Today, if a computer
gives Plenipo no way to check, the call is let through with a warning. On a Mac that would be every
call. From now on, if Plenipo cannot check, it says no.

## Context

- ADR-034 §1 binds a tool ticket to the AI tool's program tree. On Windows and Linux, Plenipo finds
  the program at the other end of the connection and its parents, and a failed lookup is refused.
  It ends: "Only where the operating system offers no way to tell at all (macOS) is the connection
  served, and both a notice and a trail event say the check was not possible
  (`tool_server.ticket_unchecked`)."
- No Mac lookup exists yet (`crates/capabilities/src/process.rs`), so on a Mac every connection
  would be served unchecked (`Admission::Unchecked` in `crates/capabilities/src/broker.rs`). A test
  expects that today.
- No customer has met this, because no Mac download has shipped.

## Decision

1. **A connection to a worker's tools is served only after the check has passed.** On any system
   where the check cannot run, the connection is refused like a bad ticket, the owner sees one
   plain notice, and the Activity trail records it as a refusal that says the check was not
   possible. The ticket itself is never recorded.
2. **Wave 1 builds the Mac lookup** (which program holds a local connection, and its parents), with
   tests on GitHub's Mac. Until it works, workers on a Mac cannot use Plenipo's tools, and no Mac
   download ships (ADR-150 §5).
3. **Windows and Linux do not change:** both can already check, and both already refuse when the
   lookup fails.
4. The test that expects "allowed, and said so" changes to expect a refusal.

## Consequences

- A copied ticket is useless on every system, as ADR-034 meant.
- A future system with no lookup gets no tool calls until it has one, instead of unchecked ones.

## Alternatives considered

- **Keep "allow, and say so".** On a Mac, any program could use a copied ticket, which is the exact
  thing ADR-034 prevents.
- **Ask the owner each time the check cannot run.** The owner cannot tell which program is asking
  either, and the questions would come for every call.
