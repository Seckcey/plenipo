# Security policy

Plenipo runs AI workers on your own computer with real permissions — files, programs, git, a
browser, and, when you allow it, the screen, mouse, and keyboard. Security reports are taken
seriously.

## Supported versions

Plenipo is under active development and only the latest release is supported. Please reproduce
against the newest version before reporting.

| Version        | Supported |
| -------------- | --------- |
| Latest release | Yes       |
| Anything older | No        |

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

**Current intake status (September 27, 2026):** GitHub private vulnerability reporting is
disabled for this repository. There is no verified private reporting address documented here.
Maintainers need to enable private reporting or publish a monitored private contact.

Check the [Security tab](https://github.com/Seckcey/plenipo/security) for an available
**Report a vulnerability** button. If it is absent, do not put vulnerability details, exploits,
logs, or secrets into public issues or discussions. You may ask in
[Discussions](https://github.com/Seckcey/plenipo/discussions) for a private reporting channel
without including any sensitive details. No response-time commitment is made while intake is unavailable.

## What counts as a vulnerability here

Plenipo's security promises, in plain words — a way around any of these is a vulnerability:

- A worker stays inside its project's folder. It cannot read or change files outside it.
- Files on your blocked list (`.env` files, keys, and the like) are never read or changed by a
  worker: not through the file tools, and not through the git tools either, which never stage,
  show, or commit one. A push that would send one is not refused, but its approval card names
  the file so you can decide.
- A worker cannot do anything its permission set does not allow.
- Sensitive actions — deploying, DNS, passwords, payments, publishing, running as administrator —
  stop and wait for the owner's approval.
- Secrets live in the Windows Credential Manager. Workers never see them, and secrets are redacted
  from the record. A stored secret is given only to the installed program the owner named, found on
  PATH — never to a file inside a project folder with the same name — and a program that would be
  given one asks the owner first, with the secret's name on the approval card, unless the owner's
  rule names both the program and the secret (ADR-043).
- Plenipo's browser uses its own profile. Your own browser, your sign-ins, and your saved
  passwords are never used. Plenipo controls its browser over a private pipe between the two
  programs, not a network port, so no other program on your computer can connect to the browser
  and drive it. Plenipo's browser never saves files to your computer: it refuses every download
  from the moment it starts, and the worker is told why its click did nothing (ADR-042).
- Workers never type passwords or secrets. Plenipo can handle some CAPTCHAs automatically and
  can hand checks to the owner. It uses no solving service. Behavior and results depend on the
  installed version, browser policy, and website. Follow the [release notes](https://github.com/Seckcey/plenipo/releases)
  for changes; neither successful completion nor permission from a website is guaranteed.
- Sending, buying, and signing in ask for approval by default. The owner can explicitly enable
  the corresponding **without asking** switches for allowed websites. Other permission and
  Guard checks still apply; these switches are off by default.
- In Plenipo's browser, the network gate works like this (ADR-035):
  - **Asked before the action:** a click or key press whose control looks like sending, buying,
    or signing in; Enter in any text box (a form field, a chat or comment composer, inside a form
    or not); and any click, Enter, or Space on a page that has a live connection (a WebSocket),
    because Plenipo cannot see what goes through one.
  - **Held for approval:** data the page sends right after a worker's click or key press — any
    page, form, script, or beacon request (`Document`, `XHR`, `Fetch`, `Ping`, `Other`) that is
    not a plain read (GET, HEAD, OPTIONS) — until the owner approves or refuses.
  - **Stopped, and the worker told:** data the page tries to send on its own, with no worker
    action running (a form it submits by itself, a script's POST on a timer, a beacon).
  - **Not seen:** what goes through a live connection (WebSocket frames) or a plain page load
    (a link that changes something with a GET); those are covered only by the asks above.
  - **Kept in one tab:** a page never gets a second tab (ADR-041). A new tab it opens (a link
    to a new tab, `window.open`, a form aimed at a new window) is closed before it loads; during
    a worker's action, the worker's own tab goes to that address instead, checked like any page.
    The worker is told either way.
- Taking control of the screen, mouse, or keyboard asks the owner every time.
- Everything a worker does is recorded in the Ledger and the Activity trail.

Programs a worker is allowed to run (the approved list, and anything you approve when asked)
run with your own account, the same as if you had started them. Plenipo checks which program
starts, and it gives a worker's tools only to the AI tool's own program and the programs that
AI tool starts (ADR-034); it does not yet put those programs in a sandbox (a box that limits
what a program can touch). A project's own build and test scripts run as part of a program
like `cargo test` or `npm run`, so treat a project's scripts as code you trust, and approve
script runners only for projects you trust.

## Out of scope

- Whatever the AI models themselves decide to write or say. Plenipo constrains what a worker
  _can do_, not what a model thinks.
- Problems in Claude Code, Codex, Grok, Kimi, or Ollama themselves — report those to their
  vendors.
- Anything that needs an attacker to already be signed in as the owner on that Windows account.
