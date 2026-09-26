# ADR-003: Provider-independent roles

- **Status:** Accepted (specified by ROLLOUT_PLAN.md)
- **Date:** 2026-09-25
- **Phase:** 0

## Context

Plenipo organizes work by role (Superintendent, Project Coordinator, Senior Developer, Code
Reviewer, Documentation Writer, …). Which AI provider and model fills a role will change as
models, pricing, subscriptions, and availability change. Hard-coding a provider into a role or
workflow would force code changes for what is really a policy decision.

## Decision

- A **Role** is defined by purpose, persistence, and required capabilities — never by vendor.
- **Model policy is configuration.** Preferred models, fallbacks, and requirements per role live
  in user-managed settings or discovered provider metadata (Phase 6), not in code or prompts.
- **Core types are vendor-neutral.** Use `RuntimeAdapter`, `ModelPolicy`, `AgentInstance`,
  `Task`, `Session`. Do not introduce types like `ClaudeWorkerTask` or `CodexDepartment`.
- **Provider specifics live only in runtime adapters** (Phase 3) behind a common contract.
- **No silent provider switching.** Any fallback is explicit, policy-driven, recorded in the
  Ledger, and explained to the user. Paid API fallback is disabled unless the owner enables it.
- **Model names are data.** Transient commercial model names must not be hard-coded through
  application logic.

## Consequences

- Changing a role's model is a settings change, not a release.
- Adding a provider means implementing one adapter, not touching Core, Liaison, Guard, or Ledger.
- Adapters must normalize provider output into common result types, which costs some
  provider-specific richness.
- Code review should reject vendor names appearing in Core domain types.

## Alternatives considered

- **Per-provider workflows** — faster to demo, but duplicates orchestration logic and locks
  roles to vendors.
