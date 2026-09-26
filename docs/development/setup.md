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

## 3. AI tools: Claude Code and Codex (Phase 3, optional)

The **Workers** view runs tasks on the Claude Code and Codex command-line tools that are already
installed **and signed in with your subscription** on this computer. Plenipo never asks for a
password or API key, and refuses API-key sign-ins (no API billing). The desktop apps do not need
to be open.

| AI tool     | Install (PowerShell)                                      | Sign in (once, in a terminal)                    |
| ----------- | --------------------------------------------------------- | ------------------------------------------------ |
| Claude Code | `irm https://claude.ai/install.ps1 \| iex` (native build) | `claude auth login` — choose your Claude account |
| Codex       | `npm install -g @openai/codex` (needs Node.js)            | `codex login` — choose **Sign in with ChatGPT**  |

Then open **AI tools** in Plenipo and choose **Re-check**: each tool should show **Ready**
with its version and "Signed in (subscription)". If a card says what is missing (not installed,
not signed in, API key), follow the hint on the card.

Notes:

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
- Workers you start in **Workers** cannot change anything: Claude Code runs with no tools
  (conversation only), Codex in its read-only sandbox, each conversation in its own empty folder
  under `%LOCALAPPDATA%\com.eightwest.plenipo\runtime\agent-workspaces\`. Organization
  workers get Plenipo's own tools, within their permissions (below).
- Handoffs (Phase 4) need both AI tools Ready. In **Workers**, tick **Allow handoffs to other
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

## 4. Build a release and installer

```powershell
pnpm build
```

Outputs:

- App: `target\release\plenipo-desktop.exe`
- Installer: `target\release\bundle\nsis\Plenipo_<version>_x64-setup.exe` (per-user install,
  no admin rights required)

The installer is not code-signed yet (planned for Phase 13), so Windows SmartScreen may warn.

## 5. Verify everything locally (same as CI)

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

## 6. End-to-end tests

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
protocol) on `PATH` as `claude` and `codex`; they never start a real CLI or use an account.
The Phase 5 tests build an organization on the canvas and give its supervisor objectives such
as `[handoff:role:Senior Developer+delay:6000]`, which make the fake supervisor hand that
position a task whose worker takes six seconds. The Phase 6 tests set a role's model choices in
Settings → AI models and check that the next worker follows them (`+usage-limit` makes a worker
report a usage limit). The Phase 7 tests give a project a folder and hand its Senior Developer
tool calls such as `<<tool:read_file {"path":"README.md"}>>`, which the fake worker makes
through Plenipo's real tool relay (`plenipo-desktop --plenipo-tools=…`), then approve the push
that stops for approval.

## 7. Linux (development / CI only)

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
