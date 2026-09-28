# ADR-043: Secrets reach only the programs they are for

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 7 (follow-up)
- **Amends:** [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md),
  sections 7 (commands) and 12 (the Vault); builds on
  [ADR-034 (approved programs run as the owner)](ADR-034-approved-programs-run-as-the-owner.md),
  decision 3

## Context

The owner binds a stored secret to a program by its name: "give the GitHub token to `gh`", "give
the deploy key to `printenv`". The Vault keeps the value; when a worker's program runs, Plenipo
sets the variable for it and tells the worker only the secret's name.

Two things about that hand-over were looser than they should be:

1. **The name alone decided.** Plenipo matched the file name of whatever it was about to run. A
   worker may run a file inside the project folder by naming it as a path (`./gradlew`), and
   such a file has whatever name the project gives it. So a project file named like a trusted
   program was handed that program's secret when its run was approved or on the approved list.
2. **An approved rule quietly unlocked the secret.** When a command matched an approved rule,
   it ran without asking, and the secret came with it. A secret bound to a program that runs
   scripts (`python`, `node`, `bash`, ...) therefore reached every script a worker ran with that
   program under a rule such as `python *`.

Redaction hides the exact value in results and in the trail, but by then the value had already
been given out. ADR-034 closed this for `npm`, `pnpm`, `yarn`, `make`, and `npx`, which never get
stored secrets; this record covers the rest.

## Decision

1. **Only the installed program gets a secret.** A stored secret is given only to a program
   Plenipo found on PATH, or in its own search folders and known places (Windows PowerShell's
   folder). A program the worker names as a path inside the project folder is the project's own
   file, whatever it is called: it gets no stored secret, and when one is bound to that name the
   worker's result says so: "(Plenipo gives a stored secret only to the installed program of
   that name, so no stored secrets were given (the program is not from PATH).)" ADR-034 stands:
   `npm`, `pnpm`, `yarn`, `make`, and `npx` never get stored secrets, and the worker is told.
2. **A run that would be given a secret asks first**, even when its command is on the approved
   list, unless one of the owner's rules names both the program and the secret. Guard's command
   rules gain a fourth list, "with secrets": each entry is a command rule written like an approved
   command (`gh pr *`) plus the names of the stored secrets it may be given ("GitHub token"). Such
   an entry approves the command too, so one line does both. Every secret the run would be given
   must be named by an entry that matches the command; a PowerShell script has no command a rule
   could name, so a script that would be given a secret always asks. The always-ask list, the
   blocked list, and an "Ask me" permission still come first. Guard's reason on the card reads
   "Run gh pr list needs your approval: it would be given the stored secret GitHub token." The
   owner can always approve on the card; Settings → Permissions says so next to the command lists,
   and the editor for the new list comes later.
3. **The card names the secrets.** The approval card's details end with a line "Will be given:
   GitHub token" — names only, never values. The same detail is recorded with the call in the
   trail (`capability.used`).
4. **Interpreters carry a warning.** When a secret is bound to `node`, `python`, `python3`,
   `bash`, `sh`, `zsh`, `pwsh`, `powershell`, `cmd`, `deno`, or `bun`, Settings → Secrets shows
   "Every script run with `<program>` would get this secret." next to the binding, in the list and
   in the form as the programs are typed. The warning is the Settings screen's own (the list of
   names lives in `SecretList.tsx`); Guard needs no new field for it.
5. **Plenipo's own git and GitHub tools are unchanged.** They run `git` and `gh` from PATH with
   Plenipo's own arguments under their own permissions (git and GitHub, read and write). A secret
   bound to `gh` or `git` reaches them as before, so a stored GitHub token keeps working for the
   pull-request tools without a card for each listing. The new asking applies where the worker
   chooses what runs: `run_command` and `run_powershell`.

## Consequences

- A project's own files never see a stored secret. A project script that needs one is run
  another way: the worker runs the installed program itself (`gh ...`), or the owner runs the
  script.
- Owners who bound a secret to a program on their approved list see an approval card for each
  such run until they add a rule naming the program and the secret. Until Settings can edit that
  list, the card is the way; the reason and the "Will be given" line say exactly why it asks.
- A secret bound to an interpreter is still possible (a token for `python -m twine upload` is a
  real need), but the owner is warned where it is set, and every such run asks first.
- The `guard` setting's `commands` gains `withSecrets` (empty in older documents). Saving the
  command lists from Settings keeps it as it is.
- Tests: Guard's engine (`a_stored_secret_asks_unless_a_rule_names_program_and_secret`) and
  configuration (`rules_are_validated`); the broker's unit tests
  (`stored_secrets_go_only_to_programs_from_path`,
  `a_program_in_the_project_folder_is_not_the_installed_one`,
  `stored_secrets_never_go_to_script_runners`); and end to end
  (`a_stored_secret_asks_first_unless_a_rule_names_program_and_secret`, and
  `plan_secret_redaction` now runs under a rule naming program and secret).

## Alternatives considered

- **Comparing the program's full path with a list of known install folders.** PATH already is
  the owner's list of installed programs, and it is the same on every computer Plenipo runs on;
  a hand-kept list would go stale.
- **Asking for Plenipo's own `gh` and `git` runs too.** Every pull-request listing would then
  need a card. Those runs use Plenipo's arguments, and the owner's GitHub and git permission levels
  already say what they may do.
- **Writing the secret names into the approved list's lines** (`gh pr * +GitHub token`). One
  list would carry two meanings; a separate list with a rule and names is clearer, and a later
  editor can show it as a table.
- **Never giving a secret to an interpreter.** It would break real uses; a warning where the
  binding is made plus asking before each run keeps them possible with the owner's eyes on them.
