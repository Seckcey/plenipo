# Free and Pro

Plenipo is one app and one public codebase. A license key decides how far it scales and which
business departments you can create.

> **Status:** this page is the plan, not what ships today. Releases up to v1.5.0 have no license
> check and no limits — everything is unlocked. The Free and Pro split arrives in a coming release
> ([ADR-021](adr/ADR-021-editions-and-license.md), Free and Pro editions under the Elastic
> License).

## What you get

| What                                                           | Free           | Pro       |
| -------------------------------------------------------------- | -------------- | --------- |
| Departments                                                    | 1              | Unlimited |
| Projects                                                       | 1              | Unlimited |
| Workers on the job at the same time                            | 3              | Unlimited |
| Development department (developer, reviewer, QA, docs)         | Yes            | Yes       |
| Sales department on HubSpot, and business departments after it | No             | Yes       |
| AI tools: Claude Code, Codex, Grok, Ollama                     | All            | All       |
| Your own sign-ins, never API keys                              | Yes            | Yes       |
| Which AI model each role gets, and how hard it thinks          | Yes            | Yes       |
| Permissions, Guard, folder limits, and your approval           | Yes            | Yes       |
| Plenipo's own browser, and the screen, mouse, and keyboard     | Yes            | Yes       |
| Your servers over SSH, and the Operations Engineer role        | Yes            | Yes       |
| Ledger, Activity trail, and screenshots of every step          | Yes            | Yes       |
| Ranks, titles, and the rest of Personalization                 | Yes            | Yes       |
| Full source code, and the right to change it for your own use  | Yes            | Yes       |
| Support                                                        | Issues on here | Priority  |

## What stays free, always

- **Every safety feature.** Permissions, folder limits, blocked programs, the approval cards, the
  Vault, the control center with **Take over** and **Stop all**, and the full record in the Ledger.
  Nothing that keeps a worker in bounds is ever behind a paid tier.
- **Every AI tool.** Free is not limited to one vendor. Claude Code, Codex, Grok, and Ollama all
  work on the Free edition, on your own sign-ins.
- **The whole chain of command.** Worker → Supervisor → Manager → VP → President, with one
  department and one project.
- **The source.** You can read it, build it, change it, and run your build.

Free is meant to be enough to run one real project start to finish. Pro is for running a business
on it.

## How Pro will be unlocked

**Settings → License → Enter a license key.** The key is checked on your own PC and unlocks the
limits above. Your work never leaves your computer to make this happen — Plenipo stays local-first
([ADR-002](adr/ADR-002-local-first-architecture.md), local-first architecture).

Pricing is announced with the release that adds the split. To ask about a Pro license, or a
license for a team, open a
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
