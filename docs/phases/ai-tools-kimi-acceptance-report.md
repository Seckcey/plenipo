# AI tools: Kimi — Acceptance Report

**Branch:** `claude/ai-tools-kimi` · **Status:** built, tested with the fake Kimi, and checked by
the owner on Windows with the real CLI on 2026-09-27 (§6, §8). Ready for review; releases as
v1.5.0 ([release notes](../releases/v1.5.0.md)).

Decisions: ADR-014 (adding AI tools ahead of Phase 15), ADR-015 (running AI tools over ACP), and
[ADR-027](../adr/ADR-027-acp-file-access-through-plenipo.md) (Kimi over ACP, with its file reads
and writes going through Plenipo), accepted by the owner on 2026-09-26 as ADR-016 and renumbered
on 2026-09-27, first to ADR-022 and then to ADR-027 (`main` uses ADR-016 to ADR-024 for other
decisions, and Phase 11 uses ADR-025 and ADR-026). Checklist:
[`ai-tools-kimi-checklist.md`](ai-tools-kimi-checklist.md).

## 1. How Kimi got here

The owner checked Kimi Code 0.34.0 on Windows 11 with a Kimi subscription (step 0; evidence in
[`crates/runtime/tests/fixtures/kimi-0.34.0/`](../../crates/runtime/tests/fixtures/kimi-0.34.0/README.md)).
Its one-task mode (`kimi -p`) fails two rules: the task text must be an argument, and it wrote a
file without asking. Its ACP mode (`kimi acp`) passes both: the task text is a message on its
input, and every change waited for permission. Unlike Grok, Kimi's own tools cannot be switched
off, so ADR-027 has Plenipo offer Kimi file access and answer every file request itself, through
Guard. The adapter waited for Grok's shared ACP driver (ADR-015), which reached `main` in v1.1.0.

## 2. What was built

- `crates/runtime/src/agent/kimi.rs`: the adapter (sign-in check, models, settings, and the
  driver options below).
- `crates/runtime/src/agent/acp.rs`: the shared driver gains ADR-027's options (file access
  through Plenipo, session settings, allowed modes, `session/load` to resume, tool names in
  titles, and an early refusal). Grok's behaviour is unchanged.
- The runtime and the broker: a file request from an AI tool is carried out through Guard while
  the task goes on (§4).
- The fake `kimi` persona, tests, on-screen text, setup guide, and docs.

### How Plenipo runs Kimi

1. Before every task: `kimi provider list`. Only the Kimi subscription provider
   (`managed:kimi-code`, `source=oauth`) lets a task run.
2. One `kimi acp` program per task, supervised like every other (time limit, cancel, record).
3. `initialize` offers file reads and writes, and no terminal.
4. `session/new` (or `session/load` for a follow-up), with Plenipo's tool server when the worker
   has permissions.
5. Settings, each checked in Kimi's answer: mode `default` (or `plan` for a worker without
   permissions), the model (K3 when none is chosen, so Kimi's own default setting is never
   used), and the thinking level when one is chosen. Only `kimi-code/…` models run; any other
   is refused before a conversation opens.
6. The task text, then Kimi's answer. Along the way:
   - every file Kimi reads or writes comes to Plenipo and is carried out through Guard, like the
     worker's own `read_file` / `write_file`; without permissions, every file is refused;
   - Kimi's own shell is refused (workers run programs with Plenipo's `run_command`); its file
     changes are allowed once only for a worker that may change files; Plenipo's own tools are
     allowed (Guard decides inside); anything else is refused; nothing is approved for a whole
     session;
   - a mode other than `default` or `plan`, or a file change Kimi reports done that never came
     to Plenipo, stops the task.

## 3. Tests

| Where                                 | What                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `kimi.rs` (16 unit tests)             | Kimi's real outputs: sign-in check, version, the settings before the prompt, `session/load` and its replay, the real permission requests (write allowed once or refused; shell refused), real file requests answered in any order, models checked against Kimi's own list, refusals.                                                                                   |
| `acp.rs` (existing driver tests)      | Grok unchanged: the same answers, permissions, and messages as before.                                                                                                                                                                                                                                                                                                 |
| `crates/runtime/tests/contract.rs`    | Kimi passes the contract suite (model and effort may travel in messages).                                                                                                                                                                                                                                                                                              |
| `crates/runtime/tests/agents.rs`      | Every shared test runs on Kimi (task, resume, cancel, errors, …), plus: settings reach Kimi; a worker without permissions reads, changes, and runs nothing; `yolo` stops the task; a non-subscription model is refused.                                                                                                                                                |
| `crates/capabilities/tests/broker.rs` | The real Guard, broker, relay, and Ledger: a Kimi Supervisor ("Read only") reads inside the folder, is refused outside it and for `.env`, sees secrets hidden, cannot change files or use its shell, and uses Plenipo's `read_file` by its bare name; a Kimi developer writes through Guard, a blocked file stays blocked, and a change around Plenipo stops the task. |
| `crates/capabilities/src/files.rs`    | A file read for an AI tool is returned as it is (line endings kept, never cut short).                                                                                                                                                                                                                                                                                  |
| `tests/e2e`                           | Kimi Ready on the AI tools page; a Kimi task with its shell refused (in plain words in the activity); Kimi's models in the model menu.                                                                                                                                                                                                                                 |

