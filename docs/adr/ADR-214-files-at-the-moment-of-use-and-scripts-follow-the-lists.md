# ADR-214: Files are checked at the moment of use, and scripts follow the command lists

- **Status:** Proposed (the Development Coordinator and the independent security reviewer settled
  the design on 2026-10-04; the owner accepts it at review)
- **Date:** 2026-10-04
- **Phase:** none (security hardening: findings P-GUARD-3 and P-GUARD-4 of the 2026-10-02 security
  review; the first part of P-GUARD-3 was done in Phase 23, Wave 1, as finding G23-3)
- **Builds on:** [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md),
  [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md), [ADR-201 (light by default)](ADR-201-light-by-default.md),
  [ADR-213 (build and test commands ask first)](ADR-213-build-and-test-commands-ask-first.md). No
  earlier decision is changed.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

## In short

Two gaps from the October 2 security review, both under Plenipo's first promise ("a worker stays
inside its project's folder") and its second ("a worker cannot do anything its permission set does
not allow").

**Files.** Plenipo checks a path when the worker names it, and again right before a file tool uses
it (Phase 23). The open itself was still the last unguarded instant: on Windows a link put at the
file's own name in that instant was followed, and a file that is also another file somewhere else
(a hard link, which no path check can see) was changed through its project name. Now, on every
system, the open refuses a link at the file's own name; a hard-linked file is read but never
written or edited; Watch's "before" text, an edit's before-text, and a search read a file the same
safe way; a move never replaces a file that appeared after the name was checked; and deleting a
link to a folder removes the link, never the folder.

**Scripts.** The owner's never-run list and always-ask list applied to the program a worker names
on a command line, not to the programs inside a PowerShell script. Under Light (where Plenipo
starts), a script that said `rm -rf ../other` ran with no card while the same words on a command
line were refused. Now Plenipo reads a script as statements and gives each program it names the
same lists and the same "outside the project folder" check. A statement Plenipo cannot read (text
run as code, an encoded command, a program named by a variable) asks the owner and says why. The
text a shell is handed on a command line (`bash -c "…"`, `cmd /c …`, `powershell -Command …`) is
read the same way.

