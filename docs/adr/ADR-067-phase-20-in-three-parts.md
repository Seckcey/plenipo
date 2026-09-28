# ADR-067: Phase 20 in three parts — Microsoft 365 first, then Slack and Google, then the rest

- **Status:** Proposed (2026-09-28): the owner's choice (the
  [Phase 20 checklist](../phases/phase-20-checklist.md#choices-for-you), choice 1). If the owner
  chooses one release instead, this record is marked **Rejected** and Phase 20 ships as one
  v1.13.0.
- **Date:** 2026-09-28
- **Phase:** 20

> **On screen:** nothing. This record only changes how Phase 20 is built and released.

## In short

Phase 20 is large: six services, add-on tools, and a new sign-in. This record splits it into three
parts, each its own pull request and release: **20A** (the rules, Settings → Connections, and
Microsoft 365 — everything the plan's acceptance test needs), **20B** (Slack and Google), and
**20C** (HubSpot, Stripe, WordPress and WooCommerce, and add-on tools). Accepting it means Phase 20
counts as delivered when 20C is merged, and each part has its own checklist section, acceptance
report, and release notes.

## Context

The plan's Phase 20 lists its services "in this order": Microsoft 365, Slack, Google, HubSpot,
Stripe, WordPress and WooCommerce, then others as the owner asks. Its acceptance criteria test only
Microsoft 365. Each service needs something only the owner can do before it can be tried for real:
8 West's Microsoft app registration, a Slack app and a Google app (and their reviews), and keys for
HubSpot, Stripe, and the website. One pull request for all of it would be hard to review and would
wait on the slowest of those steps.

## Decision

| Part    | What                                                                                                                                                                                                 | Version  | Waits on the owner for                                                        |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------- |
| **20A** | ADR-062 (the rules), ADR-063 (sign-in and the Vault), Settings → Connections, the two new permissions, fences, records, and **Microsoft 365** (Mail, Calendar, OneDrive, SharePoint, Teams; ADR-065) | `1.13.0` | 8 West's Microsoft app ID (to try it for real; the tests use a stand-in)      |
| **20B** | **Slack** and **Google** (Gmail, Google Calendar, Google Drive) on the same rules                                                                                                                    | `1.13.1` | A Slack app and a Google app (ADR-064 §2, §3), and the choices for them       |
| **20C** | **HubSpot**, **Stripe**, **WordPress and WooCommerce**, and **add-on tools** (ADR-066)                                                                                                               | `1.13.2` | Keys the owner creates in each service, typed into Settings (never into chat) |

- **Version numbers** follow the owner's instruction: 1.13.0, then 1.13.x for each later part.
- **Each part** is one pull request from its own branch (`claude/phase-20`, then
  `claude/phase-20b` and `claude/phase-20c`), with every check green, Windows included; the
  checklist is ticked part by part; each part has its own acceptance report section and release
  notes; the plan's Phase 20 status line says which parts are delivered.
- **20A carries the acceptance criteria.** 20B and 20C each repeat the plan's per-connection tests
  (connect, read, write with approval, disconnect) against their own stand-ins.
- **The order of work** (ADR-061) keeps Phase 20 as **Next** until 20C is merged. Phase 16 starts
  after.

## Consequences

- The owner can use Microsoft 365 from Plenipo as soon as 20A is merged and 8 West's app is
  registered, without waiting for Slack's or Google's app review.
- Three smaller pull requests, each easier to review and to test.
- Three rounds of release paperwork instead of one.

## Alternatives considered

- **One release (v1.13.0) with everything.** Simpler paperwork; one large, slow pull request that
  waits on every outside step.
- **A part per service (six parts).** More paperwork than it saves; Slack and Google share their
  shape (chat and mail from a big company with an app review), and the last three share theirs
  (keys the owner creates).
