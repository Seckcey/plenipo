# Contributing to Plenipo

Thanks for taking an interest. Plenipo is a Windows desktop app: a React + TypeScript front end
over a Rust core, packaged with Tauri 2. This page covers how to get it running, the checks your
change has to pass, and the few house rules that keep the codebase consistent.

## Before you start

- **Small fix or an obvious bug?** Open a pull request directly.
- **New feature, new screen, or anything that changes how Plenipo behaves?** Open an issue or a
  discussion first. Plenipo is built phase by phase against
  [`ROLLOUT_PLAN.md`](ROLLOUT_PLAN.md), so it helps to agree on where a change fits before you
  write it.
- **Security problem?** Do not open an issue. See [`SECURITY.md`](SECURITY.md).

## Your first contribution

You do not need to build the app to improve a confusing documentation step or report a
reproducible bug. Useful starting points:

- Check the Windows setup instructions against your own machine and describe the exact missing step.
- Submit a small bug reproduction with the version and expected result; see [SUPPORT.md](SUPPORT.md).
- Contribute a real app screenshot with its version and synthetic or sanitized data. See
  [the screenshot guide](docs/images/README.md). Do not submit generated UI mockups as product evidence.
- Browse [open issues](https://github.com/Seckcey/plenipo/issues), and comment before starting a larger change.

## Getting set up locally

Full step-by-step Windows instructions, including the Rust and Node versions and the optional AI
tools, are in [`docs/development/setup.md`](docs/development/setup.md).

```powershell
git clone https://github.com/Seckcey/plenipo.git
cd plenipo
corepack enable
pnpm install
pnpm dev
```

You do not need any API keys, provider accounts, or a `.env` file to build and launch Plenipo. To
actually run workers you need at least one AI tool installed and signed in — Claude Code, Codex,
Grok, Kimi, Ollama, or Antigravity.

## The checks your change must pass

Run all of these locally before you push. CI checks the frontend and Rust on Linux and also
builds, tests, and smoke-tests the Windows installer. Explain any check you could not run;
do not describe an unrun check as passing.

```powershell
pnpm check                                                   # versions, format, lint, typecheck, frontend tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm bindings                                                # then confirm no diff in packages/types/src/generated
```

`pnpm bindings` regenerates the TypeScript types from the Rust types. Never hand-edit anything in
`packages/types/src/generated` — change the Rust type and regenerate.

## House rules

1. **Plain words on screen.** Every word a person can see in Plenipo uses simple, everyday
   language. "AI tool", not "runtime". The chain of command reads Worker → Supervisor → Manager →
   VP → President. The full list of preferred words is
   [`docs/design/vocabulary.md`](docs/design/vocabulary.md) — add a pair there whenever you replace
   a word. Internal code names stay as they are.
2. **Decisions get written down.** Anything that changes the architecture, or deviates from the
   rollout plan, gets an Architecture Decision Record in [`docs/adr/`](docs/adr/README.md). Name
   decisions when you refer to them — "ADR-010 (plain words and rank names)", not just a number.
3. **Tests come with the change.** Rust logic gets unit tests in its crate, UI logic gets Vitest
   tests, and a user-visible flow gets an end-to-end test in [`tests/e2e/`](tests/e2e).
4. **No unsafe Rust.** The workspace forbids it.
5. **Permissions are not a suggestion.** Anything that touches files, programs, the network, the
   browser, or the screen goes through Plenipo Guard and the capability broker. Do not add a path
   around them.

## Commit messages and pull requests

Commits follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(browser): …`,
`fix(guard): …`, `docs(readme): …`, `test(e2e): …`, `ci(release): …`.

Keep a pull request to one subject. Say what changed, why, and how you checked it. If the change
is visible on screen, include a screenshot.

## How contributions are licensed

Plenipo is licensed under the [Elastic License 2.0](LICENSE), with Free and paid Pro editions
planned ([`docs/editions.md`](docs/editions.md)). So that contributed code can go into both:

By opening a pull request, you confirm that the work is yours to give, and you license it to 8 West
Ventures, LLC under the Elastic License 2.0 together with a perpetual, worldwide, royalty-free,
irrevocable right to use, change, and distribute it — including in the Pro edition, and under any
license 8 West later applies to Plenipo. You keep the copyright in your own contribution.

If your employer owns your work, get their sign-off before you contribute.

## Where things live

See the repository layout in the [README](README.md#repository-layout) and the
[architecture overview](docs/architecture/overview.md).
