# GitHub repository settings

Some of how the repository presents itself cannot live in a file — it is stored in GitHub's own
settings. This page is the record of what those settings should say, so they can be redone after a
mistake, checked at a glance, or copied to another repository.

Everything here is set in the browser, signed in as the repository owner.

## The About panel

Repository home page → right sidebar → **About** → the gear icon.

**Description** — paste exactly:

```text
Run your own AI workforce on your PC. Hand it an outcome and a team of AI workers, supervisors, and managers gets it done — with permissions, approvals, and a full record. Uses your Claude Code, Codex, Grok, Kimi, and Ollama sign-ins, not API keys.
```

**Website** — the 8 West product page for Plenipo once it exists. Until then:
`https://github.com/Seckcey/plenipo/releases/latest`, so a visitor's first click is a download.

**Topics** — paste these in one at a time, or paste the whole line and press Enter after each.
Topics are how GitHub's own search and its topic pages find Plenipo, so this is the single highest
-value field on the page. Twenty is GitHub's maximum:

```text
ai-agents ai-workforce agent-orchestration multi-agent multi-agent-systems autonomous-agents ai-orchestration claude-code codex grok ollama local-first desktop-app tauri tauri-app rust react typescript windows developer-tools
```

**Tick boxes** in the same dialog: **Releases** on, **Packages** off, **Deployments** off.

## Features

**Settings → General → Features.**

| Feature     | Set to  | Why                                                                                                                                                                                       |
| ----------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Issues      | On      | Bug reports. The issue forms in `.github/ISSUE_TEMPLATE/` appear automatically.                                                                                                           |
| Discussions | **On**  | The issue-form footer and the README both link to it. Questions and ideas belong here, not in issues — and a discussion thread keeps people coming back in a way a closed issue does not. |
| Wikis       | **Off** | Empty, and the documentation is in `docs/`. An empty wiki tab looks abandoned.                                                                                                            |
| Projects    | On      | Harmless, and useful once the roadmap moves to a board.                                                                                                                                   |

## Social preview

The image that shows when the repository URL is shared in Slack, Discord, X, LinkedIn, or iMessage.
Without one, all of those show a grey GitHub placeholder.

Export the PNG from `docs/brand/social-preview.svg` and upload it — the full steps are in
[`docs/brand/README.md`](../brand/README.md).

## Labels

**Issues → Labels → New label.** GitHub surfaces repositories on its own
"good first issues" pages using these two exact names, so spelling matters:

| Label              | Color     | Description                                 |
| ------------------ | --------- | ------------------------------------------- |
| `good first issue` | `#2F7BF6` | A small, self-contained first contribution. |
| `help wanted`      | `#0E8A16` | Wanted, and nobody is on it.                |

They only do anything once open issues carry them. Filing three or four small, genuinely scoped
issues and labelling them is worth more than any README wording.

## Pin it to your profile

Your profile → **Customize your pins** → tick **plenipo**. Anyone who lands on the 8 West or
personal profile sees it first.

## Check the result

**Insights → Community Standards.** Every row should be ticked once this branch is merged:
description, README, code of conduct, contributing, license, security policy, issue templates, pull
request template.

## Keeping people interested

Settings get people to the page. These keep them:

- **Screenshots.** The biggest single gap right now. See [`docs/images/README.md`](../images/README.md).
- **Releases with real notes.** Already being done — every release links its notes in `docs/releases/`.
  Keep attaching the Windows installer so "Download" always works.
- **Answer every issue and discussion quickly**, even to say "not soon, here's why". A repository
  that answers looks alive; one that does not looks dead no matter how good the code is.
- **A short demo recording.** Thirty seconds of the Organization view working an objective, as a GIF
  in the README, is worth more than any paragraph in it.
