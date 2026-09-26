# Plain words — the words Plenipo uses on screen

**Rule (owner direction, 2026-09-26):** everything a person sees in Plenipo uses simple,
everyday words. If a word needs a computer background to understand, use the everyday word
instead. Read new screen text out loud: a busy business owner should understand it without
asking what it means.

This list is the reference for every screen, message, and error the owner can see, including
messages that come from the Rust side (the server does not know which title set was chosen, so
it always uses the Business words). Add a pair here whenever you replace a word.

## The chain of command

Top to bottom, with the Business titles (the default):

| Rank           | Who it is                         | In the code (unchanged) |
| -------------- | --------------------------------- | ----------------------- |
| **President**  | You, the owner                    | the owner               |
| **VP**         | Runs the organization for you     | `superintendent`        |
| **Manager**    | Runs a department                 | `departmentManager`     |
| **Supervisor** | Leads a project and its team      | `projectCoordinator`    |
| **Worker**     | Does the tasks the team is handed | `worker`                |

**Settings → Personalization → Titles** swaps the rank names shown in the app
(`apps/desktop/src/org/titles.ts`):

| Titles            | You       | VP        | Manager    | Supervisor          | Worker         |
| ----------------- | --------- | --------- | ---------- | ------------------- | -------------- |
| Business          | President | VP        | Manager    | Supervisor          | Worker         |
| U.S. Army         | General   | Colonel   | Captain    | Sergeant            | Private        |
| U.S. Navy         | Admiral   | Captain   | Lieutenant | Chief Petty Officer | Seaman         |
| U.S. Air Force    | General   | Colonel   | Captain    | Master Sergeant     | Airman         |
| U.S. Marine Corps | General   | Colonel   | Captain    | Gunnery Sergeant    | Lance Corporal |
| U.S. Coast Guard  | Admiral   | Captain   | Lieutenant | Chief Petty Officer | Seaman         |
| U.S. Space Force  | General   | Colonel   | Captain    | Master Sergeant     | Specialist     |
| Mafia             | Don       | Underboss | Capo       | Soldier             | Associate      |

Only the rank names change. Job titles ("Website Supervisor", "Senior Developer") stay as the
owner wrote them, and agents are always told the Business titles (ADR-010).

## Say this, not that

| Say                                          | Not                                      |
| -------------------------------------------- | ---------------------------------------- |
| AI tool (Claude Code, Codex)                 | runtime, agent runtime, provider         |
| full-time (one agent holds the position)     | persistent                               |
| on call (a new worker for each task)         | on-demand, ephemeral                     |
| brought in (a worker)                        | spawned                                  |
| rank                                         | class, kind                              |
| supervisor, manager, VP                      | coordinator, superintendent, head        |
| you                                          | the owner (in text written to the owner) |
| conversation                                 | session                                  |
| task (one objective and its answer)          | turn                                     |
| run (of a program)                           | execution                                |
| approved program                             | launch profile                           |
| program                                      | process                                  |
| time limit                                   | maximum runtime                          |
| this version of Plenipo                      | this build                               |
| permissions                                  | capabilities                             |
| what it can do                               | capabilities (of an AI tool)             |
| AI model, model                              | model (fine as is)                       |
| model choices (of a role)                    | model policy                             |
| first choice                                 | preferred model                          |
| backups (tried in order)                     | fallback models                          |
| Automatic (follows the role's choices)       | policy-routed, routing: policy           |
| fixed (an AI tool you set)                   | pinned                                   |
| AI company (OpenAI, Anthropic)               | provider (when the company is meant)     |
| a different AI company                       | cross-provider                           |
| usage limit                                  | usage cap, rate limit, capacity          |
| pay-per-use API billing                      | API fallback, API billing                |
| sees images / makes images / uses a computer | vision / image generation / computer use |
| context size (tokens are pieces of words)    | context window                           |
| why this model                               | routing explanation                      |
| effort (how hard the model thinks)           | reasoning effort, thinking budget        |
| short names (of an AI tool's models)         | model aliases                            |
| Extra high (effort)                          | xhigh                                    |

## Where technical words may stay

- **Diagnostics** and raw output: PIDs, exit codes, stdout and stderr stay available for
  troubleshooting (rollout plan, Phase 12: "raw diagnostics remain available").
- **Code and developer documents:** identifiers, Ledger event types (`org.worker_spawned`),
  ADRs, and architecture notes keep the rollout plan's terms. The mapping above is the bridge.
- **Product names:** Plenipo Ledger, Liaison, and Guard.
- **The owner's own words:** objective, handoff, QA evaluator, security auditor, oversight.
- **Agent-facing protocol:** handoff addresses such as `codex` and `role:Senior Developer`.

## Known leftovers

None known on the main screens (Organization, Workers, AI tools, Activity, Settings, including
AI models); the
Diagnostics page and raw output keep technical details on purpose. Some refusal messages from
earlier phases can still use an engineering word in rare error cases. Report any technical word
you find on a screen, and fix it with this list.
