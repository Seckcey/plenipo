# Free and Pro

Plenipo is one app and one public codebase. A license key decides how far it scales and which
business departments you can create.

> **Status:** this page is the plan, not what ships today. Releases up to v1.8.0 have no license
> check and no limits — everything is unlocked. The Free and Pro split arrives in a coming release
> ([ADR-021](adr/ADR-021-editions-and-license.md), Free and Pro editions under the Elastic
> License, and [ADR-022](adr/ADR-022-subscription-and-license-check.md), subscription pricing
> and the weekly license check), built as [Phase 11A](../ROLLOUT_PLAN.md) in the rollout plan.

## What you get

| What                                                           | Free           | Pro       |
| -------------------------------------------------------------- | -------------- | --------- |
| Departments                                                    | 1              | Unlimited |
| Projects                                                       | 1              | Unlimited |
| Workers on the job at the same time                            | 3              | Unlimited |
| Development department (developer, reviewer, QA, docs)         | Yes            | Yes       |
| Sales department on HubSpot, and business departments after it | No             | Yes       |
| AI tools: Claude Code, Codex, Grok, Kimi, Ollama               | All            | All       |
| Your own sign-ins, never API keys                              | Yes            | Yes       |
| Which AI model each role gets, and how hard it thinks          | Yes            | Yes       |
| Permissions, Guard, folder limits, and your approval           | Yes            | Yes       |
| Plenipo's own browser, and the screen, mouse, and keyboard     | Yes            | Yes       |
| Switches in Settings: what workers may use, and when they ask  | Yes            | Yes       |
| Workers that learn from their work (lessons)                   | No             | Yes       |
| Your servers over SSH, and the Operations Engineer role        | Yes            | Yes       |
| Ledger, Activity trail, and screenshots of every step          | Yes            | Yes       |
| Ranks, titles, and the rest of Personalization                 | Yes            | Yes       |
| Full source code, and the right to change it for your own use  | Yes            | Yes       |
| Support                                                        | Issues on here | Priority  |

## What stays free, always

- **Every safety feature.** Permissions, folder limits, blocked programs, the approval cards, the
  Vault, the control center with **Take over** and **Stop all**, the switches in **Settings →
  Switches**, and the full record in the Ledger.
  Nothing that keeps a worker in bounds is ever behind a paid tier.
- **Every AI tool.** Free is not limited to one vendor. Claude Code, Codex, Grok, Kimi, and Ollama
  all work on the Free edition, on your own sign-ins.
- **The whole chain of command.** Worker → Supervisor → Manager → VP → President, with one
  department and one project.
- **The source.** You can read it, build it, change it, and run your build.

Free is meant to be enough to run one real project start to finish. Pro is for running a business
on it.

## What Pro costs

| Plan    | Price                                                          |
| ------- | -------------------------------------------------------------- |
| Monthly | **$9 a month**                                                 |
| Yearly  | **$99 a year** — that is **$8.25 a month**, and one month free |

Free is free, with no card and no account, and stays that way.

## How Pro will be unlocked

**Settings → License → Enter a license key.** The key unlocks the limits above straight away, with
no restart.

Because Pro is a subscription, Plenipo has to be able to tell that a subscription is still running.
So a **Pro** copy checks in with 8 West about **once a week**. Exactly what that check sends:

- your license key id
- which version of Plenipo you are running

That is the whole list. It never sends your projects, folder names, file paths, objectives, what
your workers did, which models they used, or anything from the Ledger. Your work stays on your
computer ([ADR-002](adr/ADR-002-local-first-architecture.md), local-first architecture). A test
checks the contents of that request byte for byte, and you can read it in the source.

**A Free copy never checks in at all.** If you have not paid, Plenipo never contacts 8 West.

**Checking for a new version is separate, and the same for Free and Pro** (from v1.9.0,
[ADR-038](adr/ADR-038-updates.md), updates). Every copy asks GitHub once a day whether a newer
Plenipo is out, by reading a public file from Plenipo's GitHub Releases. It sends nothing about you
or your work, and has nothing to do with a license. GitHub sees what any website sees (your
internet address, and that a copy of Plenipo asked). Nothing is downloaded or installed until you
choose **Install now**.

**No internet is fine.** Pro keeps working for **30 days** between successful checks, so a flight,
a dead router, or an 8 West outage never locks you out. If the check fails for any reason, Pro
stays on and tries again later.

## What happens if you stop paying

Nothing you made is taken away.

Plenipo drops back to Free at the end of the period you paid for. Every department, project,
worker record, and Ledger entry stays exactly where it is — visible, readable, and able to finish
what it started. The only thing that stops is **creating** something new past a Free limit.

To ask about Pro, a team licence, or an invoice, open a
[discussion](https://github.com/Seckcey/plenipo/discussions) or contact 8 West Ventures, LLC.

## What the license lets you do

Plenipo is licensed under the [Elastic License 2.0](../LICENSE). In plain words, and without
changing what the license itself says:

- **You may** read the source, build it, run it, change it for your own use, and share your
  changes.
- **You may not** sell Plenipo to other people as a hosted or managed service.
- **You may not** remove or work around the license-key check, or strip the copyright notices.

That is the whole difference from a fully open license: you get the source and the freedom to use
it, and nobody gets to take Plenipo and sell it back to you.

If you contribute code, see [`CONTRIBUTING.md`](../CONTRIBUTING.md) for how contributions are
licensed.
