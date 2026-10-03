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

On a Mac and on Linux, rows that name Windows (where keys are kept, Start Plenipo with Windows, the
shells, this PC, the tray, notices, Windows closed Plenipo) use the words in
[Words that change with the system](#words-that-change-with-the-system) instead (ADR-155).

| Say                                                                            | Not                                                                                        |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| AI tool (Claude Code, Codex)                                                   | runtime, agent runtime, provider                                                           |
| full-time (one agent holds the position)                                       | persistent                                                                                 |
| on call (a new worker for each task)                                           | on-demand, ephemeral                                                                       |
| brought in (a worker)                                                          | spawned                                                                                    |
| rank                                                                           | class, kind                                                                                |
| supervisor, manager, VP                                                        | coordinator, superintendent, head                                                          |
| you                                                                            | the owner (in text written to the owner)                                                   |
| conversation                                                                   | session                                                                                    |
| task (one objective and its answer)                                            | turn                                                                                       |
| run (of a program)                                                             | execution                                                                                  |
| approved program                                                               | launch profile                                                                             |
| program                                                                        | process                                                                                    |
| time limit                                                                     | maximum runtime                                                                            |
| this version of Plenipo                                                        | this build                                                                                 |
| permissions                                                                    | capabilities                                                                               |
| permission set                                                                 | capability profile                                                                         |
| Allowed / Ask me / Blocked                                                     | allow / require approval / deny                                                            |
| permission limit (of a project, department)                                    | project policy, department policy                                                          |
| permissions in use (a worker's, now)                                           | runtime grant                                                                              |
| waiting for your approval                                                      | pending approval, awaiting approval                                                        |
| Approve / Deny                                                                 | accept / reject                                                                            |
| Not approved (a request)                                                       | rejected                                                                                   |
| Revoke (a worker's permissions)                                                | revoke grant                                                                               |
| approved (programs that run without asking)                                    | allowlisted commands                                                                       |
| Never run / never open                                                         | deny rules, denylist                                                                       |
| sensitive action                                                               | high-risk action, risk class                                                               |
| project folder                                                                 | workspace, working directory                                                               |
| Plenipo's tools                                                                | MCP server, tool server, broker                                                            |
| secret                                                                         | credential, secret reference                                                               |
| Windows Credential Manager (where it's kept)                                   | Vault, keyring, credential store                                                           |
| hidden by Plenipo                                                              | redacted                                                                                   |
| Blocked: … tried to …                                                          | denied, policy violation                                                                   |
| what it can do                                                                 | capabilities (of an AI tool)                                                               |
| AI model, model                                                                | model (fine as is)                                                                         |
| model choices (of a role)                                                      | model policy                                                                               |
| first choice                                                                   | preferred model                                                                            |
| backups (tried in order)                                                       | fallback models                                                                            |
| Automatic (follows the role's choices)                                         | policy-routed, routing: policy                                                             |
| fixed (an AI tool you set)                                                     | pinned                                                                                     |
| AI company (OpenAI, Anthropic)                                                 | provider (when the company is meant)                                                       |
| a different AI company                                                         | cross-provider                                                                             |
| usage limit                                                                    | usage cap, rate limit, capacity                                                            |
| pay-per-use API billing                                                        | API fallback, API billing                                                                  |
| sees images / makes images / uses a computer                                   | vision / image generation / computer use                                                   |
| context size (tokens are pieces of words)                                      | context window                                                                             |
| why this model                                                                 | routing explanation                                                                        |
| effort (how hard the model thinks)                                             | reasoning effort, thinking budget                                                          |
| the AI tool's models (e.g. Claude Code's)                                      | known models, model aliases, presets                                                       |
| Who made it (the AI company that made a model)                                 | maker, vendor, provider (for a model)                                                      |
| made by DeepSeek                                                               | vendor: deepseek, provider: deepseek                                                       |
| Group by: Who made it / AI tool                                                | group by maker, group by provider, group by runtime                                        |
| not known (who made a model)                                                   | unknown maker, null provider                                                               |
| now Opus 5.5 (what a name like "opus" points to now)                           | alias target, resolves to, snapshot                                                        |
| exact version (of a model, e.g. claude-opus-5-5)                               | pinned model, snapshot, model ID                                                           |
| Antigravity (Google's AI tool)                                                 | agy, Antigravity CLI, Jetski                                                               |
| paid AI credits                                                                | G1 credits, overage                                                                        |
| Antigravity's own settings folder                                              | isolated home, HOME override, sandbox profile                                              |
| GitHub Copilot (GitHub's AI tool)                                              | copilot, Copilot CLI, GH Copilot                                                           |
| Copilot sign-in, GitHub CLI sign-in                                            | OAuth token, gh token, authType, user auth                                                 |
| paid extra use (GitHub may charge once your allowance runs out)                | overage, premium request overage, additional usage, BYOK                                   |
| allowance (what your plan includes each month)                                 | quota, entitlement, premium interactions, quota snapshot                                   |
| the check before each task                                                     | preflight, headless probe, JSON-RPC handshake                                              |
| Copilot's own settings folder                                                  | COPILOT_HOME, config dir override                                                          |
| Auto (Copilot picks the model itself)                                          | auto mode, model router, auto routing                                                      |
| Cursor's agent (not an AI tool in Plenipo yet)                                 | cursor-agent, Cursor CLI                                                                   |
| on-demand use (Cursor's paid use past the plan)                                | usage-based pricing, hard limit, spend limit                                               |
| Ultra (effort)                                                                 | ultra                                                                                      |
| Extra high (effort)                                                            | xhigh                                                                                      |
| working copy (of the project folder)                                           | worktree, git worktree                                                                     |
| branch                                                                         | branch (fine as is)                                                                        |
| pull request                                                                   | PR                                                                                         |
| result (of an objective)                                                       | outcome report, objective report                                                           |
| open findings (from a review)                                                  | unresolved findings                                                                        |
| Changes requested / Approved (a review)                                        | request-changes / approve                                                                  |
| Committed / Not committed (a file)                                             | staged, dirty, working tree                                                                |
| Pushed (a branch, to the server)                                               | published, upstream                                                                        |
| Projects (the page)                                                            | project dashboard                                                                          |
| hands it to (a lead to its team)                                               | delegates, dispatches                                                                      |
| its job / what it hands back / what it must not do / when it asks for help     | duties / deliverables / constraints / escalation (role prompt)                             |
| Plenipo's browser (its own profile)                                            | managed browser, browser runtime                                                           |
| Visit websites / Use websites                                                  | browser navigation / browser automation                                                    |
| See the screen / Use the mouse and keyboard                                    | computer observe / computer control, computer use                                          |
| Screen, mouse, and keyboard (the permission set)                               | Computer use                                                                               |
| website lists: Allowed / Blocked / Other websites                              | domain policy, allowlist, denylist                                                         |
| the sign (a worker is using the browser)                                       | session indicator                                                                          |
| Take over (you take control; the worker stops)                                 | user takes control, handover, preempt                                                      |
| Stop all / Allow again                                                         | emergency stop, kill switch, re-enable                                                     |
| signing in                                                                     | authentication, login                                                                      |
| screenshot (of the page or the screen)                                         | screen capture, vision pipeline                                                            |
| terms of use (of a website)                                                    | terms of service, ToS                                                                      |
| Switches (Settings), on / off                                                  | feature flags, toggles, enabled / disabled                                                 |
| without asking you (on your allowed websites)                                  | auto-approve, bypass approval                                                              |
| a check that a person is using a website (CAPTCHA, said once)                  | CAPTCHA challenge, bot check, human verification                                           |
| Hand me checks (you solve it; the worker waits)                                | CAPTCHA hand-off, human-in-the-loop                                                        |
| lesson / what it has learned                                                   | agent memory (for lessons), learned knowledge                                              |
| Keep / Discard (a lesson)                                                      | accept / reject, persist                                                                   |
| Learn on its own (a role)                                                      | auto-accept lessons, autonomous learning                                                   |
| Worker learning                                                                | continual learning, agent memory                                                           |
| server, remote computer (in Settings → Servers)                                | host, remote host, host registry                                                           |
| Connect to servers (the permission)                                            | SSH capability, ssh.connect                                                                |
| server ID / pin it (Check the server ID)                                       | host key, host key fingerprint / host key pinning                                          |
| This server's ID changed                                                       | host key mismatch, REMOTE HOST IDENTIFICATION HAS CHANGED                                  |
| sign in as (a user on the server)                                              | SSH user, login                                                                            |
| How Plenipo signs in: a private key / a password / my SSH agent                | credential reference, auth method                                                          |
| Test / Staging / PRODUCTION (what a server is)                                 | environment classification, development environment                                        |
| Remote computers (SSH) (the switch in Settings → Switches)                     | SSH feature flag                                                                           |
| the kinds of commands (Look around, Start, stop, and restart services, …)      | command classes                                                                            |
| Run as administrator                                                           | sudo, privilege escalation                                                                 |
| When to ask you (every command / anything that changes / only as allowed)      | approval policy                                                                            |
| folders (on a server)                                                          | remote working directory policy                                                            |
| forwarded port                                                                 | SSH local port forward, direct-tcpip tunnel                                                |
| Disconnect (you stop a worker's server work)                                   | Take over (for server sessions), terminate session                                         |
| Showing (where you are: everything, a department, or a project)                | scope selector, context switcher                                                           |
| Notifications (the bell: what waits for you)                                   | notification badge, alert center                                                           |
| notices (above the page)                                                       | banners, banner slot, advisories                                                           |
| Switch to the light theme / dark theme                                         | theme toggle, color scheme                                                                 |
| Gallery (every building block of the screens)                                  | component gallery, storybook                                                               |
| activity (a strip of the last 24 hours)                                        | activity strip, sparkline, time series                                                     |
| Cards / List (how a list is shown)                                             | card/list view toggle                                                                      |
| Columns (choose which ones show)                                               | column customization                                                                       |
| Rows per page                                                                  | page size                                                                                  |
| Filters / Clear filters                                                        | facets, facet panel, reset                                                                 |
| Live / Back to live (a timeline)                                               | live mode, scrubber                                                                        |
| Home (the first page: how your company is doing, and what needs you)           | dashboard, overview, landing page                                                          |
| Waiting for you / What's stuck / Who's working / Just finished (Home)          | pending approvals / blocked tasks / active agents / recent completions                     |
| Current objectives                                                             | active objectives, open workflows                                                          |
| a department's health: Working / Quiet / Someone can't work / N stuck          | department health status, health score                                                     |
| page (of a department, project, worker, or task)                               | detail view, entity page                                                                   |
| Back (to the page before)                                                      | navigate back, history pop                                                                 |
| History (a department's, project's, or worker's events) / Show older           | event log, audit history / load more, pagination                                           |
| Task tree / Delegation tree (who handed what to whom)                          | delegation graph, DAG                                                                      |
| Done when (what counts as done)                                                | acceptance criteria                                                                        |
| Decisions (approvals answered, refusals, lessons kept, stops)                  | decision log                                                                               |
| Terminal (the panel at the bottom or on the right)                             | console, TTY, shell panel                                                                  |
| New terminal / This PC / a server's name                                       | spawn a shell, local host / remote host                                                    |
| shell (Windows PowerShell, PowerShell 7, Command Prompt) — fine as is          | command interpreter                                                                        |
| watch tab ("Operations Engineer · Shop": a worker's server work, read-only)    | session mirror, read-only PTY                                                              |
| Stop (the command running now) / Disconnect (the worker from the server)       | SIGTERM/SIGKILL / terminate session                                                        |
| notice (a pop-up from Windows)                                                 | toast, push notification                                                                   |
| Notifications (Settings: which notices you get)                                | notification preferences                                                                   |
| Send a test notice                                                             | test notification                                                                          |
| Only while Plenipo's window is not in front                                    | focus-aware notifications, suppress when focused                                           |
| Local paths (where Plenipo keeps its files)                                    | data directory, app data folder                                                            |
| sections (of Settings, the list on the left)                                   | settings tabs, navigation pane                                                             |
| About Plenipo                                                                  | about dialog                                                                               |
| Pip (Plenipo's helper, the character in the logo)                              | mascot                                                                                     |
| Keep Plenipo in the tray (closing hides the window; the work goes on)          | background daemon, service, minimize to tray                                               |
| Start Plenipo with Windows                                                     | autostart, launch at login, Run key                                                        |
| Start and close (Settings)                                                     | lifecycle settings, startup behavior                                                       |
| Plenipo closed unexpectedly / Windows closed Plenipo (restart, sign-out)       | crash, unclean shutdown, reboot detected                                                   |
| Plenipo was stopped while updating the Ledger                                  | interrupted migration                                                                      |
| the window stopped responding and was reloaded / opened again                  | WebView crash, renderer process failure                                                    |
| Run again / Leave stopped (a task Plenipo's closing stopped)                   | resume, retry, re-enqueue / discard                                                        |
| update / a new version / Install now                                           | OTA update, patch, updater                                                                 |
| Update ready (the top bar)                                                     | update available badge                                                                     |
| signed by 8 West (an update Plenipo trusts)                                    | minisign signature, updater public key                                                     |
| backup (of the Ledger): daily, before a new version, before an update          | snapshot, pre-upgrade/pre-migration dump                                                   |
| Restore (a backup) / Restore and restart                                       | rollback DB, recover snapshot                                                              |
| log files (what Plenipo did, kept to five)                                     | log rotation, trace output                                                                 |
| diagnostics file (one file to send when something went wrong)                  | support bundle, diagnostics bundle, crash dump                                             |
| Reset to starting settings                                                     | factory reset, reset config                                                                |
| rule (models and effort: the organization's, a department's, a role's)         | layer, policy layer, precedence                                                            |
| its own rule (one agent's); the closest rule wins                              | cascade, inheritance, override chain                                                       |
| effort for any other model                                                     | default effort, fallback effort                                                            |
| AI companies never to use (they add up across the rules)                       | provider denylist, blocked providers                                                       |
| specialty (a role's area of work: Senior Developer (Database))                 | sub-role, variant, role profile                                                            |
| Archived (the list) / Archive / Bring back                                     | restore (an agent), unarchive, soft delete                                                 |
| Delete for good                                                                | purge, hard delete, permanent delete                                                       |
| short record (what stays in the Ledger after Delete for good)                  | tombstone                                                                                  |
| Experience (a score: how much an agent has learned and done)                   | XP, rating                                                                                 |
| experienced (above your organization's average experience)                     | senior agent, high performer                                                               |
| Workforce (agents you saved to hire again) / Save to my Workforce              | talent pool, bench, agent library                                                          |
| full instructions / short reminder (what a worker is sent with a task)         | full brief, system prompt, context re-injection                                            |
| shortened its memory (an AI tool left out older parts of a conversation)       | compaction, context compaction                                                             |
| tabs (of the details: Overview, Job, AI model, Work, Team, Manage)             | panes, property sheet                                                                      |
| Plenipo's own text (its size, with a task)                                     | prompt overhead, token overhead                                                            |
| tile (one box on the canvas: you, the organization, an agent, a live worker)   | node, card, vertex                                                                         |
| Arrange (place tiles by hand) / Tidy up (back in neat rows)                    | layout, auto-layout, re-layout                                                             |
| Select / Move the view (drag to move around the canvas)                        | pan, hand tool, viewport                                                                   |
| line end (the round handle you drag to rewire a line)                          | edge endpoint, connector, anchor                                                           |
| Move here / Lend for one objective / Lend until I send it home / Send home     | reassign, loan, borrow, secondment                                                         |
| lent (helping another team for now)                                            | on loan, seconded, borrowed                                                                |
| trash can (drop an agent to archive it)                                        | recycle bin, delete zone                                                                   |
| Filters / Legend / Guide to the canvas                                         | facets, key, onboarding                                                                    |
| Where / thinks in Anthropic's cloud / runs on this PC / touching a folder      | compute location, data locality, host, workload placement                                  |
| hand-off (moving along a line)                                                 | message, event, RPC                                                                        |
| Watch (see a worker write code as it happens)                                  | live diff, code stream, file watcher                                                       |
| being written — not saved yet / saved / refused / not saved                    | streaming, partial tool input, pending write, committed                                    |
| new / changed lines, lines removed here                                        | diff, hunks, additions, deletions                                                          |
| Follow along / Pin this file                                                   | auto-scroll, lock, tail                                                                    |
| Your picture / status (Available, Busy, Away, Do not disturb) / mood / message | avatar, presence, profile                                                                  |
| Sign in / Reconnect / Sign out (an AI tool, from its card)                     | login, logout, re-auth, OAuth flow                                                         |
| sign-in tab ("Sign in · Codex": the AI tool's own sign-in program)             | login shell, auth terminal, device flow                                                    |
| Usage / tokens (pieces of words) read, reused, written                         | token usage, input / cached / output tokens                                                |
| today / this week (Monday to Sunday) / last week / the last 14 days            | rolling window, time bucket, period                                                        |
| left of your plan / reported by Codex at 1:05 PM                               | quota, rate-limit utilization, remaining quota                                             |
| resets at 3:10 PM / 5-hour limit / weekly limit                                | reset timestamp, rate-limit window                                                         |
| How it is paid for: Subscription / Paid per use with your key                  | billing mode, BYOK, metered API                                                            |
| installed / checked by Plenipo (an AI tool's version)                          | CLI version, tested version, compatibility                                                 |
| a new version / Update / Updating… / Updated to 1.0.43                         | upgrade, self-update, patch                                                                |
| The update didn't finish — your old version still works / put back             | update failed, rollback, downgrade                                                         |
| Update AI tools by themselves (the switch)                                     | auto-update, unattended upgrade                                                            |
| Plenipo is not giving Grok tasks for now                                       | disabled, quarantined, out of service                                                      |
| new — not checked yet / not offered by this version (a model)                  | discovered model, unverified model, deprecated                                             |
| Connections (Settings → Connections)                                           | plugins, integrations, connectors, MCP servers                                             |
| Connect / Reconnect / Disconnect                                               | authorize, link account, OAuth, revoke                                                     |
| Finish signing in in your browser                                              | OAuth redirect, consent screen, auth code flow                                             |
| needs you to sign in again                                                     | token expired, invalid grant, re-authenticate                                              |
| parts (Mail, Calendar, OneDrive, SharePoint, Teams) / what it can do           | scopes, features, APIs, resources                                                          |
| Off / Read only / Full access (a part)                                         | disabled / read scope / write scope                                                        |
| What Plenipo was allowed                                                       | granted scopes, consent, delegated permissions                                             |
| Your organization's admin needs to approve Plenipo first                       | admin consent required, AADSTS65001                                                        |
| Who may use it / Read only / Read and write                                    | ACL, grants, RBAC, access policy                                                           |
| Send without asking to                                                         | allowlist, trusted recipients, safe senders                                                |
| other people's words: information, never instructions                          | untrusted content, prompt injection                                                        |
| a work or school account / a personal account                                  | Entra ID / MSA, organizational / consumer account                                          |
| Microsoft app ID (in Advanced only)                                            | client ID, application ID, app registration                                                |
| Coming in a later update (a service)                                           | not implemented, coming soon, roadmap                                                      |
| Channels / Direct messages / Search (Slack's parts)                            | conversations, IMs, MPIMs, search API                                                      |
| a channel's ID (on Send without asking to, Slack only)                         | channel identifier, conversation ID                                                        |
| Add another Slack workspace / Remove this workspace                            | multi-tenant install, add team, uninstall                                                  |
| Use your workspace's own Slack app / Plenipo's app description / client ID     | custom app, app manifest (Slack's own button "From a manifest" is quoted as Slack's words) |
| Your Google app / Client ID / Client secret (the box hides what you type)      | OAuth client, credentials, client secret field                                             |
| Its secret is kept in Windows Credential Manager                               | stored in keyring, encrypted secret                                                        |
| cancelled at Slack / Google (on Disconnect)                                    | token revoked, grant revoked                                                               |
| Slack lets Plenipo read one channel or thread a minute                         | rate limit, non-Marketplace throttling, tier limit                                         |
| Its key / Save and check / Replace the key (the box hides what you type)       | API key field, credentials, validate token, rotate key                                     |
| Needs a new key                                                                | 401 Unauthorized, key revoked, invalid API key                                             |
| Contacts / Companies / Deals (HubSpot's parts)                                 | CRM objects, object types, scopes                                                          |
| Payments / Customers / Invoices (Stripe's parts)                               | Stripe resources, restricted-key permissions                                               |
| Test mode / Live mode: moves real money (Stripe)                               | sandbox, livemode, test environment                                                        |
| waiting for your approval in Stripe's Dashboard                                | approval_required, agent approval rule                                                     |
| Posts and pages / Store (the website's parts)                                  | wp/v2, wc/v3, REST namespaces                                                              |
| Your site's address / WordPress user name / Application Password               | site URL, base URL, REST credentials, basic auth                                           |
| WooCommerce key (optional) / Consumer key / Consumer secret                    | REST API key, ck/cs pair                                                                   |
| everyone who visits the site (who publishing reaches)                          | public audience, anonymous users                                                           |
| WooCommerce asks the payment company to send the money back                    | api_refund, gateway refund                                                                 |
| Add-on tools / Add a program / Look at its tools                               | MCP servers, custom MCP, tools/list                                                        |
| Switch on / Switch off (an add-on)                                             | enable / disable server                                                                    |
| Off / Reading / Changing (an add-on's tool)                                    | tool annotations, readOnlyHint, destructiveHint                                            |
| Changed — look again (an add-on's tool)                                        | tool drift, schema change, rug pull                                                        |
| the program's words (what an add-on says)                                      | tool description, untrusted tool output                                                    |
| Stored secrets to give it (an add-on)                                          | environment variables, secret injection                                                    |
| panel (Terminal, Files) / Move to the left, right, bottom                      | pane, dock zone, drawer, sidebar                                                           |
| Pop out / Put back (a panel in its own window)                                 | detach, undock, reattach, webview, window label                                            |
| Reset layout                                                                   | restore defaults, clear layout state                                                       |
| Files (the file view) / Project folder / working copy                          | file explorer, tree view, repository root, worktree                                        |
| Open in Plenipo / Open in another program / Show in folder                     | open with default handler, shell open, reveal in Explorer                                  |
| Save anyway (the file changed on disk)                                         | overwrite, force write, conflict, hash mismatch                                            |
| Senior Developer is writing in this working copy / Wait / Stop the worker      | file lock, write lease, locked by another process, cancel turn                             |
| organization / Your organizations                                              | tenant, workspace, instance, profile, database                                             |
| New organization / Use a template / Copy from one of your organizations        | provision, clone, fork, seed                                                               |
| Switch to / Open in a new window / Show its window                             | change context, rebind window, new webview                                                 |
| Archive organization / Bring back / Delete for good                            | deactivate, restore, purge, hard delete                                                    |
| Spending caps / a monthly cap / the business's cap                             | budget, spend limit, quota, billing threshold                                              |
| Let workers use paid AI keys (the switch)                                      | enable BYOK, metered API access, API billing                                               |
| set aside (the most a paid task could cost, before it starts)                  | reservation, hold, pre-authorization, escrow                                               |
| not priced yet (counted at the most it could have cost)                        | unpriced, cost unknown, null cost                                                          |
| 80% of a cap is used / Paid AI work stopped                                    | soft limit, budget alert, hard limit, quota exceeded                                       |
| the month starts over (the 1st, Pacific time)                                  | billing cycle reset, period rollover                                                       |
| Paid per use with your key, within your spending caps                          | BYOK, metered API, pay-as-you-go billing                                                   |
| Key check / Your key works / No key yet / Key not in use                       | auth status, credential validated, missing credential                                      |
| Save and check / Replace key / Remove key (a paid AI tool's key)               | validate API key, rotate key, revoke credential                                            |
| Comes with Plenipo (an AI tool that is Plenipo's own helper)                   | built-in runtime, bundled adapter, bridge                                                  |
| Anthropic (the card for an AI company's own service, with your key)            | Anthropic API, direct integration, API provider                                            |
| Plenipo has not checked … with a real key yet                                  | unverified integration, untested endpoint                                                  |
| $3.00 a million tokens read, $15.00 a million written                          | $/Mtok, input/output pricing, per-token rate                                               |
| also on Ollama, OpenRouter (the same model on other AI tools)                  | model alias, provider route, model mapping                                                 |
| It costs money (a paid route) / A worker on it answers in text only            | metered route, paid fallback, no tool use                                                  |
| Free / Plenipo Pro (Phase 11A)                                                 | edition, tier, entitlement, SKU                                                            |
| Plenipo Partner (for companies that run Plenipo for clients)                   | MSP tier, reseller SKU, multi-tenant plan                                                  |
| part of Pro / Part of Plenipo Pro                                              | blocked, gated, entitlement denied, upgrade required                                       |
| license key / the key's ID                                                     | token, license token, JWT                                                                  |
| the weekly check with 8 West                                                   | check-in, license ping, phone home, heartbeat, validation call                             |
| Pro keeps working for 30 days between checks                                   | grace period, offline grace, fail-open                                                     |
| Paid / Cancelled (Pro until the paid period ends) / Ended                      | subscription state, active, past_due, canceled, lapsed                                     |
| waits its turn (Free runs 3 workers at a time)                                 | concurrency limit, slot, queued for capacity                                               |
| paused (Connections, add-on tools, and lessons on Free)                        | disabled, deactivated, revoked                                                             |
| Use Plenipo from another device (the switch, Phase 14)                         | remote access, remote control, mobile client                                               |
| Settings → Devices / your phones                                               | paired devices, endpoints, clients                                                         |
| Add a phone / Pair this phone                                                  | enroll device, register client, provision                                                  |
| picture code (QR code) / typed code                                            | QR payload, pairing token, PSK, one-time secret                                            |
| Is this your phone? / Add / Cancel                                             | confirm device fingerprint, approve enrollment                                             |
| your face, fingerprint, or passcode / Check it's you                           | passkey, WebAuthn, biometric assertion, user verification                                  |
| Signed in / Sign out (a phone)                                                 | session, authenticated, token expiry                                                       |
| 8 West's relay / sealed end to end                                             | relay server, WebSocket tunnel, end-to-end encryption, Noise                               |
| Your PC can't be reached. Nothing was changed.                                 | host offline, 503, connection refused                                                      |
| This phone is no longer on your PC's list                                      | device revoked, unauthorized, 401                                                          |
| Approve on your PC (an approval kept on the PC)                                | remote approval disabled, policy-restricted action                                         |
| Paused after 3 failed checks / Un-pause                                        | locked out, rate limited, lockout                                                          |
| Coming soon (phone access before 8 West's relay is ready)                      | feature flag off, not provisioned                                                          |
| notice (on your phone) / Notices on this phone / Notices on my phones          | push notification, web push, subscription                                                  |
| On the lock screen: Show what it is / Show only “Something needs you”          | notification privacy, redacted payload                                                     |
| Already answered                                                               | stale notification, conflict, 409                                                          |
| Add Plenipo to your Home Screen (iPhone)                                       | install the PWA, A2HS                                                                      |

## Words that change with the system

Plenipo runs on Windows, on a Mac, and on Linux (Phase 23). Where the words differ, each system
uses its own (ADR-155). The rows above that name Windows are the Windows column of this table.
The Rust side picks the words for the system it runs on (`crates/core/src/words.rs`); the
screens get them with the app's information before the first paint (`apps/desktop/src/system/words.ts`)
and never guess. Keyboard labels come from one helper there, `shortcut`. Names in the
code and in the Ledger (such as \`windowsRestart\`) stay as they are.

| What it is                                        | Windows                                                               | Mac                                                                                         | Linux                                                                 |
| ------------------------------------------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| This computer                                     | this PC                                                               | this Mac                                                                                    | this computer                                                         |
| Where keys are kept                               | Windows Credential Manager                                            | your Mac's Keychain                                                                         | your computer's password store (GNOME Keyring or KWallet)             |
| No place to keep keys (Linux only)                | —                                                                     | —                                                                                           | This computer has no password store, so Plenipo can't save keys here. |
| Start at sign-in (the switch)                     | Start Plenipo with Windows                                            | Open Plenipo when you log in                                                                | Start Plenipo when you sign in                                        |
| Where Plenipo waits with its window closed        | the tray                                                              | the menu bar                                                                                | the tray                                                              |
| A notice                                          | a pop-up from Windows                                                 | a notification from your Mac                                                                | a notification from your desktop                                      |
| Where to change notices                           | Windows Settings → System → Notifications                             | System Settings → Notifications                                                             | your desktop's notification settings                                  |
| The system closed Plenipo                         | Windows closed Plenipo (restart, sign-out)                            | Your Mac closed Plenipo (restart, log out)                                                  | Your computer closed Plenipo (restart, sign-out)                      |
| Show a file                                       | Show in folder                                                        | Show in Finder                                                                              | Show in folder                                                        |
| The program that shows files                      | File Explorer                                                         | Finder                                                                                      | your file manager                                                     |
| The terminal's shells (fine as is)                | Windows PowerShell, PowerShell 7, Command Prompt                      | Your shell (zsh), zsh, bash, fish                                                           | Your shell (bash), zsh, bash, fish                                    |
| Keys in labels                                    | Ctrl, Alt, Shift                                                      | Cmd (⌘), Option (⌥), Control (⌃), Shift (⇧)                                                 | Ctrl, Alt, Shift                                                      |
| Removing Plenipo                                  | Settings → Apps → Plenipo → Uninstall ("Also delete my Plenipo data") | Delete my Plenipo data, then drag Plenipo to the Trash                                      | Delete my Plenipo data, then remove it with your software manager     |
| Letting a worker see the screen and use the mouse | —                                                                     | Allow Plenipo in System Settings → Privacy & Security → Accessibility, and Screen Recording | Your desktop asks to share the screen and allow control (Wayland)     |
| The SSH agent                                     | Windows' OpenSSH Authentication Agent, or Pageant                     | ssh-agent                                                                                   | ssh-agent                                                             |

## Where technical words may stay

- **Diagnostics** and raw output: PIDs, exit codes, stdout and stderr stay available for
  troubleshooting (rollout plan, Phase 12: "raw diagnostics remain available").
- **Code and developer documents:** identifiers, Ledger event types (`org.worker_spawned`),
  ADRs, and architecture notes keep the rollout plan's terms. The mapping above is the bridge.
- **A service's own words, quoted so you can find them:** its menus, buttons, and permission
  names (Stripe's **Developers → API keys**, WooCommerce's **Advanced → REST API**, HubSpot's
  `crm.objects.contacts.read`), as the key steps and cards give them.
- **Product names:** Plenipo Ledger, Liaison, and Guard. "Guard" names the part of Plenipo
  that decides; the owner sees "permissions" and "approvals".
- **The owner's own words:** objective, handoff, QA evaluator, security auditor, oversight.
- **Agent-facing protocol:** handoff addresses such as `codex` and `role:Senior Developer`.

## Known leftovers

None known on the main screens (Organization, Projects, Workers, AI tools, Activity, Approvals,
Settings, including AI models, Permissions, and Connections); the
Diagnostics page and raw output keep technical details on purpose. Some refusal messages from
earlier phases can still use an engineering word in rare error cases. Report any technical word
you find on a screen, and fix it with this list.
