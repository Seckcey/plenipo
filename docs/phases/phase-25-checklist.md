# Phase 25 — Implementation Checklist

**Status: started 2026-10-03, beside Phases 23 and 24; Wave 1 in progress.** The owner answered
ADR-190's six questions the same day ([the owner's answers](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). Builds on v1.20.0. Below, "[x]"
is done.
Plenipo is made by 8 West Ventures, LLC.

Source: the owner's two lists of 2026-10-03, and
[ADR-190 (Phase 25 starts: fixes and a simpler Plenipo before launch)](../adr/ADR-190-phase-25-starts.md).

**Numbers:** ADR-190 to ADR-199. Phase 24 is already using numbers in the 170s, so Phase 25
starts at 190 to keep clear of it. Dates are Pacific time. Screen words follow
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
  - [ ] Each card starts closed. Its header shows the light from 1.3, one line like "Subscription
        connected · Claude Max", and a **Sign in** (or **Reconnect**) button.
  - [ ] A card opens by itself when it needs you, or when a link points at it. Plenipo remembers
        which cards you opened.
  - [ ] Uses the shared open/close part from 2.2.
  - [ ] From 1.3: fold the company key cards (Anthropic, OpenAI, xAI, Moonshot, Google) into their
        subscription card; OpenRouter and the companies with no subscription keep their own.
  - [ ] From 1.2: the subscription card shows its key's usage as a second line ("with your key").
- **Tests:** `aiTools.test.tsx` (tests open the card first).

### 2.2 Connections cards start closed — M

- **You said (first list):** "Make the connections tab easier to use and make each card
  collapsible. They should all start collapsed."
- **Why:** Connections is one long page. It has 6 services, about 19 on/off controls, about 18 text
  boxes, and the same safety text repeated on every card (`settings/connections/`).
  `packages/ui` has no open/close part.
- **Do:**
  - [ ] A shared **Disclosure** part in `packages/ui`, with a Gallery entry and a test, used by 2.1
        and here.
  - [ ] Each Connections card and add-on card starts closed and shows one line: connected or not,
        which account, "3 of 5 parts on", and "2 people may use it". A card that needs you starts
        open.
  - [ ] Show the repeated safety text once at the top. Each card keeps a short "Learn more" link.
- **Tests:** `connections.test.tsx` (29 tests open the card first).

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
  - [ ] Wherever you pick a model for a role, department, or organization, show **every model of
        every AI tool**, grouped by AI tool, subscriptions first. Picking one adds it to Your models
        by itself.
  - [ ] Rename the "(default model)" entries to "Claude Code: its own choice".
- **Tests:** choose Fable for Senior Developer and Sonnet for Documentation Writer in one screen.
  Each role then uses its model.

### 2.6 One "who uses what" screen — M

- **You said:** "Plenipo is WAY too complicated."
- **Why:** picking Fable for one role takes 3 screens and 4 forms today. Model rules live at seven
  levels, and "never use" lists add up across four of them on three screens. Some settings (cost
  class, context size, image boxes) do nothing until filled in.
