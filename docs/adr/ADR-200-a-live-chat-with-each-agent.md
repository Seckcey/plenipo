# ADR-200: A live chat with each agent, as in Claude Code

- **Status:** Proposed. The direction is the owner's own, 2026-10-03: "a live chat with each agent
  from the org canvas", "chat windows per agent that work exactly like Claude Code, ChatGPT, or
  claude.ai, streaming the agent's thoughts and work", "not a bare 'thinking'", and "I don't know
  where an agent's output went". The builder's choices below are for the owner to accept at review.
- **Date:** 2026-10-03
- **Phase:** none (the owner's direction, built beside Phases 23, 24, and 25)
- **Number:** ADR-200 to ADR-209 are this work's block (see ADR-201).
- **Touches:** [ADR-007 (one AI tool process for each step)](ADR-007-runtime-adapters.md)
  (what the chat shows while a step starts), [ADR-092 (the workspace: docks and pop-outs)](ADR-092-panels-and-windows.md)
  (a third panel), and [ADR-131 (tools for text-only AI tools, parked)](ADR-131-tools-for-any-model.md)
  (the chat says plainly when an AI tool can only answer in words).
- **Overlaps:** Phase 25's items 3.1 (Live conversation) and 3.5 (Side chats), planned on another
  branch. This record builds a version of both; whoever builds Phase 25 builds on it instead of
  making a second one.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): **Chat** in the top bar and on the map,
> **Side by side**, **Tasks**, **Stop**, **Files saved**, and **Open folder**. A conversation is a
> "chat" or a "conversation", never a "session". This record keeps the code's words (`ChatPanel`,
> `AgentEvent::Status`, `PanelId::Chat`).

## In short

You wanted to talk to each agent the way you talk to Claude Code: type, press Enter, and watch it
answer as it writes, see what it is doing, and know where its files went. Before this, the answer
came all at once, after a long wait that only said "thinking", and only the Workers page showed
anything live.

Now there is a **Chat** panel. It sits at the side of the window (or in its own window), with a
tab for each agent. In each chat:

- The answer appears **word by word**, as the agent writes it.
- What the agent is doing **right now** shows under it, with a **timer that counts up**: "Saving a
  file · 12 s", "Running a program", or "Claude's servers are busy. Trying again in 4 s (try 2 of
  10)". No more bare "thinking".
- Claude's **thinking** shows as one line ("Thought for 6 s") that you can open.
- Each run of tool calls is **one line** ("Saved plan.md, ran 2 programs") that you can open to see
  every step.
- A **Files saved** card under the answer says which files it saved, **where they are**, and has an
  **Open folder** button.
- If you write while the agent is busy, your message **waits its turn** and goes by itself when the
  agent finishes. **Stop** stops the work now.
- Beside the chat, **Tasks** lists the work the agent handed to its team, and where each piece
  stands. Choose one to watch that worker's own chat.
- **Side by side** shows up to four chats at once, each streaming as it works.

Open a chat from the **map** (the Chat button on the agent you chose, and on every agent at work),
from the **Inspector** ("Chat"), or from the **Chat** button in the top bar.

Accepting this record means keeping the chat as built. It does **not** make the AI tools start
faster: each step still starts its own AI tool program (ADR-007). The chat now shows that wait,
and why, instead of hiding it.

## Context

What the code did at v1.21.0 (read at `47228bb`):

- **One AI tool program for each step** (ADR-007). Claude Code runs as
  `claude -p --output-format stream-json --verbose --include-partial-messages …`, a new program for
  every step. Before it answers, Plenipo checks its sign-in and opens the agent's permissions (for
  a project, a working copy with `git worktree add`).
- **The live events already existed.** The runtime sent each piece of an answer to the window
  (`plenipo://agents`, `AgentUpdate::Activity`), but only the Workers page drew them, as a list.
- **Claude's thinking was dropped**, and its `system` lines were ignored. One of them,
  `system/api_retry`, is Claude Code retrying a busy or failing request, up to ten times with growing
  waits. That is the most likely reason one request took about three minutes while another app
  answered in under a second: nothing on screen said "retrying".
- **A busy agent refused** a new objective ("A turn is already running in this session"). There was
  no waiting line.
- **You could not tell where a file went.** Work with no project had no folder at all (ADR-201),
  and nothing on screen named the folder of the work that did.

