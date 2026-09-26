# ADR-010: Plain words, the chain of command, and choosable rank names

- **Status:** Accepted (owner, 2026-09-26)
- **Date:** 2026-09-26
- **Phase:** 5

## Context

The rollout plan names the persistent roles Superintendent, Department Manager, and Project
Coordinator, and the app used engineering words on screen: "runtime", "persistent",
"on-demand", "session", "turn", "spawned". During Phase 5 the owner asked for plain words —
"nobody knows what a runtime is; nobody calls their boss a coordinator" — for the chain of
command people use (Worker → Supervisor → Manager → VP → President), and for a personalization
option that names the ranks after a U.S. military branch or the Mafia.

The Workforce model (ADR-009), its Ledger schema, and Liaison's `role:<title>` routing were
already built around the plan's names.

## Decision

1. **Plain words on every screen.** [`docs/design/vocabulary.md`](../design/vocabulary.md) is
   the word list: "AI tool" for runtime, "full-time"/"on call" for persistent/on-demand,
   "conversation" and "task" for session and turn, "brought in" for spawned, and so on. It
   covers messages from the Rust side the owner can see (refusals, status details, run labels).
   Raw diagnostics keep their technical detail.
2. **The chain of command.** On screen, the owner is the **President**, then **VP**
   (`superintendent`), **Manager** (`departmentManager`), **Supervisor**
   (`projectCoordinator`), and **Worker**. Code, the Ledger schema, event types, and the
   generated types keep the plan's names; only what people see changes.
3. **Seeded leadership roles are renamed in place.** The role templates are now VP, Manager,
   and Supervisor with plain descriptions and purposes. A template carries its former names;
   when the Ledger seeds templates it renames a template role found under a former name (same
   role, so every position keeps it; `org.role_renamed`) instead of adding a duplicate. An
   owner's own role that happens to share an old name is left alone. New projects' leads
   default to "<Project> Supervisor".
4. **Titles are a choice stored with the organization.** `TitleTheme` — `business` (default),
   `army`, `navy`, `airForce`, `marineCorps`, `coastGuard`, `spaceForce`, or `mafia` — is kept
   in the organization's settings next to its name and returned in the snapshot; the new
   `set_organization_titles` command changes it (Settings → Personalization → Titles).
   Settings writes merge fields in one transaction, so renaming the organization and choosing
   titles never undo each other.
5. **Display only.** The chosen set renames the ranks on the map, in the details panel,
   dialogs, the list view, the hire palette, and the app's own hints. Job titles stay as the
   owner wrote them, and agents are always told the Business titles: a mafia or military
   persona never reaches an agent's instructions, and routing by title is unaffected.

## Consequences

- The owner reads the organization in words they use; the plan's names remain the developer
  vocabulary, with the mapping in the word list and this ADR.
- Messages from the Rust side always use the Business words, even when another title set is
  chosen; the canvas checks most rules first with the chosen words.
- Adding a title set is one entry in `apps/desktop/src/org/titles.ts`; no migration.
- Brand and legal care: the military sets use rank names only — no official insignia, seals,
  or logos (those are protected and would need permission), and nothing suggests endorsement
  by the U.S. armed forces. The Mafia set is opt-in and never the default; some people find
  Mafia stereotypes offensive, so it stays a playful choice the owner makes deliberately.

## Alternatives considered

- **Rename the internal kinds, schema, and types** — large churn in the Ledger, the generated
  bindings, and every crate for no user benefit; the display layer is enough.
- **Store the choice in the browser** — no backend change, but a per-device preference; the
  organization's names belong with the organization (and later remote views need them).
- **Put themed ranks into job titles and agent instructions** — fun, but a rank change would
  rewrite the owner's titles, and a persona would leak into agents' behavior.
