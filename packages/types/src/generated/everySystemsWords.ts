// Each system's own words on screen (Phase 23, ADR-155), written by `pnpm bindings` from crates/core/src/words.rs. Do not edit by hand.
import type { System } from "./System";
import type { SystemWords } from "./SystemWords";

export const SYSTEM_WORDS: Record<System, SystemWords> = {
  "linux": {
    "fileProgram": "your file manager",
    "keyStore": "your computer's password store (GNOME Keyring or KWallet)",
    "noticeSettings": "your desktop's notification settings",
    "notices": "notifications",
    "showFile": "Show in folder",
    "signingOut": "signing out",
    "sshAgent": "ssh-agent",
    "startAtSignIn": "Start Plenipo when you sign in",
    "startAtSignInHint": "Plenipo starts in the tray, with no window, when you sign in.",
    "startingAtSignIn": "Starting Plenipo when you sign in",
    "system": "linux",
    "systemName": "Linux",
    "theSystem": "your computer",
    "thisComputer": "this computer",
    "waitsIn": "the tray",
    "waitsInIcon": "the tray icon",
    "waitsInMenu": "the tray menu",
    "whenYouSignIn": "When you sign in"
  },
  "mac": {
    "fileProgram": "Finder",
    "keyStore": "your Mac's Keychain",
    "noticeSettings": "System Settings → Notifications",
    "notices": "notifications",
    "showFile": "Show in Finder",
    "signingOut": "logging out",
    "sshAgent": "ssh-agent",
    "startAtSignIn": "Open Plenipo when you log in",
    "startAtSignInHint": "Plenipo opens in the menu bar, with no window, when you log in. System Settings → General → Login Items can turn it off too.",
    "startingAtSignIn": "Opening Plenipo when you log in",
    "system": "mac",
    "systemName": "macOS",
    "theSystem": "your Mac",
    "thisComputer": "this Mac",
    "waitsIn": "the menu bar",
    "waitsInIcon": "the menu bar icon",
    "waitsInMenu": "its menu in the menu bar",
    "whenYouSignIn": "When you log in"
  },
  "windows": {
    "fileProgram": "File Explorer",
    "keyStore": "Windows Credential Manager",
    "noticeSettings": "Windows Settings → System → Notifications",
    "notices": "pop-up notices",
    "showFile": "Show in folder",
    "signingOut": "signing out",
    "sshAgent": "Windows' OpenSSH Authentication Agent, or Pageant",
    "startAtSignIn": "Start Plenipo with Windows",
    "startAtSignInHint": "Plenipo starts in the tray, with no window, when you sign in. Windows' own Settings → Apps → Startup can turn it off too.",
    "startingAtSignIn": "Starting with Windows",
    "system": "windows",
    "systemName": "Windows",
    "theSystem": "Windows",
    "thisComputer": "this PC",
    "waitsIn": "the tray",
    "waitsInIcon": "the tray icon",
    "waitsInMenu": "the tray menu",
    "whenYouSignIn": "When you sign in to Windows"
  }
};
