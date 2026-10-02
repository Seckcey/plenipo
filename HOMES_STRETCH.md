# The Home Stretch

**As of 2026-10-01. Plenipo is at version v1.18.1.** Plenipo is made by 8 West Ventures, LLC.

This page tells you where Plenipo stands in its [rollout plan](ROLLOUT_PLAN.md). It uses short
sentences and everyday words on purpose.

## The plan in one minute

Plenipo is a program on your PC. You give it a goal. It hands the goal to a team of AI workers.
The workers do the job. You stay the boss. You approve the risky things.

The plan builds Plenipo one piece at a time. Each piece is called a **phase**. Each phase has a
number. A phase is done when its checks pass and a report is written.

There are 27 pieces in all. Here is the score:

| Where it stands                                      | How many | Which ones                                 |
| ---------------------------------------------------- | -------- | ------------------------------------------ |
| Done                                                 | 20       | 0 to 8, 10, 11, 11A, 12A, 12, 13, 17 to 21 |
| Mostly done (3 of 4 parts finished, the last parked) | 1        | 16                                         |
| Built, store not open yet (another session is on it) | 1        | 22                                         |
| The final push, not started                          | 3        | 14, 23, 24                                 |
| Parked, on hold until you say go                     | 2        | 9, 15                                      |

You are past the halfway point. On 2026-10-01 you set the order for the home stretch. It is called
**the final push**. It is written down in
[ADR-132](docs/adr/ADR-132-the-final-push.md). An ADR is a short note that records a big decision
and why it was made. ADR-132 is the choice to finish with three phases, in a set order, and to put
everything else on hold.

## The final push, in one picture

First, another session finishes opening the store (Phase 22). When that is done, the work goes in
this order:

1. **Phase 14.** Plenipo on your phone.
2. **Phase 23.** Mac and Linux.
3. **Phase 24.** Community.

Nothing else is scheduled. Phase 16's last part, Phase 15, and Phase 9 are **parked**. Parked means
they stay in the plan, but no one starts them until you say so.

## Part 1: What is done

Think of Plenipo as a building. These phases built the walls, the wiring, the locks, and the
furniture. The building works today.

| Phase | What it built, in plain words                                                                                                           | Version            |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| 0     | The foundation. The empty project, the rules, and the checks that keep the work tidy.                                                   | before v0.2.0      |
| 1     | The desktop window. The first thing you can open and see.                                                                               | v0.2.0             |
| 2     | The Ledger. A notebook where Plenipo writes down every task and event so nothing is forgotten.                                          | v0.3.0             |
| 3     | The first two AI tools, Claude Code and Codex. Plenipo can now start them and talk to them.                                             | v0.4.0             |
| 4     | The message bus. Workers can pass work to each other, even when they use different AI tools.                                            | v0.5.0             |
| 5     | The workforce. Departments, managers, supervisors, and workers, like a real company chart.                                              | v0.6.0             |
| 6     | Picking the right AI model for each job, by rules you can change.                                                                       | v0.7.0             |
| 7     | Guard and the approval box. Guard is Plenipo's security guard. It checks every move a worker wants to make. You approve the risky ones. | v0.8.0             |
| 8     | The Development department. A team that writes, reviews, tests, and documents code.                                                     | v1.0.0             |
| 10    | A browser and screen control. Workers can use a website or your screen, only where you allow.                                           | v1.3.0             |
| 11    | Servers over SSH. Workers can work on Linux servers, with Guard watching.                                                               | v1.6.0             |
| 12A   | One look for every screen. Same buttons, same colors, light and dark.                                                                   | v1.7.0             |
| 12    | The everyday screens. Home, a page for each thing, pop-up notices, Settings, and a terminal.                                            | v1.8.0             |
| 13    | Installing, updating, and recovering on Windows. Plenipo lives in the tray and can start with Windows.                                  | v1.9.0             |
| 17    | Your control over workers. Pick the model, pick how hard it thinks, archive a worker, bring one back.                                   | v1.10.0            |
| 18    | The organization canvas. A picture of your company, and you can watch workers write code as it happens.                                 | v1.11.0            |
| 19    | The AI tools page. Sign in, see how much you have used, and update each tool.                                                           | v1.12.0            |
| 20    | Connections. Plenipo can work with Microsoft 365, Slack, Google, HubSpot, Stripe, and WordPress.                                        | v1.13.0 to v1.14.2 |
| 21    | The workspace. Panels, windows, files, and more than one organization, like yours and a client's.                                       | v1.16.0            |
| 11A   | Free and Pro. A license key decides which edition you have. Free has limits. Pro lifts them.                                            | v1.18.0            |

