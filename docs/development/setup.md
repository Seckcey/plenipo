# Developer Setup (Windows 11)

Tested target: Windows 11 x64. Linux works for development and CI; macOS is not a target.

## 1. Prerequisites

Install these once. Versions are minimums.

| Tool                      | Version           | Install                                                                                                             |
| ------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------- |
| Microsoft C++ Build Tools | VS 2022           | [Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) → select **Desktop development with C++** |
| WebView2 Runtime          | Evergreen         | Preinstalled on Windows 11. If missing: [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)        |
| Rust                      | stable (≥ 1.85)   | [rustup-init.exe](https://rustup.rs) — choose the default MSVC toolchain                                            |
| Node.js                   | 22 LTS            | [nodejs.org](https://nodejs.org) or `winget install OpenJS.NodeJS.LTS`                                              |
| pnpm                      | 10 (via Corepack) | `corepack enable` (ships with Node)                                                                                 |
| Git                       | any recent        | `winget install Git.Git`                                                                                            |

Quick install with winget (run PowerShell **as your normal user**; the Build Tools installer
will prompt for elevation):

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
winget install Git.Git
```

Open a **new** terminal afterwards so `PATH` updates, then verify:

```powershell
rustc --version; cargo --version; node --version; corepack enable; pnpm --version
```

The repository's `rust-toolchain.toml` pins the Rust version (currently **1.98**) and makes
rustup install it plus `rustfmt` and `clippy` automatically on first build, so local builds and
CI always use the same compiler and lints. Upgrading Rust is a deliberate one-line change there.

## 2. Clone, install, run

```powershell
git clone https://github.com/Seckcey/plenipo.git
cd plenipo
pnpm install
pnpm dev
```

The first `pnpm dev` compiles the Rust backend (several minutes); later runs are incremental.
A window titled **Plenipo** opens showing the shell with **Core: Connected**.

No `.env` file, API keys, or provider logins are required to build or launch.

## 3. AI tools: Claude Code, Codex, Grok, Kimi, and Ollama (optional)

<a id="3-ai-tools-claude-code-and-codex-phase-3-optional"></a>
<a id="3-ai-tools-claude-code-codex-and-grok-optional"></a>
<a id="3-ai-tools-claude-code-codex-grok-and-ollama-optional"></a>

The **Workers** view runs tasks on the Claude Code, Codex, Grok, Kimi, and Ollama tools that are
already installed **and signed in with your subscription** on this computer. Plenipo never asks
for a password or API key, and refuses API-key sign-ins (no pay-per-use API billing). The desktop
apps do not need to be open.

| AI tool     | Install (PowerShell)                                            | Sign in (once, in a terminal)                                                  |
| ----------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Claude Code | `irm https://claude.ai/install.ps1 \| iex` (native build)       | `claude auth login` — choose your Claude account                               |
| Codex       | `npm install -g @openai/codex` (needs Node.js)                  | `codex login` — choose **Sign in with ChatGPT**                                |
| Grok        | `irm https://x.ai/cli/install.ps1 \| iex` (Grok Build)          | `grok login` — sign in with the X account that has SuperGrok or X Premium Plus |
| Kimi        | Kimi Code's official installer (moonshotai.github.io/kimi-code) | `kimi login` — sign in with the Kimi account that has your Kimi subscription   |
| Ollama      | The installer from ollama.com/download                          | `ollama signin` — finish in the browser                                        |

Then open **AI tools** in Plenipo and choose **Re-check**: each tool should show **Ready**
with its version and "Signed in (subscription)". If a card says what is missing (not installed,
not signed in, API key), follow the hint on the card.

Notes:

- Ollama (ADR-017, Ollama's cloud models through its service on your PC): Plenipo uses only
  **cloud models** (for example `gpt-oss:120b-cloud`), which run on Ollama's servers under your
  ollama.com sign-in, so no graphics card is needed. Ollama must be running (its tray icon);
  Plenipo talks to it only on `127.0.0.1:11434`, never uses an Ollama API key, and ignores
  `OLLAMA_HOST`. On the free plan only some cloud models answer (gpt-oss and Nemotron when
  checked); models marked "(paid plan)" need a paid Ollama plan. The card shows "Signed in" with your plan (for example "Ollama sign-in (free
  plan)"). Ollama workers are conversation only for now: they answer in text and cannot read
  files or run programs. Plenipo keeps each Ollama conversation in its session folder
  (`.plenipo-ollama-<id>.json`) and sends it with every task. To use another cloud model, name
  it for a position (details panel → **Edit title, AI tool, or model**) exactly as
  `ollama list` shows it after `ollama pull <name>`.
- On Windows, Plenipo runs only native `.exe` builds. An npm-installed Claude Code (`claude.cmd`)
  is reported as unsupported — install the native build above. For Codex, Plenipo uses the
  native binary inside the npm package automatically.
- If a Codex task fails with _"The '…' model is not supported when using Codex with a ChatGPT
  account"_, Codex's own settings name a model your ChatGPT plan can't use. Update Codex
  (`npm install -g @openai/codex@latest`). If it still fails, back up
  `%USERPROFILE%\.codex\config.toml` and remove its `model = …` line, or run `codex` and pick
  a model with `/model`. `codex exec --skip-git-repo-check "Say hi"` should then answer. You
  can also name a model for one position in Plenipo (details panel → **Edit title, AI tool, or
  model**).
- Grok (xAI's Grok Build, checked with version 1.0.41). The installer puts `grok.exe` in
  `%USERPROFILE%\.grok\bin`; open a new PowerShell window afterwards so `grok` works there (Plenipo
  also looks in that folder). To check the sign-in yourself, run `grok models`: its first line
  should say you are signed in, not "You are not authenticated." or "You are using XAI_API_KEY.".
  What Plenipo checks and does:
  - Before every task it runs `grok models`. A key in `XAI_API_KEY`, or a key set on a model in
    `%USERPROFILE%\.grok\config.toml`, makes Grok report an API key, and Plenipo refuses to run it.
  - It starts Grok with `GROK_DISABLE_API_KEY_AUTH=1`, so Grok itself refuses API keys, and it
    never passes `XAI_API_KEY`.
  - Grok's one-task mode cannot take the task text on its input, so Plenipo runs
    `grok agent --no-leader stdio` and talks to it over ACP ([ADR-015](../adr/ADR-015-acp-ai-tools.md),
    running AI tools over ACP): one program per task, and the task text goes in on its input.
  - Grok gets none of its own tools, helpers, memory, or web access, and none of your Claude Code
    or Cursor settings. When Grok asks to use a tool, Plenipo answers for you: Plenipo's own
    tools yes (Guard still decides each call), everything else no.
  - Models (Grok 1.0.41, signed in): **grok-4.7** (Grok's default), **grok-4.7-build-fast**
    ("Grok 4.7 Fast"), and **grok-4.6**, each low to extra high effort, and **grok-4.5** (low to
    high).
  - Keep Grok up to date with `grok update`. Plenipo was checked with 1.0.41; an old version
    (1.0.13, for example) may not have the options Plenipo uses.
- Kimi (Moonshot AI's Kimi Code, checked with version 0.34.0). The installer puts `kimi.exe` in
  `%USERPROFILE%\.kimi-code\bin` (Plenipo also looks in that folder). To check the sign-in
  yourself, run `kimi provider list`: it should show `managed:kimi-code … source=oauth`, your
  Kimi subscription. What Plenipo checks and does
  ([ADR-022](../adr/ADR-022-acp-file-access-through-plenipo.md), Kimi over ACP, with its file
  reads and writes going through Plenipo):
  - Before every task it runs `kimi provider list`, and runs the task only when the Kimi
    subscription provider is there with `source=oauth`. Other providers you added to Kimi (an API
    key, for example) are never used.
  - It runs only the subscription's models (`kimi-code/…`) and names one on every task (K3 when
    you choose none), so a different default in Kimi's own settings is never used. It passes no
    Kimi or Moonshot key variables, and never reads `%USERPROFILE%\.kimi-code`.
  - Kimi's one-task mode takes the task text only on its command line and changes files without
    asking, so Plenipo runs `kimi acp` and talks to it over ACP
    ([ADR-015](../adr/ADR-015-acp-ai-tools.md), running AI tools over ACP): one program per task,
    and the task text goes in on its input.
  - Kimi's own tools cannot be switched off, so every file Kimi reads or writes comes to Plenipo,
    which answers it through Guard with the worker's permissions: inside the project folder,
    never a blocked file, secrets hidden, recorded, and with your approval where you asked for
    it. A worker without permissions gets every file refused.
  - Kimi's own command line is always refused; workers run programs with Plenipo's
    `run_command`. Plenipo approves each request once, never "for this session".
  - Kimi runs in its **default** mode (it asks before acting), or **plan** (read-only) for a
    worker without permissions — never **auto** or **yolo**. If Kimi switches itself to another
    mode, Plenipo stops the task.
  - Models (Kimi 0.34.0, signed in): **kimi-code/k3** ("K3", Kimi's default; thinking low, high,
    or max), **kimi-code/k3-256k** ("K3-256k"), **kimi-code/kimi-for-coding** ("K2.8 Preview"),
    and **kimi-code/kimi-for-coding-highspeed** ("K2.7 Code Highspeed"; thinking low — its other
    level, "on", is not one Plenipo offers). Plenipo sets the model and thinking level at the
    start of each task.
  - Kimi still loads your own Kimi settings, such as skills you installed and `AGENTS.md` files.
    Files it reads for them go through Plenipo too, so the folder rules still apply.
  - Kimi reports no token counts, so its tasks show none.
- Workers you start in **Workers** cannot change anything: Claude Code and Grok run with none of
  their own tools (conversation only), Kimi in its read-only mode with every file refused, Codex
  in its read-only sandbox, each conversation in its own empty folder
  under `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\agent-workspaces\`. Organization
  workers get Plenipo's own tools, within their permissions (below).
- Handoffs (Phase 4) need two AI tools Ready. In **Workers**, tick **Allow handoffs to other
  workers**, then give an objective that invites a second opinion — for example, on Codex:
  _"Write a function that parses ISO dates. Before you finish, ask claude-code to review it."_
  The task shows **Waiting for replies** while the Claude Code worker runs, then continues with
  its review as step 2. **Open worker conversation** shows the reviewer's own conversation;
  **Activity**
  shows the delegation tree and the full trail. Handoff workers get the same permissions as
  any worker.
- The organization (Phase 5) uses the same AI tools. In **Organization**, create a department
  (it comes with its manager) and a project in it (it comes with its supervisor; tick the AI
  tools its team may use), then drag roles from the **Hire** palette onto the supervisor — or
  click a role and choose who it reports to. Select the supervisor and give it an objective that
  names its team, for example: _"Add input validation to the signup form. Ask the Senior
  Developer to implement it and the Code Reviewer to review it, then summarize."_ Each team
  member it hands work to appears under its position as a live worker and leaves when its task
  ends. Every position's AI tool is your choice (details panel → **Edit title, AI tool, or
  model**). **Settings → Personalization → Titles** renames the ranks (for example after the
  U.S. Army or the Mafia) without changing job titles.
- Permissions (Phase 7). Give the project a **Project folder** (when you create it, or
  **Edit project** in the details panel) — workers work only inside it. **Settings → Permissions** shows each role's permission
  set (the Senior Developer starts with **Developer**: read and change files, run approved
  programs, save to git; pushing asks you). Add the programs your team may run without asking
  under **Approved** (for example `npm test *`). When a worker asks for something sensitive, a
  banner appears on every page; **Review** opens **Approvals**, where the card says exactly what
  will run. Secrets (for example a GitHub token for `gh`) go under **Secrets**: the value is
  stored in Windows Credential Manager (look for `com.eightwest.plenipo` under **Windows
  Credentials**), never in Plenipo's files, and is passed only to the programs you name.
- The Development department (Phase 8). Open **Projects → Set up a Development project**: name
  the project, give its **Project folder** (a git repository with at least one commit) and, for
  GitHub, its **Repository URL** (`https://github.com/<owner>/<name>`). Plenipo creates the
  Development VP (if there is none yet), the project's Supervisor, and its team. Type an
  objective in **Give an objective** — for example _"Implement the login page and get it ready
  for review: a draft pull request."_ — and follow the **result** below it. Each objective works
  on a new branch `plenipo/…` in its own working copy under
  `%LOCALAPPDATA%\com.eightwest.plenipo\working-copies\`; your own checkout is never changed.
  Merge the branch yourself when you are happy with it, and **Remove** the working copy when the
  objective is done (the branch stays). To work in the folder itself instead, untick **Work on a
  separate branch for each objective** in the project's settings.
- GitHub (Phase 8, optional). Install GitHub CLI (`winget install --id GitHub.cli`), then run
  `gh auth login` once in a terminal and choose your GitHub account. Workers can then read the
  project's pull requests, checks, and issues, and ask to open a draft pull request (it always
  waits for your approval). Instead of signing in, you can store a GitHub token under
  **Settings → Permissions → Secrets** for the `gh` program, as `GH_TOKEN`. Add your project's
  test command to **Approved** (for example `npm test *` or `cargo test *`) so QA can run it
  without asking.

## 4. Plenipo's browser, and the screen, mouse, and keyboard (Phase 10, optional)

Workers that you allow to visit or use websites work in **Plenipo's browser**: the Microsoft
Edge that comes with Windows 11, or Google Chrome if you prefer it. Nothing to install.

- **Which browser:** Plenipo takes Edge, then Chrome. To use another Chromium-based browser, set
  the full path in `PLENIPO_BROWSER` before starting Plenipo. **Settings → Permissions →
  Websites** shows which one it found.
- **Its own profile:** `%LOCALAPPDATA%\com.eightwest.plenipo\browser-profile`. Your own browser
  profile, sign-ins, and saved passwords are never used. Delete that folder to sign Plenipo's
  browser out of everything.
- **Signing in to a website workers will use:** **Settings → Permissions → Websites → Open
  Plenipo's browser** (with the website's address), sign in yourself, then close the tab.
  Workers never sign in or type passwords.
- **Screenshots** are kept in `%LOCALAPPDATA%\com.eightwest.plenipo\screenshots`. They can show
  whatever was on the page or screen; delete a task's folder when you no longer need it.
- **The mouse and keyboard:** no built-in role gets them. Give the **Screen, mouse, and keyboard**
  permission set to a role of your own only when no API, program, or website will do; taking control asks
  you every time. Windows' own administrator prompts (UAC) cannot be clicked by any program,
  which is as it should be.
- **Stop:** **Stop all** on the sign in the app, **Stop all browser and desktop control** in the
  tray menu, or **Stop** on the small window shown while a worker has the mouse and keyboard.

`cargo test --workspace` includes the Phase 10 browser tests, which start Edge or Chrome
without a window against a small test website on this computer (no internet). Set
`PLENIPO_TEST_BROWSER` to the browser's full path to pick one; without a browser they are
skipped locally and fail in CI.

## 5. Build a release and installer

```powershell
pnpm build
```

Outputs:

- App: `target\release\plenipo-desktop.exe`
- Installer: `target\release\bundle\nsis\Plenipo_<version>_x64-setup.exe` (per-user install,
  no admin rights required)

The installer is not code-signed yet (planned for Phase 13), so Windows SmartScreen may warn.

## 6. Verify everything locally (same as CI)

```powershell
pnpm check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Launch smoke test (exits 0 when the shell renders and reaches Core, 1 on timeout):

```powershell
$env:PLENIPO_SMOKE_TEST = "1"
$p = Start-Process target\release\plenipo-desktop.exe -PassThru; $null = $p.Handle; $p.WaitForExit(); $p.ExitCode
Remove-Item Env:PLENIPO_SMOKE_TEST
```

## 7. End-to-end tests

`pnpm e2e` drives the real release build through WebDriver. It runs in CI on Linux; locally:

```bash
# once
sudo apt-get install -y webkit2gtk-driver xvfb
cargo install tauri-driver --locked
# each run
pnpm --filter @plenipo/desktop tauri build --no-bundle
cargo build --release -p plenipo-runtime --bin plenipo-fake-agent
xvfb-run -a pnpm e2e        # or plain `pnpm e2e` on a desktop session
```

Set `PLENIPO_E2E_SCREENSHOTS=<dir>` to save screenshots. Each run uses a throwaway `HOME`, so
it never touches your real Plenipo data. The Phase 3–6 tests put `plenipo-fake-agent` (a
test double that speaks the Claude Code and Codex stream formats and Liaison's handoff
protocol) on `PATH` as `claude` and `codex` — one copy per AI tool it stands in for
(`installFakeTools` in `tests/e2e/lib/app.mjs`); they never start a real CLI or use an account.
Adding an AI tool: see [adding-an-ai-tool.md](adding-an-ai-tool.md).
The Phase 5 tests build an organization on the canvas and give its supervisor objectives such
as `[handoff:role:Senior Developer+delay:6000]`, which make the fake supervisor hand that
position a task whose worker takes six seconds. The Phase 6 tests set a role's model choices in
Settings → AI models and check that the next worker follows them (`+usage-limit` makes a worker
report a usage limit). The Phase 7 tests give a project a folder and hand its Senior Developer
tool calls such as `<<tool:read_file {"path":"README.md"}>>`, which the fake worker makes
through Plenipo's real tool relay (`plenipo-desktop --plenipo-tools=…`), then approve the push
that stops for approval.
The Phase 8 test sets up Development on the Projects page with a scratch git repository and a
bare "server" repository next to it. Each member of the team follows a script
(`~/.plenipo-fake-agent/script.json`: for each position title, one step per turn — what it
says, the tools it calls, its handoffs, and its review verdict). `installFakeTools` also puts
two stand-in programs on `PATH` (`plenipo-fake-agent --helpers`): `gh`, which records pull
requests in a file next to it, and `verify FILE WORD`, a test that passes when the file
contains the word.
The Phase 10 test starts a small test website on `127.0.0.1` (no internet), allows it in
Settings → Permissions → Websites, and gives a Web Assistant browser tool calls; the app uses
the Edge or Chrome it finds (set `PLENIPO_BROWSER` to choose, for example a Chromium without
Chrome installed). It approves the form it sends, then takes over and stops the next ones.

## 8. Linux (development / CI only)

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev build-essential
```

## Troubleshooting

| Symptom                                | Fix                                                                                    |
| -------------------------------------- | -------------------------------------------------------------------------------------- |
| `link.exe not found`                   | Install the C++ Build Tools workload, then reopen the terminal.                        |
| `pnpm: command not found`              | Run `corepack enable`.                                                                 |
| `Port 1420 is already in use`          | Another `pnpm dev` is running; close it.                                               |
| Blank window on launch                 | Confirm WebView2 Runtime is installed.                                                 |
| ESLint errors about TypeScript version | Run `pnpm install`; TypeScript is pinned to 6.0.x for typescript-eslint compatibility. |
