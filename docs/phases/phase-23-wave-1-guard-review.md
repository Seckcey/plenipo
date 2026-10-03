# Phase 23, Wave 1: the Guard safety review on a Mac and Linux

|               |                                                                                                                                                                                                                         |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Date**      | October 2, 2026 (Pacific)                                                                                                                                                                                               |
| **Code**      | `main` at `419d59e0` (Wave 1 through pull request #148), with pull request #149 (Guard's path rules, files that run when opened)                                                                                        |
| **Kind**      | The review that ends Wave 1 ([ADR-150](../adr/ADR-150-phase-23-starts.md)): the code that decides what a worker may do, read again for a Mac and Linux. Every finding is fixed in the same pull request as this record. |
| **For**       | 8 West Ventures, LLC (Plenipo is made by 8 West Ventures, LLC)                                                                                                                                                          |
| **Checklist** | [Phase 23](phase-23-checklist.md), Wave 1, "Done when"                                                                                                                                                                  |

## 1. In short (plain words)

Wave 1 got Guard, the part of Plenipo that decides what a worker may do, ready for a Mac and
Linux. This review read that work again, looking for one thing: any way a worker could do more on
a Mac or a Linux PC than on Windows. It found six. All six are fixed now.

1. **Stop did not always stop everything.** On a Mac and Linux, a program can step out of the
   worker's group, as a build tool's background helper does. It kept running after Stop, and
   even after Plenipo quit. Now every program a worker starts carries a mark, and Plenipo ends
   whatever still carries it. ([ADR-158](../adr/ADR-158-programs-that-leave-their-group.md), a
   program that leaves its group still ends with its work.)
2. **Some Mac and Linux programs could get around Guard if the owner said yes to a card** without
   knowing what they do: programs that move the mouse and type, take a picture of the screen,
   read the clipboard, or read the saved keys on Linux. They are blocked now, as the Mac's own
   `osascript` and `security` already were.
3. **A file could be swapped for a link at the wrong moment.** Plenipo checked a path once, then
   used it later. On a Mac and Linux any program can make a link, so a folder could become a link
   to somewhere else in between. Plenipo now checks again right before it reads or writes.
4. **A copy into a link that leads outside the project did not count as "outside".** It does now.
5. **If someone started Plenipo as root** (the computer's administrator account), everything a
   worker started would have been root too. Plenipo now starts nothing for a worker then.
6. **Plenipo's short checks of an AI tool** were not on the keeper's list, so a crash at that
   moment could leave one running. They are on it now.

Nothing was found that lets a stranger in. Nothing lets a worker get past Guard without the
owner's yes, or without one of the owner's approved programs running code. `SECURITY.md` already
says that approved programs run as the owner.

### Numbers

| Severity | Found | Fixed |
| -------- | ----- | ----- |
| Critical | 0     | 0     |
| High     | 0     | 0     |
| Medium   | 3     | 3     |
| Low      | 3     | 3     |

## 2. How the review was done

- **Read by hand,** at the commits above: the Mac's tool-ticket lookup
  (`crates/capabilities/src/process.rs`) and the tool server's admission (`broker.rs`, `admit`);
  the supervisor, the keeper, the child environment, and the added program folders
  (`crates/runtime/src/supervisor.rs`, `keeper.rs`, `policy.rs`, `program_dirs.rs`); Guard's
  command rules, decisions, default lists, sensitive-action check, and path rules
  (`crates/guard/src/commands.rs`, `engine.rs`, `defaults.rs`, `sensitive.rs`, `paths.rs`); the
  terminal, the file tools, the owner's file screens, and the Vault
  (`crates/capabilities/src/terminal.rs`, `files.rs`, `broker/owner_files.rs`, `vault.rs`).
- **Compared** each part with what Windows does today. Wave 1's promise is the same safety on
  every system.
- **Tested** each fix on GitHub's Linux and Mac machines (this PC runs Windows).
- **Severity** follows the [October 2 security report](../../PLENIPO-SECURITY-REPORT.MD): how bad
  it would be if used, not how likely.

## 3. Findings

### G23-1 [Medium] A program that leaves its group outlives Stop and a crash

- **Where:** `crates/runtime/src/supervisor.rs` (`wrapped_command`: a process group on a Mac and
  Linux, a job object on Windows), `crates/runtime/src/keeper.rs` (ends listed groups only).
- **What it did:** a worker's program leads a process group. Stop, a time limit, and the keeper
  end that group. A program that starts a session of its own is no longer in it. Some build
  tools' servers do that (Bazel's does), as do `tmux`, `ssh-agent`, and `setsid`. ADR-157 listed
  this as a known limit.
- **Why it matters:** "Stop" is how the owner ends work, and ADR-150 says no Mac or Linux download
  ships until program trees hold. Windows' job object ends such programs.
- **Fix:** every program Plenipo starts on a Mac and Linux carries `PLENIPO_RUN=<this
Plenipo>/<run>`, which its own programs inherit. When a run ends, is stopped, or runs past its
  time, Plenipo ends the owner's programs still carrying that mark. The keeper, given this
  Plenipo's part of the mark, ends every one still carrying it when Plenipo goes away. Probes are
  marked too. (`crates/runtime/src/marks.rs`, ADR-158.)
- **Tests:** `a_program_that_left_its_group_ends_when_the_run_is_stopped`,
  `…_ends_with_its_run`, `…_does_not_outlive_a_crashed_owner` (`crates/runtime/tests/supervisor.rs`,
  a Mac and Linux), with a harmless diagnostic copy that leaves its group.
- **Limit:** a program that empties its own environment, or one the system hides, is not found.
  It is also refused by the tool server, since it no longer descends from the AI tool.

### G23-2 [Medium] Guard's blocked list missed a Mac's and Linux's tools that reach past Guard

- **Where:** `crates/guard/src/defaults.rs` (`default_commands`).
- **What it did:** a program on no list asks the owner first, which is safe when the owner can
  tell what it does. These could not easily be told apart from harmless ones, and each reaches
  past a part of Guard:
  - `xdotool`, `ydotool` and others move the mouse and type outside computer use, which asks
    every step and shows the sign.
  - `screencapture` and others take a picture of the screen.
  - `pbpaste`, `xclip` and others read the clipboard.
  - `secret-tool` and others read the password store, where Plenipo keeps its own keys on Linux.
  - `setsid`, `systemd-run` and `at` start a program Stop would not end.
  - `busctl`, `gdbus` and others talk to the desktop's services, which can do most of the above.
  - `shortcuts` runs a Mac's automations.
  - `gsettings`, `networksetup`, `nmcli` and others change settings, as Windows' `reg` does.
  - `poweroff` and `halt` turn the computer off.
  - `pkill` and `killall` end programs by name, Plenipo's own included.
- **Fix:** new installs block them. A Mac's and Linux's other "open" programs (`gio`,
  `kde-open`, and others) ask, as `open` and `xdg-open` do. No Mac or Linux owner has a saved
  list yet. Most of these names do not exist on Windows; the few that do (Nushell, Python's
  `keyring`) are a shell and a key reader there too.
- **Test:** `mac_and_linux_tools_that_reach_past_guard_are_blocked` (`defaults.rs`).

### G23-3 [Medium on a Mac and Linux] A path is checked once and used later

- **Where:** `crates/guard/src/paths.rs` (`Workspace::resolve`), `crates/capabilities/src/files.rs`.
  This is the October 2 report's P-GUARD-3, rated Low there because making a link on Windows needs
  rights a worker usually lacks. On a Mac and Linux any program can make one.
- **What it did:** a path was checked against the project folder, then used as a plain path. If
  a folder on the way became a link in between, perhaps while a card waited, the read or write
  went wherever the link led.
- **Fix:** a checked path remembers its project folder and checks itself again
  (`Resolved::still_inside`) right before a file tool lists, reads, writes, edits, moves, or
  deletes. On a Mac and Linux the file is also opened without following a link at its last part
  (`O_NOFOLLOW`), so a link made after the check is refused, not followed.
- **Tests:** `a_path_is_checked_again_when_it_is_used` (`paths.rs`),
  `a_folder_that_became_a_link_is_not_used` and `the_last_part_is_never_followed` (`files.rs`).

### G23-4 [Low] A copy or move into a link that leads outside counted as inside

- **Where:** `crates/guard/src/sensitive.rs` (`escapes`).
- **What it did:** for programs that delete, move, copy, or overwrite, an argument outside the
  project folder makes the step ask. The check compared names only, so `cp notes.txt out/` went
  ahead when `out` was a link to somewhere else.
- **Fix:** the check also follows links already on the disk, from the deepest part of the path
  that exists.
- **Test:** `a_path_through_a_link_that_leaves_is_outside` (`sensitive.rs`).

### G23-5 [Low] Running as root

- **Where:** `crates/runtime/src/supervisor.rs` (`launch`).
- **What it did:** Wave 1 made the terminal and the browser refuse when Plenipo runs as root.
  AI tools, commands, and add-ons still started, as root, with the whole computer.
- **Fix:** on a Mac and Linux the supervisor starts nothing for a worker while Plenipo runs as
  root, and says why: "Plenipo is running as root, so everything a worker starts would run as
  root too. Close Plenipo and start it as yourself." Windows does not change.
- **Test:** `nothing_a_worker_starts_runs_as_root` (`crates/runtime/tests/supervisor.rs`).

### G23-6 [Low] An AI tool's short checks were not on the keeper's list

- **Where:** `crates/runtime/src/agent/discovery.rs` (`run_probe_with`, `run_talk`).
- **What it did:** Plenipo checks an AI tool's version and sign-in by starting it for a moment.
  Those checks were grouped and timed, but the keeper did not know them, so a crash in that
  moment could leave one running.
- **Fix:** they are on the keeper's list while they run, and marked (G23-1).

## 4. Checked and found sound

- **Tool tickets on the Mac (ADR-156).** The lookup:
  - asks the Mac's own `lsof` by its full path, with no PATH and a five-second limit;
  - matches both ends of the connection exactly;
  - refuses when anything fails;
  - admits a connection only when every program holding it belongs to the AI tool.

  A program that leaves its group also leaves the AI tool's family, so it is refused.

- **Program names (ADR-150).** A rule that allows names a program exactly on a Mac and Linux; one
  that blocks or asks catches any spelling. A worker cannot name a program by its full path, and
  a link from the project folder to a program elsewhere is refused.
- **What workers' programs get to see.** No screen, session bus, or SSH agent, and a test says
  so. A worker's own code can still find the owner's desktop session by its usual place on Linux.
  That is the same limit as above: approved programs run as the owner (`SECURITY.md`).
- **Saved keys on Linux (ADR-153).** Any program of the owner's can read the desktop's password
  store, as any program can read Windows Credential Manager; a Mac's Keychain asks first. The
  direct tools are blocked now (G23-2).
- **Added program folders.** They come after the system's own, so a program dropped in a home
  folder never takes the place of a system program.
- **The keeper's own input.** On Linux another of the owner's programs could write to it. That
  gives such a program nothing it could not already do itself.
- **`nohup` and `disown`** keep a program in its group, so Stop ends it.
- **Files that run when opened** and **Guard's path rules** on each system's disk: pull request
  #149, tested on GitHub's Mac and Linux machines.

## 5. Not covered here

- What is the same on every system. The October 2 report covers that, and its open findings
  stay as it says (P-GUARD-1, -2, -4, -5, -6, -8). P-GUARD-3 is fixed here (G23-3), and
  P-GUARD-7 was fixed in Wave 1 (`program_path` compares after links).
- Computer use on a Mac (Wave 3) and under Wayland (Wave 4), Mac signing, the installers, and
  installing updates (Waves 2 and 3). Each gets its own check when it is built.
