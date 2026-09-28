# Security hardening (2026-09-27 sweep) — Checklist

**Status:** Group A (the four High findings) merged (PR #69). Group B (the eleven Medium findings) merged through PR #74 (B1–B6), PR #76 (B7–B9), and PR #78 (B10 and the part-2 follow-ups); B11 is in the last Group B pull request. Group C follows in a later pull request.

The sweep's findings live in the repository's private security advisories (GHSA-m2rr-m89h-jp56,
GHSA-87xq-h83r-hmpg, GHSA-gv7h-v8h5-m9c9, GHSA-4f58-pwvq-9vmf for Group A; GHSA-2fq6-vq5f-685h, GHSA-phg2-6j94-84g9, and GHSA-2hxf-v9c3-44q2 for Group B). This checklist
records what was changed and how it was checked, in plain words, without the attack details.

## Group A: High

### A1 · Plenipo's browser is controlled over a private pipe (GHSA-m2rr-m89h-jp56)

- [x] The browser starts with `--remote-debugging-pipe`, never `--remote-debugging-port`: no
      network door on the PC, no `DevToolsActivePort` file
- [x] The DevTools protocol runs over two pipes (`crates/runtime/src/pipes/`), handed to the
      browser by the supervisor so it stays in Plenipo's process tree: on Unix through the
      pre-start hook, on Windows through a direct process start with inheritable handles and the
      same job object
- [x] `cdp.rs` speaks the same protocol over the pipe (NUL-separated messages, same size cap and
      close detection); the WebSocket transport stays for tests only
- [x] Tests: the app's own start arguments carry the pipe flag and no port flag; a real start over
      the pipe reads a page and leaves no port file; the browser tests keep a test-only port for
      the "owner's own hand" simulation
- [x] The workspace's minimum Rust is now 1.87 (`std::io::pipe`); the pinned toolchain is 1.98

### A2 · Approved programs run as the owner (GHSA-87xq-h83r-hmpg, ADR-034)

- [x] A tool ticket is honored only from the AI tool's own process or one of its descendants:
      the tool server finds which program holds the connection (Linux `/proc`, Windows TCP table
      and process snapshot) and walks its parents; a failed lookup refuses (never admits)
- [x] A refused or unchecked connection is a Ledger event the owner can see in the Activity trail
- [x] New installs' default approved list has no script runners (`npm run`, `pnpm run`,
      `yarn run`, `make`); saved lists are not rewritten; Settings says approved programs run with
      the owner's full account
- [x] Stored secrets are never given to `npm`, `pnpm`, `yarn`, `make`, or `npx`
- [x] `unsafe` stays forbidden across the workspace except one documented module for the
      Windows lookup (`crates/capabilities/src/process.rs`; the crate denies unsafe elsewhere)
- [x] SECURITY.md says plainly that approved programs are not sandboxed yet; sandboxing is the
      recorded follow-up

### A3 · ACP permission answers come from the tool's own name (GHSA-gv7h-v8h5-m9c9)

- [x] A permission request counts as one of Plenipo's tools only when the AI tool names it so
      (`_meta.toolName` / `toolName` with the `plenipo` prefix); the call's own arguments are
      never trusted for that decision
- [x] A shell command from the AI tool's own tools is refused first, before any other rule; with
      file access through Plenipo (Kimi) always, for others unless the name proves it is
      Plenipo's `run_command`
- [x] One approval covers one action: `allow_always` is never offered
- [x] A call allowed by its title (Kimi) leaves a notice in the task's activity
- [x] Tests for each case; the fake AI tool names its Plenipo calls as a real one does

### A4 · The network gate covers every way a page sends (GHSA-4f58-pwvq-9vmf, ADR-035)

- [x] Enter in any text box (inside or outside a form) asks the owner
- [x] Beacons and other non-page sends are intercepted like XHR and fetch (`EventSource` is not:
      the browser refuses that pattern, and it only receives)
- [x] Data a page sends on its own, outside the worker's action, is stopped and the worker is
      told; GET reads still go
- [x] On a page with a live connection (a WebSocket), every click, Enter, and Space asks first,
      since what goes over the connection cannot be held
- [x] Tests: a chat page that sends over a WebSocket, and a page that sends late after a click,
      against the synthetic website

## Group B: Medium

### Batch 1 (GHSA-2fq6-vq5f-685h)