### Phase 16 is mostly done

Phase 16 is "every AI model worth having". It comes in four parts. The plan calls them waves.

| Wave | What it did                                                                                                                          | Version                   | State                |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------- | -------------------- |
| 1    | Every model now shows who made it. Antigravity (Google's AI tool) was added.                                                         | v1.14.0                   | Done                 |
| 2    | GitHub Copilot was added. Cursor was checked and has to wait, because nothing can tell Plenipo whether Cursor may charge extra.      | v1.15.0                   | Done                 |
| 3    | Paid AI keys. You can type in your own key and pay per use. Plenipo prices every paid task. Spending caps are there if you want one. | v1.17.0, fixed in v1.18.1 | Done                 |
| 4    | Tools for every model, and ready-made specialist jobs.                                                                               | not yet                   | **Parked** (ADR-132) |

### Phase 22 is built but not open

Phase 22 is the 8 West account service. It is the online store and front desk for Plenipo Pro. It
lives in a private repository named `plenipo-account`. The code is written and tested. It is not
open to real customers, and it only plays with pretend money for now. **Another session is
finishing it.** See Part 3 for what that takes.

## Part 2: What is not done

### Being finished in another session

| Phase        | What it is, in a few words                   | State                                  |
| ------------ | -------------------------------------------- | -------------------------------------- |
| 22 (go live) | Open the Plenipo Pro store to real customers | Built, launch steps being finished now |

### The final push, in order

These start when Phase 22 is live. Phase 14 does not start until you say the store is live.

| Order | Phase | What it is, in a few words | State                                   |
| ----- | ----- | -------------------------- | --------------------------------------- |
| 1     | 14    | Plenipo on your phone      | Not started. Next after the store opens |
| 2     | 23    | Mac and Linux              | Not started. After Phase 14             |
| 3     | 24    | Community                  | Not started. Last                       |

### Parked

These stay in the plan, exactly as written. No one starts them until you schedule them.

| Phase      | What it is, in a few words                                   | State                              |
| ---------- | ------------------------------------------------------------ | ---------------------------------- |
| 16, Wave 4 | Tools for every AI model, then ready-made specialist jobs    | Parked (ADR-132)                   |
| 15         | More AI tools and departments, Windows servers, and Milepost | Parked (ADR-132)                   |
| 9          | A Sales department that uses HubSpot                         | Parked (postponed before, ADR-018) |

The plan says you sell Pro once the app is finished. The final push is now the finish line.

## Part 3: What is left in the plan, as a checklist

Phases 0 to 15 were the first plan. Phases 16 to 24 were added later. Each line below says which
is which, in italics.

**Being finished in another session**

- [ ] **Phase 22, go live.** Do the launch steps (listed below), then open the store.
      _Added later (2026-09-28)._

**The final push, in this order**

- [ ] **1. Phase 14.** Plenipo on your phone. _In the first plan, then changed on 2026-09-27._
- [ ] **2. Phase 23.** Mac and Linux. _Added later (2026-09-28)._
- [ ] **3. Phase 24.** Community. _Added later (2026-09-28)._

**Parked, not scheduled**

- [ ] **Phase 16, Wave 4, part 1.** Give workers on Ollama, OpenRouter, and paid keys real tools.
      _Added later (2026-09-27). Changed on 2026-09-30._
- [ ] **Phase 16, Wave 4, part 2.** Add ready-made specialist jobs, like Security Reviewer.
      _Added later._
- [ ] **Phase 16, small leftover.** Add more Ollama cloud models once your paid Ollama plan starts.
      _Added later._
- [ ] **Phase 15.** More AI tools and departments, Windows servers, and the Milepost link.
      _In the first plan. Windows servers and Milepost were added 2026-09-27._
- [ ] **Phase 9.** Sales department on HubSpot. _In the first plan. Postponed on 2026-09-27._

### Launch steps for the store (Phase 22)

The report says these come before real customers. Nothing here is a surprise.

- [ ] Do one practice purchase, start to finish, in Stripe's test mode. The key must arrive by
      email, and Plenipo must accept it.
- [ ] Put the secret settings in place: the server, Stripe, Microsoft 365, and Cloudflare. Secrets
      are never pasted into chat or into the code.
- [ ] A lawyer reads the terms and the privacy policy.
- [ ] A second security check of the store, because it faces the internet.
- [ ] Turn on real money. The owner flips this switch on purpose.
- [ ] Answer a few small questions the builder left for you. For example, should switching from
      monthly to yearly send a new key?

### Checks only you can do

The reports say these were left for you to do on a real Windows PC. I did not check whether you
have done any of them since.

- [ ] Phase 11A: type in a real license key and watch Pro turn on. Restart. Remove the key. Then
      uninstall with "delete my data" and see the key gone.
- [ ] Phase 21: the walk-through of several organizations on a real PC.
- [ ] Phase 17: the walk-through with real AI tools signed in.
- [ ] Phase 11: the check against a real SSH server.

## Part 4: The final phases, in simple words

First comes the store, which another session is finishing. Then come the three phases of the final
push. The parked phases are at the end, so you remember what is waiting.

### Before the push: Phase 22, open the store

**The picture.** Plenipo Pro is the paid edition. Phase 22 is the shop counter. A customer visits
the website, pays, and gets a key by email. They type the key into Plenipo, and Pro turns on.

**What is built.** Accounts. Buying with Stripe (Stripe is the company that handles the money, so
card numbers never touch 8 West). The key. The weekly check. Emails. An admin page for you.

**What is left.** The launch steps in Part 3. The big ones are a practice purchase, the secret
settings, a lawyer's read of the terms, and a second security check.

**The money, as it stands today.** Pro is $19 and covers 3 organizations. Partner plans cover 10,
25, or any number. This is from
[ADR-119](docs/adr/ADR-119-editions-and-prices-revised.md), the decision that changed the price and
the plans. Free stays free and is fully useful.

**One safety promise.** If the store ever breaks, Pro stays on for people who paid. Plenipo works
offline for 30 days at a time.

**Done looks like:** a new customer buys Pro on the website, gets the key, enters it, and Pro turns
on. If they cancel, they keep Pro until the paid time runs out, then go back to Free. Nothing is
lost.

### Final push 1: Phase 14, Plenipo on your phone

**The picture.** Today you have to sit at your PC to approve a risky step. This phase lets you do
it from your phone.

**Why it comes first.** It is a Pro feature, so the store has to be open. It may also pair your
phone through your 8 West account, which the store builds.

**How it works.** You open a web page on your phone. No app to install. The phone talks to your PC
through a relay (a middle stop that only passes sealed messages along). 8 West already runs this
relay for Milepost. Nobody can read the messages in the middle.

**What you can do from the phone.** Approve or refuse a step. Send a goal to a manager. Stop a
task. Look at what the workers are doing. You can even tap Approve right on a notice.

**What stays on your PC only.** The terminal, your files, the screen, your secrets, and anything
that changes what workers are allowed to do. A stolen phone cannot widen the rules.

**Good to know.** Your PC stays in charge, and Guard decides every request. You can switch the
whole thing off, and every phone is cut off at once. The relay needs a small change made in its own
repository, with your approval there. The decision is in
[ADR-040](docs/adr/ADR-040-phone-web-interface.md).

**Done looks like:** from a phone, you approve, refuse, and send a goal. Guard on the PC decides
each one, and the Ledger records it.

### Final push 2: Phase 23, Mac and Linux

**The picture.** Today Plenipo is a Windows program. This phase teaches it to run on a Mac and on
Linux, with the same safety.

**Why it is real work.** About 110 places in the code only make sense on Windows. Each one needs a
Mac or Linux version. Examples are how Plenipo stops a program and everything it started, how it
starts at sign-in, and how it shows in the tray.

**What it needs from you.** A Mac to test on, and an Apple developer account.

**Done looks like:** you install Plenipo on a Mac and on a Linux PC, sign in to Claude Code on
each, and run a Development goal from start to finish with the same approvals as on Windows.

### Final push 3: Phase 24, Community

**The picture.** This is a neighborhood for Plenipo owners. It is the very last phase.

**What it adds.** Optional public profiles. Private messages. Linked organizations, where one owner
can send a goal to another, and the other owner's Guard and approval decide. Collaborators, who
can be viewers, approvers, or managers in your organization. Block, report, and leave, everywhere.

**The firm rule.** Nobody reaches into anyone else's PC, files, sign-ins, or keys. Words from other
people are treated as untrusted, like a stranger's email.

**What it needs first.** Phase 22 (accounts) and Phase 14 (the phone connection). The final push
order covers both. A lawyer must also help write the terms of service, the age rule, the
moderation plan, and the privacy policy.

**Done looks like:** two owners link their organizations. One sends the other a goal. It runs only
after the other owner approves it. Both Ledgers show every step, and nothing else crossed over.

### Parked: what is waiting

These are not in the final push. They stay in the plan. You can schedule any of them later.

**Phase 16, Wave 4: hands for every AI model.**

- _The picture._ Some workers are allowed inside the workshop. They can pick up tools, open files,
  and run programs. Claude Code, Codex, Grok, and Kimi are like that. Other workers can only talk
  through a window. Ollama, OpenRouter, and the paid-key AI companies are like that. Wave 4 lets
  them in, with Guard at the door.
- _Part 1:_ a worker on one of those models can ask for a tool. Plenipo does the work for it, after
  Guard says yes. Every move is written in the Ledger. Only models that can use tools get them.
  Each task has a most-steps limit, so a worker that gets stuck stops.
- _Part 2:_ ready-made jobs you pick when you add a worker: Security Reviewer, Code Reviewer,
  Researcher, Writer, and IT Support for Windows servers. Any AI tool can do them.
- _Why it changed._ You dropped an outside program called Hermes Agent on 2026-09-30, because
  Plenipo can build the same thing itself and keep Guard in charge. See
  [ADR-131](docs/adr/ADR-131-tools-for-any-model.md).
- Until then, workers on those models answer in text only, as they do today.

**Phase 15: more tools, more departments, Windows servers, Milepost.**

- A written rulebook so a new AI tool or department can be added without touching the core.
- Starter departments: Marketing, and Operations (the team that watches servers).
- Windows servers. Every 8 West IT client runs them, as old as Server 2016. Today Plenipo can only
  work on Linux servers.
- A link to Milepost, 8 West IT's own remote management tool. It comes last and is not a priority.
  Every server reached this way counts as Production, so every command asks you first. Milepost
  needs a small new door built in its own repository first.

**Phase 9: a Sales department.**

- A second department next to Development, with a Sales Manager and sales workers. They use your
  HubSpot account as the customer list.
- The big rule: HubSpot owns the customer records. Plenipo owns the tasks and the approvals. No
  message goes out to a customer without your approval.
- The HubSpot connection is already built (Phase 20). It is a Pro feature.
- You postponed it on 2026-09-27 and dropped an earlier idea to bring in an outside product called
  Paperclip. See [ADR-018](docs/adr/ADR-018-sales-on-hubspot-no-paperclip.md).
- **Heads up:** the Pro table in [Free and Pro](docs/editions.md) now marks Sales as planned, so no
  buyer is promised it.

## Where this page comes from

- [ROLLOUT_PLAN.md](ROLLOUT_PLAN.md): the phases, their order, and the status lines.
- [ADR-132](docs/adr/ADR-132-the-final-push.md): the final push, and what is parked.
- [Phase 22 acceptance report](docs/phases/phase-22-acceptance-report.md): what is left before
  launch.
- [Phase 16, Wave 3 acceptance report](docs/phases/phase-16-wave-3-acceptance-report.md): the paid
  keys.
- [Free and Pro](docs/editions.md), the [roadmap](docs/roadmap.md), and the release notes in
  [docs/releases](docs/releases/).
