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

Use GitHub's private reporting instead:

1. Go to the [Security tab](https://github.com/Seckcey/plenipo/security).
2. Choose **Report a vulnerability**.
3. Describe what you found, how to reproduce it, and what an attacker could do with it.

Only the maintainers can see that report. You should get a first response within a few days. If a
fix is needed, we will agree on a disclosure date with you before anything is published.

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
- Workers never type passwords or secrets, and never get past a CAPTCHA.
- Submitting a form, buying, signing in, and sending anything always wait for the owner's
  approval, with a screenshot.
- Taking control of the screen, mouse, or keyboard asks the owner every time.
- Everything a worker does is recorded in the Ledger and the Activity trail.

## Out of scope

- Whatever the AI models themselves decide to write or say. Plenipo constrains what a worker
  _can do_, not what a model thinks.
- Problems in Claude Code, Codex, Grok, Kimi, or Ollama themselves — report those to their
  vendors.
- Anything that needs an attacker to already be signed in as the owner on that Windows account.
