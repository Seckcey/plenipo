# Frequently asked questions

## Who is Plenipo for?

Developers and small teams who use several AI coding tools and want one place to organize
projects, delegate work, review results, and decide what workers may do. Start with one project
and a small objective you can review.

## Is it offline? Does my work stay on my PC?

Plenipo's control plane, project folders, and SQLite activity record are local. Connected AI
providers process prompts and task context through their services. Local-first does **not** mean
offline inference or that no project content leaves the computer. Provider terms, subscriptions,
availability, and usage limits still apply.

The Ollama adapter currently uses **Ollama cloud models**, not local models. Its workers answer
in text and cannot read files or run programs. See the [AI tool setup guide](development/setup.md#3-ai-tools-claude-code-codex-grok-kimi-and-ollama-optional).

## Do I need API keys or all five providers?

No. Plenipo uses the supported tools' account sign-ins and rejects API-key authentication.
Install and sign in to at least one supported tool to run workers. No provider login is needed
just to build and open the app. Provider subscriptions and their limits are separate from Plenipo.

## Which computers are supported?

The published installer targets **Windows 11 x64**. Linux is used for development and automated
tests; it is not a published desktop distribution. macOS is not a current target. The SSH feature
connects to Linux/Unix servers; Windows servers are not supported by that feature.

## Why does the screenshot look different from my download?

The README labels its real app capture as **v1.7.0 development**, with synthetic test data.
At the September 27, 2026 documentation review, **v1.6.0** was the latest published installer.
A version number on the main branch does not mean that installer has been released.
The [latest release](https://github.com/Seckcey/plenipo/releases/latest) is the download source of truth.

## Is Plenipo free? What is Pro?

The published v1.6.0 app has no license check or edition limits. The Free/Pro split and subscription
pricing described in the [edition plan](editions.md) are planned, not a currently shipped purchase
flow. Plenipo's price does not include AI provider subscriptions.

## Is it open source?

Plenipo is **source-available under the Elastic License 2.0**. Read the [license](../LICENSE)
for the actual permissions and restrictions. The public source and contribution process do not
change those terms. [Contributor licensing terms](../CONTRIBUTING.md#how-contributions-are-licensed)
also apply to submitted changes.

## What can a worker do without asking?

That depends on the role's permissions, the project, and your Settings switches. Roles can be
allowed, asked, or blocked for supported actions. Sending, buying, and signing in ask by default;
you can explicitly allow those actions without asking on allowed websites. Production SSH
commands ask every time. Review your permissions before delegating, and use **Stop all** or
**Take over** when needed. See [the security policy](../SECURITY.md) and the
[v1.6.0 release notes](https://github.com/Seckcey/plenipo/releases/tag/v1.6.0) for known limits.

## Where can I get help or suggest a feature?

Use [Discussions](https://github.com/Seckcey/plenipo/discussions) for questions and ideas, and
[issue forms](https://github.com/Seckcey/plenipo/issues/new/choose) for reproducible bugs.
The [support guide](../SUPPORT.md) explains what information to include and how to keep reports safe.
