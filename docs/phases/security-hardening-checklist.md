# Security hardening (2026-09-27 sweep) — Checklist

**Status:** Group A (the four High findings) merged (PR #69). Group B (the eleven Medium findings) merged through PR #74 (B1–B6), PR #76 (B7–B9), and PR #78 (B10 and the part-2 follow-ups); B11 is in the last Group B pull request. Group C (the ten Low findings) is built, reviewed, and tested on PR #86; the owner merges it.

The sweep's findings live in the repository's private security advisories (GHSA-m2rr-m89h-jp56,
GHSA-87xq-h83r-hmpg, GHSA-gv7h-v8h5-m9c9, GHSA-4f58-pwvq-9vmf for Group A; GHSA-2fq6-vq5f-685h, GHSA-phg2-6j94-84g9, and GHSA-2hxf-v9c3-44q2 for Group B; GHSA-c86x-xcxc-pgf6 for Group C). This checklist
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

## Group C: Low (GHSA-c86x-xcxc-pgf6)

Ten fixes in four clusters, each built with a test that failed first, read by three independent
reviewers (security completeness; correctness and Windows; conventions), and pushed once their
blocking notes were fixed and the sensible smaller ones carried.

### Browser

- [x] **C1 · A control is read again just before the action.** A click, typing, a key press, or a
      choice goes ahead only on the control the owner saw on the card: what the control is (its
      kind, words, link, form and where it sends, whether it takes a password) is compared again,
      and a changed one is refused with the worker told what changed and to read the page again.
- [x] **C2 · A handed tab comes back only with the gate on.** After the owner solves a check, the
      checks on the page's requests go back on before the worker gets the tab; if the browser
      will not turn them on, the tab is stopped instead (fail closed) and the worker is told.
      Browser calls for a tab in the owner's hands, taken over, or stopped are refused, however
      the calls came in.
- [x] **C3 · The owner's sign cannot be taken off the page.** The sign sits in the browser's top
      layer with styles the page cannot override; a page that removes, restyles, hides, or closes
      it gets it put back at once. A page that keeps doing so stops that worker's use of the
      browser, the Ledger records it (`browser.tab_stopped`), the Activity trail says so in plain
      words, and the worker's next browser call says why. A page's own dialog or top-most widget
      is not a fight.
- [x] Tests against the synthetic website: a form re-aimed and a field turned into a password
      field while the owner decides; a page that hides the sign once (it comes back), one that
      keeps removing it (the worker is stopped), and one with its own dialog in front (nothing
      happens).

### Guard

- [x] **C4 · Web addresses in the record keep the page and its field names (ADR-057).** Approval
      cards, the `capability.used` and `guard.*` records of the browser tools, the control
      center's notes, and screenshot records keep an address's website, page, and the names of
      its fields only (`?to=…&amount=…`), never a user name or password (`safe_address`).
      Addresses are cleaned before secrets are hidden. The worker still reads the address; a
      program's command line is kept as it is; the AI tool's own activity keeps what it said.
- [x] **C5 · A folder listing respects the blocked-files list.** `list_directory` puts the folder
      itself through Guard's blocked-files check (a blocked folder is refused), and the listing
      leaves blocked entries out, uncounted; without its settings, it lists nothing.
- [x] **C6 · Each allowed change of an AI tool's own is matched to its write.** Over ACP, a write
      that came through Plenipo and was carried out clears only the one allowed change it is
      for (the same file, worked out against the task's folder), never all of them, so a second
      change that never comes through Plenipo is still caught and stops the task. At most 64
      wait; the next is refused, never an older one forgotten.
- [x] Tests: an address with a sign-in token and a session key reaches the worker but no approval
      card, `capability.used` or `guard.*` record, control center note, or screenshot record
      (against the synthetic website); a listing with `.env`, a key file, and a blocked folder;
      allowed Kimi changes where one write never comes, one is refused, one names another
      folder's file, and one too many is asked.

### Desktop and Ledger

- [x] **C7 · Only ordinary keys and shortcuts on the desktop.** `screen_keys` takes ordinary
      keys, and Ctrl, Shift, or Alt with letters, digits, and the moving keys (an allow-list, so
      a shortcut nobody listed is refused too). Refused, among others: Alt+F4, Ctrl+W, Ctrl+F4,
      Ctrl+Q, Alt+Tab, Ctrl+Alt+Tab, Alt+Esc, Ctrl+Esc, Ctrl+Shift+Esc, Ctrl+Alt+Delete,
      Alt+Space, F1, F12, a browser's developer tools, Shift+Delete, and a letter from another
      alphabet; the Windows key was refused already (this extends ADR-020; ADR-049's asking is
      unchanged).
- [x] **C8 · Typed text holds only what the card shows.** `screen_type` refuses control
      characters other than tab and line breaks, and invisible ones (zero-width, right-to-left
      marks, tag characters); line breaks are made one kind; at most 500 characters a step, so
      the card shows all of it.
- [x] **C9 · The Ledger's files are kept from other accounts.** On Unix (Linux is used for
      development and CI) the app's data folder and the Ledger's folder are readable by the
      owner's account only (`0700`), also when an older version left them open, and so are new
      backups, exports, the database, and a restored copy (`0600`); a folder that cannot be made
      so gives a notice. On Windows nothing changes: the account's app-data folder is private
      already through its access control list, and files have no mode bits
      (`crates/ledger/src/owner_only.rs`).
- [x] Tests: the allowed and refused combinations, the hidden characters, and the modes of the
      Ledger's folder, database, backups, exports, a chosen folder (left as it is), and a restored
      copy.

### Web

- [x] **C10a · The website's version is never typed by hand.** The page carries a mark the build
      fills from the root `package.json`, or from `PLENIPO_VERSION` (a container build has no
      repository and stops without it); `pnpm versions:check` refuses a version typed into the
      page. The root version runs ahead of GitHub Releases (1.9.0 on `main`, v1.10.0 published,
      no v1.9.0 installer), so the deploy runbook passes the latest published release and checks
      its installer link first (`docs/development/website.md`).
- [x] **C10b · The desktop app's page policy names its inline styles.** `style-src` is `'self'`
      alone. Inline styles are allowed by name only where the terminal (xterm.js) needs them: its
      own `<style>` elements (`style-src-elem`) and the `style` attributes it sets for true colors
      and contrast fixes (`style-src-attr`). In today's WebView2 that allows the same inline
      styles as before; the gain is that the app's own code is held to none (a test checks it,
      with raw HTML), and the test fails when the terminal stops needing them, so the policy can
      drop them. Drawing the terminal with its WebGL renderer would remove most of the need; it
      falls back to the same styles when WebGL is lost, so it is a follow-up, not this fix.
- [ ] Owner's check on Windows: the terminal (including colored output, for example Claude
      Code's diffs) and every page look right in the pull request's **Windows** build (the
      policy change).

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
| GHSA-c86x-xcxc-pgf6 | C1–C10   | PR #86                                                       | after it merges  |

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