The research behind this record (2026-10-03) compared Claude Code's own web view (the owner's screen
recording), the claude.ai and ChatGPT patterns, and what the installed Claude Code 2.1.288 really
sends (`api_retry`'s fields, `status: "requesting"`, and `--thinking-display`).

## Decision

### Where the chat lives

1. **A third workspace panel, Chat** (`PanelId::Chat`), beside Terminal and Files: in the right dock
   to start, movable to any dock, and able to pop out into its own window, like the others
   (ADR-092). A window layout kept from before this version gets the new panel in its starting place
   and keeps everything else where you put it.
2. **A tab for each chat**, kept on this PC (at most twelve; opening a thirteenth closes the one in
   front longest ago). A tab is a full-time agent's position (you talk to its agent) or a
   conversation (you watch an on-call worker).
3. **Side by side** shows up to four chats in a grid.
4. **Ways in:** a Chat button on the chosen tile on the map and on each tile at work; Chat in the
   Inspector; the Chat button in the top bar (it says how many agents in the open chats are working).

### What a chat shows

5. **The conversation is rebuilt from what Plenipo already records**: the turn records and the live
   pieces (`AgentActivity`). Words are joined as they arrive, so a long answer never grows a list.
   Nothing new is stored.
6. **What the agent is doing now**, with a timer, from the newest piece: a tool call running, Claude
   thinking, waiting for its team, or a **status** the runtime sends while nothing else happens.
7. **A new live event, `Status`** (`waiting` or `thinking`), never stored in the Ledger. Claude Code's
   `api_retry` becomes "Claude's servers are busy. Trying again in 4 s (try 2 of 10)." and
   `status: "requesting"` becomes "Waiting for Claude".
8. **Claude's thinking:** `--thinking-display summarized` is passed when the installed Claude Code is
   2.1.288 or newer (older ones refuse the flag). Thinking pieces become `Reasoning` events.
9. **Tool calls carry their IDs**, so each result goes to its own call, even when several run at
   once.
10. **Agents' words are drawn with a small Markdown reader** of Plenipo's own, as React elements,
    never as HTML, so nothing an agent writes can run. Only the last part redraws as words arrive.
    A link never opens by itself: choosing it copies its address.
11. **Accessible:** the conversation is a log a screen reader can move through; it hears only when
    an answer starts, finishes, stops, or fails, never each word. Moving pictures stop when "reduce
    motion" is on.

### Talking to an agent

12. **Send** gives the position's agent its next objective (`give_objective`, as the Inspector
    does) or, for a conversation you started on the Workers page, continues it.
13. **A message sent while the agent is busy waits**, listed above the box with a button to take it
    back, and goes when the agent finishes. **Stop** cancels the work now; what waits keeps waiting.
14. **On-call workers can be watched, not messaged.** They take work from their lead (Liaison's
    rule since Phase 4); the chat says so and points to the lead.
15. **An AI tool that only answers in words** (Ollama, OpenRouter, a direct key, GitHub Copilot,
    Antigravity) gets a note: it cannot save files or run programs, and the Model tab is where to
    give the agent another AI tool. Giving such tools Plenipo's tools is Phase 16's parked Wave 4.

### Where the files went

16. **Files saved:** under a finished answer, the files its tool calls saved or changed, the folder
    they are in (Plenipo's own folder, the project's folder, or a working copy), and **Open folder**.
17. **Plenipo finds the folder in its own record** of the task (`guard.grant_opened`), through
    `get_work_folder` and `open_work_folder` (ADR-201). The page names only the task, never a path,
    and only a folder is ever opened, never a file, so nothing can run.

### The plan beside the chat

18. **Tasks:** the hand-offs the agent made for the message it is working on, each with where it
    stands (a dotted circle while waiting, a turning one while working, a filled one when done).
    Choosing one opens that worker's chat. It shows when the chat is wide enough, and over the chat
    on request when it is not.

## Your choices (recommended first)

- **One Chat panel with tabs and Side by side.** _Or:_ a separate window for every chat, or small
  chat bubbles over the map.
- **Keep one AI tool program for each step (ADR-007) for now, and show the wait.** _Or:_ keep one
  Claude Code program running for a whole conversation (`--input-format stream-json`), which would
  start each message faster. That changes how every AI tool runs and is its own decision.
- **A message sent while busy waits its turn.** _Or:_ refuse it, as before.

## Consequences

- The wait before the first word is the same as before, but you can see it and its reason.
- Each window's chats come from the same live updates as the Workers page, so both always agree.
- A chat that is open keeps its conversation in memory while the window is open.
- Phase 25's items 3.1 and 3.5 start from this instead of from nothing.

## Alternatives considered

- **Claude Code's own permission bypass** to make agents "just work": rejected (ADR-201).
- **Drawing agents' Markdown with a library that writes HTML:** rejected. A small reader of our own
  keeps every word as text.
- **Opening links in the browser:** not done. There is no safe "open in your browser" command yet,
  and a link an AI wrote should not open by itself.

## As built

- **Runtime** (`crates/runtime/src/agent`): `AgentEvent::Status` and `StatusPhase`; tool call IDs on
  `ToolUse` and `ToolResult`; `AgentRuntimeInfo.uses_tools`; `TurnRequest.cli_version`. Claude Code:
  `--thinking-display summarized` from 2.1.288, thinking pieces, `api_retry`, and
  `status: "requesting"` (`claude_code.rs`, with tests).
- **Workspace:** `PanelId::Chat` (`crates/core/src/workspace.rs`), the layout and its upgrade for a
  layout kept from before (`apps/desktop/src/workspace/layout.ts`), the dock button, and the
  pop-out window.
- **The chat** (`apps/desktop/src/chat/`): `ChatProvider` (live updates, the waiting line, Stop),
  `tabs.ts`, `model.ts` (the conversation from turns and live pieces), `words.ts`, `markdown.ts` and
  `Markdown.tsx`, `Transcript.tsx`, `Composer.tsx`, `PlanPanel.tsx` and `plan.ts`, `ChatWindow.tsx`,
  `ChatPanel.tsx`, and `ChatButton.tsx`; the styles under "Chat (ADR-200)" in `styles.css`; a chat
  icon in `packages/ui`.
- **Ways in:** the map's Chat button (`TopologyCanvas.tsx`, wired in `OrganizationView.tsx`) and
  Chat in the Inspector (`OverviewTab.tsx`).
- **Tests:** `chat/*.test.ts(x)` (the model, the Markdown reader, the tabs, the plan, and a whole
  chat: streaming, the waiting line, Stop, a refusal, watching an on-call worker, and closing a
  tab), the layout's upgrade, and the runtime's Claude Code tests.
