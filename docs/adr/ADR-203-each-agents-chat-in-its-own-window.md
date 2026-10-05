# ADR-203: Each agent's chat in a window of its own

- **Status:** Proposed. The direction is the owner's own, 2026-10-04: "When you click on an agent
  in the org chart, I want to be able to pop out their session like a claude cli or a codex cli
  interface. So I can see each chat just like I'm seeing ours right now." His answers to the
  design's four questions are below.
- **Date:** 2026-10-05
- **Phase:** none (the owner's request after v1.25.0)
- **Touches:** [ADR-092 (panels and windows)](ADR-092-panels-and-windows.md) (a pop-out can now be
  one chat, not only a whole panel), [ADR-200 (a live chat with each agent)](ADR-200-a-live-chat-with-each-agent.md)
  (a chat can leave the Chat panel), and [ADR-202 (the chain of command)](ADR-202-the-chain-of-command.md)
  (unchanged: an on-call position's messages go through its lead, and a worker's handed-off
  conversation is watched, not messaged).
- **Made by:** 8 West Ventures, LLC, for Plenipo.

> **On screen** (ADR-010, plain words and rank names): **Pop out chat**, **Pop out**, **Put back in
> the Chat panel**, **Show its window**, **Put back here**, "Development Manager's chat is in its
> own window", and "From its lead". This record keeps the code's words (`PopOutTarget`,
> `ChatWindows`, `slot`).

## In short

Since v1.24.0 the **Chat** panel shows each agent's work live, like this conversation with Claude
Code, and the whole panel can pop out. Now **each chat can have a window of its own**, so you can
watch several agents at once, each in its own window, beside your work.

- **Pop out** a chat from the agent's details on the map (**Pop out chat**), from its Worker page,
  from the chat's own header in the Chat panel, or from the top of its Live conversation. Or
  **double-click** the agent on the map.
- The window is the same chat: your messages (or its lead's), its answer as it writes it, its
  thinking and its steps as short lines you can open, newest at the bottom, and its message box.
  Enter sends; Shift+Enter starts a new line.
- **Up to six** at once. A seventh opens in the Chat panel and says why.
- **Put back** in the window's header, or close the window, and the chat is back in the panel.
- After a restart, the windows open again where they were.

Accepting this record means keeping these windows as built, and filling in the earlier steps of a
chat after a restart (the second part, below).

## Context

- A chat (ADR-200) is drawn from the turns Plenipo records and the live pieces of each turn. The
  Chat panel shows one chat at a time, or four side by side, and the whole panel can pop out
  (ADR-092).
- A pop-out window is drawn by its organization's window, so it is the same part, not a copy, and
  it has no commands of its own (`capabilities/popout.json`). Until now, one pop-out was one whole
  panel.
- The runtime keeps each turn's live pieces in memory only. After a restart, a chat shows each
  message and its final answer, but not the steps in between, though the Ledger keeps them.

## The owner's answers (2026-10-04)

1. **Keep the message box** in each window: "I want to talk directly to them when I want." It
   gives the agent its next objective, as the Chat panel does.
2. **Up to six** windows at once.
3. **Pop out in the details panel, on the Worker page, on each chat, and at the top of Live
   conversation, plus a double-click on the map.** Also: Enter sends, Shift+Enter starts a new
   line (built in v1.25.0).
4. **Show the steps after a restart.**
5. **No orders straight to an on-call worker** (asked separately: "no"). The chain of command
   stands: see "Unchanged" below.

## Decision

### A chat in a window of its own

1. **A pop-out is a panel or a chat.** `PopOutTarget` is `{ kind: "panel", panel }` or
   `{ kind: "chat", slot }`, with slots 1 to 6. `prepare_pop_out`, `focus_pop_out` and
   `close_pop_out` take it, and the `plenipo://windows` notice carries it. These are the same
   commands as before, with no new ones, and still the organization's window's alone.
2. **Labels** are `popout-<key>--<window>--<n>`, the key being the panel's (`terminal`, `files`,
   `chat`) or `chat_<slot>`. A chat's window is a pop-out like any other: **no commands**. Its
   place on screen is kept by its slot (`popout-chat_3--main`), never by an agent's name. Its
   title bar names its agent ("Plenipo · Website Supervisor"): the page sends the name with the
   request, and Plenipo cleans it to one line of at most 80 characters. A title is shown, never
   kept, and never a label.
3. **The Chat keeps which chats have windows** (`popped` beside the open tabs, kept on this
   computer). A chat takes the first free slot. The workspace opens the windows one at a time,
   with the panels'. The Chat draws each one into its window (`ChatWindows`), so a window shows
   the same live chat, message box and all.
4. **Six at most.** A seventh opens in the panel, with a note: "6 chats have windows of their own
   already, so this one opened here. Put one of them back to give this one its own window."
