# Phase 16 — Implementation Checklist (Wave 3)

**Status:** in progress (2026-09-30). Pull request 0 (the Windows-only failures) and part 1
(spending caps, pricing, records, and the switch) are built; parts 2 and 3 are next.

Source: `ROLLOUT_PLAN.md`, Phase 16 — Every AI Model Worth Having, **Wave 3 only** ("spending caps
first, then paid routes"):

- spending caps: for the business, a department, and one position; monthly amount, warning at 80%,
  hard stop;
- pricing and recording of every paid task in the Ledger;
- "Let workers use paid AI keys" switch in Settings, off by default;
- paid keys in the Vault, reaching only the AI tool they were saved for;
- more than one route to a model, in the owner's order, with fallback when a route is
  usage-limited, signed out, or over its cap;
- OpenRouter through a Plenipo helper, built like the Ollama helper (ADR-017);
- direct keys for Anthropic, OpenAI, xAI, and Google.

Records it follows:

- [ADR-036 (every AI model worth having)](../adr/ADR-036-every-ai-model.md): §2 keys and caps,
  §4 routes, §5 Wave 3;
- [ADR-017 (Ollama's cloud models through the Ollama service on this PC)](../adr/ADR-017-ollama-cloud-models.md):
  the shape of a helper run per task;
- [ADR-081 (who made each model)](../adr/ADR-081-who-made-each-model.md): every model says who
  made it;
- [ADR-023 (Settings → Switches)](../adr/ADR-023-settings-switches.md): where the new switch
  lives;
- [ADR-014 (adding AI tools)](../adr/ADR-014-adding-ai-tools.md) and
  [ADR-007 (how Plenipo runs AI tools)](../adr/ADR-007-runtime-adapters.md): the rules Wave 3
  changes, in its own record.

New records (numbers from Phase 16's range, ADR-085 to ADR-089, kept by ADR-090):

- **ADR-085 (paid AI keys with spending caps).** Amends ADR-003 (roles never tied to one AI
  company: no silent paid fallback), ADR-007 §4 (how Plenipo runs AI tools: sign-ins and
  billing), ADR-011 (how Plenipo picks each worker's model: paid use was fixed off), and ADR-014
  (adding AI tools: subscription sign-in only). It amends them; it does not rewrite them.
- **ADR-086 (OpenRouter through a Plenipo helper).** A helper run per task, like ADR-017's.
- **ADR-087 (direct keys for every AI company whose models take one).** The plan named four
  companies; the owner asked for every one (choice 14).

## Step 0 — checks on the owner's PC (2026-09-30)

- [x] The PC builds v1.15.0 (`3535d03`): Rust 1.98.1 (the repository's pin), Node.js 26, pnpm
      10.33.0, Visual Studio 2026 Build Tools, WebView2. `pnpm build` makes the app and the
      installer, and the launch smoke test exits 0.
- [x] Three checks fail on Windows only, and GitHub's checks miss them: two tests in
      `scripts/check-doc-links.test.mjs` (paths with `\`), clippy's `unused_mut` at
      `crates/capabilities/src/broker/add_on_calls.rs:161` (the `mut` is needed only on Unix),
      and `a_busy_supervisor_takes_a_handed_over_objective_when_it_is_free` (passes alone,
      fails every time with the rest of `crates/workforce/tests/workforce.rs`; GitHub's Windows
      job passes it).
- [x] No AI company key is set on the PC (names checked, never values): no `ANTHROPIC_*`,
      `OPENAI_*`, `XAI_*`, `GEMINI_*`, `GOOGLE_API_KEY`, `OPENROUTER_*`, or `MOONSHOT_*` for the
      user or the machine.
- [x] OpenRouter's public model list answers without a key: 464 models from 64 makers, each with
      a price (`prompt`, `completion`, `input_cache_read`, `input_cache_write`, `web_search`, per
      token). Kimi K3 (`moonshotai/kimi-k3`): $3 in and $15 out per million tokens. Asking about
      a key without one answers `401`.
- [x] Ollama 0.34.4 is signed in, but still on the **free plan** (`/api/me`: `plan: free`); Kimi
      K3 on Ollama answers `402 Payment Required`. The owner's example ("Kimi runs out, the paid
      Ollama plan carries the work") is built and tested with stand-ins, and checked for real
      when the paid plan shows.
- [x] Linux test machine (Coastline) reachable: Ubuntu 24.04, 4 cores, 7 GB, Docker only. GitHub's
      Linux end-to-end job is the Linux check; Coastline is used, in Docker, only to debug it.
- [x] Keys the owner has for the final check: Anthropic, OpenAI, xAI (Grok), and Kimi. OpenRouter
      and Google keys come later; those routes are built and tested with stand-ins and marked "not
      checked with a real key yet" until then.

## The owner's choices (2026-09-30)

1. **The business cap first:** no key can be saved until the whole-business cap exists. Department
   and position caps are extras. Every paid task counts against its position, its department,
   and the business; the smallest amount left decides.
2. **The month:** starts on the 1st at midnight, Pacific time.
3. **The hard stop never goes over:** before a paid request, Plenipo sets aside the most it could
   cost (the answer's length is capped) and does not start a request that could pass a cap. Work
   can stop a little before 100%. If a real bill still passes a cap, the task stops at once and
   the owner is told.
4. **No known price, no paid key:** the Router skips such a model on a paid route and says "not
   priced yet". A finished task whose cost could not be read is recorded as "not priced yet",
   never as zero.
5. **Direct keys run through Plenipo's own helper**, like OpenRouter, not through the companies'
   own programs. The "no keys" tests for every subscription AI tool stay as they are.
6. **Paid workers answer in text only** in this wave. Plenipo's tools (files, programs, git,
   through Guard) come later, for Ollama and paid keys together. The Router's reason says "text
   only" when it picks such a route.
7. **OpenRouter's list:** a short checked list (the newest general and coding models from Qwen,
   Mistral, and Meta's Llama, about six), plus any other OpenRouter model the owner names exactly.
   Prices come from OpenRouter's public list before each task.
8. **Privacy on OpenRouter: no limits.** Plenipo does not narrow which hosting companies OpenRouter
   may use. (ADR-036's "to watch" about client data stands: a paid route is used only by a
   position the owner put it on.)
9. **Wave 1's leftover joins this wave:** more Ollama cloud models once the paid plan shows,
   checked against what Ollama really lists.
10. **The three Windows-only failures are fixed first**, in a small pull request, and GitHub's
    Windows job gains clippy so they cannot come back unseen.
11. **Three pull requests, in order:** (1) caps, pricing, records, and the switch; (2) keys in the
    Vault, routes, and OpenRouter; (3) direct keys and v1.16.0. Each merges when every check
    passes, Windows included.
12. **v1.16.0:** the last pull request sets the version.
13. **The owner's go-ahead:** create and merge this phase's pull requests, and run any GitHub
    Actions needed, the release included.
14. **Direct keys for every AI company whose models take one**, not only the plan's four: "We
    should be able to add api keys to any of the models that support api keys." Recorded in
    ADR-087 as a change to the plan.

## Owner's rules for every pull request

- [ ] No key, password, or token asked for in chat or committed. Keys are typed only into
      Plenipo's own screen and kept only in the Vault.
- [ ] Everything that touches files, programs, the network, the browser, or the screen goes through
      Guard and the capability broker.
- [ ] Nothing loads code into Plenipo while it runs (ADR-014). A price list read from OpenRouter is
      data, checked and capped in size, never code.
- [ ] New desktop commands are for the main window only; the sign window and web pages are refused,
      with IPC tests.
- [ ] Logs and the diagnostics file never hold a key (or any part of one) or what the owner types in
      the terminal.
- [ ] No model names in commits, branch names, or pull requests.
- [ ] Plain words on screen ([the word list](../design/vocabulary.md)); decision records named, not
      only numbered; 8 West Ventures, LLC credited.
- [ ] Connections (`crates/capabilities/src/connections/`, `crates/guard/src/connections.rs`,
      `apps/desktop/src/settings/connections/`) are not changed. Paid keys get their own place in
      the Vault.
- [ ] `main` merged in whenever it moves; in shared files, both sides kept. Pacific-time dates.
- [ ] Before pushing: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `CARGO_INCREMENTAL=0 cargo test --workspace --locked` (rerun until the whole suite finishes),
      `pnpm bindings` with no diff in `packages/types/src/generated`, and `pnpm docs:check` for
      docs. Never push while GitHub's checks are still running.

## Pull request 0 — the Windows-only failures (choice 10)

- [ ] `scripts/check-doc-links.mjs`: `linkedPaths` builds repository paths with `/` on every
      system (`node:path`'s `posix`), so its tests pass on Windows.
- [ ] `crates/capabilities/src/broker/add_on_calls.rs`: the folder builder is mutable only where it
      is changed (Unix), so clippy passes on Windows.
- [ ] `a_busy_supervisor_takes_a_handed_over_objective_when_it_is_free`: find why it fails only when
      run with the rest of its file on this PC, and make it wait for what it really needs.
- [ ] `.github/workflows/ci.yml`: the Windows job runs clippy as well.

## Pull request 1 — spending caps, pricing, records, and the switch

### Money and months

- [x] Money is kept as whole millionths of a dollar (`u64`), never as a floating-point number.
- [x] A month is the calendar month in Pacific time (choice 2), daylight saving included.
- [x] Prices: per million tokens for input, cached input, and output (`pricing::Price`, rounded
      up, exact decimal dollars). The dated built-in price list for direct-key models comes with
      PR 3, and OpenRouter's prices from its public list with PR 2.

### Caps (a new setting, recorded in the Ledger)

- [x] A cap: who it covers (the business, one department, or one position), a monthly amount, when
      it was set, and by whom. The 80% warning and the hard stop are fixed, not settings.
- [x] Setting, raising, lowering, and removing a cap is recorded as an event. The business cap
      cannot be removed while a paid key exists.
- [x] **The gate** (one place, used by every paid route): before a paid task, add up this month's
      spending plus what is set aside for running tasks, for the position, its department (worked
      out from who it reports to, as today), and the business. Refuse if the most the task could
      cost does not fit under every cap; otherwise set that amount aside. After the task, record
      the real amount and free the rest.
- [x] The position and department are stored with each spending record, so a later
      reorganisation or a loan does not move past spending.
- [x] Crossing 80% of a cap: an event, a banner on every page, and one Windows notification per cap
      per month.
- [x] Reaching the hard stop: new paid tasks under that cap are refused with the reason; a bill
      that passes a cap names the caps it passed, for the caller to stop that task at once (the
      caller is PR 2's paid helper); the banner stays until the cap is raised or the month turns
      over.
- [x] After a restart, money still set aside counts at the most its task could have cost ("not
      priced yet"), never freed, so a cap is never passed unseen (the service may have billed it).

### Recording every paid task (Ledger migration 0012)

- [x] A spending record per paid task: task and execution, position and department, AI tool and
      model, which key by its name (never the key), the amount or "not priced yet", how it was
      priced (the service's own bill, or the price list), the Pacific month, and the time.
- [x] Nothing about subscription tasks changes.

### The switch

- [x] `Switches.paidAiKeys` ("Let workers use paid AI keys"), off by default; older settings read
      it as off.
- [x] Off: no key can be saved, no paid route is offered or run, and every test that forbids keys
      passes unchanged. (In part 1 there are no keys or paid routes at all; PR 2 tests the switch
      with them.)
- [x] The AI tools page's "How it is paid for" row: the paid choice stays locked, with the reason,
      until the switch is on and the business cap exists (PR 2 unlocks it for tools that take a
      key).

### Screens (Settings)

- [x] **Spending caps:** the business, each department, and each position with a cap; this
      month's spending against each, what is set aside, and when the month turns over.
- [x] The switch in Settings → Switches, with a plain sentence about what it allows.
- [x] The banner for a warning or a hard stop, like the approvals banner, linking to Spending caps.

### Desktop commands (main window only)

- [x] Read spending and caps; set a cap; remove a cap. Each is listed in `build.rs`, granted only in
      `capabilities/default.json` (the main window), and has IPC tests that refuse the sign window,
      another window, and a web page.

### Tests

- [x] Money arithmetic and Pacific months (turning over, daylight saving).
- [x] Warning at 80% of a cap; hard stop at 100%, with the work stopped and the owner told (the
      gate and the notices; a paid task stopped end to end comes with PR 2).
- [x] A cap enforced for the business, a department, and one position; the smallest amount left
      decides.
- [x] A task that could pass a cap is not started; after a restart, what was set aside counts
      once, at the most it could have cost.
- [x] "Not priced yet" is never counted as zero.
- [x] With the switch off: no key can be saved and no paid route is offered.
- [x] Vitest for the Spending caps screen, the switch, and the banner; IPC tests for every new
      command.

## Review of part 1

Four reviewers each read part 1 for one area: money and caps, security and desktop commands, the
Ledger's data, and the screens. A second reviewer then checked every finding, reproducing most of
them. Confirmed findings were fixed with a test; one was refuted.

| Area     | Finding                                                                                   | Second reviewer        | What was done                                                                            |
| -------- | ----------------------------------------------------------------------------------------- | ---------------------- | ---------------------------------------------------------------------------------------- |
| Money    | A late bill from last month reset this month's warning and stop, and told them again      | Confirmed (medium)     | Warnings and stops are marked for the current month only                                 |
| Money    | "Passed the cap" named every cap already over, not only a bill over its set-aside         | Confirmed (low)        | Named only for a bill larger than what was set aside for it                              |
| Money    | More cached tokens than input could be priced low; cached input dearer than input allowed | Confirmed (low)        | Priced apart, never less; such a price list is refused                                   |
| Money    | Cache writes, priced above input by some services                                         | Plausible (part 2)     | The helpers never ask for them (ADR-085 §3.5)                                            |
| Money    | "12,50" typed as a cap read as $1,250                                                     | Confirmed (low-medium) | A comma only between groups of three digits                                              |
| Money    | A refusal could read "could cost $4.00, and $4.00 is left"                                | Confirmed (low)        | The cost rounds up, what is left rounds down                                             |
| Money    | A bill beyond any cap was refused, so it later counted as less                            | Confirmed (low)        | Recorded at the most any cap could be, with a note                                       |
| Data     | A bill was lost when the caps setting could not be read                                   | Confirmed (low-medium) | The bill is always recorded; only the warnings wait                                      |
| Data     | Restoring an older backup forgot the month's spending                                     | Confirmed (medium)     | The kept "before restore" backup's spending records are carried into the restored Ledger |
| Data     | No way in the app to reset unreadable caps                                                | Confirmed (low)        | Settings problem **Spending caps**, with reset to none after a backup                    |
| Data     | The export left out spending                                                              | Confirmed (low)        | Added (with workspaces, missing before), and a test that every table is exported         |
| Security | A warning found while counting last run's money reached no notice                         | Confirmed (low)        | Counted after the notices start, leaving this run's own tasks alone                      |
| Security | Removing the business cap does not yet check for saved keys                               | Note (part 2)          | Part 2 passes the real check, with an IPC test                                           |
| Screens  | A cap on an inactive department was hidden                                                | Confirmed (low-medium) | Shown as "(inactive)", so it can be changed or removed                                   |
| Screens  | Positions with the same title looked the same                                             | Confirmed (low)        | Named with their department ("Senior Developer · Engineering")                           |
| Screens  | The banner stayed after the month started over                                            | Confirmed (low-medium) | The page and the banner look again when the month starts over                            |
| Screens  | "80% of" at 97%; "Operations's cap"; a status styled as an error                          | Confirmed (nits)       | "80% or more of", "the cap for Operations", and a note instead                           |
| Screens  | A cap's amount field could keep old text                                                  | Refuted                | Only its own editor changes a cap                                                        |

## Pull request 2 — keys in the Vault, routes, and OpenRouter

### Keys (typed only into Plenipo's own screen)

- [x] A paid key is saved from the AI tool's card, only while the switch is on and the business
      cap exists. It is checked with one cheap read call, then kept only in the Vault (its own
      names, `paid-key-…`, beside servers and connections). Settings keep a name, the AI tool it is
      for, and when it was saved; never the key.
- [x] The key is never shown again. Replace it or remove it. Uninstall's "delete my data" removes
      it (`vault::stored_ids`).
- [x] The key is added to the secret filter, so it cannot appear in a log, the diagnostics file,
      the Ledger, or a task's activity, even if a service echoes it back.
- [x] The key reaches only its own helper, on the helper's standard input, only for that AI tool,
      only while the switch is on. It is never put in an environment variable or on a command line,
      so the contract suite's "no key variables" check stays unchanged for every AI tool.

### Routes (ADR-036 §4)

- [x] A route is one model, the AI tool that runs it, and how it is paid for (subscription or a
      named paid key). Models that are the same model on different AI tools are linked (for
      example Kimi K3 on Kimi Code, on Ollama, and on OpenRouter), so the owner sees one model with
      more than one way to reach it.
- [x] A position's (or role's) list is in the owner's order. With the switch on and no order set,
      subscriptions come first (ADR-036 §2.6).
- [x] The Router moves to the next route when one is usage-limited, signed out, over its cap, not
      priced yet, or its key is missing, and says why. Its reason names the route it chose, says
      whether it costs money, and says "text only" for a route without tools (choice 6).
- [x] A route over its cap is skipped until the month turns over or the cap is raised; a usage
      limit is remembered with its reset time, as today.

### OpenRouter through a Plenipo helper (ADR-086)

- [x] A helper run per task (`plenipo-desktop --plenipo-paid openrouter check|chat`), supervised
      like the Ollama helper: an ID, a time limit, cancel, the Ledger record, restart handling.
- [x] One fixed address (`https://openrouter.ai/api/v1/…`), no redirects followed. Guard gains a
      purpose for paid AI services with each company's own addresses; the app checks the address
      before the helper starts, and the helper checks it again before it connects. Refusals are
      recorded.
- [x] The secure connection uses Windows' own TLS through the `reqwest` library already in
      Plenipo, inside the helper only, never in the app itself.
- [x] The helper keeps the conversation in the session's folder, like the Ollama helper, and caps
      the answer's length so the gate knows the most a task can cost.
- [x] Streamed answer, then the service's own bill for the request (usage with cost), recorded as
      the spending. Errors: `401` → "Needs a new key"; `402` → "out of credit on OpenRouter";
      `429` → usage limited, with the reset time when given.
- [x] The short model list (choice 7), each with its maker; any other model by its exact name;
      prices from OpenRouter's public list before each task, capped in size.
- [x] An `openrouter` persona in `plenipo-fake-agent` and the full contract suite.

### Wave 1's leftover (choice 9)

- [ ] Once the paid Ollama plan shows, the paid-plan models are checked against what Ollama lists,
      and the "(paid plan)" labels follow what really answers. (The plan starts 2026-10-01; checked
      then, before part 3 merges.)

### Tests

- [x] A key cannot be saved while no business cap exists, or while the switch is off.
- [x] No key, and no part of a key, appears in the Ledger, a task's activity, a log, or the
      diagnostics file.
- [x] Route fallback: first route usage-limited → second route runs, and the reason says so.
- [x] Route fallback: first route over its cap → skipped until the cap resets or is raised.
- [x] The owner's example: Kimi Code usage-limited → Kimi K3 on Ollama carries the work (stand-ins).
- [x] A paid task records what it spent, against which caps, and which key by name.
- [x] The contract suite still refuses key variables for every subscription AI tool; the OpenRouter
      helper passes the suite with its own persona.
- [x] IPC tests for the key commands; Vitest for the key form, routes, and the reason's words;
      end-to-end in the real app: OpenRouter's card with paid keys switched off (the key form
      locked, nothing sent). A key saved and a paid task run end to end use the fake helper in the
      contract suite, since the real app's helper would reach OpenRouter itself.

## Review of part 2

Five reviewers each read part 2 for one area: secrets and the Vault, money and caps, Guard and the
network, desktop commands and routes, and the screens and words. A second reviewer then checked
every finding against the code (and OpenRouter's own documentation where it mattered), and said
confirmed, plausible, or refuted. Confirmed and plausible findings were fixed with a test, or are
listed below as a limit.

| Area    | Finding                                                                                              | Second reviewer           | What was done                                                                                                                                |
| ------- | ---------------------------------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Network | The key check's price list (464 models, about 71 KB) was cut at 64 KB, so no key could be saved      | Confirmed (high)          | A paid check keeps up to 1 MB and gets 75 seconds; a test with 1,000 models                                                                  |
| Money   | OpenRouter could send a request to a dearer company, or add a fee, past what was set aside           | Confirmed (high)          | Each request carries the price set aside as its most (`max_price`, no request fee); a model with a fee beyond its token prices is not priced |
| Money   | With the owner's own company keys on OpenRouter, the bill read was only OpenRouter's 5%              | Confirmed (high, if used) | The bill adds what the company billed (`upstream_inference_cost`)                                                                            |
| Routes  | A paid route was still chosen with less left than its shortest task, and failed every time           | Confirmed (medium)        | Skipped when less is left than its shortest task could cost, and the reason says so                                                          |
| Routes  | Paid keys switched on or off, or the business cap set, did not refresh OpenRouter                    | Partly confirmed          | OpenRouter is checked again at once when either changes                                                                                      |
| Routes  | At startup the first check could run before the paid gate existed                                    | Confirmed (medium)        | The AI tools are checked, and Liaison starts, after the gate is in place                                                                     |
| Network | 403 (a guardrail or moderation) said "needs a new key"                                               | Confirmed (medium)        | "Refused this request", not billed, and the key is kept                                                                                      |
| Network | A usage limit's reset time was not read                                                              | Confirmed (medium)        | Read from `X-RateLimit-Reset` or `Retry-After`, so OpenRouter is held only until then                                                        |
| Network | Requests that never reached a model (400, 404, 413, 422) counted at the most they could cost         | Plausible                 | Not billed; counted as not sent                                                                                                              |
| Secrets | A removed key stayed in Windows Credential Manager if the delete failed, with nothing pointing to it | Confirmed (medium)        | The key is erased first; if that fails it stays listed, and Remove says so                                                                   |
| Secrets | Two saves at once could undo each other and leave a key unlisted                                     | Plausible                 | Saves and removes take turns; a failed check puts back only its own save                                                                     |
| Secrets | Plenipo stopping during a key's check left the old key unlisted                                      | Confirmed (low)           | The new key's record keeps the old one's name until the check passes (`replaces`)                                                            |
| Secrets | An echoed key cut short before it was hidden                                                         | Confirmed (low)           | Hidden first, then cut                                                                                                                       |
| Secrets | A key pasted as its name would be kept in the Ledger                                                 | Plausible                 | A name that looks like a key is refused before anything is kept                                                                              |
| Money   | Missing token counts priced as zero                                                                  | Confirmed (low)           | Without both counts or the service's bill, the step is "not priced yet"                                                                      |
| Money   | Words in a conversation file other than plain text counted as nothing                                | Plausible                 | The file keeps only each message's role and words, and only its newest part                                                                  |
| Money   | A task's record that could not be read left only the business cap                                    | Confirmed (low)           | The step does not start                                                                                                                      |
| Money   | A bill the Ledger was too busy to record was lost                                                    | Confirmed (low)           | Two more tries                                                                                                                               |
| Money   | Helper errors before sending counted at the most                                                     | Confirmed (low)           | Counted as not sent                                                                                                                          |
| Network | No time limit while an answer was silent; a cut-off answer could pass as finished                    | Plausible                 | Five minutes without a byte ends it; an answer counts only when the service said it was done                                                 |
| Network | The app did not check or record the address before the helper                                        | Confirmed (low)           | The gate checks it with Guard, which records a refusal                                                                                       |
| Routes  | "Prefer another company" could put a listed paid route before a subscription listed first            | Plausible                 | A paid route listed after a subscription stays after it                                                                                      |
| Routes  | A signed-out OpenRouter showed among the whole list's choices                                        | Confirmed (nit)           | A paid AI tool is known by what it is, not by its sign-in                                                                                    |
| Screens | "Vault" on screen; the words said the key is kept before it is checked                               | Confirmed (medium)        | "If it works, it is kept in Windows Credential Manager"                                                                                      |
| Screens | The card flickered to sign-in words while loading; the pill and the key check could disagree         | Confirmed (medium)        | Rows wait for the page; switched off counts as "Key not in use" everywhere                                                                   |
| Screens | Old refusal and old name stayed after Cancel or Remove; button names; two "Spending caps" buttons    | Confirmed (low)           | Cleared; "Replace key for OpenRouter"; "Open Switches" and "Open Spending caps"                                                              |
| Screens | "$3.00 in, $15.00 out"                                                                               | Nit                       | "$3.00 a million tokens read, $15.00 a million written", as the word list says                                                               |
| Secrets | Another secret's save could overwrite a paid key                                                     | Refuted                   | A secret's name never matches a paid key's                                                                                                   |

**Limits, recorded:** some companies behind OpenRouter do not count thinking inside the answer's
length limit, so such a model's thinking can pass what was set aside (the bill is still recorded
as it came); an on-demand overseer from another department is counted by the Router against its
own department, while the gate counts the team's (the gate decides, so nothing is overspent); a
bill still unrecorded after three tries counts only at what was set aside; and the key form's
Ledger and Vault writes run on a background worker, not the window.

## Pull request 3 — direct keys and v1.16.0 (ADR-087)

- [ ] **Every AI company whose models take a key (choice 14):** the makers Plenipo lists today, and
      the makers on OpenRouter's short list that sell keys themselves:

      | Company              | Address (fixed)                     | The owner's key         |
      | -------------------- | ----------------------------------- | ----------------------- |
      | Anthropic            | `api.anthropic.com`                 | yes                     |
      | OpenAI               | `api.openai.com`                    | yes                     |
      | xAI (Grok)           | `api.x.ai`                          | yes                     |
      | Moonshot AI (Kimi)   | `api.moonshot.ai`                   | yes                     |
      | Google (Gemini)      | `generativelanguage.googleapis.com` | later                   |
      | DeepSeek             | `api.deepseek.com`                  | later                   |
      | Z.ai (GLM)           | `api.z.ai`                          | later                   |
      | MiniMax              | `api.minimax.io`                    | later                   |
      | NVIDIA               | `integrate.api.nvidia.com`          | later                   |
      | Mistral              | `api.mistral.ai`                    | later                   |
      | Alibaba Cloud (Qwen) | `dashscope-intl.aliyuncs.com`       | later                   |

      Each address is checked against the company's own documentation when built. Meta's Llama is
      reached through OpenRouter. GitHub Copilot and Cursor sell no key; Ollama's own key is not a
      pay-per-use key, so it stays refused (ADR-017).

- [ ] The same helper for all of them, with one small part per way of talking: Anthropic's own,
      and the OpenAI-style chat most of the others offer. Each company is a row: its fixed
      address, its key check, its models with their maker, and its dated prices.
- [ ] Each company's models checked against its own list with the owner's key where the owner has
      one; the others marked "not checked with a real key yet" until the owner's key arrives.
- [ ] A persona per way of talking in `plenipo-fake-agent`, and the full contract suite for every
      company.
- [ ] Version 1.16.0 (1.17.0 if Phase 21 merges first; ADR-090 §6), and the release started once
      every check passes (choice 13).

## Paperwork

- [ ] [ADR-085](../adr/), [ADR-086](../adr/), and [ADR-087](../adr/), each with "As built"; the ADR
      index; notes in ADR-003, ADR-007, ADR-011, and ADR-014 pointing to ADR-085.
- [ ] Setup guide (paid keys, caps, and the switch), the adding-an-AI-tool guide (a paid helper),
      and README.
- [ ] New word pairs in [the word list](../design/vocabulary.md) (spending cap, paid key, the ways
      to reach a model, "not priced yet").
- [ ] The acceptance report, release notes v1.16.0, a row in
      [versioning](../development/versioning.md), and the plan's Phase 16 line and order-of-work
      row.
- [ ] A review across several areas (secrets and the Vault, money and caps, Guard and the network,
      desktop commands, screens and words), each finding checked by a second reviewer; each
      confirmed finding fixed with a test, or recorded as a design limit.

## Not in this wave

- Plenipo's tools (files, programs, git) for paid and Ollama workers (choice 6).
- Hermes Agent (Wave 4).
- A limit on which hosting companies OpenRouter uses (choice 8: none).
