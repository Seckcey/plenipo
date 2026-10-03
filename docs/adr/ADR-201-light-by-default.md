# ADR-201: Light by default — agents save files and run programs, and the warnings live in Settings

- **Status:** Proposed. The direction is the owner's own, 2026-10-03: "We need to be LIGHT on
  restrictive permissions and let the user disable it if they want … We should be enabling saving
  of files and running programs … Put the warnings in the settings." The builder's choices below
  are for the owner to accept at review.
- **Date:** 2026-10-03
- **Phase:** none (the owner's direction, built beside Phases 23, 24, and 25)
- **Number:** ADR-200 to ADR-209 are this work's block, so no session picks the same number as
  another (`main` stands at ADR-173, and Phase 25 holds ADR-190 to ADR-199).
- **Amends:**
  [ADR-013 (Guard, the capability broker, and human approval)](ADR-013-guard-capability-broker.md)
  (what each role starts with, and when a program asks),
  [ADR-016 (the Development department)](ADR-016-development-department.md) (work that belongs to
  no project gets a folder), and
  [ADR-034 (approved programs run as the owner)](ADR-034-approved-programs-run-as-the-owner.md)
  (a program on no list runs without asking while Safety is Light).
  It adds one setting beside [ADR-023 (switches)](ADR-023-settings-switches.md).
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): **Safety** in Settings, with **Light**,
> **Careful**, and **Strict**; the permission set **Everyday work**; and a folder named for your
> organization inside **Documents → Plenipo**. This record keeps the code's words (permission set,
> `Safety`, `safety_cap`).

## In short

On 2026-10-03 you asked an agent to write a script that clears your temp files. It said: "This
session cannot save files or run programs." You wanted the opposite: agents that **can** save files
and run programs, with the warnings kept in Settings, where you can turn the dial up if you want.

The agent was right about itself. Plenipo had given it **no tools**. The VP and the Manager had no
permission set at all, so everything was blocked. The Supervisor could only read. And work that
belongs to no project had no folder, so a file had nowhere to go. Nothing told you any of this.

This record changes the starting point:

- The VP, the Manager, and the Supervisor start with a new permission set, **Everyday work**: they
  read and save files, run programs and scripts, and commit, in their own folder. The built-in
  Developer set stops asking before every PowerShell script.
- A new setting, **Settings → Safety**, has three positions. **Light** is where Plenipo starts:
  agents save files and run programs and scripts without asking you. **Careful** is how earlier
  versions behaved: a program that is on no list, and every PowerShell script, asks first.
  **Strict** lets agents only read. The warnings are written there, in plain words.
- Work with no project gets a folder you can find: **Documents → Plenipo → your organization →
  the agent's name**. The agent is told its folder, and to say where it saved a file.
- Plenipo's own safety rules are **not** in the dial. Secrets and blocked files, the Never run list,
  deleting or overwriting outside the folder, running as administrator, sending or publishing, and
  money still stop or ask, on every setting.

Accepting this record means building it as written below. One thing it does **not** change: an AI
tool that only answers in words (Ollama, OpenRouter, a direct key, GitHub Copilot, Antigravity)
still cannot use any tool. Giving every model tools is the parked Wave 4 of Phase 16 (ADR-131), and
only the owner can schedule it. The chat window (ADR-200) says so in plain words when it applies.

## Context

What the code did at v1.21.0 (read at `47228bb`), and the evidence that the agent had no tools:

- **VP and Manager: no permission set.** `crates/guard/src/defaults.rs` gave starting sets to ten
  built-in roles (`template_sets`) and left these two out. Guard blocks everything for a role with
  no set (`crates/guard/src/engine.rs`, `level_for`), so the broker offered nothing and Claude Code
  ran with `--tools ""` (`crates/runtime/src/agent/claude_code.rs`). No `guard.grant_skipped` event
  was written in that case, so the Activity trail said nothing.
- **Supervisor: Read only.** It could read files, git history, and GitHub, and nothing else. You can
  give objectives only to full-time positions (VP, Manager, Supervisor), so every agent you could
  reach was one of these.
- **No project, no folder.** The broker used a project's folder, and without one the agent got "this
  work belongs to no project, so there is no folder" (`crates/capabilities/src/broker.rs`,
  `try_open`). A task started on the Workers page has no position, so no permissions either (the
  screen already says so).
- **A program on no list always asked.** Even a Developer asked for every program that was not on
  the approved list, and for every PowerShell script (`PowershellExec` was **Ask**).
- **Text-only AI tools** cannot use Plenipo's tools at all (`accepts_tools()` is false for them).

The model's sentence is not in the repository. The model wrote it, from the note Plenipo gives an
agent with no tools ("you cannot open or change files, run programs …").

## Decision

### The dial: Safety

1. **One setting for the organization, in Guard's settings** (`safety`, event
   `guard.safety_changed`): `Light` (the default, also what an older settings file reads as),
   `Careful`, or `Strict`. It never gives an agent more than its role's permission set; it decides
   how much of that goes ahead without asking.
2. **Light:** a program that is on no list runs without asking. This is the one rule that changes in
   the decision engine: "not on your approved commands list → ask" is skipped.
3. **Careful:** how earlier versions started. A program on no list asks. `PowershellExec` is capped
   at **Ask**, whatever the role's set says.
