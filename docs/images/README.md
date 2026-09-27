# Screenshots

Screenshots are the single biggest thing missing from the README. A visitor decides in about five
seconds whether Plenipo looks real, and a picture of the Organization map does that far better than
any paragraph.

Drop PNGs in this folder with these exact names, and the README picks them up — the image blocks are
already written there, commented out, waiting for the files.

| File                 | What to capture                                                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `organization.png`   | The **Organization** view with a department, a project, and a few workers on the job. The money shot — do this one first. |
| `approval.png`       | An approval card waiting on a sensitive action, showing exactly what will run.                                            |
| `permissions.png`    | **Settings → Permissions**, with a role's permission set open.                                                            |
| `activity-trail.png` | The Activity trail on a browser task, with a screenshot of the page in it.                                                |
| `models.png`         | **Settings → AI models**, showing which model each role gets and why.                                                     |

## How to take them

1. Run the app: `pnpm dev`, or install a release build.
2. Set the window to about **1600×1000** so the PNG stays sharp but not enormous.
3. Use **Win + Shift + S** (Snipping Tool) and capture the app window only — no desktop, no taskbar,
   no other windows.
4. Save as PNG in this folder under the name above.

## Before you commit one

- **No real data.** No client names, no real project or folder names, no email addresses, no file
  paths with your username in them, no API keys or tokens on screen. Use made-up names like
  "Website" and "Acme".
- Keep each file under about 500 KB. If a shot is heavier, scale it to 1600 px wide.
- Light or dark, but be consistent across all five.

Then open the README, find the commented-out image block for that screenshot, and uncomment it.
