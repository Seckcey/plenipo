# ADR-001: Tauri 2 + React/TypeScript + Rust desktop stack

- **Status:** Accepted (specified by ROLLOUT_PLAN.md)
- **Date:** 2026-09-25
- **Phase:** 0

## Context

Plenipo is a Windows-first desktop application that will supervise local processes, hold
credential references, enforce permissions, and present a rich UI. It needs a privileged
local layer that is memory-safe and a UI layer that is fast to iterate on. It must be
installable as normal Windows software.

## Decision

- **Tauri 2** as the desktop framework, using the system WebView2 on Windows.
- **React + TypeScript + Vite** for the UI.
- **Rust (stable)** for the backend command layer and all privileged logic.
- **pnpm** workspaces for JavaScript; a **Cargo workspace** for Rust.
- The UI talks to Rust only through **typed Tauri commands**. DTOs are defined in Rust and
  TypeScript types are generated with `ts-rs`, so there is one source of truth.
- Tauri's **capability system** gates every command; no filesystem/shell/HTTP plugins are
  installed until a phase requires them, and then only behind Core validation.

## Consequences

- Small installers and memory footprint compared with Electron; no bundled Chromium.
- Privileged code is in Rust with `unsafe_code = "forbid"` at the workspace level.
- The WebView is treated as untrusted; security depends on keeping the command surface small
  and validated.
- Contributors need both Rust and Node toolchains, plus MSVC Build Tools on Windows.
- WebView2 behavior differs slightly from other platforms' WebViews; Windows is the tested target.
- TypeScript is pinned to 6.0.x because typescript-eslint 8 does not yet support TypeScript 7.
  Revisit when typescript-eslint adds support.

## Alternatives considered

- **Electron** — larger footprint and a Node.js main process as the privileged layer, which is
  a weaker fit for enforcing a strict capability boundary.
- **.NET (WinUI/WPF)** — strong Windows integration, but a slower UI iteration loop and less
  reuse with future web surfaces such as CrewOS.
- **Pure web app + local agent** — would split the privileged runtime from the UI and
  contradicts ADR-002.