## 4. Changes to shared code, and why

| Change                                                                                    | Why                                                                                                              | Decision   |
| ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ---------- |
| `ToolProvider::file_access`, `Parsed::files`, `TurnParser::file_answered`                 | Kimi's file requests arrive on its output; the broker must answer them through Guard while the task goes on.     | ADR-027 §2 |
| `Broker::file_request` (the same path as a tool call; recorded with `fileRequest: true`)  | One place where Guard decides, whether the worker or its AI tool asks.                                           | ADR-027 §2 |
| `ToolServer::tools` (the tool names a grant offers)                                       | Kimi names a tool server's tool by its own name; only names the grant offers count.                              | ADR-027 §3 |
| Model names may carry one provider prefix (`kimi-code/k3`), in the runtime and the Ledger | Kimi's model names; ADR-027 §6 names them. Still never a flag or a path (`../x`, `/x`, `C:/x`, `a/b/c` refused). | ADR-027 §6 |
| Contract suite: model and effort may travel in an ACP tool's messages                     | Kimi takes them as session settings, not arguments.                                                              | ADR-027 §5 |

## 5. Security notes

- Kimi never reads a file itself: every read comes to Plenipo (checked on the real CLI in step
  0). Plenipo answers only inside the project folder, never a blocked file, with secrets hidden,
  and records each one. A worker without permissions gets none.
- Kimi's own shell never runs. Its writes are approved once each, only for a worker that may
  change files, and the write itself is Guard's decision. If Kimi ever reported a write done
  without asking Plenipo, the task stops.
