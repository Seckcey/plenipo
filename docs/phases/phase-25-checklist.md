# Phase 25 — Implementation Checklist

**Status: started 2026-10-03, beside Phases 23 and 24; Wave 1 in progress.** The owner answered
ADR-190's six questions the same day ([the owner's answers](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). Builds on v1.20.0. Below, "[x]"
is done.
Plenipo is made by 8 West Ventures, LLC.

Source: the owner's two lists of 2026-10-03, and
[ADR-190 (Phase 25 starts: fixes and a simpler Plenipo before launch)](../adr/ADR-190-phase-25-starts.md).

**Numbers:** ADR-190 to ADR-199. Phase 24 is already using numbers in the 170s, so Phase 25
starts at 190 to keep clear of it. Waves 1 and 2 and the first Wave 3 items used the whole block (ADR-199 is Stop all work), so
any later Phase 25 decision continues at ADR-250. (It first used ADR-200 to ADR-204; on
2026-10-03 another session put ADR-200 to ADR-202 on `main`, so Phase 25 moved to the 250s,
which no one else uses.) Dates are Pacific
time. Screen words follow
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal:** "Plenipo works the first time, is simple to set up, lets you see and steer every worker,
and makes your AI plans last."

## In short, for the owner

- **What this is.** Your 17 bug reports and changes, plus the 6 ideas you kept from the first list
  (token use, templates, the plan-ran-out suggestion, prompt caching, the Connections cards, and
  catching made-up answers). Everything is in one phase, in **four waves**. A wave is a group of
  fixes that ship together.
- **Wave 1 fixes what's broken.** The freeze, the missing usage numbers, the yellow light, Grok
  asking for a key, the Copilot error, Watch, and the "What's stuck" tile. These are mostly small,
  and they ship first.
- **Wave 2 makes Plenipo simpler.** Fewer questions, cards that start closed, choosing the exact
  model for each job, using the team you already hired, templates, and a real setup tour.
- **Wave 3 lets you see and steer the work.** Watch the worker type live, like a chatbot. A Stop
  button on every worker, and one big **Stop all** button. Side chats with any manager or
  supervisor.
- **Wave 4 makes your AI plans last and catches made-up answers.** Prompt caching, stepping down
  to a smaller model instead of stopping, spreading use across the week, and supervisors checking
  answers against what really happened.
- **About 20 to 27 build sessions in all.** A build session is one Claude Code session like the
  ones that built each part of the earlier phases. This is a rough guess.

## Where each of your items went

**Your first list** (the ones you kept):

| Your item                                          | Where it is in this plan |
| -------------------------------------------------- | ------------------------ |
| 3. Manage token use with effort and smaller models | 4.3, 4.4, 4.5, 4.6       |
| 4. Templates                                       | 2.8                      |
| 5. Suggest a reset when a plan runs out            | 4.2                      |
| 6. Prompt caching for Anthropic models             | 4.1                      |
| 8. Connections easier, cards start closed          | 2.2                      |
| 10. Supervisors and up catch made-up answers       | 4.7, 4.8                 |

The workflow canvas, the command-line version, installing AI tools for you, making every number
clickable, and the website copy are **dropped** at your direction (2026-10-03).

**Your second list:**

| Your item                                                   | Where it is in this plan |
| ----------------------------------------------------------- | ------------------------ |
| 1. Froze when starting a new organization                   | 1.1                      |
| 2. Usage numbers not showing for Anthropic and OpenAI       | 1.2                      |
| 3. "Key not in use" yellow light                            | 1.3, 1.4                 |
| 4. Subscription status missing; cards should start closed   | 2.1                      |
| 5. Context size: fill it in or remove it                    | 2.3                      |
| 6. Can only choose the company, not the model, for a job    | 2.5                      |
| 7. Grok asks for a paid key; subscriptions not used first   | 1.5, 4.4                 |
| 8. A much better setup tour, with Driver.js                 | 2.9                      |
| 9. Too many questions (images, computer use); simplify      | 2.4, 2.6                 |
| 10. Copilot as code reviewer refused: "who made it unknown" | 1.6                      |
| 11 and 12. Watch shows nothing; output should stream live   | 1.8, 3.1, 3.2            |
| 13. Watch is not on every tile                              | 1.8                      |
| 14. The manager hired new workers instead of using mine     | 2.7                      |
| 15. Side chats with managers and supervisors                | 3.5                      |
| 16. Stop on every worker; Stop all on every page            | 3.3, 3.4                 |
| 17. Make "What's stuck" clickable                           | 1.7                      |

## How each item is written

Each item says:

- **You said:** your words.
- **Why:** what the code does today. File paths are from v1.20.0.
- **Do:** what will change.
- **Tests:** what proves it works.
- **Size:** S small (part of a session), M medium (about one session), L large (two or more).

---

## Wave 1 — Fix what's broken

Ships first, in small releases. About 2 to 3 build sessions.

### 1.1 Making a new organization freezes Plenipo — S

- **You said:** "Plenipo froze when starting a new org. Had to force quit the app."
- **Why:** making the organization works and is saved. The freeze comes right after that, when
  Plenipo opens the new organization's window. `open_organization_window`
  (`apps/desktop/src-tauri/src/org_commands.rs`) is a **sync** command, so Tauri runs it on the
  window's own thread. Building a new window from there locks up on Windows (WebView2). The whole
  app stops, because every window shares that thread. `switch_organization` and
  `get_organizations` are also sync, and they read every organization's Ledger on that thread.
  The tests use Tauri's pretend runtime, which has no WebView2, so they could not catch it.
  - Confidence: high, but not proven on a real PC. The new organization should still be in the
    list after a restart.
- **Do:**
  - [x] Make `open_organization_window`, `switch_organization`, and `get_organizations` `async`.
        Tauri's own notes confirm the cause: building a window "deadlocks when used in a
        synchronous command and event handlers" on Windows.
  - [x] Add a source test to `org_commands.rs` that every command is `async`, like the one in
        `ai_tools_commands.rs`.
  - [x] Check `start_close::recreate_main_window`, called from the tray, for the same problem. It
        had it: the tray and a second launch now rebuild a closed window off their own thread.
- **Tests:**
  - [x] the new source test
  - [ ] the owner makes a new organization on Windows 11 and its window opens, five times in a row

### 1.2 Usage numbers missing for Claude Code and Codex — S–M

