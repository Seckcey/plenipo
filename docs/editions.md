# Free and Pro

Plenipo is one app and one public codebase. A license key decides how far it scales and which
business departments you can create.

> **Status:** from v1.18.0, this page is what ships ([Phase 11A](../ROLLOUT_PLAN.md) in the
> rollout plan). Releases up to v1.17.0 had no license check and no limits. The decisions:
> [ADR-021](adr/ADR-021-editions-and-license.md) (Free and Pro editions under the Elastic
> License), [ADR-022](adr/ADR-022-subscription-and-license-check.md) (subscription pricing and the
> weekly license check), and ADR-100 to ADR-118 (the owner's answers for Phase 11A and the 8 West
> account service, starting with
> [ADR-100](adr/ADR-100-phase-11a-22-owners-answers.md)).

## What you get

| What                                                                                    | Free           | Pro                                |
| --------------------------------------------------------------------------------------- | -------------- | ---------------------------------- |
| Organizations (Phase 21: yours and a client's, each in its own window)                  | 1              | 3 (Partner: 10, 25, or any number) |
| Departments                                                                             | 1              | Unlimited                          |
| Projects                                                                                | 1              | Unlimited                          |
| Workers on the job at the same time (the next one waits its turn)                       | 3              | 4 in each organization             |
| Development department (developer, reviewer, QA, docs)                                  | Yes            | Yes                                |
| Sales department on HubSpot, and business departments after it (planned, Phase 9)       | No             | Yes                                |
| AI tools: Claude Code, Codex, Grok, Kimi, Ollama, Antigravity, GitHub Copilot           | All            | All                                |
| Your own sign-ins, or your own paid AI keys; spending caps optional (Phase 16)          | Yes            | Yes                                |
| Which AI model each role gets, and how hard it thinks                                   | Yes            | Yes                                |
| Permissions, Guard, folder limits, and your approval                                    | Yes            | Yes                                |
| Plenipo's own browser, and the screen, mouse, and keyboard                              | Yes            | Yes                                |
| Switches in Settings: what workers may use, and when they ask                           | Yes            | Yes                                |
| Workers that learn from their work (lessons)                                            | No             | Yes                                |
| Connections: Microsoft 365, Slack, Google, HubSpot, Stripe, and your website (Phase 20) | No             | Yes                                |
| Add-on tools you set up (Phase 20)                                                      | No             | Yes                                |
| GitHub's tools for the Development department                                           | Yes            | Yes                                |
| Your servers over SSH, and the Operations Engineer role                                 | Yes            | Yes                                |
| Plenipo on your phone, with notices (Phase 14, from v1.19.0; notices from v1.19.2)      | No             | Yes                                |
| Ledger, Activity trail, and screenshots of every step                                   | Yes            | Yes                                |
| Ranks, titles, and the rest of Personalization                                          | Yes            | Yes                                |
| Full source code, and the right to change it for your own use                           | Yes            | Yes                                |
| Support                                                                                 | Issues on here | Priority                           |

## What stays free, always

- **Every safety feature.** Permissions, folder limits, blocked programs, the approval cards, the
  Vault, the control center with **Take over** and **Stop all**, the switches in **Settings →
  Switches**, and the full record in the Ledger.
  Nothing that keeps a worker in bounds is ever behind a paid tier.
- **Every AI tool.** Free is not limited to one vendor. Claude Code, Codex, Grok, Kimi, Ollama, Antigravity, and
  GitHub Copilot all work on the Free edition, on your own sign-ins.
- **The whole chain of command.** Worker → Supervisor → Manager → VP → President, with one
  department and one project.
- **The source.** You can read it, build it, change it, and run your build.

**More than one organization** is Pro ([ADR-091](adr/ADR-091-phase-21-owners-answers.md) §2, the
owner's answer): Free keeps one organization, so it cannot step around the one-project limit by
making one organization per project. Free's limits count across the whole PC
([ADR-110](adr/ADR-110-one-person-any-of-their-pcs.md), one person, any of their PCs). Pro covers
three organizations; companies that run Plenipo for clients use a **Partner** plan, which covers
10, 25, or any number ([ADR-119](adr/ADR-119-editions-and-prices-revised.md), editions and prices).
The number is written in the license key and counted on your PC; nothing is sent to 8 West.

**Connections and add-on tools** are Pro ([ADR-068](adr/ADR-068-connections-are-pro.md),
Connections and add-on tools are part of Pro). When Pro ends, they pause: nothing is deleted,
running tasks finish, new work cannot use them until Pro is back, and **Disconnect** always works.
**Lessons** pause the same way ([ADR-112](adr/ADR-112-lessons-pause-on-free.md)): workers write no
new ones and use no kept ones on Free, and every kept lesson stays for when Pro is back.

**Workers at the same time** ([ADR-113](adr/ADR-113-workers-at-the-same-time.md)): on Free, a
fourth worker is never refused. It waits its turn, with a plain note, and starts by itself when one
finishes.

Free is meant to be enough to run one real project start to finish. Pro is for running a business
on it.

## What Pro costs

| Plan                  | Who it is for                                          | Organizations | Monthly          | Yearly (2 months free) |
| --------------------- | ------------------------------------------------------ | ------------- | ---------------- | ---------------------- |
| **Pro**               | One owner running their own business                   | 3             | **$19 a month**  | **$190 a year**        |
| **Partner 10**        | IT companies and agencies that run Plenipo for clients | 10            | **$79 a month**  | **$790 a year**        |
| **Partner 25**        | Same                                                   | 25            | **$149 a month** | **$1,490 a year**      |
| **Partner Unlimited** | Same                                                   | Any number    | **$299 a month** | **$2,990 a year**      |

Partner plans include everything in Pro, and are priced per technician (one person, any of their
own PCs). A **Business** plan for companies with many staff (fleet installs, company-wide
settings, invoices) comes later ([ADR-119](adr/ADR-119-editions-and-prices-revised.md)).

Free is free, with no card and no account, and stays that way.

## How Pro is unlocked

**Settings → License → Enter a license key.** The key unlocks the limits above straight away, with
no restart. Keys come from 8 West's store at getplenipo.com, by email. One Pro or Partner license
is for one person, on any of their own PCs, and covers its number of organizations on that PC
([ADR-110](adr/ADR-110-one-person-any-of-their-pcs.md)).

Because Pro is a subscription, Plenipo has to be able to tell that a subscription is still running.
So a **Pro** copy checks in with 8 West about **once a week**. Exactly what that check sends:

- your license key id
- which version of Plenipo you are running

That is the whole list. It never sends your projects, folder names, file paths, objectives, what
your workers did, which models they used, or anything from the Ledger. Your work stays on your
computer ([ADR-002](adr/ADR-002-local-first-architecture.md), local-first architecture). A test
checks the contents of that request byte for byte, and you can read it in the source.

**A Free copy never checks in at all.** If you have not paid, Plenipo never contacts 8 West.

## Plenipo on your phone, and what it sends to 8 West's relay

**Plenipo on your phone is part of Pro** (from v1.19.0, Phase 14:
[ADR-143](adr/ADR-143-the-relay-and-the-lock.md), the relay and the lock, and
[ADR-145](adr/ADR-145-what-a-phone-may-ask.md), the fixed list of what a phone may ask). Turn it on
at your PC in **Settings → Switches → Use Plenipo from another device**, and add your phone in
**Settings → Devices**. Your phone opens Plenipo's page at `remote.getplenipo.com` and talks to your
PC through 8 West's relay, sealed end to end: the relay passes messages along and cannot read or
change them. Your PC stays in charge. Guard decides every request, and Activity shows each one with
the phone that sent it. The relay is Plenipo's own, run by 8 West
([ADR-149](adr/ADR-149-plenipo-runs-its-own-relay.md), Plenipo runs its own relay): its code is in
this repository too, and you can read exactly what it does. It is on from v1.19.4; in v1.19.0 to
v1.19.3 the switch said **Coming soon** while the relay was set up.

Exactly what Plenipo sends to the relay, and nothing else (a test checks it):

- **From your PC:** its relay key (a public key Plenipo made for this), its proof that it holds that
  key, 8 West's signed weekly answer (the one the weekly check brings back: the key's ID, whether
  Pro is paid, and until when), the relay ID of each phone you remove, and sealed messages.
- **From your phone:** the pass your PC gave it, and sealed messages.

The relay can see that a PC and some phones are connected, when, how much they send, and their
internet addresses. **It cannot see** what they say: no approvals, objectives, workers, names, or
anything from your projects. It keeps nothing.

**Notices on your phone** (from v1.19.2) do not go through the relay. Your PC sends each one straight
to your phone's own notice service (Apple's, Google's, Mozilla's, or Microsoft's), sealed with your
phone's keys so only your phone can read it, and signed with your PC's notice key. The notice
service sees that a notice went to your phone, when, and how big it was, never what it says. What
a notice says is the same line your PC's own notice shows, and your phone can show only "Something
needs you" instead.

**A Free copy never connects to the relay**, even with the switch saved on. When Pro ends, phone
access pauses: your phones stay on the list, and work again when Pro is back.

**Checking for a new version is separate, and the same for Free and Pro** (from v1.9.0,
[ADR-038](adr/ADR-038-updates.md), updates). Every copy asks GitHub once a day whether a newer
Plenipo is out, by reading a public file from Plenipo's GitHub Releases. It sends nothing about you
or your work, and has nothing to do with a license. GitHub sees what any website sees (your
internet address, and that a copy of Plenipo asked). Nothing is downloaded or installed until you
choose **Install now**.

**No internet is fine.** Pro keeps working for **30 days** between successful checks, so a flight,
a dead router, or an 8 West outage never locks you out. If the check fails for any reason, Pro
stays on and tries again later. The 30 days count from 8 West's own signed time, so changing the
PC's clock does not change them ([ADR-116](adr/ADR-116-the-weekly-answer-is-signed.md)).

## What happens if you stop paying

Nothing you made is taken away.

Plenipo drops back to Free at the end of the period you paid for. Every organization, department,
project, worker record, and Ledger entry stays exactly where it is — visible, readable, and able to
finish what it started. What stops is **creating** something new past a Free limit; and
Connections, add-on tools, and lessons pause until Pro is back, with nothing in them deleted.

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