- **Do:**
  - [ ] Settings → AI models becomes one table: **each role, its model, its backup, its effort**,
        plus one row each for "the whole organization" and each department. Advanced rules stay
        under **More**.
  - [ ] "Never use" lists move to the organization level only. Existing lists at other levels are
        kept and shown under More, each with a Remove button.
  - [ ] Cost class moves under More.
  - [ ] Every place that shows a model choice says where it came from ("from Senior Developer's
        choice", "from the organization").
- **Tests:** router tests unchanged (rules still apply); screen tests for the table.

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
  - [ ] **No copies at setup.** For each job the team needs, use a matching worker already in the
        department, and hire only what's missing. The setup dialog shows "Use Alex (Senior
        Developer, Claude Fable)" or "Hire new" for each job.
  - [ ] **Share across the department.** A supervisor looks in its own team first, then at the
        department's other workers. On-call workers keep the AI tool and model you chose for them.
        A busy full-time worker's task waits in line. The work is done under the asking project's
        folder and limits.
  - [ ] **Bring in one only when none exists.** If the department has no one for the job, Plenipo
        asks you first: "The Website supervisor needs a QA Engineer. Hire one?" A switch in Settings →
        Switches lets it hire on its own ([answer 3](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). Free's limits still apply.
  - [ ] The supervisor's and manager's instructions say: "Use the people you have first."
  - [ ] **Change ADR-016 (the Development department) and ADR-054 (move or lend an agent).**
- **Tests:**
  - [ ] setup with existing workers hires no copies
  - [ ] a supervisor's hand-off reaches a department worker outside its team
  - [ ] a missing job asks before hiring
  - [ ] the fan-out to developer, reviewer, and QA still happens

### 2.8 Templates for organizations, projects, businesses, and enterprises — M

- **You said (first list):** "Add templates for organizations, projects, businesses and
  enterprises."
- **Why:** the plumbing is already there and waiting. "Use a template" is already in the
  new-organization dialog, greyed out, and the code says "Templates are coming later." One team
  template exists (Development), and `set_up_team` can apply any team template.
- **Do:**
  - [ ] **Templates:**
    - **Small business:** Operations and Marketing.
    - **Software project:** Development (after 2.7, it reuses existing workers).
    - **Agency:** Development, Design, and Marketing.
    - **Enterprise:** several departments, each with a manager and a supervisor.
    - **IT services:** Operations and Documentation.
    - Each template sets starting models, permissions, and rank names.
  - [ ] A template picker in the new-organization, new-project, and new-department dialogs.
  - [ ] Templates with more than one department need Pro (Free keeps one department).
  - [ ] **Save my organization as a template** (setup only: no keys, no files, no history), and use
        it for a new organization. It reuses the setup copy in `ledger/src/workforce/copy.rs`.
  - [ ] This carries out part of parked Phase 15 (department templates). Phase 15 itself stays
        parked.
- **Tests:** each template applies cleanly, and on Free the Pro templates are locked.

### 2.9 A real setup tour, with Driver.js — M–L

- **You said:** "Need a MUCH better tutorial for new organization setup and initial sign up that
  walks the user through setting up AI tools subscriptions, adding a department and setting up
  their first project and then assigning models to the hired tiles. Use the Driver.js library."
- **Why:** the only tour is a 6-card tour of the canvas, with no highlighting
  (`org/tour.ts`). There's no first-run flow, and nothing checks that an AI tool is signed in, so a
  new user can build a whole organization before learning no worker can run.
- **Do:**
  - [ ] **Change ADR-030 (one design system):** allow Driver.js (MIT license) for the tour. It
        works with Plenipo's security settings as they are, with no change.
  - [ ] **The tour, step by step, moving on when the step is really done:**
    1. Welcome.
    2. Open the AI tools page and sign in to at least one subscription (waits for a green light).
    3. Name your organization, or pick a template (2.8).
    4. Add a department.
    5. Set up the first project.
    6. Hire the team, reusing workers (2.7).
    7. Choose a model for each tile (2.5).
    8. Give the first objective.
    9. Watch it work (3.1).
  - [ ] It moves between pages by itself, waits for each part of the screen to appear, and brings
        canvas tiles into view.
  - [ ] It pauses while a dialog is open, remembers where you stopped, and works per
        organization.
  - [ ] **Take the setup tour again** in Settings and on Home. The old canvas tour waits while it
        runs.
  - [ ] Colors use Plenipo's theme, in light and dark.
  - [ ] Steady anchors: add `data-tour` marks to the sidebar buttons, + Department, + Project, the
        Add menu, and the dialogs.
- **Tests:**
  - [ ] unit tests for each step's "done" check
  - [ ] one full run in the end-to-end suite

---

## Wave 3 — See and steer the work

About 5 to 7 build sessions.

### 3.1 Watch the worker think and type, live — M–L

- **You said:** "We need the output to output like regular chatbots in the browser or in their IDE.
  The output streams in real time. Plenipo just says 'Working' and that's it. We don't know what
  it's working on or how far along it is."
- **Why:** live text already arrives from Claude Code, Grok, and Kimi (`TextDelta`). It is shown
  only on the Workers page, in "Live activity". Everywhere else (Task page, Worker page, canvas
  tile, Inspector) shows just "Working". Codex sends whole messages only, no live text. Tool use is
  shown late, and plans ("step 3 of 7") are thrown away.
- **Do:**
  - [ ] One **Live conversation** part, chat-style:
    - the worker's words appear as it types them
    - its steps in plain words: "Reading index.ts", "Writing app.tsx", "Running `npm test`",
      "Asked the Code Reviewer…"
    - a progress line: step 3 of 7, 4 minutes, 12 steps, tokens so far
  - [ ] Put it in the Watch tab (next to the file changes), the Task page, the Worker page, the
        Inspector (last 3 lines), and a one-line "now: Running npm test" on canvas tiles.
  - [ ] From 1.8: the Watch button on the Task page, Home's "Who's working" rows, List mode, and
        the worker popup.
  - [ ] Read plans and progress from every AI tool that sends them: Grok and Kimi plans, Codex's
        to-do list, and Claude Code's tool calls as soon as they start.
  - [ ] Codex live text needs its other connection method (app-server). That's a stretch goal,
        built last.
- **Tests:** the store joins live text correctly. Each tool's plan becomes progress.

### 3.2 Watch shows changes made by commands too — M–L

- **You said:** "The watch doesn't seem to let the user watch the agent code."
- **Why:** Watch only sees files written through Plenipo's own file tools. Files changed by a
  command (`npm create`, `sed`, a git step) don't show (ADR-055, watching a worker write code).
- **Do:**
  - [ ] After each command or git step, compare the working copy before and after. Show what
        changed as "made by a command". Files Guard keeps private are skipped.
  - [ ] **Change ADR-055 (Watch).**
- **Tests:** a file made by a command shows in Watch. A private file never does.

### 3.3 A Stop button on every worker — S–M

- **You said:** "We need a stop button on all agents popups."
- **Why:** Stop exists only on the Workers page ("Cancel task"), in Watch (only when a change is on
  screen), in the file editor, and on the phone. It is missing from canvas tiles, the Inspector, the
  worker popup, the Worker page, the Task page, and Home.
- **Do:**
  - [ ] One **Stop** part (asks "Stop Alex's task?" first), on every one of those places.
  - [ ] Stopping a full-time worker stops only its current task, not its whole conversation.
- **Tests:** Stop from each place ends the task, and the parent sees it.

### 3.4 Stop all: one red button on every page that shows work — M–L

- **You said:** "A stop all emergency button on all pages that monitor tasks. Especially on the org
  canvas."
- **Why:** today's "Stop all" only stops browser, screen, and server control. It does not stop the
  AI work, and it shows only while control is on. The tray's "Stop all programs" stops programs,
  but new work keeps getting handed out.
- **Do:**
  - [ ] A new **Stop all work**. In every organization it:
    - stops browser, screen, and server control
    - stops every running task
    - holds all waiting work until you press **Allow again**
  - [ ] Written in the Ledger.
  - [ ] A red button in the top bar of every page that shows work (Home, Organization, Projects,
        Workers, Task, Worker), and on the canvas toolbar.
  - [ ] The phone's Stop all and the tray's Stop all do the same.
- **Tests:**
  - [ ] after Stop all, nothing runs and nothing new starts
  - [ ] Allow again resumes

### 3.5 Side chats with any manager or supervisor — M–L

- **You said:** "We need a way to open side chats to ask each agent a question if I don't want to
  go through the chain of command. Mainly with managers and supervisors while they wait for workers
  to complete their tasks."
- **Why:** the only way to talk to a full-time agent is to give it an objective. A busy agent,
  including one waiting on its workers, says no. The Workers page refuses to continue a member's
  conversation.
- **Do:**
  - [ ] **Ask a question** on the Inspector and the Worker page. It works while the agent is busy
        or waiting.
  - [ ] The side chat starts from a **copy** of the agent's conversation, so it knows what's
        going on, plus a short "status now". Its real work is never touched.
    - Claude Code: `--fork-session`.
    - Paid keys and Ollama: Plenipo copies its own saved conversation.
    - Codex, Grok, and Kimi: their fork, once checked.
    - Otherwise: a fresh conversation with a short briefing.
  - [ ] **Answer only:** no tools and no hand-offs
        ([answer 4](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)). It still counts
        toward your plan's usage and Free's three-at-once limit.
  - [ ] Side chats are listed in Workers as "Side chat with Alex".
- **Tests:**
  - [ ] a side chat during a running task leaves the task alone
  - [ ] it has no tools
  - [ ] its hand-off blocks are ignored

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
  - [ ] **Change ADR-085 §3.5:** caching is allowed for Anthropic models, on the direct key and
        through OpenRouter.
  - [ ] Mark the reusable start of each request (instructions and earlier turns) for caching.
  - [ ] Count the cost right: writing to the cache costs a little more, and reading from it costs
        much less. The Anthropic price rows get a cache-write price, and the spending gate sets
        aside the most a step could cost.
  - [ ] Show "saved by caching" on the Usage tab.
- **Tests:**
  - [ ] the request carries the cache markers
  - [ ] the cost math matches Anthropic's prices, checked on the owner's PC per ADR-081 §8

### 4.2 When a plan runs out: say what you can do, and pick the work back up — S–M

- **You said (first list):** "Suggest using a subscription reset when model usage limit is
  reached." And later: "OpenAI and Claude offer full resets that they give out every once in a
  while. I have one for each right now. If you can't track it then don't worry about it."
- **Why:** when a limit hits, the task **fails**. You get a general notice and **Try again now**.
  Nothing restarts the work after the reset.
- **Do:**
  - [ ] The notice says: "Claude Code is out until 3:00 PM. You can:
    - wait (Plenipo picks the work back up at 3:00)
    - use a usage reset, if Anthropic gave you one
    - use your Anthropic key or another AI tool"
  - [ ] Each choice is a button. "Use a reset" opens the company's own page in your browser.
        **Plenipo never uses a reset or buys anything for you.**
  - [ ] Check whether Claude Code or Codex reports a waiting reset. If one does, show "You have a
        reset waiting" on its card. If not, only remind, and never guess.
  - [ ] Work stopped by a limit **restarts by itself** after the reset, unless you said Leave
        stopped.
- **Tests:**
  - [ ] the notice's choices
  - [ ] work restarts after the reset time
  - [ ] Leave stopped is respected

### 4.3 Better plan numbers — S–M

- **Why:**
  - Plenipo can't tell Claude Code's 5-hour limit from its weekly one (it doesn't read
    `rateLimitType`).
  - Codex's real reset time isn't used. Plenipo waits a flat hour instead.
  - OpenRouter's key limit is fetched and then thrown away.
  - Grok, Kimi, Ollama, and Antigravity report no plan numbers at all. Plenipo only uses what each
    AI tool reports (ADR-060), so those show "not reported".
- **Do:**
  - [ ] Label each window: "5-hour: 62% used, resets 3:00 PM" and "Week: 40% used, resets
        Monday".
  - [ ] Use the reported reset time for every hold.
  - [ ] Show OpenRouter's key limit and balance.
  - [ ] From 1.2: tell the open page when a background plan check saves a new report.
- **Tests:** each tool's report is read correctly.

### 4.4 Your subscription first, then the same company's key — M

- **You said:** "Plenipo is not using subscription tokens before API tokens like it's supposed to."
- **Why:** choosing an AI tool pins one tool, with no backup. Plenipo already knows when two tools
  run the same model (`KnownModel.same`), but routing never uses it. Claude Code and the Anthropic
  key aren't linked at all.
- **Do:**
  - [ ] Choosing a model (for example "Claude Sonnet") means: use it on your **subscription** first,
        and when the plan runs out, on the **same company's key**. The key is used only if paid
        keys are on, a key is saved, and the spending cap has room.
  - [ ] Link the same models across Claude Code and the Anthropic key, and across Codex and the
        OpenAI key.
  - [ ] **Change ADR-085 §6.**
  - [ ] Every switch is shown on the worker and the card, and written in the Ledger. Plan rule
        §3.2, "no silent provider switching", still holds.
- **Tests:**
  - [ ] a subscription limit moves the same model onto the key
  - [ ] it never does when paid keys are off or the cap is full

### 4.5 Step down instead of stopping — M

- **You said (first list):** "Manage your token use through effort levels and model downgrading."
- **Why:** effort already reaches every AI tool that has it. Nothing lowers it, or moves to a
  smaller model, when a plan runs low.
- **Do:**
  - [ ] A step-down ladder for each role:
    1. lower the effort
    2. use a smaller model from the same company (Fable → Opus → Sonnet → Haiku)
    3. use the same model on your key (4.4)
    4. wait for the reset
  - [ ] Reviewers and anything you pinned by name never step down without asking.
  - [ ] It starts when a plan passes a line you set (default 80% used). Every step is shown and
        written in the Ledger.
  - [ ] **On by default**, with a switch in Settings → Switches to turn it off
        ([answer 6](../adr/ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03)).
- **Tests:** each rung of the ladder. Pinned and reviewer roles hold their model.

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
  - [ ] Every answer handed back up the chain carries **Plenipo's facts** under the worker's
        words.
  - [ ] A plain check runs first:
    - "says tests passed, but no test ran"
    - "names a file that didn't change"
    - "says it opened a pull request, but none was opened"
    - "a review with no verdict"
  - [ ] When a check fails, the answer goes **back to the worker** with the reason, once. If it
        fails again, it goes to the supervisor marked "doesn't match the record".
  - [ ] Supervisors and managers are told to compare words with facts before passing work up.
- **Tests:** each mismatch is caught and sent back. A true answer passes.

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