4. **Strict:** `FilesystemWrite`, `ShellExec`, `PowershellExec`, `GitWrite`, and `GithubWrite` are
   capped at **Blocked**. Agents only read.
5. **The cap is applied where Guard already applies the switches** (`narrowed` in
   `crates/guard/src/engine.rs`, through `safety_cap`), so what Settings → Permissions shows, what a
   grant snapshots, and what a tool call is judged by all agree. A running agent's **next** call is
   judged by the new setting.
6. **What no choice changes:** blocked files and secrets; the Never run list (`rm`, `del`, `format`,
   `shutdown`, `ssh`, `curl`, the shells themselves, and the rest); the always-ask list; every
   sensitive kind (deleting or overwriting outside the folder, running as administrator, sending
   or publishing, money, sign-in, DNS, credentials, live systems); websites; and the file tools'
   confinement to the agent's own folder.

### Who starts with what

7. **A new permission set, Everyday work** (`everyday`): read and change files, run programs, run
   PowerShell scripts, read and save to git, and read GitHub. Pushing, pull requests, websites, the
   screen, and servers are not in it.
8. **The VP, the Manager, and the Supervisor start with it** (`template_sets`). Before, the first two
   had no set and the third was Read only.
9. **The built-in Developer set no longer asks before a script.** An install whose Developer set the
   owner never changed is brought up to date by the existing upgrade (`earlier_sets`); one the owner
   changed is left alone.
10. **Once, for an install from before this version:** a Supervisor still on **Read only**, whose set
    the owner never chose (no `guard.role_assigned` event for it), moves to **Everyday work**, and
    `guard.leaders_lightened` records it. A VP or Manager with no entry is seeded as for a new
    install. A role the owner decided about, even "no access", is never touched. A role the owner
    puts back to **Read only** afterwards stays there.

### A folder for work with no project

11. **Plenipo makes a folder** for an agent whose work has no project folder, at
    `<Documents>/Plenipo/<organization>/<project or position name>`, when the agent's permissions
    use files, programs, or git. The names are cleaned of marks a file name cannot hold. The agent
    works there in place: no branch, no working copy. File tools stay confined to it.
12. **The host sets the place** (`BrokerConfig.files_dir`): the operating system's Documents folder,
    or Plenipo's own data folder where there is none. Without it (tests), work with no project still
    has no folder, as before.

### What the agent is told

13. **With a folder of its own:** "Your folder is …. It is Plenipo's own folder for work that belongs
    to no project, and the owner can open it … Save the files you make for the owner here."
14. **Always, when it can save files:** "When you save a file, say its name and where you saved it, in
    plain words, so the owner can find it."
15. **Under Light:** "programs … run at once; anything risky, such as deleting outside your folder or
    running as administrator, waits for the owner's approval."

### Settings → Safety

16. **A new section after Permissions**, with the three choices, what each means, and a warning in
    plain words for the one that is on (for Light: an agent can create, change, and delete files in
    its own folder and run programs and scripts without asking; a script can do anything you can do
    on this PC; Plenipo cannot see everything inside one; what still asks; "Choose Careful if you
    want to be asked first"). Under it: what always asks, and what is never allowed.
17. **One new command, `set_safety`,** the main window's alone, refused from the sign window and from
    any web page, with an IPC test. Only the three names are accepted.

## Your choices (recommended first)

- **Light is where Plenipo starts** (your order). _Or:_ start on Careful and say so on the first run.
- **A program on no list runs without asking under Light.** _Or:_ Light asks once for each new
  program name and remembers it ("always allow"), which would add the "remember this" that
  ADR-013 decided against.
- **A Supervisor still on its starting Read only moves up once.** _Or:_ only new installs get the
  lighter start, and every older install keeps what it has until the owner changes it.

## Consequences

- **A script is as powerful as you are.** Under Light, an agent can run a script that deletes
  anything your account can delete. Guard checks the words it can see in a script and asks for the
  sensitive kinds, but it cannot see everything. This is the price of "light", and Settings → Safety
  says it. Careful puts the question back in one click.
- Agents you could only talk to before can now do work. The VP and Manager are told to "do small
  objectives yourself" and now can.
- Existing tests that assumed "a program on no list asks" now set **Careful** on purpose, and new
  tests cover **Light** and **Strict**.
- A worker's note changes, so ADR-044's short reminder is sent in full the next time.
- Phase 25's item 2.7 (use the team you hired) and this record touch the same roles; they do not
  conflict.

## Alternatives considered

- **Turn every switch on by default.** Rejected: the switches that are off protect money (paid keys,
  buying), your phone's reach into your PC, and your mouse and keyboard. None of them blocked "save
  a file".
- **Use Claude Code's own permission bypass** (`--dangerously-skip-permissions`, `bypassPermissions`).
  Rejected: it would take Guard out of the way for that AI tool, and Guard is what stops secrets,
  the Never run list, and deleting outside the folder.
- **Give agents tools by changing the AI tool's own settings.** Rejected: Plenipo's tools are the
  only tools an agent has (ADR-007 §5), so every call is checked and recorded.
- **Make text-only AI tools work now.** Not done: it is Phase 16's parked Wave 4 (ADR-131, ADR-132).
  The chat window explains why such an agent cannot save files, and how to give it another AI tool.

## As built

_Filled in when it is built (see the checklist)._
