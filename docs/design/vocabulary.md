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

| Say                                                                        | Not                                                            |
| -------------------------------------------------------------------------- | -------------------------------------------------------------- |
| AI tool (Claude Code, Codex)                                               | runtime, agent runtime, provider                               |
| full-time (one agent holds the position)                                   | persistent                                                     |
| on call (a new worker for each task)                                       | on-demand, ephemeral                                           |
| brought in (a worker)                                                      | spawned                                                        |
| rank                                                                       | class, kind                                                    |
| supervisor, manager, VP                                                    | coordinator, superintendent, head                              |
| you                                                                        | the owner (in text written to the owner)                       |
| conversation                                                               | session                                                        |
| task (one objective and its answer)                                        | turn                                                           |
| run (of a program)                                                         | execution                                                      |
| approved program                                                           | launch profile                                                 |
| program                                                                    | process                                                        |
| time limit                                                                 | maximum runtime                                                |
| this version of Plenipo                                                    | this build                                                     |
| permissions                                                                | capabilities                                                   |
| permission set                                                             | capability profile                                             |
| Allowed / Ask me / Blocked                                                 | allow / require approval / deny                                |
| permission limit (of a project, department)                                | project policy, department policy                              |
| permissions in use (a worker's, now)                                       | runtime grant                                                  |
| waiting for your approval                                                  | pending approval, awaiting approval                            |
| Approve / Deny                                                             | accept / reject                                                |
| Not approved (a request)                                                   | rejected                                                       |
| Revoke (a worker's permissions)                                            | revoke grant                                                   |
| approved (programs that run without asking)                                | allowlisted commands                                           |
| Never run / never open                                                     | deny rules, denylist                                           |
| sensitive action                                                           | high-risk action, risk class                                   |
| project folder                                                             | workspace, working directory                                   |
| Plenipo's tools                                                            | MCP server, tool server, broker                                |
| secret                                                                     | credential, secret reference                                   |
| Windows Credential Manager (where it's kept)                               | Vault, keyring, credential store                               |
| hidden by Plenipo                                                          | redacted                                                       |
| Blocked: … tried to …                                                      | denied, policy violation                                       |
| what it can do                                                             | capabilities (of an AI tool)                                   |
| AI model, model                                                            | model (fine as is)                                             |
| model choices (of a role)                                                  | model policy                                                   |
| first choice                                                               | preferred model                                                |
| backups (tried in order)                                                   | fallback models                                                |
| Automatic (follows the role's choices)                                     | policy-routed, routing: policy                                 |
| fixed (an AI tool you set)                                                 | pinned                                                         |
| AI company (OpenAI, Anthropic)                                             | provider (when the company is meant)                           |
| a different AI company                                                     | cross-provider                                                 |
| usage limit                                                                | usage cap, rate limit, capacity                                |
| pay-per-use API billing                                                    | API fallback, API billing                                      |
| sees images / makes images / uses a computer                               | vision / image generation / computer use                       |
| context size (tokens are pieces of words)                                  | context window                                                 |
| why this model                                                             | routing explanation                                            |
| effort (how hard the model thinks)                                         | reasoning effort, thinking budget                              |
| the AI tool's models (e.g. Claude Code's)                                  | known models, model aliases, presets                           |
| Ultra (effort)                                                             | ultra                                                          |
| Extra high (effort)                                                        | xhigh                                                          |
| working copy (of the project folder)                                       | worktree, git worktree                                         |
| branch                                                                     | branch (fine as is)                                            |
| pull request                                                               | PR                                                             |
| result (of an objective)                                                   | outcome report, objective report                               |
| open findings (from a review)                                              | unresolved findings                                            |
| Changes requested / Approved (a review)                                    | request-changes / approve                                      |
| Committed / Not committed (a file)                                         | staged, dirty, working tree                                    |
| Pushed (a branch, to the server)                                           | published, upstream                                            |
| Projects (the page)                                                        | project dashboard                                              |
| hands it to (a lead to its team)                                           | delegates, dispatches                                          |
| its job / what it hands back / what it must not do / when it asks for help | duties / deliverables / constraints / escalation (role prompt) |
| Plenipo's browser (its own profile)                                        | managed browser, browser runtime                               |
| Visit websites / Use websites                                              | browser navigation / browser automation                        |
| See the screen / Use the mouse and keyboard                                | computer observe / computer control, computer use              |
| Screen, mouse, and keyboard (the permission set)                           | Computer use                                                   |
| website lists: Allowed / Blocked / Other websites                          | domain policy, allowlist, denylist                             |
| the sign (a worker is using the browser)                                   | session indicator                                              |
| Take over (you take control; the worker stops)                             | user takes control, handover, preempt                          |
| Stop all / Allow again                                                     | emergency stop, kill switch, re-enable                         |
| signing in                                                                 | authentication, login                                          |
| screenshot (of the page or the screen)                                     | screen capture, vision pipeline                                |
| terms of use (of a website)                                                | terms of service, ToS                                          |
| server, remote computer (in Settings → Servers)                            | host, remote host, host registry                               |
| Connect to servers (the permission)                                        | SSH capability, ssh.connect                                    |
| server ID / pin it (Check the server ID)                                   | host key, host key fingerprint / host key pinning              |
| This server's ID changed                                                   | host key mismatch, REMOTE HOST IDENTIFICATION HAS CHANGED      |
| sign in as (a user on the server)                                          | SSH user, login                                                |
| How Plenipo signs in: a private key / a password / my SSH agent            | credential reference, auth method                              |
| Test / Staging / PRODUCTION (what a server is)                             | environment classification, development environment            |
| Remote computers (SSH) (the switch in Settings → Switches)                 | SSH feature flag                                               |
| the kinds of commands (Look around, Start, stop, and restart services, …)  | command classes                                                |
| Run as administrator                                                       | sudo, privilege escalation                                     |
| When to ask you (every command / anything that changes / only as allowed)  | approval policy                                                |
| folders (on a server)                                                      | remote working directory policy                                |
| forwarded port                                                             | SSH local port forward, direct-tcpip tunnel                    |
| Disconnect (you stop a worker's server work)                               | Take over (for server sessions), terminate session             |

## Where technical words may stay

- **Diagnostics** and raw output: PIDs, exit codes, stdout and stderr stay available for
  troubleshooting (rollout plan, Phase 12: "raw diagnostics remain available").
- **Code and developer documents:** identifiers, Ledger event types (`org.worker_spawned`),
  ADRs, and architecture notes keep the rollout plan's terms. The mapping above is the bridge.
- **Product names:** Plenipo Ledger, Liaison, and Guard. "Guard" names the part of Plenipo
  that decides; the owner sees "permissions" and "approvals".
- **The owner's own words:** objective, handoff, QA evaluator, security auditor, oversight.
- **Agent-facing protocol:** handoff addresses such as `codex` and `role:Senior Developer`.

## Known leftovers

None known on the main screens (Organization, Projects, Workers, AI tools, Activity, Approvals,
Settings, including AI models and Permissions); the
Diagnostics page and raw output keep technical details on purpose. Some refusal messages from
earlier phases can still use an engineering word in rare error cases. Report any technical word
you find on a screen, and fix it with this list.
