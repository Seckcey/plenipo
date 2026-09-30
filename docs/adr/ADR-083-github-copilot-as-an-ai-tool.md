# ADR-083: GitHub Copilot as an AI tool — checked before every task over its two-way link

- **Status:** Proposed (2026-09-30), built at the owner's direction with the owner's three answers
  (the [Wave 2 checklist](../phases/phase-16-wave-2-checklist.md#the-owners-choices-2026-09-30)):
  "1. as recommended. Grok is cursor. 2. accept either 3. as recommended".
- **Date:** 2026-09-30
- **Phase:** 16, Wave 2 ("one AI tool, one decision record each"), built beside Phase 21
  ([ADR-090 (building Phase 21 alongside Phase 16's second wave)](ADR-090-phase-21-alongside-phase-16-wave-2.md))
- **Follows:** [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md), whose bar Copilot failed on
  its first try ([the finding](../phases/ai-tools-copilot-finding.md)) and passes now
  ([the owner's check](../phases/evidence/phase-16-wave-2/owner-check/README.md)); the
  [notes for this record](../phases/ai-tools-copilot-decision-notes.md).
- **Amends:** [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md) bar item 4 — the check
  before every task may be a short two-way talk with the tool, not only a command.
- **Adds to:** [ADR-007 (how Plenipo runs AI tools)](ADR-007-runtime-adapters.md) — two small
  parts of the adapter contract (§7 below); [ADR-082 (Antigravity as an AI tool)](ADR-082-antigravity-as-an-ai-tool.md)
  — a settings folder of its own named by the tool's own variable.

> **On screen** (ADR-010, plain words and rank names): **GitHub Copilot** (the AI tool),
> **GitHub** (its company), **Copilot sign-in**, **GitHub CLI sign-in**, **paid extra use**, and
> "Conversation only". Never "copilot", "Copilot CLI", "headless", "quota", "overage",
> "premium requests", "runtime", or "provider".

## In short

GitHub Copilot joins Plenipo as its seventh AI tool. Its first try failed two rules: nothing said
how it was signed in, and nothing stopped GitHub from charging for extra requests once the monthly
allowance ran out. Both are answered now. Before every task, Plenipo asks Copilot, over the same
two-way link GitHub's own Copilot SDK uses, how it is signed in and whether GitHub may charge for
extra use. A task runs only on Copilot's own sign-in or the GitHub CLI's, with **paid extra use
off**; then GitHub itself refuses anything past the allowance, so a task can never cost money.
Workers on Copilot answer in text only. Accepting this record means keeping Copilot as written
below, with the limits in Consequences.

## Context

- **The first try** (Copilot 1.0.88, 2026-09-26): the one-task mode passed everything but a
  sign-in check and a guard against paid extra requests. The finding named what would change the
  answer: a two-way route for the check before each task, `copilot --headless --stdio`, which
  answers `auth.getStatus` and `account.getQuota`.
- **Step 0 on the owner's PC** (Copilot 1.0.89, 2026-09-30):
  - `auth.getStatus`: `isAuthenticated: true`, `authType: "user"` after `copilot login`; in an
    empty settings folder, `authType: "gh-cli"` (the GitHub CLI's sign-in).
  - `account.getQuota`: `chat` (200), `completions` (2000), and `premium_interactions` (0), each
    with `overageAllowedWithExhaustedQuota: false`. The owner's GitHub page agrees: the budget
    for **All AI Credit SKUs** is **$0** with **Stop usage: Yes**.
  - One task with its words on standard input, JSON lines out, and a second task continuing it
    by its ID (`--resume`): both worked.
  - With its tools off, asked to make a file, the model only wrote a pretend request as text; no
    file was made.
  - `models.list`: only **Auto**. Auto chose Microsoft's `mai-code-1.1-flash`.
- **On the build machine** (1.0.89, signed out): the link answers `connect`, and says "Not
  authenticated" cleanly; each message is framed by a `Content-Length` header; closing its input
  before every answer arrived dropped answers (the first finding's evidence).
- **Pay-per-use routes:** a token in `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, or `GITHUB_TOKEN`; a
  custom model provider (`COPILOT_PROVIDER_*` variables, "BYOK"). Plenipo passes none of them.

## Decision

### 1. The AI tool

- **GitHub Copilot**, company **GitHub** (`github`), program `copilot`. Plenipo looks for it on
  the PATH, in WinGet's links folder (`winget install GitHub.Copilot`), and inside npm's global
  package, where it runs the real `copilot.exe`, never npm's shim.
- It comes after Antigravity in the AI tools' order.

### 2. The check before every task

- Plenipo starts `copilot --headless --stdio --no-auto-update --log-level none`, sends
  `connect`, `auth.getStatus`, and `account.getQuota`, and **keeps its input open until each has
  an answer** (or the time runs out), then closes it.
- **Signed in:**
  - `user` → **Copilot sign-in**; `gh-cli` → **GitHub CLI sign-in**. Both count (the owner's
    choice 2: "accept either").
  - `env` or `token` (a token in a variable), `api-key`, `hmac` → a key or token, **never used**.
  - anything else → not recognized, never used.
  - not signed in → signed out.
  - no answer, an error, or a check that did not start → not known, never used.
- **Paid extra use:** every allowance `account.getQuota` lists must say
  `overageAllowedWithExhaustedQuota: false`. If any says `true`, or does not say, or no allowance
  is listed, **no task runs**, and Copilot's card says, in plain words, to set GitHub's budget for
  AI Credits to $0 with Stop usage on, then choose Check again.
- The account name the link reports is never kept.
- Its task stream never names the sign-in, so only this check lets a task run (as Codex, Grok, and
  Antigravity).

### 3. One task

- `copilot --output-format json --no-auto-update --available-tools=plenipo_no_tools
--disable-builtin-mcps --no-ask-user --no-custom-instructions`, plus `--model=<name>` when a
  model is set, and `--session-id=<ID Plenipo chose>` for a new conversation or
  `--resume=<ID>` to continue one. The task's words go in on standard input, never on the
  command line.
- The answer streams as it is written (`assistant.message_delta`); `assistant.message` gives the
  answer, `session.auto_mode_resolved` the model Auto chose, and `result` the conversation ID and
  whether it succeeded.
- **Errors by their kind**, not their wording (`session.error`'s `errorType`): `quota` and
  `rate_limit` → a usage limit (the Router's rules then apply: wait, or the next choice);
  `authentication` → sign in again; anything else → failed.
- Plenipo never sets Copilot's "switch to Auto after a rate limit" (`continueOnAutoMode`).

### 4. Least privilege

- **A settings folder of its own:** every process of Copilot (the check, tasks, sign-in, update)
  runs with `COPILOT_HOME` pointed at a folder Plenipo keeps in its app data
  (`runtime\ai-tool-homes\copilot`). None of the owner's own Copilot settings, hooks, add-ons,
  agents, or skills apply. The home folder stays the owner's, so the GitHub CLI's sign-in still
  works.
- **Its own tools off:** only a tool that does not exist is allowed (`--available-tools`), so the
  model is offered none; GitHub's own add-on server is off; the folder's instruction files
  (`AGENTS.md`) are not read.
- **Checked in every task:**
  - one of its own tools that finishes, or fails for any reason other than "does not exist" or a
    refused permission, **stops the task**; a refused one is shown and the task goes on;
  - a model billed per use (`isByok: true` in any event) **stops the task**.
- **Workers on Copilot are conversation only** (`accepts_tools: false`), the owner's choice 3.
- **Self-updates off in tasks and checks:** `COPILOT_AUTO_UPDATE=false` and `--no-auto-update`
  ([ADR-059 (Plenipo keeps the AI tools up to date)](ADR-059-plenipo-updates-the-ai-tools.md):
  between tasks).

### 5. Models, makers, and effort

- **No model is listed.** The owner's plan offers only Auto, which is Copilot's default ("Its
  default"). Auto picks the model itself, so **who made it is not known**, and it plays safe
  ([ADR-081 (who made each model)](ADR-081-who-made-each-model.md) §7): work done on Copilot can
  never be reviewed by a worker that **must** come from a different company, a review that must
  come from a different company never picks Copilot's default, and a role with AI companies never
  to use skips it.
- The model Auto chose is recorded with each task, as the AI tool reported it; like every model a
  tool reports, it never says who made the model (ADR-081, as built).
- Copilot's model list (`models.list`) shows on its card's **Models** tab as "new, not checked
  yet". A plan that offers more models gets them listed, with who made each, when that plan is
  checked.
- **No effort setting** yet: none was checked on Auto.

### 6. The AI tools page (ADR-058 to ADR-060)

- **Sign in** opens a terminal tab that runs `copilot login` (its browser sign-in), with
  Plenipo's settings folder for it. Copilot has no sign-out command.
- **Update** runs `copilot update`; its newest version comes from GitHub's npm package
  `@github/copilot` (added to Guard's short list of release addresses). Installed with npm, it
  cannot update itself, and the card says what to type.
- **Models and plan left** come from the same link (`models.list`, `account.getQuota`), with no
  conversation and no prompt.

### 7. Adding to the adapter contract

- **`RuntimeAdapter::auth_talk`**: the check before every task as a short talk. Default: none, so
  every other tool keeps its status command.
- **`Framing::Headers`**: a talk whose messages are framed by `Content-Length` headers. Talks
  framed one message per line (Codex, Grok, Kimi) are unchanged.
- **`RuntimeAdapter::home_variable`**: a settings folder of its own named by the tool's own
  variable instead of its home folder. Default: none.

## Consequences

- The owner can use GitHub Copilot through a Copilot plan, or the GitHub CLI's sign-in, without a
  token or key, and it can never cost money: with paid extra use off, GitHub refuses a request
  past the allowance, and Plenipo sees that as a usage limit.
- **Known limit:** the check runs before each task, not during it. If the owner turns paid extra
  use on in GitHub while a task is running, that task finishes; the next one does not start.
- **Known limit:** with only Auto on the owner's plan, Copilot's work never counts as another
  company's in cross-company review.
- **Known limit:** Copilot workers cannot touch files or programs, or open web pages. A role that
  needs them should use another AI tool until Plenipo's tools are wired in for Copilot.
- The link is what GitHub's official Copilot SDK uses, but GitHub marks its calls "experimental".
  If a later Copilot version renames them, the check answers "not known" and no task runs until
  Plenipo is updated; nothing is silently allowed.
- Signing out has to be done in Copilot itself (`/logout`), or with `gh auth logout` for the
  GitHub CLI's sign-in.

## Alternatives considered

- **Keep the finding.** The link answers both questions the finding asked; waiting would leave
  Copilot out for no reason.
- **Run tasks over the link too** (the SDK's `session.create`). More to build and check; the
  one-task mode already passes the bar.
- **Copilot's ACP mode (`--acp`).** It has no call for the sign-in type or the allowance.
- **Trusting GitHub's budget setting without checking it.** Rejected in the first finding;
  `account.getQuota` now checks it before every task.
- **`--max-ai-credits`.** A soft cap on all use, included or paid; not a stop at the allowance.
- **Only Copilot's own sign-in.** The owner chose to accept the GitHub CLI's too (choice 2); both
  are the owner's own Copilot plan.
- **Plenipo's own tools for Copilot workers now.** The owner chose conversation only (choice 3); a
  later step, checked on the owner's PC.
