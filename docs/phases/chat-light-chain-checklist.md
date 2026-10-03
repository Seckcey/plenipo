# A live chat with each agent, light by default, and the chain of command — Checklist

**Status:** delivered in v1.23.0 ([pull request #165](https://github.com/Seckcey/plenipo/pull/165)).
See the [acceptance report](chat-light-chain-acceptance-report.md). The owner's check on Windows is
at the end.

On 2026-10-03 the owner asked for:

- **A live chat with each agent**, opened from the organization map, that works "exactly like
  Claude Code, ChatGPT, or claude.ai": streaming the agent's thoughts and work, never a bare
  "thinking", with its own window per agent.
- **Lighter permissions by default.** "We need to be LIGHT on restrictive permissions and let the
  user disable it if they want … We should be enabling saving of files and running programs … Put
  the warnings in the settings." An agent had answered: "This session cannot save files or run
  programs."
- **Speed.** A request took about three minutes, where another app answered in under a second.
- **Where the output went.** "I don't know where an agent's output went."
- **The chain of command.** Orders go down and reports go up one level at a time — developer to
  supervisor, supervisor to manager or VP — and each level tracks it; and research on how a chain of
  command reports.

These are not in the rollout plan. They are recorded in three decision records:

- **ADR-200 (a live chat with each agent, as in Claude Code)**
- **ADR-201 (light by default: agents save files and run programs; Settings → Safety)**
- **ADR-202 (the chain of command: skipped leads are told; reports come back up one level at a
  time)**

## Light by default (ADR-201)

- [x] Guard: `Safety` (Light, the default and what an older settings file reads as; Careful;
      Strict), `set_safety` with event `guard.safety_changed`, and `safety_cap` where the switches
      already narrow permissions
- [x] Light: a program on no list runs without asking; Careful asks for it and for every
      PowerShell script; Strict blocks saving, programs, scripts, and git and GitHub writes
- [x] What no choice changes: blocked files and secrets, the Never run list, files outside the
      agent's folder, and every sensitive kind (sending work out, such as `git push`, still asks)
- [x] VP, Manager, and Supervisor start with **Everyday work**; a Supervisor left on Read only,
      with no choice of the owner's, moves to it once (`guard.leaders_lightened`); the Developer
      set no longer asks before a script
- [x] Work with no project folder is done in `<Documents>/Plenipo/<organization>/<position or
project>`; folder names cleaned, never a Windows device name
- [x] The agent is told its folder, to say where it saved a file, and (on Light) that programs
      run at once
- [x] **Settings → Safety**: the three choices, what each means, the warning for the one on, and
      what always asks
- [x] **Where did it save?** `get_work_folder` and `open_work_folder` find the folder in
      Plenipo's own record of the task; the page names only the task; only a folder is opened
- [x] IPC tests: `set_safety`, `get_work_folder`, and `open_work_folder` are the main window's
      alone; bad task IDs are refused

## A live chat with each agent (ADR-200)

- [x] Runtime: a live `Status` event (Claude Code's `api_retry` and `status: "requesting"`),
      Claude's thinking (`--thinking-display summarized` from 2.1.288), tool call IDs, and
      `usesTools`
- [x] A **Chat** panel beside Terminal and Files (`PanelId::Chat`): in a dock or its own window;
      an older saved layout gains it without losing anything
- [x] A tab for each chat (at most twelve, kept on this PC), and **Side by side** for up to four
- [x] Each answer streams word by word; what the agent is doing now shows with a timer that
      counts up; thinking and each run of tool calls are one line each that opens
- [x] A message sent while the agent works waits its turn and goes when it finishes; it can be
      taken back; **Stop** stops the work now
- [x] **Files saved** under an answer: which files, in which folder, and **Open folder**
- [x] **Tasks** beside the chat: the agent's hand-offs with where each stands; choose one to
      watch that worker's chat
- [x] Ways in: the map's Chat button (the chosen tile, and every tile at work), Chat in the
      Inspector, and Chat in the top bar
- [x] On-call workers' conversations can be watched, not messaged; an AI tool that writes answers
      only says so, with where to change it
- [x] Agents' words drawn by Plenipo's own Markdown reader as React elements, never as HTML; a
      link copies its address
- [x] Accessible: a log a screen reader can move through, announcements only when an answer
      starts or ends, and no motion when "reduce motion" is on

## The chain of command (ADR-202)

- [x] `chain.order` on the task an order started: who it is for, the lead whose conversation
      took it, the leads it went past (nearest first), and the owner's words
- [x] `chain.report` when the task ends: one for each lead, nearest first, each from the one
      below, with how it ended and the start of the answer; once for each task
- [x] An on-call position's order goes through its lead, which is asked to hand it on as it is
- [x] A lead's agent hears the news it has not heard with its next objective, quoted and cut
      short, once (`chain.told`)
- [x] `get_chain_orders`: a position's orders (given, passed on, or told), newest first, with
      where each stands and what came back; **Chain of command** in the chat's Tasks list
- [x] The Activity trail says each record in plain words
- [x] Workforce tests for both kinds of order; an IPC test for `get_chain_orders`

## Checks

- [x] `pnpm check`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
--locked -- -D warnings`, `cargo test --workspace --locked`, and `pnpm bindings` with no
      change in `packages/types/src/generated` (see the acceptance report for how the tests were
      run here)

## The owner's check on Windows

- [ ] Give an agent a small job ("write a script that clears my temp files") and watch it stream
      in the Chat panel: its words, what it is doing now, and the timer
- [ ] **Open folder** under the answer opens `Documents\Plenipo\…` in File Explorer
- [ ] Settings → Safety: switch to **Careful**, and a program that is on no list asks first
- [ ] Give an order straight to a Supervisor, then open its Manager's chat: **Chain of command**
      shows it, and **Reported:** when it is done
- [ ] Chat with an on-call Developer: the message goes to its Supervisor's chat, which hands it on