5. **Back to the panel:**
   - Put back (in the window's header), or the window's own close button: the chat comes back
     to the panel, in front.
   - Closing a chat's tab closes its window too.
   - Reset layout (ADR-092 §12) puts every chat back.
6. **After a restart** the windows open again in their slots and places. One that cannot open
   goes back to the panel.
7. **In the panel**, a chat with a window shows where it is, with **Show its window** and **Put
   back here**. Side by side leaves it out. The panel never drops a chat that has a window to
   make room for a new tab.
8. **Ways in:** Pop out chat (details panel); Pop out chat (Worker page); the chat's header in the
   Chat panel; Pop out (top of Live conversation); a double-click on a tile that has a chat. A
   tile without one still zooms in on a double-click.
9. **Who a message is from:** a worker handed its work by its lead shows "From its lead" over each
   message.

### The earlier steps after a restart

10. When the runtime no longer holds a recent turn's live pieces (after a restart, or for a turn
    older than its 50-turn memory), `get_agent_session` fills them in from the turn's own
    Ledger rows: its messages, tool calls, tool results and notes. These are already capped
    and redacted as stored. Nothing new is stored. Built in its own pull request, after this
    one.

### Unchanged

- **Allow-first** (ADR-201): a window adds no permission and no question. Light, Careful and
  Strict work as before.
- **The chain of command** (ADR-202), exactly as in the Chat panel:
  - An on-call position's own chat still takes a message. The message goes to its lead, who hands
    it on, and the chat says so ("Sent to …").
  - Only a worker's handed-off conversation (work its lead gave it) is watched, not messaged.
- **Privacy:** a window shows only what the Chat panel shows. Live pieces are redacted by the
  runtime before they leave it, and Ledger rows were redacted before they were stored.

## Consequences

- The owner can watch several agents at once, each in its own window, and talk to any of them.
- Every pop-out command now names its target. A page and Plenipo from different versions do not
  mix (both ship together).
- More windows mean more drawing. Each window is drawn by the one organization's window from the
  same live updates, so six windows still mean one listener.

## Alternatives considered

- **Six more Chat panels** that each pop out: rejected. A panel is a dock's tab with its own
  layout, and six would crowd the docks.
- **A separate page per window with its own commands:** rejected, as in ADR-092. It would be a
  copy, and would need permissions of its own.
- **Labels with the agent's name:** rejected. Window labels and `windows.json` keep no names.
- **"Who asked" on every message** (you, a lead, Plenipo's check-in): left for later. A turn does
  not yet say who asked it, and saying it would change the turn's record in two parts of the app.

## As built

- **Core** (`crates/core/src/workspace.rs`): `PopOutTarget` (`key`, `from_key`, `is_valid`,
  `title`, `CHAT_WINDOWS = 6`); `PopOutNotice::Closed { target }`; tests for keys, slots and the
  wire format.
- **App** (`apps/desktop/src-tauri/src/workspace_windows.rs`, `workspace_commands.rs`):
  - labels, requests, places, and closing by target;
  - `prepare_pop_out` refuses a slot that is not 1 to 6, and takes a chat window's title
    (`window_title`: one line, at most 80 characters) for its title bar;
  - IPC tests: a chat's window can call nothing; the commands refuse a bad target, an old
    `panel` argument, and slot 7.
- **Page** (`apps/desktop/src/`):
  - `workspace/` (pop-outs by target; `openChatWindow`, `closeChatWindow`, `focusChatWindow`,
    `onChatWindowClosed`);
  - `chat/tabs.ts` (`popped`, `popOutTab`, `putBackTab`, `slotOf`, `tabInSlot`,
    `readChatTabs`), `chat/ChatProvider.tsx` (the windows follow `popped`), `chat/ChatWindows.tsx`,
    `chat/ChatWindow.tsx` (Pop out, Put back), `chat/ChatPanel.tsx` (where it is),
    `chat/Transcript.tsx` (From its lead), `chat/positionTarget.ts`;
  - the ways in: `components/org/inspector/OverviewTab.tsx`, `pages/WorkerPage.tsx`,
    `live/LiveConversation.tsx`, `components/org/TopologyCanvas.tsx` with
    `views/OrganizationView.tsx`;
  - styles under "Chat" in `styles.css`.
- **Tests:** `chat/tabs.test.ts`, `chat/ChatWindows.test.tsx` (a window with the live chat and its
  message box, Put back, the window closed, Reset layout, a restart, six at most, From its lead),
  `workspace/Workspace.test.tsx`, and the ways in (`OverviewTab.test.tsx`, `pages.test.tsx`,
  `live.test.tsx`, `OrganizationView.test.tsx`); `workspace.e2e.mjs` in the real app.