Accepting this record means: a worker's script that names a program on the never-run list is now
refused, and one that names a program on the always-ask list, deletes outside the folder, or cannot
be read shows a card, under Light too; a file that is a hard link is no longer changed by a worker
(pnpm's `node_modules` are hard links into a store outside the project); Plenipo's capabilities
crate gets one Windows-only dependency, `winapi-util`, with no unsafe code of Plenipo's own; and
the gaps listed below stay open and known.

## Context

What the code did at `98448455` (after Phase 23's Wave 1 and ADR-213):

- `crates/guard/src/paths.rs`: `Workspace::resolve` checks a path when the worker names it;
  `Resolved::still_inside` follows the links on its way again right before a file tool uses it
  (G23-3 of the Phase 23 Guard review, the first part of P-GUARD-3).
- `crates/capabilities/src/files.rs`: on a Mac and Linux the file was opened with `O_NOFOLLOW`, so
  a link at its last part was refused. On Windows `opened()` did nothing: Windows has no such flag
  for a data open. Watch's "before" text (`before_change`), an edit's before-text (`edit_watched`),
  and `search` opened files plainly. `move_path` renamed over whatever was at the new name;
  `delete` tried `remove_file` on a junction to a folder. Nothing looked at hard links.
- `crates/capabilities/src/broker.rs`, `Action::Script`: a script runs as
  `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command -` with the text on
  stdin; Guard's request carries `script` and no `command`.
- `crates/guard/src/engine.rs`: the never-run list, the always-ask list, and the approved list were
  consulted only `if let Some(cmd) = request.command`. `sensitive::script` ran the word classifier
  only; the "deletes, moves, or overwrites outside the project folder" check ran only for a
  command line. Under Careful the Safety cap already made every script ask (ADR-201); under Light
  a script ran at the role's level, with `PowershellExec` **Allowed** in the Everyday work and
  Developer sets.

Measured on the owner's PC on 2026-10-03 and 2026-10-04, from an ordinary program using Rust's
standard library:

- A cloud-only OneDrive file (Files On-Demand) shows to an ordinary program **without** the reparse
  attribute (attributes `RECALL_ON_DATA_ACCESS` and `ARCHIVE` only), with or without a Windows 10
  manifest: the cloud filter "disguises" placeholders for ordinary programs and exposes them only to
  sync engines and programs under `%SystemRoot%`. Plenipo's own Windows manifest declares only
  Common Controls, so Plenipo is an ordinary program here. Opening such a file for its attributes
  only, with `FILE_FLAG_OPEN_REPARSE_POINT`, downloaded nothing.
- A junction (`mklink /J`, which any account may make) reports the reparse attribute, and Rust's
  `file_type().is_symlink()` is true for it, by path and from a handle opened with the flag.
- Rust's standard library calls a Windows reparse point a symlink exactly when its tag has the
  **name surrogate** bit (`0x20000000`): symbolic links (`0xA000000C`), junctions and mount points
  (`0xA0000003`), WSL symlinks (`0xA000001D`), global reparse (`0xA0000019`), WCI and ProjFS
  tombstones, and dataless CIM files. Cloud files (`0x9000x01A`, OneDrive `0x80000021`), compressed
  files (`0x80000017`, `compact.exe`), deduplicated files (`0x80000013`), ProjFS (`0x9000001C`), and
  app execution aliases (`0x8000001B`) are not.
- Rust exposes a file's link count, volume serial number, and file index on Windows only behind
  the unstable `windows_by_handle` feature. `winapi-util` (BurntSushi's, used by ripgrep; already
  in Plenipo's lockfile through `walkdir` and `same-file`) wraps `GetFileInformationByHandle`
  safely. Making a symbolic link needs a right this account lacks (error 1314); a junction and a
  hard link need none.

## Decision

### Files (P-GUARD-3)

1. **One way to open a file for a tool** (`files::open_checked`). On a Mac and Linux: one open with
   `O_NOFOLLOW`; a link at the file's own name is refused ("became a link after it was checked, so
   nothing was read or changed"). On Windows: two opens. First the name is opened for its
   **attributes only**, with `FILE_FLAG_OPEN_REPARSE_POINT` and `FILE_FLAG_BACKUP_SEMANTICS`, to see
   what is really there now. What Rust calls a symlink (a name surrogate: a symbolic link or a
   junction) is refused. Then the data is opened the **ordinary way**, and the two handles are
   proved to be one object on the disk (equal volume serial number and file index), or the open is
   refused ("changed while it was being opened"). A file that does not exist yet is made with
   `create_new`, which fails rather than follow a link that appeared in the meantime.
2. **Data never goes through a handle opened with `FILE_FLAG_OPEN_REPARSE_POINT`.** That flag
   bypasses the filters that make some ordinary files work: OneDrive Files On-Demand, compressed
   and deduplicated files, ProjFS. Those are reparse points that are not name surrogates, and they
   keep working because the data open is ordinary. The identity proof is what makes the two opens
   safe: a name swapped for a link between them leads to another object.
3. **A hard-linked file is never written or edited.** `write` and `edit` refuse a file whose link
   count is more than one: "is shared with another place on this disk (a hard link), so Plenipo did
   not change it; copy it first, or change it there". Reading, moving, and deleting are unchanged
   (deleting removes one name only). On a Mac and Linux the count comes from the standard
   library's `nlink()`; on Windows from `winapi-util`.
4. **Every read goes the same way:** Watch's "before" text, an edit's before-text, and a search
   use `open_checked` too, so the Watch panel can never show a file from outside the folder.
5. **A move refuses a name that appeared** after it was checked ("appeared after it was checked,
   so nothing was moved"). It stays a check, then a rename: the standard library has no rename that
   refuses to replace. **Deleting a link to a folder** removes the link, never the folder.
6. **New dependency: `winapi-util` 0.1**, Windows only, in `crates/capabilities`, for the link
   count and the identity proof. The unsafe calls live inside that crate. The workspace rule
   `unsafe_code = "forbid"` and the capabilities crate's `unsafe_code = "deny"` are unchanged;
   Plenipo adds no unsafe code of its own. The crate was already in `Cargo.lock` through
   `walkdir`; the lockfile gains one line.

### Scripts (P-GUARD-4)

7. **A script is read as statements** (`crates/guard/src/scripts.rs`): comments (`# …`, `<# … #>`)
   and here-strings (`@" … "@`, `@' … '@`) are set aside as units; the text is split at line ends,
   `;`, `|`, `&&`, `||`, `&`, braces and parentheses, never inside a quoted string. A statement
   names a program literally when its first word is one (`rm`, `Remove-Item`, `.\tool.ps1`), or
   when the call operator (`&` or `.`) is followed by a plain word or a literal string (`& 'rm'`).
   Keywords, assignments (`$x = …`, read after the `=`), member calls, and parameters are not
   programs.
8. **Two tiers under Light.** A program on the never-run list, named literally, is **refused**:
   "Blocked: the script runs rm (line 2), which is on your blocked commands list ("rm *")." A
   statement the reader **cannot follow asks** and says why: text run as code (`Invoke-Expression`,
   `iex`), an encoded command (`-EncodedCommand`, `-e`, `-ec`, `-enc`), a program named by a
   variable or an expression (`& $tool`, `& (…)`), a string put together (`& "r$m"`), a backtick
   inside a name, an alias made or changed on the spot (`New-Alias`, `Set-Alias`, `Import-Alias`,
   `nal`, `sal`, `ipal`), a job or a remote command (`Start-Job`, `sajb`, `Invoke-Command`, `icm`),
   `Add-Type`, a direct call into .NET's file, process, or script classes (`[System.IO.…]`,
   `[IO.…]`, `[System.Diagnostics.Process]`, `[scriptblock]::Create`), and `$env:ComSpec`: "needs
   your approval: line 3 of the script runs text as code (Invoke-Expression), so Plenipo cannot
   see which programs it runs." A program on the **always-ask list** asks, naming the program and
   the line.
9. **The "outside the project folder" check** runs per statement, as for a command line: a program
   that deletes, moves, or overwrites, given a place outside the folder, asks (or is refused where
   the owner set that kind to blocked). PowerShell's short aliases (`ri`, `mi`, `cpi`, `sc`, `rni`)
   join the list of such programs.
10. **Aliases count as their cmdlets.** A rule catches a program under the name written and under
    the cmdlet it stands for (`Remove-Item *` catches `ri`; `rm *` catches `rm`). The table is
    **Windows PowerShell 5.1's**, which Plenipo runs on Windows: `rm`, `ri`, `del`, `erase`, `rd`,
    `rmdir`, `mi`, `mv`, `move`, `cpi`, `cp`, `copy`, `sc`, `rni`, `ren`, `curl`, `wget`, `iwr`,
    `irm`, `start`, `saps`, `kill`, `spps`, `iex`, `icm`, `sajb`, `nal`, `sal`, `ipal`, `ni`, `gci`,
    `ls`, `dir`. Where Windows PowerShell is not installed, Plenipo falls back to PowerShell 7
    (`pwsh`), which drops `curl` and `wget` (they then name the real programs) and keeps the rest; a
    rule still catches those two because the written name is checked as well.
11. **Wrappers, one level deep.** `Start-Process X` names `X` too. The text a shell is handed on a
    **command line** is read the same way — `sh`, `bash`, `zsh`, `dash`, `ksh`, `fish` with `-c`;
    `cmd` with `/c` or `/k`; `powershell` and `pwsh` with `-Command` (an encoded command is "cannot
    follow") — so `bash -c "rm -rf x"` is refused by the never-run list, and `sh -c "$CMD"` asks,
    when the owner has taken the shells themselves off the list. The default list blocks the shells
    outright, so for most owners this layer never speaks.
12. **What does not change.** The approved list is not applied to scripts: under Careful every
    script already asks (the Safety cap, ADR-201), and under Light nothing is approved-list-gated
    by design. A **script file** (`powershell -File x.ps1`, `& .\x.ps1`) is a program like any
    other under Light, and inside the project folder it is approved under Careful only by a rule
    that names its own path (ADR-213's reasoning). The Settings wording is unchanged.

## What a person sees

- **Files, every system:** nothing new in the ordinary case. A worker that tries to change a
  hard-linked file (for instance a file in pnpm's `node_modules`, which is a hard link into pnpm's
  store outside the project) reads: "… is shared with another place on this disk (a hard link), so
  Plenipo did not change it; copy it first, or change it there". A link swapped in at the wrong
  moment reads "… became a link after it was checked, so nothing was read or changed". OneDrive
  files, compressed files, and ProjFS folders work as before.
- **Scripts, Light (the default) with scripts allowed (Everyday work, Developer):** a script that
  names a program on the never-run list is refused with the program and the line. A script that
  names a program on the always-ask list, deletes or moves or copies outside the folder, or has a
  statement Plenipo cannot read shows an approval card that says which line and why. Ordinary
  scripts (`Get-ChildItem`, `cargo build`, `Copy-Item a.txt out/`) run as before.
- **Scripts, Careful and Strict:** unchanged (every script asked already, or none runs).
- **A command line that hands text to a shell** is read too; with the default lists the shells are
  refused by name before this matters.

## Known gaps

The following stay open and are written down here so nobody mistakes them for oversights.

**Files**

- **The instant between the re-check and the open, for a folder on the way.** The file's own name
  is covered on every system now. A folder higher on the path swapped for a link in the instant
  after `still_inside` and before the open is still followed. Closing that needs opening the path
  one part at a time relative to a handle (`openat` with `O_NOFOLLOW` on a Mac and Linux;
  `NtCreateFile` relative to a directory handle on Windows). The standard library has neither; a
  vetted crate (`cap-std`) would bring them. **The owner's decision, recorded, not taken now.**
- **Listing a folder and making a new file's parent folders** (`read_dir`, `create_dir_all`) have
  the same instant.
- **A move is a check, then a rename.** A file that appears between the two is replaced. The
  standard library has no rename that refuses to replace.
- **A bind mount, a `subst` drive, or a mapped network share** that places an outside location
  inside the project folder is invisible to a path check on every system.
- **A hydrated OneDrive placeholder is still a reparse point** (a cloud tag, not a name surrogate),
  and to an ordinary program it looks like a plain file. **Do not "fix" the file tools by refusing
  every reparse point:** OneDrive, compressed files, deduplicated files, and ProjFS would stop
  working. The rule is name surrogates only, which is what the standard library already calls a
  symlink.
- **What was run and what was reasoned.** Run on the owner's PC (Windows) on 2026-10-04: the 22
  file-tool unit tests, including the new junction (`mklink /J`) and hard-link tests, and the 180
  Guard unit tests, including the new script-reader tests. The two-step open and the identity proof
  are on the path of every one of those file tests. **Reasoned, not run:** the file-symlink part of
  `a_link_at_the_last_part_is_refused_on_windows` (this PC's ordinary account may not make a
  symbolic link, so that part says so and is skipped here; GitHub's Windows machines run it), and
  the data open of a real OneDrive placeholder through the ordinary path (its attribute-only open
  was measured; the test suite has no cloud placeholder to work with).
- **Another program running as the owner** can do anything the owner can. Same-user attackers are
  out of scope in `SECURITY.md`.

**Scripts**

- **The reader is a net for naive and accidental calls under Light, not a bound on a determined
  worker.** It refuses a blocked program named literally and asks about the ways it knows a name
  can be hidden. PowerShell can hide a name in more ways than any reader follows: the format
  operator (`'{0}m' -f 'r'`), `Get-Command`, splatting, `Invoke-Item`, .NET reflection beyond the
  listed classes, a program started through an argument string of another program. Those run at
  the role's level under Light. An owner who wants a bound sets Safety to **Careful**, where every
  script asks, or sets `PowershellExec` to **Ask** in the permission set.
- **False refusals, on the safe side.** A quoted string that spans lines without here-string syntax
  can put a program's name at the start of a line, and a `#` inside a string cuts the rest of its
  line; either can make a harmless script refused or asked about. The reason names the line, and
  the worker can use `run_command` instead.
- **The alias table is Windows PowerShell 5.1's**; aliases an owner's profile adds are not read
  (Plenipo runs scripts with `-NoProfile`, so they do not exist for a worker either).
- **Shell text is read for `-c`, `/c`, `/k`, and `-Command` only**; other ways of handing a shell
  text (a script file, stdin, `-File`) name a program and are treated as one.

## Alternatives considered

- **Refuse every reparse point on Windows.** Rejected: OneDrive placeholders, compressed and
  deduplicated files, and ProjFS are reparse points and must keep working.
- **Read and write through the handle opened with `FILE_FLAG_OPEN_REPARSE_POINT`.** Rejected: that
  bypasses the cloud filter and the others; a dehydrated placeholder would read as empty, and a
  write could desynchronise it. Two opens with an identity proof cost one extra attribute open.
- **Rust's `windows_by_handle`.** Rejected: unstable, nightly only.
- **Win32 calls of Plenipo's own** for the link count and the identity. Rejected: the workspace
  forbids unsafe code; `winapi-util` wraps the same call safely and was already in the lockfile.
- **Treat a script as never approved** (the report's first suggestion). Moot: under Careful every
  script asks already (ADR-201); under Light the approved list gates nothing.
- **Ask, instead of refusing, when a script names a blocked program.** Rejected: the never-run list
  means never, on a command line and in a script alike. The reader refuses only a name written
  literally at the start of a statement, so a false refusal is rare and says why.
- **A real PowerShell parser.** Rejected: a large dependency for a safety net; the two tiers give
  the owner a card wherever the simple reader stops.

## How to check

- `crates/capabilities/src/files.rs`: `a_folder_that_became_a_junction_is_not_used`,
  `a_link_at_the_last_part_is_refused_on_windows`, `a_hard_linked_file_is_not_changed`,
  `a_move_refuses_a_name_that_appeared`, `deleting_a_folder_link_removes_only_the_link`,
  `watch_never_shows_text_through_a_link`, and the Phase 23 tests, which still pass.
- `crates/guard/src/scripts.rs`: `statements_of_a_script`,
  `statements_the_reader_cannot_follow_are_said_so`,
  `a_rule_catches_a_program_in_a_script_by_either_spelling`,
  `a_shell_on_a_command_line_hands_over_its_text`.
- `crates/guard/src/engine.rs`: `a_script_gets_the_command_lists`;
  `crates/guard/src/sensitive.rs`: `scripts_are_checked_too`.
