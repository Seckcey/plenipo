# ADR-033: Approved programs run as the owner

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 7 (follow-up)
- **Amends:** [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md),
  sections 3 (the relay), 7 (commands), and 12 (the Vault)

## Context

When a worker runs a program through Plenipo's tools, the program runs with the owner's own
account: it can read every file the owner can, and reach the network. Plenipo checks _which_
program starts (the approved list, or an approval), but it does not yet put the program in a
sandbox (a box that limits what a running program can touch).

Many everyday programs run the project's own scripts. `npm run build`, `pnpm run test`,
`yarn run lint`, `make test`, and `make build` do nothing but run whatever the project's
`package.json` or `Makefile` says. `cargo test`, `python -m pytest`, and `./gradlew test` run the
project's own code too. So an approved program is only as safe as the project it runs in.

A review found what such a program could do while a worker's step runs:

1. **Use another worker's tools.** Every open grant has a ticket file in Plenipo's private
   folder. The relay reads its own ticket and connects to Plenipo's tool server, which
   admitted any connection that presented a valid ticket. A program run by one worker could
   read a second worker's ticket, connect as that worker, and have its calls carried out and
   recorded under that worker's name and permissions.
2. **Get stored secrets meant for the program, not for the scripts.** A secret bound to `npm`
   (say, a registry token) is given to `npm` as a variable, and every script `npm run` starts
   inherits it.

## Decision

1. **A ticket is bound to the AI tool's process tree.** When a connection presents a valid
   ticket, the tool server finds the program at the other end of the connection (Windows: the
   TCP table with owning process IDs; Linux: `/proc/net/tcp` and each program's open files)
   and checks that it is the AI tool Plenipo started for that grant's step, or a program that
   AI tool started (following parent process IDs). The relay always is, since the AI tool
   starts it. Any other program is closed without a word, like a bad ticket, and the owner
   sees `Blocked: a program outside …'s AI tool tried to use …'s tools` in the Activity trail
   (`tool_server.ticket_refused`, with both program IDs; never the ticket). When the lookup
   itself fails on Windows or Linux (the connection table cannot be read, or no program is
   found holding the connection), the connection is refused the same way: a failed check never
   lets a program through. Only where the operating system offers no way to tell at all
   (macOS) is the connection served, and both a notice and a trail event say the check was not
   possible (`tool_server.ticket_unchecked`).
2. **Script runners are not approved for every project.** The default approved list no longer
   has `npm run *`, `pnpm run *`, `yarn run *`, `make test *`, or `make build *`. `cargo test *`,
   `cargo build *`, `python -m pytest *`, and `./gradlew test *` stay, since a developer cannot
   work without them; the Settings screen says next to the list that approved programs run
   with the owner's full account, and to approve script runners only for projects the owner
   trusts. Only the defaults for new installations change: an owner's saved lists are never
   rewritten.
3. **Script runners get no stored secrets.** Plenipo never gives a stored secret to `npm`,
   `pnpm`, `yarn`, `make`, or `npx`, even when the owner bound the secret to that program. The
   worker's result says so: "(Plenipo does not give stored secrets to npm, pnpm, yarn, make, or
   npx, because they run the project's own scripts.)" A secret bound to `gh` or `cargo` is given
   as before.

## Consequences

- A stolen ticket is useless to any program the AI tool did not start. The check adds a
  moment to each connection (one table read and a few parent lookups).
- On macOS the binding cannot be checked yet; the owner is told, and the other two decisions
  still apply.
- New installations ask before `npm run`, `pnpm run`, `yarn run`, `make test`, and `make build`.
  Owners who trust a project add them back in Settings, as before.
- A secret an owner bound to a script runner is no longer given; the owner binds it to the
  program that really needs it (for example `gh`), or runs the script another way.
- Plenipo had no unsafe code (the workspace forbids it). The Windows lookup of which program
  holds a connection needs calls into Windows, which Rust counts as unsafe. The capability
  broker crate now denies unsafe code instead of forbidding it, and allows it in that one
  module only, with the reason each call is sound written next to it. Anywhere else in the
  crate, unsafe code is still a build error.
- The follow-up is a sandbox for approved programs (a limit on which folders and which
  network they may use), so that what a program starts is checked, not only what it is.

## Alternatives considered

- **Keeping tickets in memory only and passing them on the relay's command line.** A program
  running as the owner can read the command lines of the owner's other programs, so this moves
  the problem without solving it.
- **One ticket per connection, used once.** An AI tool may start the relay more than once
  during a step, so a ticket has to keep working for the same AI tool. Binding to the process
  tree lets it, and still shuts out every other program.
- **Removing every program that runs project code from the defaults.** `cargo test` and
  `pytest` are the reason most developers get the program permission at all; taking them away
  would move every build and test through an approval, and owners would approve them without
  reading. A clear note next to the list is more honest.
- **Sandboxing approved programs now.** The right fix, and a larger one (job objects and
  restricted tokens on Windows, namespaces on Linux); it is the follow-up, not this change.
