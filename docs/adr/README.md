# Architecture Decision Records

An ADR records one architecturally significant decision: context, the decision, and its
consequences. Per the rollout plan (§8.8), any deviation from `ROLLOUT_PLAN.md` that alters
architecture must be recorded here.

## Process

1. Copy [`ADR-000-template.md`](ADR-000-template.md) to `ADR-NNN-short-title.md` (next number).
2. Status starts as **Proposed**; the owner marks it **Accepted**.
3. Never rewrite an accepted ADR's decision. Supersede it with a new ADR and set the old one to
   **Superseded by ADR-NNN**.

## Index

| ADR                                                                  | Title                                                                                                                  | Status   |
| -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | -------- |
| [001](ADR-001-desktop-stack.md)                                      | Tauri 2 + React/TypeScript + Rust desktop stack                                                                        | Accepted |
| [002](ADR-002-local-first-architecture.md)                           | Local-first architecture                                                                                               | Accepted |
| [003](ADR-003-provider-independent-roles.md)                         | Provider-independent roles                                                                                             | Accepted |
| [004](ADR-004-repository-layout.md)                                  | Minimal monorepo layout, grow crates per phase                                                                         | Accepted |
| [005](ADR-005-runtime-supervisor.md)                                 | Runtime supervisor boundary                                                                                            | Accepted |
| [006](ADR-006-ledger.md)                                             | Plenipo Ledger (SQLite system of record)                                                                               | Accepted |
| [007](ADR-007-runtime-adapters.md)                                   | Provider runtime adapters (Codex, Claude Code)                                                                         | Accepted |
| [008](ADR-008-liaison.md)                                            | Liaison message bus and cross-provider handoffs (amended by 259)                                                       | Accepted |
| [009](ADR-009-workforce.md)                                          | Workforce organization engine and topology canvas (amended by 039)                                                     | Accepted |
| [010](ADR-010-plain-titles.md)                                       | Plain words, chain of command, choosable ranks                                                                         | Accepted |
| [011](ADR-011-model-policy-routing.md)                               | Router: model registry, role policies, routing                                                                         | Accepted |
| [012](ADR-012-brief-agent-messages.md)                               | Brief messages between agents                                                                                          | Accepted |
| [013](ADR-013-guard-capability-broker.md)                            | Guard, capability broker, and human approval (amended by 048)                                                          | Accepted |
| [014](ADR-014-adding-ai-tools.md)                                    | Adding AI tools ahead of Phase 15                                                                                      | Accepted |
| [015](ADR-015-acp-ai-tools.md)                                       | Running AI tools over ACP                                                                                              | Accepted |
| [016](ADR-016-development-department.md)                             | Development department: delegation, working copies, GitHub, result                                                     | Accepted |
| [017](ADR-017-ollama-cloud-models.md)                                | Ollama cloud models through its service                                                                                | Accepted |
| [018](ADR-018-sales-on-hubspot-no-paperclip.md)                      | Phase 9 postponed: no Paperclip; a Sales department later, on HubSpot                                                  | Accepted |
| [019](ADR-019-role-working-instructions.md)                          | Every role knows its job: working instructions for all roles                                                           | Accepted |
| [020](ADR-020-browser-and-computer-use.md)                           | Plenipo's browser and computer use, through Guard (amended by 023, 028, 035, 046, 047, 049)                            | Accepted |
| [021](ADR-021-editions-and-license.md)                               | Free and Pro editions under the Elastic License 2.0                                                                    | Accepted |
| [022](ADR-022-subscription-and-license-check.md)                     | Subscription pricing and the weekly license check (amended by 211)                                                     | Accepted |
| [023](ADR-023-settings-switches.md)                                  | On/off switches in Settings                                                                                            | Accepted |
| [024](ADR-024-workers-learn-from-work.md)                            | Workers learn from their work (amended by 041, 045, 050)                                                               | Accepted |
| [025](ADR-025-servers-over-ssh.md)                                   | Servers over SSH, through Guard                                                                                        | Accepted |
| [026](ADR-026-ssh-built-in.md)                                       | SSH built into Plenipo (russh), not Windows' ssh.exe                                                                   | Accepted |
| [027](ADR-027-acp-file-access-through-plenipo.md)                    | Kimi over ACP: file access through Plenipo                                                                             | Accepted |
| [028](ADR-028-choosing-plenipos-browser.md)                          | Choosing Plenipo's browser: Automatic, Edge, or Chrome                                                                 | Accepted |
| [029](ADR-029-captcha-attempts.md)                                   | Workers try a CAPTCHA three times before handing it to the owner (amended by 032)                                      | Accepted |
| [030](ADR-030-design-system.md)                                      | One design system for every screen                                                                                     | Accepted |
| [031](ADR-031-terminal-panel.md)                                     | The terminal panel (amends 025)                                                                                        | Accepted |
| [032](ADR-032-captcha-checkbox-and-verdict.md)                       | Workers see the CAPTCHA they try, and hear how each try went (amends 029)                                              | Accepted |
| [033](ADR-033-pages-notices-settings.md)                             | Home, a page for each thing, pop-up notices, and Settings in one place                                                 | Accepted |
| [034](ADR-034-approved-programs-run-as-the-owner.md)                 | Approved programs run as the owner: tickets bound to the AI tool, safer defaults (amended by 213)                      | Accepted |
| [035](ADR-035-network-gate-covers-sockets.md)                        | The network gate covers beacons, sends on the page's own, and live connections (amends 020)                            | Accepted |
| [036](ADR-036-every-ai-model.md)                                     | Every AI model worth having: paid keys with spending caps, maker and runner, routes                                    | Accepted |
| [037](ADR-037-background-work.md)                                    | Background work: Plenipo lives in the tray, and the window comes and goes                                              | Accepted |
| [038](ADR-038-updates.md)                                            | Updates from GitHub Releases, signed twice, installed only when you say so (amended by 052)                            | Accepted |
| [039](ADR-039-owners-notes-order-of-work.md)                         | The owner's notes: eight new phases, watching code live, the order of work (amends 009)                                | Accepted |
| [040](ADR-040-phone-web-interface.md)                                | Phase 14 is Plenipo's own web interface for a phone; CrewOS leaves the plan (amended by 144)                           | Accepted |
| [041](ADR-041-model-effort-learning-layers.md)                       | Model, effort, and learning set in layers, the closest winning (amends 011, 024)                                       | Accepted |
| [042](ADR-042-specialties.md)                                        | Specialties under each role, built in and your own (amends 019)                                                        | Accepted |
| [043](ADR-043-archive-bring-back-delete.md)                          | Archive, bring back, and delete for good (carries out 039 §2.1; amends 009)                                            | Accepted |
| [044](ADR-044-prompts-sized-to-the-job.md)                           | Prompts sized to the job: measured, full instructions only when needed (amends 008, 012)                               | Accepted |
| [045](ADR-045-experience-and-the-workforce.md)                       | Experience and the Workforce: keeping your best agents (amends 024, 043)                                               | Accepted |
| [046](ADR-046-new-tabs-open-in-the-workers-tab.md)                   | New tabs open in the worker's own tab (amends 020)                                                                     | Accepted |
| [047](ADR-047-browser-never-saves-files.md)                          | Plenipo's browser does not save files (amends 020)                                                                     | Accepted |
| [048](ADR-048-secrets-only-to-their-programs.md)                     | Secrets reach only the programs they are for (amends 013)                                                              | Accepted |
| [049](ADR-049-computer-use-asks-every-step.md)                       | Computer use asks before every click and keystroke (amends 020)                                                        | Accepted |
| [050](ADR-050-lessons-kept-on-their-own.md)                          | Lessons a role keeps on its own are notes, not orders (amends 024)                                                     | Accepted |
| [051](ADR-051-codex-own-shell.md)                                    | Codex works through Plenipo's tools: its own command tool is off (amends 007)                                          | Accepted |
| [052](ADR-052-release-signing-environment.md)                        | Signing runs only for main and release tags, behind the owner's approval (amends 038)                                  | Accepted |
| [053](ADR-053-the-organization-canvas.md)                            | The organization canvas: arrange, rewire, the trash can, and a live view (amends 009)                                  | Accepted |
| [054](ADR-054-move-or-lend.md)                                       | Move or lend an agent to another team; a task's own team decides its limits (amends 009, 013)                          | Accepted |
| [055](ADR-055-watch-a-worker-write-code.md)                          | Watch: seeing a worker write code as it happens (amends 031)                                                           | Accepted |
| [056](ADR-056-the-owners-tile.md)                                    | The owner's tile: your picture, status, mood, and message                                                              | Accepted |
| [057](ADR-057-addresses-in-the-record.md)                            | Web addresses in the record keep the page and the names of its fields (amends 020)                                     | Accepted |
| [058](ADR-058-signing-in-to-ai-tools-in-the-terminal.md)             | Signing in to an AI tool in a terminal tab (amends 031)                                                                | Accepted |
| [059](ADR-059-plenipo-updates-the-ai-tools.md)                       | Plenipo keeps the AI tools up to date, between tasks (amends 007, 023)                                                 | Accepted |
| [060](ADR-060-usage-plan-left-and-new-models.md)                     | Usage, "plan left", and new models, only from what the AI tools report (amends 007, 014)                               | Accepted |
| [061](ADR-061-connections-before-new-ai-models.md)                   | Doing Connections before new AI models: Phase 20 moves ahead of Phase 16 (amends 039)                                  | Accepted |
| [062](ADR-062-connections-one-set-of-rules.md)                       | Connections: one set of rules for every connection (amends 013, 023)                                                   | Accepted |
| [063](ADR-063-signing-in-to-a-connection.md)                         | Signing in to a connection in your own browser; its token only in the Vault (amends 013)                               | Accepted |
| [064](ADR-064-how-each-connection-is-built.md)                       | How each connection is built: into Plenipo, or the service's own MCP server                                            | Accepted |
| [065](ADR-065-microsoft-365-connection.md)                           | The Microsoft 365 connection: 8 West's app, the fewest permissions, what admins approve                                | Accepted |
| [066](ADR-066-add-on-tools.md)                                       | Add-on tools you set up: other MCP servers, as approved programs, off by default (amends 013)                          | Accepted |
| [067](ADR-067-phase-20-in-three-parts.md)                            | Phase 20 in three parts: Microsoft 365, then Slack and Google, then the rest                                           | Accepted |
| [068](ADR-068-connections-are-pro.md)                                | Connections and add-on tools are part of Pro (amends 021)                                                              | Accepted |
| [069](ADR-069-website-follows-releases.md)                           | The website follows new releases by itself, with each release's notes                                                  | Accepted |
| [070](ADR-070-slack-and-google-choices.md)                           | Slack and Google: the owner's choices, and what their sign-ins need (amends 062, 063, 064)                             | Accepted |
| [071](ADR-071-keys-website-and-add-on-choices.md)                    | HubSpot, Stripe, the website, and add-on tools: the owner's choices (amends 062–064, 066)                              | Accepted |
| [080](ADR-080-phase-16-wave-1-alongside-phase-20.md)                 | Building Phase 16's first wave alongside Phase 20 (amends 061)                                                         | Accepted |
| [081](ADR-081-who-made-each-model.md)                                | Who made each model: cross-company review by maker; the list two ways (amends 011, 014)                                | Accepted |
| [082](ADR-082-antigravity-as-an-ai-tool.md)                          | Antigravity as an AI tool, with a settings folder of its own (amends 081; adds to 007, 058)                            | Accepted |
| [083](ADR-083-github-copilot-as-an-ai-tool.md)                       | GitHub Copilot as an AI tool, checked before every task over its two-way link (amends 014)                             | Proposed |
| [084](ADR-084-cursor-agent-waits.md)                                 | Cursor's agent waits: no check a program can run for paid extra use                                                    | Accepted |
| [085](ADR-085-paid-ai-keys-with-spending-caps.md)                    | Paid AI keys with spending caps and a record of every paid task (amends 003, 007, 011, 014)                            | Proposed |
| [086](ADR-086-openrouter-through-a-plenipo-helper.md)                | OpenRouter through a Plenipo helper: one fixed address, the key on standard input                                      | Proposed |
| [087](ADR-087-direct-keys-for-every-ai-company.md)                   | Direct keys for every AI company whose models take one, through Plenipo's helper                                       | Proposed |
| [090](ADR-090-phase-21-alongside-phase-16-wave-2.md)                 | Building Phase 21 alongside Phase 16's second wave; ADR-090 to 099 for Phase 21 (amends 039)                           | Accepted |
| [091](ADR-091-phase-21-owners-answers.md)                            | Phase 21: what the check found, and the owner's answers (amends 021, 033, 055)                                         | Accepted |
| [092](ADR-092-panels-and-windows.md)                                 | Panels and windows: resize, dock, pop out, drag out, and Reset layout (amends 031, 033, 055)                           | Accepted |
| [093](ADR-093-your-files-and-the-editor.md)                          | Your files and the editor; one writer at a time; files on an objective (amends 016, 055)                               | Accepted |
| [094](ADR-094-more-than-one-organization.md)                         | More than one organization: each its own Ledger, window, backups, and Vault names                                      | Accepted |
| [100](ADR-100-phase-11a-22-owners-answers.md)                        | Phase 11A and 22: what the check found, and the owner's answers; ADR-100 to 129 for them                               | Accepted |
| [101](ADR-101-account-service-repository-name.md)                    | The account service's repository is `plenipo-account`, with its own rules                                              | Accepted |
| [102](ADR-102-account-service-repository-private.md)                 | The account service's repository is private                                                                            | Accepted |
| [103](ADR-103-account-service-hosting.md)                            | Hosting, backups, and monitoring for the account service                                                               | Accepted |
| [104](ADR-104-signing-key-in-aws-kms.md)                             | The license signing key lives in AWS KMS; license keys stay Ed25519 (carries out 039 §2.14)                            | Accepted |
| [105](ADR-105-domain-and-web-address.md)                             | Plenipo's own domain is getplenipo.com; the account service at account.getplenipo.com                                  | Accepted |
| [106](ADR-106-account-email.md)                                      | Stripe sends billing email; account email goes through Microsoft 365                                                   | Accepted |
| [107](ADR-107-admin-sign-in.md)                                      | 8 West signs in to the admin page with a password and a passkey, behind Cloudflare Access                              | Accepted |
| [108](ADR-108-no-managed-payments.md)                                | No Stripe Managed Payments at launch; Stripe Tax as planned                                                            | Accepted |
| [109](ADR-109-plenipos-own-stripe-account.md)                        | Plenipo has its own Stripe account (carries out 039 §2.13)                                                             | Accepted |
| [110](ADR-110-one-person-any-of-their-pcs.md)                        | One subscription is for one person, on any of their own PCs; one license per PC                                        | Accepted |
| [111](ADR-111-refunds-and-terms-of-sale.md)                          | Refunds, and where the terms of sale and the account privacy notice live                                               | Accepted |
| [112](ADR-112-lessons-pause-on-free.md)                              | Lessons pause on Free, like Connections (carries out 021, 024)                                                         | Accepted |
| [113](ADR-113-workers-at-the-same-time.md)                           | Workers at once: Free's fourth waits its turn; Pro runs four per organization (amends 021)                             | Accepted |
| [114](ADR-114-business-departments.md)                               | What counts as a business department                                                                                   | Accepted |
| [115](ADR-115-free-never-contacts-8-west.md)                         | A Free copy never contacts 8 West                                                                                      | Accepted |
| [116](ADR-116-the-weekly-answer-is-signed.md)                        | The weekly check's answer is signed, and its time decides the grace (adds to 022; amended by 211)                      | Accepted |
| [117](ADR-117-paid-ai-keys-are-free.md)                              | Paid AI keys are in the Free edition (amends 021)                                                                      | Accepted |
| [118](ADR-118-customer-accounts.md)                                  | Customer accounts: signing in, and deleting an account                                                                 | Accepted |
| [119](ADR-119-editions-and-prices-revised.md)                        | Editions and prices, revised: Pro $19 with 3 organizations; Partner plans by organizations                             | Accepted |
| [130](ADR-130-website-domain-migration.md)                           | The public website moves to getplenipo.com; old and www addresses preserve paths in redirects                          | Accepted |
| [131](ADR-131-tools-for-any-model.md)                                | Wave 4: Plenipo's own tools for any model, then specialist jobs; no Hermes (changes 036)                               | Accepted |
| [132](ADR-132-the-final-push.md)                                     | The final push: Phase 14, then Mac and Linux, then Community; Wave 4, 15, and 9 parked                                 | Accepted |
| [140](ADR-140-phase-14-starts.md)                                    | Phase 14 starts: numbers 140 to 149, what the check found, three parts                                                 | Accepted |
| [141](ADR-141-pairing-a-phone.md)                                    | Pairing a phone: a picture code or a typed code, shown on your PC (amended by 212)                                     | Accepted |
| [142](ADR-142-the-phone-proves-it-is-you.md)                         | The phone proves it is you: a passkey, checked by your PC (amended by 212)                                             | Accepted |
| [143](ADR-143-the-relay-and-the-lock.md)                             | The relay and the lock: sealed end to end, no copies, wrong tries (amended by 147, 149)                                | Accepted |
| [144](ADR-144-notices-on-your-phone.md)                              | Notices on your phone, sealed for it, sent straight from the PC (amends 040)                                           | Accepted |
| [145](ADR-145-what-a-phone-may-ask.md)                               | The fixed list of what a phone may ask; what stays on your PC                                                          | Accepted |
| [146](ADR-146-where-the-phone-page-lives.md)                         | Where the phone's page lives: its own address, never the relay (amended by 148, 149)                                   | Accepted |
| [147](ADR-147-relay-passes-last-90-days.md)                          | Relay passes last 90 days, renewed at every sign-in (amends 143)                                                       | Accepted |
| [148](ADR-148-the-phone-page-on-its-own-server.md)                   | The phone's page on its own small AWS server, not Coastline (amends 146)                                               | Accepted |
| [149](ADR-149-plenipo-runs-its-own-relay.md)                         | Plenipo runs its own relay, from this repository, on 8 West's server (amends 143, 146)                                 | Accepted |
| [150](ADR-150-phase-23-starts.md)                                    | Phase 23 starts: numbers 150 to 159, what the check found, five waves                                                  | Accepted |
| [151](ADR-151-one-repository-for-every-system.md)                    | One repository for Windows, Mac, and Linux                                                                             | Accepted |
| [152](ADR-152-which-systems-and-in-what-order.md)                    | Which systems, and in what order: Linux first, then every Mac from macOS 13                                            | Accepted |
| [153](ADR-153-saved-keys-on-linux.md)                                | Saved keys on Linux live in the system's password store                                                                | Accepted |
| [154](ADR-154-computer-use-on-linux.md)                              | Computer use on Linux: X11 in Wave 2, Wayland in Wave 4                                                                | Accepted |
| [155](ADR-155-each-systems-own-words.md)                             | Each system's own words on screen                                                                                      | Accepted |
| [156](ADR-156-refuse-unchecked-tool-calls.md)                        | When Plenipo cannot tell which program sent a tool call, it refuses (amends 034)                                       | Accepted |
| [157](ADR-157-a-keeper-ends-programs-after-a-crash.md)               | On a Mac and Linux, a keeper ends a worker's programs after a crash (amends 005)                                       | Accepted |
| [158](ADR-158-programs-that-leave-their-group.md)                    | On a Mac and Linux, a program that leaves its group still ends with its work (amends 157)                              | Accepted |
| [160](ADR-160-phase-24-alongside-phase-23.md)                        | Building Phase 24 (Community) alongside Phase 23 (Mac and Linux) (amended by 171)                                      | Accepted |
| [161](ADR-161-phase-24-starts.md)                                    | Phase 24 starts: what the check found, six parts, the owner's answers (amended by 170 to 172)                          | Accepted |
| [162](ADR-162-your-account-in-plenipo.md)                            | Your 8 West account in Plenipo: sign in with a code, 13 and older, who needs Pro (amends 115)                          | Accepted |
| [163](ADR-163-profiles-and-finding-people.md)                        | Your profile, and finding people: the directory, an exact name, or an email                                            | Accepted |
| [164](ADR-164-private-messages-sealed.md)                            | Private messages, sealed end to end, any keyboard text, reports with a proof (amended by 172)                          | Accepted |
| [165](ADR-165-linked-organizations.md)                               | Linked organizations: link, objectives, the answer you send back, unlink                                               | Accepted |
| [166](ADR-166-collaborators.md)                                      | Collaborators: viewer, approver, manager; removed at once                                                              | Accepted |
| [167](ADR-167-block-report-leave.md)                                 | Block, report, and leave; 8 West handles reports                                                                       | Accepted |
| [168](ADR-168-what-is-kept-and-for-how-long.md)                      | What Community keeps, where, and for how long                                                                          | Accepted |
| [169](ADR-169-rewards-for-taking-part.md)                            | Rewards for taking part: points, a leaderboard, badges, thanks, a free month of Pro for invitations                    | Accepted |
| [170](ADR-170-community-follows-8-wests-switch.md)                   | Community follows 8 West's switch: Coming soon until it opens, no release to turn it on                                | Accepted |
| [171](ADR-171-switching-on-part-by-part.md)                          | Switching Community on part by part: people, then linked organizations, then collaborators                             | Accepted |
| [172](ADR-172-what-part-24c-changes.md)                              | What part 24C changes: no stickers yet, Delete for me on this PC, email invitations (amends 169)                       | Accepted |
| [173](ADR-173-leave-deletes-from-this-pc.md)                         | Leave this conversation deletes it from this PC (amends 167)                                                           | Accepted |
| [190](ADR-190-phase-25-starts.md)                                    | Phase 25 starts: fixes and a simpler Plenipo before launch, in four waves (amends 132)                                 | Accepted |
| [191](ADR-191-a-model-you-pick-by-name-is-your-choice.md)            | A model you pick by name is your choice: an unknown maker warns instead of refusing (amends 081)                       | Accepted |
| [192](ADR-192-one-set-of-paid-keys-for-the-pc.md)                    | One set of paid AI keys for the whole PC; switches and caps stay each organization's (amends 085, 094)                 | Accepted |
| [193](ADR-193-watch-shows-the-team.md)                               | Watch shows the team's work, says why it is empty, and is on every tile (amends 055)                                   | Accepted |
| [194](ADR-194-what-a-model-can-do-is-no-longer-asked.md)             | What a model can do, and its context size, are no longer asked; "sees images" comes from who made it (amends 011, 042) | Accepted |
| [195](ADR-195-who-uses-what.md)                                      | Who uses what: every model in one menu, one table, "never use" for the whole organization (amends 041, 081)            | Accepted |
| [196](ADR-196-use-the-team-you-hired-first.md)                       | Use the team you hired first, and ask before hiring a missing worker (amends 016, 054)                                 | Accepted |
| [197](ADR-197-templates.md)                                          | Templates for organizations, departments, and projects, and saving yours as one (part of parked Phase 15)              | Accepted |
| [198](ADR-198-the-setup-tour.md)                                     | The setup tour, with Driver.js: nine steps that move on when each is done (amends 030, 053)                            | Accepted |
| [199](ADR-199-stop-all-work.md)                                      | Stop on every worker, and Stop all work: every task stops and nothing starts until Allow again (amends 020, 094)       | Accepted |
| [200](ADR-200-a-live-chat-with-each-agent.md)                        | A live chat with each agent, as in Claude Code: streaming, what it is doing now, files saved                           | Proposed |
| [201](ADR-201-light-by-default.md)                                   | Light by default: agents save files and run programs; Settings → Safety; Plenipo's own folder                          | Proposed |
| [202](ADR-202-the-chain-of-command.md)                               | The chain of command: skipped leads are told, reports come back up one level at a time                                 | Proposed |
| [211][adr-211]                                                       | Pro ends a set time after the paid period (amends 022, 116)                                                            | Proposed |
| [212](ADR-212-the-owner-compares-six-digits.md)                      | The owner compares six digits; a page that refreshes by itself keeps no sign-in (amends 141, 142)                      | Proposed |
| [213](ADR-213-build-and-test-commands-ask-first.md)                  | Build and test commands ask first under Careful: off the starting approved list; Light unchanged (amends 034)          | Proposed |
| [250](ADR-250-watch-shows-changes-made-by-commands.md)               | Watch shows changes made by commands too: files noted before and compared after (amends 055)                           | Accepted |
| [251](ADR-251-side-chats.md)                                         | Side chats with a manager or supervisor: answer only, briefed on what the agent knows                                  | Accepted |
| [252](ADR-252-prompt-caching-for-anthropic-models.md)                | Prompt caching for Anthropic models on your key and through OpenRouter (amends 085 §3.5)                               | Accepted |
| [253](ADR-253-when-a-plan-runs-out.md)                               | When a plan runs out: a notice with your choices, and the work picked back up after the reset (amends 037)             | Accepted |
| [254](ADR-254-your-subscription-first-then-the-same-companys-key.md) | Your subscription first, then the same model on the same company's key (amends 085 §6, §8)                             | Accepted |
| [255](ADR-255-step-down-instead-of-stopping.md)                      | Step down instead of stopping: lower effort, then a smaller model, then your key, then wait; on to start with          | Accepted |
| [256](ADR-256-check-answers-against-what-really-happened.md)         | Check answers against what really happened: Plenipo's record under every answer, four plain checks, sent back once     | Accepted |
| [257](ADR-257-catch-made-up-answers-step-2.md)                       | Catch made-up answers, step 2: links checked, leads send work back, a notice on repeat failures (amended by 259)       | Accepted |
| [258](ADR-258-spread-use-across-the-week-and-the-month.md)           | Spread use across the week and the month: a fair pace per window, ahead steps down early, Your plans                   | Accepted |
| [259](ADR-259-leads-stop-their-team.md)                              | Leads stop their team mid-task: check-ins while the team works, a lead stops only its own requests (amends 008, 257)   | Accepted |

[adr-211]: ADR-211-pro-ends-a-set-time-after-the-paid-period.md
