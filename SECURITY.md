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
- A worker cannot do anything its permission set does not allow.
- Sensitive actions — deploying, DNS, passwords, payments, publishing, running as administrator —
  stop and wait for the owner's approval.
- Secrets live in the Windows Credential Manager. Workers never see them, and secrets are redacted
  from the record.
- Plenipo's browser uses its own profile. Your own browser, your sign-ins, and your saved
  passwords are never used.
- Workers never type passwords or secrets. Plenipo can handle some CAPTCHAs automatically and
  can hand checks to the owner. Behavior and results depend on the installed version, browser
  policy, and website. Follow the [release notes](https://github.com/Seckcey/plenipo/releases)
  for changes; neither successful completion nor permission from a website is guaranteed.
- Sending, buying, and signing in ask for approval by default. The owner can explicitly enable
  the corresponding **without asking** switches for allowed websites. Other permission and
  Guard checks still apply; these switches are off by default.
- Taking control of the screen, mouse, or keyboard asks the owner every time.
- Everything a worker does is recorded in the Ledger and the Activity trail.

## Out of scope

- Whatever the AI models themselves decide to write or say. Plenipo constrains what a worker
  _can do_, not what a model thinks.
- Problems in Claude Code, Codex, Grok, Kimi, or Ollama themselves — report those to their
  vendors.
- Anything that needs an attacker to already be signed in as the owner on that Windows account.
