# ADR-042: Specialties under each role — built in, and your own

- **Status:** Proposed (2026-09-28), waiting for the owner
- **Date:** 2026-09-28
- **Phase:** 17
- **Amends:** ADR-019 (every role knows its job: working instructions) — a specialty adds lines
  to them; ADR-009 (the organization) §10 — specialties are data, like roles

> **On screen** (ADR-010, plain words and rank names): **specialty** ("Senior Developer —
> Database"). What a specialty adds shows under the role's own lines. Suggested models and
> permissions are **suggested**: nothing changes until you choose.

## In short

A role can have specialties. A Senior Developer can be a Database developer, a Front-end
developer, and so on. A specialty adds a few lines to the worker's instructions, and suggests
which models and permissions fit. Plenipo comes with the plan's list, and you can add your own
to any role. Accepting this record means building it as written below.

## Context

Phase 17 of `ROLLOUT_PLAN.md` asks for "specialties under each role: a specialty adds its own
lines to the role's working instructions (ADR-019), suggested models, and suggested
permissions", with a built-in list, and "the owner can add specialties to any role, built-in or
the owner's own". Its technical notes: "Specialties are data, like roles (ADR-009). A position
records role and optional specialty. Lessons stay per role (ADR-024)." Its acceptance test hires
"a Senior Developer with the Database specialty".

Today (v1.9.0): twelve built-in roles and none of them has specialties. "Front-end Developer" is
only a title. Built-in roles keep their working instructions; the owner's roles have the owner's
(`crates/workforce/src/templates.rs`, `service.rs`). Plenipo refreshes the built-in roles'
instructions at every start (`Ledger::ensure_roles`), so anything stored inside a built-in role's
record would be overwritten. What a worker may do on the computer comes only from its role's
permission set in Guard (ADR-013); a role's "capabilities" are a note.

## Decision

1. **A specialty belongs to one role and narrows it**, for example "Senior Developer —
   Database". It has:
   - a **name**, and a **suggested title** for a new position ("Database Developer");
   - **its own lines** for each part of the working instructions (ADR-019): its job, what it hands
     back, what it must not do, and when it asks for help;
   - **suggested models:** what a model should be able to do (see images, make images, take a large
     context), and — for your own specialties — models from your list;
   - **suggested permissions:** the permissions its work usually needs.
2. **Kept as data, like roles.** A new table, `specialties` (Ledger layout 9), and each position
   records its optional specialty (`positions.specialty_id`). They live apart from roles because
   built-in roles are refreshed at every start. Changes are recorded as `org.specialty_created`,
   `org.specialty_updated`, and `org.specialty_removed`.
3. **Built-in specialties** are the plan's list. Plenipo adds them, keeps their lines up to date
   at every start, and you cannot edit or remove them, as with built-in roles:
   - **Senior Developer:** Front-end, Back-end, Database, UX/UI, Mobile, DevOps, Data
   - **Designer:** Brand, Web, Product
   - **Security Auditor:** Code review, Compliance, Authorized penetration testing
   - **Operations Engineer:** Windows servers, Linux servers, Networking, Microsoft 365
     administration
   - **Researcher:** Market, Technical
   - **Documentation Writer:** User guides, API documentation
4. **Your own specialties** can go under any role, built-in or yours, with your own name, title,
   and lines. You can change or remove them. Removing one is refused while an active agent has
   it; an archived agent that had it comes back without it.
5. **What the worker is told.** "Your job as Senior Developer (Database):" then the role's lines,
   then the specialty's, for each of the four parts. A position without a specialty gets its role
   alone, exactly as today.
6. **Suggested models never pick a model by themselves.** Built-in specialties never name a
   model (ADR-011: which models exist is yours to say); they say what a model should be able to
   do. The agent's AI model tab shows the suggestion and the models in your list that fit, with
   **Use these**, which makes them the agent's own list (ADR-041). Until you press it, nothing
   changes.
7. **Suggested permissions never grant anything.** The agent's Work tab shows them as a
   suggestion ("Workers with this specialty usually need: Connect to servers"). Permissions still
   come only from the role's permission set in Guard, which you change in Settings →
   Permissions. A specialty cannot widen what a worker may do.
8. **Authorized penetration testing is fenced by its own lines:** test only the systems the task
   names, and only when the task says you or your client owns them and has authorized the test;
   stop and ask when that is missing or unclear; nothing meant to overload or break a system;
   never keep or share real data you reach; never work around Plenipo's permissions. Its
   suggested permissions ask you before each program and each server.
9. **Hiring and changing.** The hire form offers the role's specialties (optional) and fills in
   the suggested title, which you can change. An agent's Job tab changes its specialty. Changing a
   specialty never hires a new agent: its next task gets the full instructions again (ADR-044).
10. **Lessons stay per role** (ADR-024): a Database developer's lessons help every Senior
    Developer, and its specialty lines do not change which lessons it gets.

## Consequences

- "Front-end Developer" becomes real: the worker is told what that means, in plain words.
- Built-in specialties improve with each version, like built-in roles; your own stay as you wrote
  them.
- A specialty's lines make instructions longer. They are short (a few lines), and ADR-044 sends
  the full instructions only when needed.
- Permissions stay per role. If one specialty needs more than its role's permission set allows,
  you give the role more, or make a role of your own for it.

## Alternatives considered

- **Specialties inside the role's record.** Rejected: built-in roles are rewritten at every start,
  so your specialties on them would be lost.
- **Specialties as separate roles** ("Database Developer" as its own role). Rejected: the plan
  asks for specialties under roles, and lessons, model choices, and permission sets would split
  for no reason.
- **A specialty that grants permissions.** Rejected: permissions come only from Guard and the
  owner (ADR-013); a specialty is a job description.
- **Built-in specialties that name models.** Rejected: model names are yours to add (ADR-011).