- **You said:** "Usage stats aren't populating in Anthropic or OpenAI."
- **Why:** the token counts are written correctly. Three things hide them:
  1. **The cards named "Anthropic" and "OpenAI" are the paid-key cards**, not Claude Code and
     Codex. They only count work done with a paid key. Your subscription use is on the
     **Claude Code** and **Codex** cards. Since v1.17.0 the key cards carry the company name,
     which makes this easy to mix up.
  2. **Usage is read only from the first organization.** The AI tools page is built from the first
     organization's Ledger (`crates/capabilities/src/ai_tools.rs`, `AiTools::ledger()`). Work in a
     second organization never shows. The after-task plan check is only hooked to the first
     organization (`ai_tools_host::listen` in `lib.rs`).
  3. **"Plan left" only updates at certain times.** Claude Code reports it only during a Plenipo
     task. Codex is asked 4 minutes after start, once a day, and after a first-organization task.
     No button asks on demand. When a new plan report is saved, nothing tells the open page.
  - Only tasks Plenipo runs are counted. Your own use of the AI tools outside Plenipo is not
    counted (ADR-060, usage only from what the AI tools report).
- **Do:**
  - [x] Add up usage across every open organization's Ledger (`AiTools::count_every_organization`).
  - [x] Hook the after-task plan check to every organization: those open at start
        (`ai_tools_host::count_every_organization`) and those made or brought back later
        (`org_commands::open`).
  - [x] A **Check plan** button on cards whose AI tool Plenipo can ask (Codex, GitHub Copilot).
        Claude Code reports its plan only during a task, so it has none.
  - [ ] Tell the open page when a background plan check saves a new report. **Moved to 4.3**
        (better plan numbers), which reworks the plan report.
  - [ ] On a subscription card, show its paid key's usage as a second line. **Moved to 2.1**, which
        folds the key into the subscription card.
  - [x] Say on the Usage tab that it counts the tasks Plenipo ran, in every organization, and not
        your own use outside Plenipo.
- **Tests:**
  - [x] usage from two organizations is added up (`usage_counts_every_organizations_tasks`)
  - [x] Check plan asks the AI tool (`aiTools.test.tsx`)

### 1.3 One light per AI company: green when the subscription or the key works — S

- **You said:** "The card shows a yellow light saying 'key not in use'. It needs to be green and say
  if subscription is connected, API key connected, or both. If either is working, it needs to be
  green."
- **Why:** a subscription (Claude Code) and its paid key (the "Anthropic" key) are two separate
  AI tools with two separate cards. Each card works out its own light (`cardStatus` in
  `apps/desktop/src/components/aiTools/AiToolCard.tsx`). The key card turns yellow whenever its
  key isn't ready or paid keys are switched off. The subscription card never looks at the key.
  Why the key reads "not in use" is item 1.4.
- **Do:**
  - [x] One light per company on the subscription card:
    - **Green** with "Subscription connected", "API key connected", or "Subscription and API key
      connected" when either one works.
    - **Yellow** with the reason when neither works but something is set up.
    - **Red** when nothing is installed and no key is saved.
  - [ ] Fold the company key cards (Anthropic, OpenAI, xAI, and the rest) into their subscription
        card. OpenRouter keeps its own card, because it has no subscription. **Moved to 2.1**,
        which rebuilds the cards.
  - [x] When a key is saved but switched off or blocked, the reason shows: the card's notice says
        it once (unchanged).
  - [ ] Say on the card that a key is only used for jobs where its model is on the list. **Moved to
        4.4**, which makes it automatic instead.
- **Tests:** [x] the three green lights, in `aiTools.test.tsx`.

### 1.4 Paid keys work in every organization — M

- **Why:** a paid key is saved into the **first** organization's Vault and Guard. Each
  organization has its own (ADR-094, more than one organization). A window showing a second
  organization looks for the key in its own Vault, doesn't find it, and shows "Key not in use".
  The paid-keys switch being off, or the key check failing at startup, gives the same light.
- **Do:** (the owner chose one set of keys for the whole PC; [answer 2](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03))
  - [x] One set of keys for the whole PC, saved once and used by every organization. Spending caps
        and the paid-keys switch stay per organization. Recorded in
        [ADR-192 (one set of paid AI keys for the whole PC)](../adr/ADR-192-one-set-of-paid-keys-for-the-pc.md).
  - [x] Every organization hides the PC's keys from its own record.
  - [x] A key is saved under the switch of the organization you're looking at, and checked there.
- **Tests:**
  - [x] a key saved once works in a second organization, never enters its Vault, and is hidden
        from its record (`a_paid_key_saved_once_works_in_every_organization`)
  - [ ] the light turns green in both: the owner's check on Windows 11

### 1.5 Subscriptions first in every AI tool picker — S

- **You said:** "I wanted to use Grok but it says 'xAI cannot take work now (No paid key is
  saved…)'. I want to use my subscription. … The subscription login buttons are nowhere to be found
  and Plenipo is not using subscription tokens before API tokens."
- **Why:** you chose the AI tool named **"xAI"**. Since v1.17.0 that is the name of the **paid xAI
  key** tool. Your Grok subscription is the tool named **"Grok"**. Every AI tool picker lists all 18
  AI tools in one list, and nothing marks which are paid (`runtimeChoiceLabel` in
  `apps/desktop/src/components/org/OrgDialogs.tsx`, and `inspector/ModelTab.tsx`). Choosing an AI
  tool for a hire pins it, with no backup (`Planner::fixed` in `crates/router/src/service.rs`), so
  subscription-first never gets a chance.
  - For automatic choices, subscriptions do come first today. Paid keys are used only where you
    listed them.
  - The error sends you to "Settings → AI tools". That screen has no Sign in buttons. They are on
    the **AI tools page**, and only when the tool is installed (`aiTools/SignIn.tsx`). When you're
    already signed in, the button says "Reconnect", not "Sign in".
- **Do:**
  - [x] Every AI tool picker shows two groups: **Your subscriptions**, then **Paid per use with
        your key**. Paid tools show only once their key works (a saved choice still shows).
        Allowed AI tools on a project mark each paid one "paid per use".
  - [x] A position fixed to a company's paid tool with no working key says: "xAI's key isn't set
        up. To use your Grok subscription, choose Grok."
  - [x] The error text points to "the AI tools page", not "Settings → AI tools".
  - [x] Settings → AI tools: a signed-out AI tool's row says **Sign in →** and opens its card,
        where the Sign in button is. The button itself moves to the card's header in 2.1.
  - [ ] The full fix, a subscription with its company's key as a backup, is item 4.4.
- **Tests:** [x] picker groups, the hint, and the Sign in rows.

### 1.6 Copilot as code reviewer: "who made it is not known" — S

- **You said:** the error "You set Senior Developer to always use GitHub Copilot (default model),
  but Senior Developer has AI companies never to use, and who made GitHub Copilot (default model) is
  not known." stayed after you unticked GitHub.
- **Why:** Copilot picks its own model ("Auto"), so Plenipo can't tell which company made it. Since
  ADR-081 (who made each model), an unknown maker is refused whenever **any** rule level has
  **any** "never use" company. That is why unticking GitHub didn't help. "Never use" lists add up
  across four levels (organization, department, role, and the agent), on three different screens.
  "Senior Developer" in the error is the **role** whose list blocked it. Either the hire dialog was
  still on its first role (Senior Developer), or you changed an existing Senior Developer that
  reviews for the team.
