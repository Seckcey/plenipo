# ADR-213: Build and test commands ask first under Careful

- **Status:** Proposed (the Development Coordinator chose the option below on 2026-10-03; the owner
  accepts it at review)
- **Date:** 2026-10-03
- **Phase:** none (security hardening, finding P-GUARD-1 of the 2026-10-02 security review)
- **Amends:** [ADR-034 (approved programs run as the owner)](ADR-034-approved-programs-run-as-the-owner.md),
  decision 2 (which commands Plenipo approves to start with). ADR-034 itself is not changed.
- **Made by:** 8 West Ventures, LLC, for Plenipo.

## In short

Plenipo used to start with about fifty build, test, and lint commands on the **Approved** list:
`cargo test`, `npm test`, `pytest`, `make check`, `./gradlew test`, and more. Each of them runs
code that the project holds. A worker that can change the project's files could put any program
there and then run it through an approved command, with no approval card. The list's own comment
said script runners were not approved for every project, but the list approved them anyway.

Now Plenipo starts with only four approved commands, which read a project's files and never run
its code: `tsc`, `ruff`, `black`, and `gofmt`. **Under Careful and Strict, build and test commands
now ask each time. Under Light (where Plenipo starts) nothing changes**, because under Light every
program on no list already runs without asking ([ADR-201 (light by default)](ADR-201-light-by-default.md)).

Accepting this record means: new installs start with the shorter list; once, an older install loses
the old defaults it still has word for word; and a program inside the project folder is approved
only by a rule that names its own path.

## Context

What the code did at `84719ee6`:

- `crates/guard/src/defaults.rs`, `default_commands`, approved `cargo build/check/test/fmt/clippy/
doc/tree`, `npm test`, `npx tsc/eslint/prettier/vitest/jest`, `pnpm test/lint/build/typecheck/
check`, `yarn test/build/lint`, `tsc`, `eslint`, `prettier`, `vitest`, `jest`, `pytest` (and the
  `python -m` and `py -m` forms), `python -m unittest`, `ruff`, `black`, `mypy`, `go build/test/vet`,
  `gofmt`, `dotnet build/test/format`, `mvn test/verify/package`, `gradle test/build`,
  `./gradlew test/build`, and `make lint/check`.
- `crates/guard/src/engine.rs`: an approved match runs with no card when Safety is Careful. Under
  Light the approved list does not matter: a program on no list runs too.
- `crates/capabilities/src/broker.rs`, `program_path`: a program named `./path` is a file inside the
  project folder, and Guard sees it as `./path`. On Windows `program_key` drops `.bat`, `.cmd`,
  `.exe`, and `.com`, so a `gradlew.bat` the worker wrote matched `./gradlew test *`.
- There is no "approve once for this project" memory: [ADR-013](ADR-013-guard-capability-broker.md)
  decided against "remember this", and ADR-201 kept that. An approval card covers one call.

Which of these run the project's own code: `npm`, `pnpm`, and `yarn` scripts, and `make` targets,
are the project's own; `cargo` runs build scripts and procedural macros, and reads aliases from
the project's `.cargo/config.toml` (`fmt` is not a built-in command, so a project alias can take
its name); tests and their setup files are code (`conftest.py`, `jest.config.js`, `go test`,
`dotnet test`, Maven and Gradle plug-ins and build files); `eslint`, `prettier`, and `mypy` load
settings files or plug-ins that are code; `npx` starts the project's own copy of a program;
`python -m` looks in the project folder first; `go build -toolexec` and `go vet -vettool` start
any program they name; `dotnet format` loads the project's analyzers.

## Decision

1. **The starting approved list** is `tsc *`, `ruff *`, `black *`, and `gofmt *`. Every other old
   default leaves it. None of them goes on the always-ask list: that list asks even under Light,
   and would put a card in front of every build.
2. **Once, for an older install.** The saved settings get a version number (`commandsVersion`,
   missing in older documents: 0). At start, a document below version 1 loses every approved entry
   that is still word for word an old default and is no longer a default. The owner's own lines
   and lines the owner changed stay. The document then has version 1, so a line the owner adds back
   is never taken off again. A new install starts at version 1. The event
   `guard.commands_trimmed` (by Plenipo, payload `removed`) is written even when nothing was
   taken off, so the move is done once. The Activity trail says: "Build and test commands that run
   a project's own code left your approved list, so under Careful they now ask first (Light is
   unchanged): …", or "Your approved commands list was checked: nothing needed to change".
3. **A program inside the project folder** (Guard sees `./path`) is approved, under Careful, only
   by an approved rule, or a rule that names a program and its secrets, whose program is that path
   with no `*` or `?` (`./gradlew test *` approves `./gradlew test`; `* *`, `./* *`, and
   `./grad* *` do not). An installed program is matched as before. This is decided in
   `engine.rs` from the command's own `./` (`commands::rule_approves`); Guard's request is not
   changed.
4. **Light and Strict are unchanged.** Under Light a program on no list, a program in the project
   folder included, runs without asking; under Strict workers run no programs.

## What a person sees

- **Light** (the default): nothing changes.
- **Careful**: `cargo test`, `npm test`, `pytest`, `make check`, `./gradlew build`, and the other
  build and test commands show an approval card each time, with "needs your approval: it is not on
  your approved commands list". The owner can add a command to **Approved** for a trusted project.
- **Settings → Permissions**: a new install's **Approved** list shows four lines. An older install
  loses the old default lines it never changed, once, and the Activity trail lists them.

## Consequences

- Under Careful, a developer who builds and tests a lot sees more cards, or adds those commands
  back. That is the trade ADR-034 described; this record makes the starting list match its words.
- The report's other ideas are not built here: a one-time sentence in the "Hire a Developer" flow,
  and an "approve once for this project" memory. The second would reverse ADR-013's decision and
  needs the owner's choice.

## Known gap

A program found through **PATH**, by its bare name, that lies inside the project folder (a PATH
entry pointing into the project) is the project's own file, and the broker already gives it no
stored secret (ADR-048, ADR-150). Guard still sees its bare name, so an approved rule for that name
(say `tsc *`) approves it. Plenipo itself searches no folder inside a project; this needs the
owner's own PATH to point into a project. Closing it means telling Guard where the program came
from, a new field in Guard's request, which is left for later.

## Alternatives considered

- **Put the build and test commands on the always-ask list** (the review's first suggestion).
  Rejected: that list asks under Light too, so every build and test would show a card, against the
  owner's Light direction (ADR-201).
- **Remember an approval for each project.** Rejected for now: a new feature (a store, a card
  button, a way to clear it), and it reverses ADR-013's "no remember this". The owner can schedule
  it.
- **Keep `cargo fmt`, `cargo tree`, `go build`, `go vet`, and `mypy`.** Rejected: each can be made
  to run a program the project names (a cargo alias, a credential helper, `-toolexec`,
  `-vettool`, a mypy plug-in).