- Kimi never runs in `auto` or `yolo`; if it switches itself, the task stops.
- Billing: only the Kimi subscription provider, only its `kimi-code/…` models, named on every
  task. No key variable is passed, and Plenipo never reads `%USERPROFILE%\.kimi-code` (nor
  `kimi provider list --json`, which prints Kimi's raw settings).
- Kimi still loads the owner's own Kimi settings (skills, `AGENTS.md`); the files it reads for
  them go through Plenipo too.

## 6. Owner check on Windows (about 20 minutes)

Use this branch's build. Never paste anything from `%USERPROFILE%\.kimi-code`, and do not open
files there. Before sending anything back, remove your email or Kimi account name if one shows.

1. **Build.** In PowerShell, in your Plenipo folder:
   `git fetch origin; git checkout claude/ai-tools-kimi; git pull; pnpm install; pnpm build`.
   Close any running Plenipo first.
2. **Kimi's version and sign-in.** Run `kimi --version` (0.34.0, or tell me the newer number).
   Run `kimi provider list` and copy what it prints (it shows provider names and where their
   sign-in comes from, never a key). The subscription line should read
   `managed:kimi-code  type=kimi  models=4  source=oauth`.
3. **No key variables.** Run
   `Get-ChildItem Env: | Where-Object Name -match 'KIMI|MOONSHOT' | ForEach-Object Name`.
   It should print nothing (it prints names only, never values).
4. **Start Plenipo:** `.\target\release\plenipo-desktop.exe`. Open **AI tools** and choose
   **Re-check**. The Kimi card should say **Ready**, "Moonshot AI", v0.34.0, and
   "Signed in (subscription) · Kimi sign-in". If not, copy the card's text and stop here.
5. **One task.** In **Workers**, pick **Kimi** and start: _"Say hello and tell me what you
   are."_ It should finish with an answer. Kimi reports no token counts, so none show.
6. **Resume.** Follow up in the same conversation: _"What did I just ask you?"_ It should
   remember.
7. **Cancel.** Start another Kimi task: _"Count slowly from 1 to 300, one number per line."_
   Choose **Cancel** while it runs. It should read **Cancelled** within about five seconds.
   Follow up with _"Continue."_: it answers in the same conversation.
8. **Model and thinking.** Start a Kimi task with **Advanced → Model** set to
   `kimi-code/kimi-for-coding-highspeed`: _"Reply with exactly: hello from highspeed"_. It should
   answer, and the conversation should show "model kimi-code/kimi-for-coding-highspeed". Then
   start one with the model typed as `moonshot/kimi-k2` (**Type another name…**): it must be
   refused with "Plenipo runs Kimi only with the Kimi subscription's models".
9. **No permissions: nothing read, changed, or run.** In **Workers**, pick **Kimi**:
   _"Read the file notes.txt in your folder, create a file named test.txt containing hi, and run
   the command `echo hi`."_ It should say it could not. Open **Live activity**: any request is
   refused there in plain words (for example "Kimi asked to open a file; this worker has no
   permission to use files, so Plenipo refused." or "Kimi asked to run a command with its own
   shell (Bash); Plenipo refused it."). Copy those lines.
10. **Reading through Guard, in a project.** In **Organization**, select the **Website
    Supervisor** (or your Development project's Supervisor). In its details panel:
    - **Allowed AI tools** must include Kimi, and **Folder** must not say "None" (choose
      **Edit project** to tick **Kimi** and set a folder with a README.md; a project made before
      Kimi existed does not allow Kimi yet).
    - **Edit title, AI tool, or model** → **Kimi** → save. (A Supervisor starts with
      **Read only**.)
    - **Give an objective:** _"Read README.md and summarize it. Then create a file named
      kimi-check.txt containing hi. Then run `dir` yourself."_

    Expected: the answer summarizes the README; **Activity** shows the read of README.md by the
    Supervisor (Plenipo carried it out); kimi-check.txt is **not** created, with "Kimi asked to
    change a file (Write); Plenipo refused it: this worker has no permission to change files.";
    and `dir` is refused ("… with its own shell (Bash); Plenipo refused it."). Copy every
    **Activity** line that mentions Kimi, a refusal, or "read".

11. **Changing files through Guard.** In the same project, choose a position whose role may
    change files (the **Senior Developer** starts with **Developer**). **Edit title, AI tool, or
    model** → **Kimi** → save. Give the **Supervisor** the objective: _"Ask the Senior Developer
    to create a file named kimi-check.txt containing the word hi, then tell me what
    happened."_

    Expected: the developer's task finishes; **Activity** shows "write kimi-check.txt" by the
    developer; the file exists (in the objective's working copy when the project works on a
    separate branch — the result on the **Projects** page lists it). If the developer's task
    instead ends with "Kimi reported a file change (Write) that did not go through Plenipo; the
    task was stopped.", Kimi wrote the file itself: copy that line, and tell me whether the file
    exists.

12. **Web.** Give the Senior Developer (still on Kimi) through the Supervisor: _"Ask the Senior
    Developer to look up today's top news headline on the web."_ Expected: it cannot, and
    **Activity** shows a refusal. If it answers with real headlines and there is no refusal in
    **Activity**, Kimi's web tools run without asking: tell me.
13. **If anything above did not happen as described,** open **AI tools**, find the program entry
    (for example "Kimi · task 1"), and copy its output.
14. **Optional: signed out.** Kimi 0.34.0 has no sign-out command in `kimi --help`. If Kimi's
    interactive screen has one (run `kimi`, then `/help`), sign out there, choose **Re-check**
    in Plenipo (the Kimi card should say **Not signed in** with the hint to run `kimi login`),
    copy what `kimi provider list` prints now, then `kimi login` again and **Re-check**:
    **Ready**. Skip this if there is no sign-out.
15. **Send back:**
    - step 2's `kimi provider list` output and version, and step 4's card text;
    - from steps 9–12: the answers and every **Activity** line about Kimi, reads, writes, or
      refusals — especially any "Kimi asked to use … Plenipo refused it" line naming a tool
      (that name tells me how Kimi asks for Plenipo's tools);
    - for anything that did not happen as described, the program output from step 13;
    - from step 14, if you did it: what `kimi provider list` printed while signed out.

## 7. Not verified here

Everything below needs the real, signed-in Kimi inside Plenipo:

- how Kimi asks permission for a tool of Plenipo's tool server (the fake asks with the tool's
  own name, as Kimi's own tools do; Plenipo also accepts `plenipo__…` and `mcp__plenipo__…`);
- that an approved Kimi write comes to Plenipo as `fs/write_text_file` (step 0 offered file
  access but refused the write, so the write itself was never seen);
- whether Kimi has web tools that run without asking;
- whether Kimi passes screenshots from Plenipo's browser and computer tools to its model
  (ADR-020, Plenipo's browser and computer use through Guard; unverified for Codex and Grok
  too);
- how Kimi finds project files: its own relative paths start from its conversation folder,
  which is outside the project, so Guard refuses them; Kimi is told the project folder, and
  Plenipo's tools take paths relative to it (step 10 shows which way Kimi goes);
- the texts of a usage limit and an expired sign-in, and `kimi provider list` signed out.

If a real message differs from the fake Kimi's, the fix belongs in `acp.rs` or `kimi.rs` and the
persona, with the recorded output added to the fixtures.

## 8. Owner results

- **§6 on Windows (2026-09-27):** the owner ran the check with the real Kimi Code, signed in with
  a Kimi subscription, and reported that it went as described. The detailed outputs (the
  `kimi provider list` text and the **Activity** lines) were not sent back, so this record rests
  on the owner's report.
- What this settles from §7: Kimi's requests for Plenipo's tools were recognized, its approved
  writes went through Plenipo, and its web request was refused.
- Still open: the texts of a usage limit and an expired sign-in, what `kimi provider list`
  prints when signed out, and the exact name Kimi puts in a permission request for Plenipo's
  tools.