- **Do:**
  - [x] **Change ADR-081:** a model you pick **by name** is your informed choice. Plenipo keeps it
        and shows a warning instead of refusing. "Play it safe" stays for automatic choices.
        Recorded in
        [ADR-191 (a model you pick by name is your choice)](../adr/ADR-191-a-model-you-pick-by-name-is-your-choice.md).
  - [x] The warning names the companies and who sets the list: "Plenipo can't tell who made GitHub
        Copilot (default model), so it can't rule out DeepSeek and xAI, which Senior Developer
        never uses. It keeps your choice."
  - [ ] Next to every AI tool picker, show the combined "never use" list and where each entry came
        from. **Part of 2.6.**
- **Tests:** [x] a pinned model whose maker is unknown is kept with the warning; a known maker on
  the list is still refused; automatic choices are unchanged
  (`a_fixed_model_is_refused_by_who_made_it`).

### 1.7 "What's stuck" opens the stuck thing — S

- **You said:** "Make the tile on the homepage that says 'What's Stuck' clickable. It should take the
  user to the stuck task or agent."
- **Why:** the Stuck tile has no click action (`pages/HomePage.tsx`). The rows in the "What's
  stuck" list below it already open their task or worker.
- **Do:**
  - [x] One stuck item: the tile opens its task page, worker page, or Settings → Servers.
  - [x] More than one: the tile scrolls to the "What's stuck" list and moves the focus there.
  - [x] "Open the worker": not needed. A stuck task opens its task page, which links its worker.
  - [x] Give the "Objectives going" tile a click action too: one opens it, more show the list.
- **Tests:** [x] `pages.test.tsx`, for one item and for many.

### 1.8 Watch on every tile, and Watch shows the team's work — S–M

- **You said:** "Watch says 'No file changes yet in this objective.' … Watch is not on every
  agent's tile."
- **Why:**
  - **Watch shows only the clicked worker's own changes** (`WatchHub::view` in
    `crates/capabilities/src/watch.rs`, and the filter in `terminal/code.ts`). Managers and
    supervisors hand the coding to their team, so watching them always shows nothing. And the
    canvas offers Watch exactly while they "work", which is when they are planning and handing off.
  - Workers whose AI tool can't change files (Copilot, Antigravity, paid keys, Ollama), and work
    with no project folder, never write anything.
  - Changes made by commands (`run_command`, git) don't show. That's item 3.2.
  - The Watch button is drawn only on full-time tiles that are working right now
    (`TopologyCanvas.tsx`). It's missing on idle tiles, on-call worker tiles, List mode, the
    worker popup, the Worker page, the Task page, and Home.
