# Phase 16 — Acceptance Report (Wave 2)

|              |                                                                                                                                                                               |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 16 — Every AI Model Worth Having, **Wave 2** ("one AI tool, one decision record each")                                                                                        |
| **Branch**   | `claude/laughing-archimedes-1s9onb`                                                                                                                                           |
| **Verified** | In progress: see section 3.                                                                                                                                                   |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                     |
| **Result**   | GitHub Copilot built (ADR-083); Cursor's agent a written finding (ADR-084). Version **1.15.0**. The walk-through with the real Copilot on Windows is the owner's (section 6). |

## 1. What Wave 2 set out to do → result

| #   | Wave 2 (ROLLOUT_PLAN.md)                                                       | Result      | Evidence                                                                                                                                                                                                                                           |
| --- | ------------------------------------------------------------------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Cursor's agent (its own models plus Anthropic's, OpenAI's, Google's, xAI's, …) | **Finding** | [ai-tools-cursor-finding.md](ai-tools-cursor-finding.md), ADR-084 (Cursor's agent waits): nothing a program can run says whether Cursor may charge for on-demand use.                                                                              |
| 2   | GitHub Copilot, second try, through its `--headless --stdio` mode              | **Done**    | ADR-083 (GitHub Copilot as an AI tool, checked before every task): the adapter, its stand-in, the whole contract suite, screen tests, and end-to-end tests. Step 0 on the owner's PC ([evidence](evidence/phase-16-wave-2/owner-check/README.md)). |

## 2. The owner's choices → as built

| Choice                          | Answer                             | As built                                                                                                                         |
| ------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 1. Cursor                       | As recommended ("Grok is cursor.") | A written finding and ADR-084; xAI's models stay on Grok.                                                                        |
| 2. Which Copilot sign-in counts | "accept either"                    | **Copilot sign-in** (`user`) and **GitHub CLI sign-in** (`gh-cli`) both count; a token, a key, or an unknown sign-in never does. |
| 3. What Copilot workers do      | As recommended: text answers only  | All of Copilot's own tools off; a tool that runs anyway, or a model billed per use, stops the task; `accepts_tools: false`.      |

## 3. Plan tests → evidence

To be completed with the review and the end-to-end run.

## 6. Checked by the owner on Windows

- GitHub Copilot through Plenipo: its card, one task, the same conversation again, a cancel, and
  a refused check (paid extra use on, or signed out).