- [x] **B2 · The browser never saves files (ADR-047).** The browser is told at start to refuse
      every download, before any tab exists; if it will not agree, Plenipo does not use it. The
      worker is told when a page tried to save a file (the file's name is cleaned and cut short).
- [x] **B1 · A page never gets a second tab (ADR-046).** A pop-up or a `target="_blank"` link is
      stopped before it runs and closed; during the worker's action, an allowed address opens in
      the worker's own tab through the same checks; a pop-up on the page's own, or while the owner
      has the tab, is just closed. The worker is told either way.
- [x] **B5 · File text, program output, GitHub text, and search results are fenced.** The same
      nonce fence page text had now wraps what a worker reads from files and searches, what a
      program prints, and GitHub issue and pull request bodies; the page's controls list too. An
      AI tool's own file reads over ACP stay exact (they are what it edits from).
- [x] **B4 · The git tools keep the blocked-files list.** `git_add` refuses a blocked file by
      name; `git_diff` leaves blocked contents out and says how many files; `git_commit` refuses
      with a blocked file staged; the `git_push` card names blocked files in the commits (renames
      seen under both names).
- [x] **B3 · Secrets reach only the programs they are for (ADR-048).** A secret goes only to a
      program found on PATH outside the project; a run that would receive one asks first unless
      the rule names the program and the secret; the card says "Will be given: …".
- [x] **B6 · A cap on approvals.** Three waiting cards per grant, ten new cards a minute, four
      tool calls at once per connection; a refused ask is told in plain words, makes no card and
      no picture, and a Ledger event records it once a minute.
- [x] Each fix reviewed by three independent readers; their remaining notes carried into five
      follow-up commits before the push
- [x] CI's Windows job green on the pull request (PR #74, merged)

### Batch 2 (GHSA-phg2-6j94-84g9)

- [x] B7 · Computer use asks before every click and keystroke (ADR-049)
- [x] B8 · Lessons a role keeps on its own are notes, not orders (ADR-050)
- [x] B9 · Codex works through Plenipo's tools: its own command tool is off (ADR-051)

- [x] Each fix reviewed by three independent readers; their remaining notes carried into three
      follow-up commits (one desktop step at a time while the owner decides; decided lessons drop
      why they waited; the Codex record says what is true of `apply_patch`)
- [x] CI's Windows job green on the pull request (PR #76, merged)

### Batch 3 (GHSA-2hxf-v9c3-44q2)

- [x] B10 · GitHub Actions pinned by commit, Dependabot (PR #78, merged)
- [x] B11 · Signing only from main and release tags, behind the owner's approval (ADR-052); the
      owner's GitHub steps are in docs/development/code-signing.md
- [x] Both fixes reviewed by three independent readers; their remaining notes carried in (ADR-038's
      pointer, a dry run fails when no installer was built, Dependabot titles in the commit style)

## Advisories: closing out

The sweep's findings live in the repository's private security advisories (**Security →
Advisories**). Plenipo's GitHub connection here cannot change advisories, so the owner closes each
one once its fix is on `main` (open the draft advisory → **Close advisory**; or on the PC:
`gh api -X PATCH repos/Seckcey/plenipo/security-advisories/<GHSA id> -f state=closed`).

| Advisory            | Findings | Fixed by                                                     | Close it         |
| ------------------- | -------- | ------------------------------------------------------------ | ---------------- |
| GHSA-m2rr-m89h-jp56 | A1       | PR #69 (merged)                                              | now              |
| GHSA-87xq-h83r-hmpg | A2       | PR #69 (merged)                                              | now              |
| GHSA-gv7h-v8h5-m9c9 | A3       | PR #69 (merged)                                              | now              |
| GHSA-4f58-pwvq-9vmf | A4       | PR #69 (merged)                                              | now              |
| GHSA-2fq6-vq5f-685h | B1–B6    | PR #74 (merged)                                              | now              |
| GHSA-phg2-6j94-84g9 | B7–B9    | PR #76 (merged)                                              | now              |
| GHSA-2hxf-v9c3-44q2 | B10–B11  | PR #78 (B10, merged) and the last Group B pull request (B11) | after B11 merges |
| GHSA-c86x-xcxc-pgf6 | C        | Group C (not started)                                        | later            |

## Checks

- [x] `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked` (Linux, bundled Chromium), `pnpm bindings` with no diff
- [x] Each fix reviewed by three independent readers (security completeness; correctness and
      Windows; conventions and tests) before it was accepted
- [x] CI's Windows job green on the pull request (the Windows pipe start and TCP-table lookup run
      there against Edge or Chrome)

## Owner's check on Windows (about 10 minutes)

1. Install the build from the pull request's **Windows** check (or the next release).
2. Give a Web Assistant a task on an allowed website. The browser should open and work as
   before. In the profile folder (`%LOCALAPPDATA%\com.eightwest.plenipo\browser-profile`) there
   should be **no** `DevToolsActivePort` file.
3. Give a Developer worker a task that runs an approved command (for example `cargo test`). The
   Activity trail should show the command and **no** "ticket refused" line. If you see one for a
   normal worker, copy it: it tells us how that AI tool starts its helper program.
4. If you use Grok workers: give one a task that uses a Plenipo tool. If every Plenipo tool call
   is refused ("Plenipo refused it"), copy one line and the Grok version; the adapter can be
   taught how Grok names its tool-server calls.
5. Open **Settings → Permissions → Programs**: the note under the approved list says approved
   programs run with your full account.