- **Do:**
  - [x] Watch on a manager or supervisor shows its own changes and **every change made in the
        work it handed on**, in its current objective, each labelled by the worker who made it.
        A new hand-off is picked up live. Recorded in
        [ADR-193 (Watch shows the team's work)](../adr/ADR-193-watch-shows-the-team.md).
  - [x] When Watch is empty, it says why when Plenipo knows ("It hasn't been given any work yet",
        or Guard's recorded reason a worker got no tools), and otherwise says plainly that it
        shows files written with Plenipo's file tools, not changes made by commands (3.2).
  - [x] The Watch button on every active tile on the canvas (quieter when idle) and on the Worker
        page.
  - [ ] Watch on the Task page, Home's "Who's working" rows, List mode, and the worker popup.
        **Moved to 3.1**, with the live conversation that goes in the same places.
- **Tests:**
  - [x] Watch on a supervisor shows its worker's changes, in memory and after a restart
        (`watch_shows_every_file_change_as_it_lands_with_its_lines`)
  - [x] the team's changes are heard live, a new hand-off is read again, and the reason shows
        (`code.test.ts`)

---

## Wave 2 — Make Plenipo simple

About 7 to 9 build sessions. The setup tour (2.9) comes last, because it walks you through the
simpler screens.

### 2.1 AI tools cards start closed, with a light and a one-line status — M

- **You said:** "Subscription status not showing in the AI tools tab. The AI tool cards need to
  start collapsed with the green, yellow or red light showing its status."
- **Why:** no card can close today. Each card always shows its tabs and its Overview
  (`AiToolCard.tsx`). About ten company key cards and OpenRouter are listed fully open too. The
  plan name (for example "Claude Max") is buried inside Overview. The light says only "Ready".
- **Do:**
  - [x] Each card starts closed. Its header shows the light from 1.3 ("Subscription connected"),
        one line ("AI company: Anthropic · Claude subscription (max)", and the saved key's name),
        and a **Sign in** (or **Reconnect**) button.
  - [x] A card opens by itself when it needs you (installed but not signed in, given no tasks, or
        installed in a way Plenipo can't use), or when a link points at it. Plenipo remembers
        which cards you opened.
  - [x] Uses the shared open/close part from 2.2.
  - [x] From 1.3: fold the company key cards (Anthropic, OpenAI, xAI, Moonshot, Google) into their
        subscription card; OpenRouter and the companies with no subscription keep their own. A
        link to a folded key card shows its subscription card.
  - [x] From 1.2: the subscription card shows its key's usage as a second line ("With your key").
- **Tests:** [x] `aiTools.test.tsx` (tests open the card first; new tests for closed cards, a card
  that needs you, and links); the real-app tests open the cards.

### 2.2 Connections cards start closed — M

- **You said (first list):** "Make the connections tab easier to use and make each card
  collapsible. They should all start collapsed."
- **Why:** Connections is one long page. It has 6 services, about 19 on/off controls, about 18 text
  boxes, and the same safety text repeated on every card (`settings/connections/`).
  `packages/ui` has no open/close part.
- **Do:**
  - [x] A shared **Disclosure** part in `packages/ui`, with a Gallery entry and a test, used by 2.1
        and here. It remembers a card you opened, and opens by itself when the card needs you. Its
        Gallery samples join the design test's look snapshots once one is taken on Windows.
  - [x] Each Connections card and add-on card starts closed and shows one line: connected or not,
        which account, "3 of 5 parts on", and "2 roles or agents may use it". A card that needs you
        (signing in, a new sign-in or key, a problem, the admin's approval, a reconnect, or a
        changed add-on tool) starts open.
  - [x] Show the repeated safety text once at the top. Each card keeps a short "Learn more" link.
- **Tests:** [x] `disclosure.test.tsx`; `connections.test.tsx` (the tests open the card first, and a
  new test checks closed cards, their line, and a card that needs you); the real-app Connections
  test opens the cards.

### 2.3 Remove context size — S

- **You said:** "Auto populate context size in Settings → AI Models or remove it."
- **Why:** it is a number you type by hand. No AI tool's model list includes it. The router uses it
  only when a role sets a minimum, and no built-in role does. If one did, every model without a
  typed-in size would be skipped. Filling it in automatically would mean checking each model's
  size on your PC, over and over (ADR-081 §8), for no real gain.
- **Do:**
  - [x] Remove the field and column from AI models, the role choices, the specialty dialog, and
        the AI model tab.
  - [x] **Keep the saved field and ignore it.** Saved settings refuse unknown fields when they are
        read. Deleting the field would make old settings unreadable. Saving a model or role keeps
        the old value untouched. Recorded in
        [ADR-194 (what a model can do is no longer asked)](../adr/ADR-194-what-a-model-can-do-is-no-longer-asked.md).
- **Tests:** [x] a saved minimum context rules nothing out
  (`saved_needs_and_context_size_rule_nothing_out`); the forms no longer ask
  (`ModelSettings.test.tsx`).

### 2.4 Fewer questions: no "make images" or "use a computer" — S–M

- **You said:** "What's the point of asking if models can view images, create images and/or use
  computer? We really need to simplify Plenipo."
- **Why:**
  - **"Makes images"** can't happen through a worker. Plenipo turns those tools off (Grok's
    `image_gen`, Codex's `view_image`).
  - **"Uses a computer"** is already decided by Guard's computer permissions, not by this box.
  - **"Sees images"** starts unticked, so every worker is told its model "is not marked as able to
    see images", even Claude and GPT. And the **Designer gets no model at all** out of the box,
    because its role asks for both image boxes.
- **Do:**
  - [x] **Change ADR-011 (the router's model rules):** remove "Makes images" and "Uses a computer".
        Keep the saved values readable, and ignore them. "Sees images" isn't asked either.
        Recorded in
        [ADR-194 (what a model can do is no longer asked)](../adr/ADR-194-what-a-model-can-do-is-no-longer-asked.md).
  - [x] "Sees images" comes from who made the model (from the AI tool's own list, ADR-081): Anthropic
        and Google, and OpenAI apart from `gpt-oss`. When it isn't known, the worker is told nothing.
  - [x] A one-time fix for existing installs: not needed. The router ignores saved requirements,
        so an old Designer gets a model too. New installs start it with none.
- **Tests:** [x] Designer gets a model on a fresh install and with an old install's requirements
  (`reviewers_come_from_another_ai_company_and_unfit_roles_are_explained`, and the real-app
  routing test).

### 2.5 Pick the exact model for each job — M

- **You said:** "It only lets you choose the company, not the model. I can't use Claude Fable for
  coding and Claude Sonnet for documentation. It only says 'default model'."
- **Why:** a role's model list can only use entries from **Your models**. That list starts with one
  "default model" entry per AI tool. Each AI tool's own models (Claude Code's fable, opus, sonnet,
  haiku) are offered only in the hire dialog and the per-agent picker (`RuleEditor.tsx`,
  `RoleChoices.tsx`). Today you'd have to add Fable under "Your models" first, then add it to
  Senior Developer, and nothing tells you that.
- **Do:**
  - [x] Wherever you pick a model for a role, department, or organization (and one agent), show
        **every model of every AI tool**, grouped by AI tool, subscriptions first. Picking one adds
        it to Your models by itself. Recorded in
        [ADR-195 (who uses what)](../adr/ADR-195-who-uses-what.md).
  - [x] Rename the "(default model)" entries to "Claude Code: its own choice". An install from
        before is renamed once; a name you gave is kept.
- **Tests:** [x] choose Fable for Senior Developer and Sonnet for the Designer in one screen
  (`ModelSettings.test.tsx`); the rename (`old_builtin_names_become_its_own_choice`). Routing
  each role to its model is the router's, unchanged.

### 2.6 One "who uses what" screen — M

- **You said:** "Plenipo is WAY too complicated."
- **Why:** picking Fable for one role takes 3 screens and 4 forms today. Model rules live at seven
  levels, and "never use" lists add up across four of them on three screens. Some settings (cost
  class, context size, image boxes) do nothing until filled in.
- **Do:**
  - [x] Settings → AI models becomes one table, **Who uses what**: each role, its model, its backup,
        its effort, plus one row each for "the whole organization" and each department. The agents'
        own rules are under **More**.
  - [x] "Never use" lists move to the organization level only. Existing lists at other levels are
        kept and shown under More, each with a Remove button.
  - [x] Cost class moves under More (with a role's "When no models are listed" and "Reviews").
  - [x] The role table says where each next worker's model came from ("from Senior Developer's
        choices", "from the whole organization"); the agent's panel already said who picked it.
- **Tests:** [x] router tests unchanged (rules still apply); screen tests for the table, the
  organization's never-use list, and an old list under More.

### 2.7 Use the team you hired before bringing in new workers — L

- **You said:** "It created new senior devs and code reviewers instead of using the ones I set up. I
  do like how giving the dev manager a task fans out a dev, reviewer and QA engineer. But we need to
  use the agents already available. If it's missing an agent the manager needs, then it can spawn
  that agent."
- **Why:**
  1. **"Set up a Development project"** (`set_up_team` in `crates/workforce/src/service.rs`)
     **always hires a fresh team** for the new project's supervisor, on automatic models. It never
     looks for workers you already have. If no department is named exactly "Development", it makes
     another department and VP too.
  2. A supervisor can only hand work to **its own team**, meaning the workers who report to it
     (`OrgView::team` in `crates/workforce/src/view.rs`). Your existing developers and reviewers sit
     under another supervisor or under the manager, so the new supervisor can't reach them.
  3. A busy full-time worker is not skipped. Its task waits in line, so that part already works.
- **Do:**
  - [x] **No copies at setup.** For each job the team needs, use a matching worker already in the
        department, and hire only what's missing. The setup dialog shows "Use Alex (Senior
        Developer, Claude Fable)" or "Hire new" for each job, and asks which department.
  - [x] **Share across the department.** A supervisor looks in its own team first, then at the
        department's other on-call workers. They keep the AI tool and model you chose for them.
        The work is done under the asking project's folder and limits. (Full-time workers outside
        the team are still reached by lending them.)
  - [x] **Bring in one only when none exists.** If the department has no one for the job, Plenipo
        asks you first on Home: "Website Supervisor needs a QA Engineer. Hire one?" A switch in
        Settings → Switches lets it hire on its own ([answer 3](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). Free's limits still apply.
  - [x] The supervisor's and manager's instructions say: "Use the people you have first."
  - [x] **Change ADR-016 (the Development department) and ADR-054 (move or lend an agent):**
        [ADR-196 (use the team you hired first)](../adr/ADR-196-use-the-team-you-hired-first.md).
- **Tests:**
  - [x] setup with existing workers hires no copies
  - [x] a supervisor's hand-off reaches a department worker outside its team
  - [x] a missing job asks before hiring, once; with the switch on, it hires
        (`a_new_project_uses_the_departments_workers_before_hiring`, `pages.test.tsx`,
        `teamReuse.test.tsx`)
  - [x] the fan-out to developer, reviewer, and QA still happens (the Phase 8 tests, unchanged)

### 2.8 Templates for organizations, projects, businesses, and enterprises — M

- **You said (first list):** "Add templates for organizations, projects, businesses and
  enterprises."
- **Why:** the plumbing is already there and waiting. "Use a template" is already in the
  new-organization dialog, greyed out, and the code says "Templates are coming later." One team
  template exists (Development), and `set_up_team` can apply any team template.
- **Do:**
  - [x] **Templates:** recorded in [ADR-197 (templates)](../adr/ADR-197-templates.md).
    - **Small business:** Operations and Marketing.
    - **Software project:** Development (after 2.7, it reuses existing workers).
    - **Agency:** Development, Design, and Marketing.
    - **Enterprise:** several departments, each with a manager and a supervisor.
    - **IT services:** Operations and Documentation.
    - Each department comes with its manager and an on-call team of built-in roles, whose
      starting models and permissions come with the role; rank names stay the organization's.
    - Enterprise's departments each get a manager; a supervisor comes with a project.
  - [x] A template picker in the new-organization, new-project, and new-department dialogs, and in
        Settings → Organization (add a template's departments to this organization).
  - [x] Templates with more than one department need Pro (Free keeps one department).
  - [x] **Save my organization as a template** (setup only: no keys, no files, no history), and use
        it for a new organization. It reuses the setup copy in `ledger/src/workforce/copy.rs`.
  - [x] This carries out part of parked Phase 15 (department templates). Phase 15 itself stays
        parked.
- **Tests:** [x] each template applies cleanly, and on Free the Pro templates are locked
  (`templates_add_departments_with_their_teams_and_free_keeps_one`, the app's organization test,
  `templates.test.tsx`).

### 2.9 A real setup tour, with Driver.js — M–L

- **You said:** "Need a MUCH better tutorial for new organization setup and initial sign up that
  walks the user through setting up AI tools subscriptions, adding a department and setting up
  their first project and then assigning models to the hired tiles. Use the Driver.js library."
- **Why:** the only tour is a 6-card tour of the canvas, with no highlighting
  (`org/tour.ts`). There's no first-run flow, and nothing checks that an AI tool is signed in, so a
  new user can build a whole organization before learning no worker can run.
- **Do:**
  - [x] **Change ADR-030 (one design system):** allow Driver.js (MIT license) for the tour. It
        works with Plenipo's security settings as they are, with no change.
        [ADR-198 (the setup tour)](../adr/ADR-198-the-setup-tour.md).
  - [x] **The tour, step by step, moving on when the step is really done:**
    1. Welcome.
    2. Open the AI tools page and sign in to at least one subscription (waits for a green light).
    3. Name your organization, or pick a template (2.8).
    4. Add a department.
    5. Set up the first project.
    6. Hire the team, reusing workers (2.7).
    7. Choose a model for each tile (2.5). It moves on once you change a model choice, or with
       **Skip this step**.
    8. Give the first objective.
    9. Watch it work (3.1). For now it points at the Watch button from 1.8; 3.1 makes what it
       opens live.
  - [x] It moves between pages by itself, waits for each part of the screen to appear, and brings
        canvas tiles into view (the first project's supervisor).
  - [x] It pauses while a dialog is open, remembers where you stopped, and works per
        organization. It starts by itself in a new organization, and comes back if Plenipo
        closed during it.
  - [x] **Take the setup tour again** in Settings and on Home ("Pick up the setup tour" after you
        stopped part way). The old canvas tour waits while it runs.
  - [x] Colors use Plenipo's theme, in light and dark.
  - [x] Steady anchors: add `data-tour` marks to the sidebar buttons, + Department, + Project, the
        Add menu, and the dialogs (also the hire palette, the AI tools list, Settings →
        Organization, Who uses what, Give an objective, and Watch).
- **Tests:**
  - [x] unit tests for each step's "done" check (`apps/desktop/src/tour/steps.test.ts`, and the
        tour on screen in `SetupTour.test.tsx`)
  - [x] one full run in the end-to-end suite (`tests/e2e/specs/setup-tour.e2e.mjs`; it runs on
        GitHub, not on this build machine)

---

## Wave 3 — See and steer the work

About 5 to 7 build sessions.

### 3.1 Watch the worker think and type, live — M–L

> **With the Chat panel (2026-10-03).** Another session built a live chat with each agent on
> `main` ([ADR-200 (a live chat with each agent)](../adr/ADR-200-a-live-chat-with-each-agent.md)),
> beside this item. Both stay: the Chat panel is where you talk to an agent; the live
> conversation here is the short live view inside the Task and Worker pages, the details panel,
> and Watch. Its **Open in Chat** button opens the same conversation in the Chat panel.

- **You said:** "We need the output to output like regular chatbots in the browser or in their IDE.
  The output streams in real time. Plenipo just says 'Working' and that's it. We don't know what
  it's working on or how far along it is."
- **Why:** live text already arrives from Claude Code, Grok, and Kimi (`TextDelta`). It is shown
  only on the Workers page, in "Live activity". Everywhere else (Task page, Worker page, canvas
  tile, Inspector) shows just "Working". Codex sends whole messages only, no live text. Tool use is
  shown late, and plans ("step 3 of 7") are thrown away.
- **Do:**
  - [x] One **Live conversation** part, chat-style (`apps/desktop/src/live/`):
    - the worker's words appear as it types them (a caret while it types)
    - its steps in plain words: "Reading index.ts", "Changing app.tsx", "Running `npm test`",
      "Searching the web for …", and a failed step as "That didn't work: …"
    - a progress line: step 3 of 7, 4 min, 12 steps, tokens so far
  - [x] Put it in the Watch tab (beside the file changes, and under "No file changes yet"), the
        Task page, the Worker page, the details panel (last 3 lines), and a one-line "now: Running
        `npm test`" on canvas tiles (in place of the rank line while it works).
  - [x] From 1.8: the Watch button on the Task page, Home's "Who's working" rows, and List mode
        (beside Stop). The details panel ("the worker popup") already had it.
  - [x] Read plans and progress from every AI tool that sends them: Grok's and Kimi's plans,
        Codex's to-do list (as it starts, each step ticked, and at the end), and Claude Code's
        to-dos. A new "plan" event, live only.
  - [ ] Claude Code's tool calls as soon as they start: **not yet.** A step shows when its call
        is complete (as before); a file it writes already shows in Watch as it is written.
  - [ ] Codex live text needs its other connection method (app-server). That's a stretch goal,
        built last. **Not yet:** Codex's words show as each message is complete.
- **Tests:** the store joins live text correctly (`agents/store.test.ts`, as before). Each tool's
  plan becomes progress: Rust `its_to_do_list_is_its_plan` (Codex), `its_to_dos_are_its_plan`
  (Claude Code), `the_plan_is_passed_on` (Grok and Kimi); `live/live.test.tsx` (plain-word steps,
  the progress line, the last 3 lines, a tile's one line).

### 3.2 Watch shows changes made by commands too — M–L

- **You said:** "The watch doesn't seem to let the user watch the agent code."
- **Why:** Watch only sees files written through Plenipo's own file tools. Files changed by a
  command (`npm create`, `sed`, a git step) don't show (ADR-055, watching a worker write code).
- **Do:**
  - [x] After each command or git step, compare the working copy before and after. Show what
        changed as "made by a command". Files Guard keeps private are skipped (never read), and so
        are `.git`, `node_modules`, and build output.
  - [x] **Change ADR-055 (Watch):** [ADR-250 (Watch shows changes made by
        commands)](../adr/ADR-250-watch-shows-changes-made-by-commands.md).
- **Tests:** a file made by a command shows in Watch. A private file never does.
  - [x] capabilities `files_a_command_makes_show_in_watch_and_a_private_one_never_does` (a real
        command through the broker; its record keeps the file and counts, never the text) and
        `command_changes` unit tests; `CodeWatchView.test.tsx` "says which files a command made".

### 3.3 A Stop button on every worker — S–M

- **You said:** "We need a stop button on all agents popups."
- **Why:** Stop exists only on the Workers page ("Cancel task"), in Watch (only when a change is on
  screen), in the file editor, and on the phone. It is missing from canvas tiles, the Inspector, the
  worker popup, the Worker page, the Task page, and Home.
- **Do:**
  - [x] One **Stop** part (asks "Stop Alex's task?" first), on every one of those places: a Stop
        chip on each working tile (the corner across from Watch), the details panel that opens
        when you click a tile (the "worker popup"), the Worker page, the Task page, and Home's
        "Who's working" rows. It shows only while there is work to stop. On an on-call position
        it stops each worker that has started; work still queued ends with the task that asked
        for it.
  - [x] Stopping a full-time worker stops only its current task, not its whole conversation.
- **Tests:** Stop from each place ends the task, and the parent sees it.
  - [x] `components/stop/stop.test.tsx` (what each kind of position stops; asks first; Keep
        working; an error stays on screen), `OrganizationCanvas.test.tsx` "Stop on a working
        tile", and the shared rows' actions in `packages/ui/src/page.test.tsx`. Every place uses
        the same part and the same command as the phone's Stop.
  - [x] Liaison `stopping_a_worker_tells_its_lead_and_the_lead_carries_on`: the lead is told
        "cancelled" and finishes its own task.

### 3.4 Stop all: one red button on every page that shows work — M–L

- **You said:** "A stop all emergency button on all pages that monitor tasks. Especially on the org
  canvas."
- **Why:** today's "Stop all" only stops browser, screen, and server control. It does not stop the
  AI work, and it shows only while control is on. The tray's "Stop all programs" stops programs,
  but new work keeps getting handed out.
- **Do:**
  - [x] A new **Stop all work**. In every organization it:
    - stops browser, screen, and server control
    - stops every running task
    - holds all waiting work until you press **Allow again** (new work you give meanwhile is
      refused at once, with what to do)
  - [x] Written in the Ledger ("work.stopped_all", "work.allowed_again").
  - [x] A red button in the top bar of every page that shows work (Home, Organization, Projects,
        Workers, Task, Worker), and on the canvas toolbar. While work is stopped it reads **Allow
        again**.
  - [x] The phone's Stop all and the tray's Stop all do the same (the tray now says "Stop all
        work"). [ADR-199 (Stop all work)](../adr/ADR-199-stop-all-work.md).
- **Tests:**
  - [x] after Stop all, nothing runs and nothing new starts (runtime
        `stop_all_work_stops_what_runs_and_holds_new_work_until_allowed_again`; IPC
        `control_and_websites_through_ipc`: new work is refused, both records written)
  - [x] Allow again resumes (the same runtime test; `App.test.tsx` "has Stop all in the top bar
        …")

### 3.5 Side chats with any manager or supervisor — M–L

> **With the Chat panel (2026-10-03).** ADR-200's chat sends your message into the agent's own
> conversation (it waits its turn while the agent works). A side chat is a separate conversation
> that only answers and never touches the work. Both stay, for those two different jobs.

- **You said:** "We need a way to open side chats to ask each agent a question if I don't want to
  go through the chain of command. Mainly with managers and supervisors while they wait for workers
  to complete their tasks."
- **Why:** the only way to talk to a full-time agent is to give it an objective. A busy agent,
  including one waiting on its workers, says no. The Workers page refuses to continue a member's
  conversation.
- **Do:**
  - [x] **Ask a question** on the Inspector and the Worker page. It works while the agent is busy
        or waiting. [ADR-251 (side chats)](../adr/ADR-251-side-chats.md).
  - [x] The side chat knows what's going on: a briefing with who it is, "status now", and its last
        four objectives and answers, from Plenipo's own saved record of its conversation. Its real
        work is never touched.
    - [ ] Claude Code: `--fork-session`. **Not yet:** every side chat starts fresh with the
          briefing (ADR-251 says why).
    - [x] Paid keys and Ollama: Plenipo's own saved record (the briefing).
    - [ ] Codex, Grok, and Kimi: their fork, once checked. **Not yet** (the briefing for now).
    - [x] Otherwise: a fresh conversation with a short briefing.
  - [x] **Answer only:** no tools and no hand-offs
        ([answer 4](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). It still counts
        toward your plan's usage and Free's three-at-once limit.
  - [x] Side chats are listed in Workers as "Side chat with Alex".
- **Tests:**
  - [x] a side chat during a running task leaves the task alone (Liaison
        `a_side_chat_leaves_the_work_alone_has_no_tools_and_hands_nothing_on`, Workforce
        `a_side_chat_with_a_busy_supervisor_knows_what_it_is_doing_and_leaves_it_alone`)
  - [x] it has no tools (it is never marked as a member's conversation, which is what Plenipo gives
        tools to)
  - [x] its hand-off blocks are ignored (the same Liaison test); `sideChat.test.tsx` for the box

---

## Wave 4 — Make your AI plans last, and catch made-up answers

About 6 to 8 build sessions.

### 4.1 Prompt caching on every Anthropic model — S–M

- **You said (first list):** "Make sure we are using prompt caching for all Anthropic models."
- **Why:**
  - **Claude Code** already caches by itself.
  - **Your paid Anthropic key** and **OpenRouter** do not, because Plenipo's helper sends no cache
    markers (`crates/capabilities/src/paid/helper.rs`).
  - That was on purpose: ADR-085 §3.5 (paid AI keys with spending caps) says Plenipo never asks a
    service to store a conversation for reuse. You've now asked for it.
- **Do:**
  - [x] **Change ADR-085 §3.5:** caching is allowed for Anthropic models, on the direct key and
        through OpenRouter.
        [ADR-252 (prompt caching for Anthropic models)](../adr/ADR-252-prompt-caching-for-anthropic-models.md).
  - [x] Mark the reusable start of each request (instructions and earlier turns) for caching.
  - [x] Count the cost right: writing to the cache costs a little more, and reading from it costs
        much less. The Anthropic price rows get a cache-write price (1.25 times input), and the
        spending gate sets aside the most a step could cost.
  - [x] Show "saved by caching" on the Usage tab.
- **Tests:**
  - [x] the request carries the cache markers (capabilities
        `anthropic_requests_carry_the_cache_marks`, on the key and through OpenRouter; none for
        other companies)
  - [ ] the cost math matches Anthropic's prices, checked on the owner's PC per ADR-081 §8. Here:
        runtime `anthropic_tasks_are_priced_with_the_cache` checks the math against the price rows.

### 4.2 When a plan runs out: say what you can do, and pick the work back up — S–M

- **You said (first list):** "Suggest using a subscription reset when model usage limit is
  reached." And later: "OpenAI and Claude offer full resets that they give out every once in a
  while. I have one for each right now. If you can't track it then don't worry about it."
- **Why:** when a limit hits, the task **fails**. You get a general notice and **Try again now**.
  Nothing restarts the work after the reset.
- **Do:**
  - [x] The notice says: "Claude Code is out until 3:00 PM. You can:
    - wait (Plenipo picks the work back up at 3:00)
    - use a usage reset, if Anthropic gave you one
    - use your Anthropic key or another AI tool"

    It is on every page, one for each AI tool at its limit, and names the work that waits
    ([ADR-253 (when a plan runs out)](../adr/ADR-253-when-a-plan-runs-out.md)).

  - [x] Each choice is a button: **Wait**, **Use a reset** (and **Pick it up now** after it),
        **Use another AI tool**, **Leave stopped**. "Use a reset" opens the company's own page in
        your browser (Claude's or ChatGPT's usage page). **Plenipo never uses a reset or buys
        anything for you.**
  - [x] Check whether Claude Code or Codex reports a waiting reset. **Neither does, in what their
        makers document:** Claude Code's `rate_limit_event` has no such field, and OpenAI's own
        Codex documentation describes none (a third-party note says newer versions have one, not
        confirmed). So Plenipo only reminds, and never guesses (ADR-253).
  - [x] Work stopped by a limit **restarts by itself** after the reset, unless you said Leave
        stopped: each organization looks once a minute (`limit_host.rs`), and each objective is
        picked up once, recorded in the Ledger. Never while Stop all work holds the work.
- **Tests:**
  - [x] the notice's choices (desktop `limits.test.tsx`; IPC `work_a_usage_limit_stopped_through_ipc`)
  - [x] work restarts after the limit is over, never before, and never while Stop all holds it
        (workforce `work_a_usage_limit_stopped_is_picked_back_up_unless_left_stopped`; Ledger
        `objectives_a_limit_stopped_wait_until_picked_up_left_or_given_again`)
  - [x] Leave stopped is respected (the same workforce test)

### 4.3 Better plan numbers — S–M

- **Why:**
  - Plenipo can't tell Claude Code's 5-hour limit from its weekly one (it doesn't read
    `rateLimitType`).
  - Codex's real reset time isn't used. Plenipo waits a flat hour instead.
  - OpenRouter's key limit is fetched and then thrown away.
  - Grok, Kimi, Ollama, and Antigravity report no plan numbers at all. Plenipo only uses what each
    AI tool reports (ADR-060), so those show "not reported".
- **Do:**
  - [x] Label each window: "5-hour: 62% used, resets 3:00 PM" and "Week: 40% used, resets
        Monday". Claude Code's `rateLimitType` (in Anthropic's own Agent SDK types) names each
        window, its weekly Opus and Sonnet limits too ("Week (Opus)"). It reports one window at a
        time, so each report adds to the last until that window resets.
  - [x] Use the reported reset time for every hold. Claude Code: the reset from its "limit
        reached" report goes into the turn's message. Codex and Copilot: their plan check is asked
        at once after a limit (not up to five minutes later), and the Router waits for the reset
        of the window that is full. One plan book is shared by every organization, as the plan is
        the owner's account. With nothing reported, an hour, as before
        ([ADR-253 (when a plan runs out)](../adr/ADR-253-when-a-plan-runs-out.md)).
  - [x] Show OpenRouter's key limit and balance: "Key limit: $10.00 · $3.20 spent · $6.80 left"
        (or "No limit on this key"), from the key check Plenipo already makes. OpenRouter's key
        answer has no account balance, so "left" is what the key may still spend.
  - [x] From 1.2: tell the open page when a background plan check saves a new report (the check
        now tells the screen, like a report during a task).
- **Tests:** each tool's report is read correctly.
  - [x] runtime `each_report_names_its_limit_and_a_reached_one_gives_its_reset`,
        `plan_reports_add_up_and_say_when_a_full_window_resets`, `the_check_reads_the_keys_own_limit`
  - [x] router `a_limit_without_a_reset_time_uses_the_plan_reports`
  - [x] desktop `aiTools.test.tsx` (window names, reset words, the key limit on OpenRouter's card)

### 4.4 Your subscription first, then the same company's key — M

- **You said:** "Plenipo is not using subscription tokens before API tokens like it's supposed to."
- **Why:** choosing an AI tool pins one tool, with no backup. Plenipo already knows when two tools
  run the same model (`KnownModel.same`), but routing never uses it. Claude Code and the Anthropic
  key aren't linked at all.
- **Do:**
  - [x] Choosing a model (for example "Claude Sonnet") means: use it on your **subscription** first,
        and when the plan runs out, on the **same company's key**. The key is used only if paid
        keys are on, a key is saved, and the spending cap has room. It also needs a price and the
        project's leave to use the key, like any paid route. Work waiting "rather than moving to
        another AI company" still moves to the key: it is the same company.
  - [x] Link the same models across Claude Code and the Anthropic key (Fable, Opus, Sonnet,
        Haiku; an alias through the exact model it points to), and across Codex and the OpenAI key
        (GPT-6.1 Sol, GPT-6 Astra, GPT-6 Luna, the three on both).
  - [x] **Change ADR-085 §6**:
        [ADR-254 (your subscription first, then the same company's key)](../adr/ADR-254-your-subscription-first-then-the-same-companys-key.md).
  - [x] Every switch is shown on the worker (its Why) and the card ("Its work moves to your
        Anthropic key while it waits"), and written in the Ledger with the worker (`onKeyFor`).
        Plan rule §3.2, "no silent provider switching", still holds.
- **Tests:**
  - [x] a subscription limit moves the same model onto the key (router
        `a_subscription_limit_moves_the_same_model_to_the_same_companys_key`; desktop
        `aiTools.test.tsx`; runtime `every_same_model_link_joins_two_ways_or_more`)
  - [x] it never does when paid keys are off or the cap is full, nor for a model with no link or
        "its own choice" (the same router test)
  - [ ] a full worker on the key in the real app: needs a stand-in paid key in the workforce
        tests, which have none yet.

### 4.5 Step down instead of stopping — M

- **You said (first list):** "Manage your token use through effort levels and model downgrading."
- **Why:** effort already reaches every AI tool that has it. Nothing lowers it, or moves to a
  smaller model, when a plan runs low.
- **Do:**
  - [x] A step-down ladder for each role
        ([ADR-255 (step down instead of stopping)](../adr/ADR-255-step-down-instead-of-stopping.md)):
    1. lower the effort (past the line)
    2. use a smaller model from the same company (Fable → Opus → Sonnet → Haiku), halfway from
       the line to the limit. Codex steps down by effort only: OpenAI does not rank its models
       by size.
    3. use the same model on your key (4.4), at the limit
    4. wait for the reset (4.2)
  - [x] Reviewers and agents you set to their own model never step down. (Asking first is not
        built: they simply hold, as the owner's answer 6 has stepping down on by default.)
  - [x] It starts when a plan passes a line you set (default 80% used; 70% or 90% in Settings →
        Switches). Every step is shown in the worker's reason and recorded with the worker; the
        setting's changes are recorded too.
  - [x] **On by default**, with a switch in Settings → Switches to turn it off
        ([answer 6](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)).
- **Tests:** each rung of the ladder. Pinned and reviewer roles hold their model.
  - [x] router `work_steps_down_as_a_plan_runs_low` (effort, smaller model, Haiku with no
        effort, an agent's own model, the switch off); the key and the wait are 4.4's and 4.2's
        tests. Reviewers: the Router is given no line for a review.
  - [x] desktop `SwitchSettings.test.tsx` (on to start with, the line, off), `format.test.ts`

### 4.6 Spread use across the week and the month — L

- **You said (first list):** "Adjust throughout the week or month so you don't run out of tokens.
  … Calculate them all and distribute accordingly."
- **Why:** nothing paces use today. **What's possible:** AI companies don't say how many tokens a
  plan includes. Claude Code, Codex, and Copilot report "% used" and a reset time. The others
  report nothing (ADR-060). So Plenipo paces by **percent of each window over time**, and counts
  its own tokens for the rest.
- **Do:**
  - [ ] For each plan window, work out a **fair pace**: how much should be used by now to last
        until the reset. Day-time and night-time weights can be changed.
  - [ ] Ahead of pace, step down early (4.5) and send low-priority work to the plan with the most
        room left.
  - [ ] Behind pace, use the best model freely.
  - [ ] A **Plans** view on the AI tools page: every plan, its pace, its reset, and paid spending
        in one place.
  - [ ] AI tools that report nothing are paced by Plenipo's own token counts against a weekly
        budget you can set, labelled "estimated".
- **Tests:** a simulated week. The pace is kept, nothing runs out before its reset, and every step
  down is recorded.

### 4.7 Catch made-up answers, step 1: check answers against what really happened — M

- **You said (first list):** "Supervisors and up detect hallucinations and act accordingly."
- **Why:** Plenipo already keeps the real facts: files actually changed (from git), commands and
  tests actually run (with pass or fail), and pull requests actually opened. That report goes only
  to you. A supervisor reads only the worker's own story, and nothing compares the two.
- **Do:**
  - [x] Every answer handed back up the chain carries **Plenipo's facts** under the worker's
        words ([ADR-256 (check answers against what really happened)](../adr/ADR-256-check-answers-against-what-really-happened.md)):
        files changed, programs run, tests and checks run (passed or failed), and pull requests
        opened, from Plenipo's own tools, each AI tool's own steps, and the working copies.
  - [x] A plain check runs first:
    - "says tests passed, but no test ran"
    - "names a file that didn't change" (on screen: "names a file it didn't change:
      src/app.ts"; a file any recorded step names counts as touched)
    - "says it opened a pull request, but none was opened"
    - "a review with no verdict" (reviewers, QA, security auditors, and workers serving a team
      through oversight; their worker record now says `verdict`)
  - [x] When a check fails, the answer goes **back to the worker** with the reason, once. If it
        fails again, it goes to the supervisor marked "doesn't match the record". On screen: a
        handoff's reply says "Doesn't match the record", and the Ledger says "Answer sent back to
        check".
  - [x] Supervisors and managers are told to compare words with facts before passing work up
        (in every message that gives them their team's replies).
- **Tests:** each mismatch is caught and sent back. A true answer passes.
  - [x] liaison `facts` tests (each check, the honest sentences that are not claims, a true
        answer), `context` `replies_carry_plenipos_record_and_a_mismatch_is_named`, and handoffs
        `an_answer_that_doesnt_match_the_record_is_sent_back_once` (sent back once with the
        reasons, marked on its second answer; a true answer goes straight up)
  - [x] desktop `Handoffs.test.tsx`, `format.test.ts`

### 4.8 Catch made-up answers, step 2 — M–L

- **Do:**
  - [ ] Check that links and pull requests named in an answer really exist, through Guard's
        outbound rules.
  - [ ] Supervisors can **stop** a worker and **send work back**, recorded like everything else.
        Workers still never control each other.
  - [ ] A notice to you when a worker's answers keep failing the check. Repeat failures show on
        its Experience.
- **Tests:**
  - [ ] a fake link is caught
  - [ ] a supervisor's stop is recorded
  - [ ] the notice fires after repeats

---

## Before each push (every wave)

- `pnpm check`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `pnpm bindings`, then no diff in `packages/types/src/generated`

## Acceptance (the owner's check, at the end of each wave)

- **Wave 1:**
  - A new organization opens without freezing.
  - Claude Code's and Codex's usage show.
  - A saved key or a signed-in subscription turns the light green.
  - Grok's subscription is used for a manager.
  - Copilot can be the code reviewer.
  - "What's stuck" opens the stuck thing.
  - Watch on a supervisor shows its team's changes.
- **Wave 2:**
  - A brand-new install, using only the setup tour, gets from the first launch to a running
    objective, with Fable for the developer and Sonnet for the docs.
  - The project reuses the workers already hired.
- **Wave 3:**
  - You watch a worker type live.
  - You stop one worker from its tile.
  - **Stop all** from the canvas stops everything, and Allow again resumes.
  - You ask a busy supervisor a question while it waits.
- **Wave 4:**
  - Over one real week, no plan runs out before its reset.
  - Every step down shows and is recorded.
  - A worker's false "tests passed" is caught and sent back.
  - The Usage tab shows what caching saved.

Each wave ends with an acceptance report in `docs/phases/`.
