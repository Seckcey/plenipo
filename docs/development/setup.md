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

The repository's `rust-toolchain.toml` makes rustup install the correct toolchain plus
`rustfmt` and `clippy` automatically on first build.

## 2. Clone, install, run

```powershell
git clone https://github.com/Seckcey/plenipo.git
cd plenipo
pnpm install
pnpm dev
```

The first `pnpm dev` compiles the Rust backend (several minutes); later runs are incremental.
A window titled **Plenipo** opens showing the shell with **Core: Connected**.

No `.env` file, API keys, or provider logins are required.

## 3. Build a release and installer

```powershell
pnpm build
```

Outputs:

- App: `target\release\plenipo-desktop.exe`
- Installer: `target\release\bundle\nsis\Plenipo_<version>_x64-setup.exe` (per-user install,
  no admin rights required)

The installer is not code-signed yet (planned for Phase 13), so Windows SmartScreen may warn.

## 4. Verify everything locally (same as CI)

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

## 5. End-to-end tests

`pnpm e2e` drives the real release build through WebDriver. It runs in CI on Linux; locally:

```bash
# once
sudo apt-get install -y webkit2gtk-driver xvfb
cargo install tauri-driver --locked
# each run
pnpm --filter @plenipo/desktop tauri build --no-bundle
xvfb-run -a pnpm e2e        # or plain `pnpm e2e` on a desktop session
```

Set `PLENIPO_E2E_SCREENSHOTS=<dir>` to save screenshots. Each run uses a throwaway `HOME`, so
it never touches your real Plenipo data.

## 6. Linux (development / CI only)

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
