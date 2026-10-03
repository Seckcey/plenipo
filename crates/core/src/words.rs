//! Words that change with the system (Phase 23, ADR-155): "this PC" on Windows, "this Mac" on a
//! Mac, "this computer" on Linux. The table is `docs/design/vocabulary.md`, "Words that change
//! with the system". This is the one place that decides: Rust's own messages use [`WORDS`], and
//! the screens get the same words in [`crate::AppInfo`], so they never guess.
//!
//! Names in the code and in the Ledger (such as `windowsRestart`) stay as they are; only the words
//! shown change.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which system Plenipo runs on, as the screens need to know it (keyboard labels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum System {
    Windows,
    Mac,
    Linux,
}

impl System {
    /// The system this code was built for.
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Linux
        }
    }
}

/// The words for one system, for Rust's own messages. The screens get them as [`SystemWords`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Words {
    pub system: System,
    /// The system's name: "Windows", "macOS", "Linux".
    pub system_name: &'static str,
    /// The owner's computer: "this PC", "this Mac", "this computer".
    pub this_computer: &'static str,
    /// The system as the one that acts ("Windows closed Plenipo"): "Windows", "your Mac",
    /// "your computer".
    pub the_system: &'static str,
    /// Where keys are kept: "Windows Credential Manager", "your Mac's Keychain".
    pub key_store: &'static str,
    /// The Start at sign-in switch: "Start Plenipo with Windows".
    pub start_at_sign_in: &'static str,
    /// Its heading: "When you sign in to Windows".
    pub when_you_sign_in: &'static str,
    /// What it does, and where else to turn it off.
    pub start_at_sign_in_hint: &'static str,
    /// Doing it, in a sentence: "Starting with Windows is not available on this computer."
    pub starting_at_sign_in: &'static str,
    /// Signing out, in a list of what closed Plenipo: "signing out", "logging out".
    pub signing_out: &'static str,
    /// Where Plenipo waits with its window closed: "the tray", "the menu bar".
    pub waits_in: &'static str,
    /// Its icon there: "the tray icon", "the menu bar icon".
    pub waits_in_icon: &'static str,
    /// Its menu there: "the tray menu", "its menu in the menu bar".
    pub waits_in_menu: &'static str,
    /// The system's notices: "pop-up notices", "notifications".
    pub notices: &'static str,
    /// Where to change them: "Windows Settings → System → Notifications".
    pub notice_settings: &'static str,
    /// The program that shows files: "File Explorer", "Finder", "your file manager".
    pub file_program: &'static str,
    /// Show a file there: "Show in folder", "Show in Finder".
    pub show_file: &'static str,
    /// The SSH agent: "Windows' OpenSSH Authentication Agent, or Pageant", "ssh-agent".
    pub ssh_agent: &'static str,
}

/// Windows' words, as Plenipo has always said them.
pub const WINDOWS: Words = Words {
    system: System::Windows,
    system_name: "Windows",
    this_computer: "this PC",
    the_system: "Windows",
    key_store: "Windows Credential Manager",
    start_at_sign_in: "Start Plenipo with Windows",
    when_you_sign_in: "When you sign in to Windows",
    start_at_sign_in_hint:
        "Plenipo starts in the tray, with no window, when you sign in. Windows' own \
                            Settings → Apps → Startup can turn it off too.",
    starting_at_sign_in: "Starting with Windows",
    signing_out: "signing out",
    waits_in: "the tray",
    waits_in_icon: "the tray icon",
    waits_in_menu: "the tray menu",
    notices: "pop-up notices",
    notice_settings: "Windows Settings → System → Notifications",
    file_program: "File Explorer",
    show_file: "Show in folder",
    ssh_agent: "Windows' OpenSSH Authentication Agent, or Pageant",
};

/// A Mac's words.
pub const MAC: Words = Words {
    system: System::Mac,
    system_name: "macOS",
    this_computer: "this Mac",
    the_system: "your Mac",
    key_store: "your Mac's Keychain",
    start_at_sign_in: "Open Plenipo when you log in",
    when_you_sign_in: "When you log in",
    start_at_sign_in_hint:
        "Plenipo opens in the menu bar, with no window, when you log in. System \
                            Settings → General → Login Items can turn it off too.",
    starting_at_sign_in: "Opening Plenipo when you log in",
    signing_out: "logging out",
    waits_in: "the menu bar",
    waits_in_icon: "the menu bar icon",
    waits_in_menu: "its menu in the menu bar",
    notices: "notifications",
    notice_settings: "System Settings → Notifications",
    file_program: "Finder",
    show_file: "Show in Finder",
    ssh_agent: "ssh-agent",
};

/// Linux's words.
pub const LINUX: Words = Words {
    system: System::Linux,
    system_name: "Linux",
    this_computer: "this computer",
    the_system: "your computer",
    key_store: "your computer's password store (GNOME Keyring or KWallet)",
    start_at_sign_in: "Start Plenipo when you sign in",
    when_you_sign_in: "When you sign in",
    start_at_sign_in_hint: "Plenipo starts in the tray, with no window, when you sign in.",
    starting_at_sign_in: "Starting Plenipo when you sign in",
    signing_out: "signing out",
    waits_in: "the tray",
    waits_in_icon: "the tray icon",
    waits_in_menu: "the tray menu",
    notices: "notifications",
    notice_settings: "your desktop's notification settings",
    file_program: "your file manager",
    show_file: "Show in folder",
    ssh_agent: "ssh-agent",
};

/// The words for a system.
pub const fn words_for(system: System) -> &'static Words {
    match system {
        System::Windows => &WINDOWS,
        System::Mac => &MAC,
        System::Linux => &LINUX,
    }
}

/// The words for the system this code was built for.
pub const WORDS: &Words = words_for(System::current());

/// "Windows closed Plenipo", "Your Mac closed Plenipo": the system's word at the start of a
/// sentence.
pub fn sentence_start(words: &str) -> String {
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + chars.as_str()
    })
}

/// The same words for the screens, in [`crate::AppInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemWords {
    pub system: System,
    /// "Windows", "macOS", "Linux".
    pub system_name: String,
    /// "this PC", "this Mac", "this computer".
    pub this_computer: String,
    /// "Windows", "your Mac", "your computer" (the one that acts).
    pub the_system: String,
    /// "Windows Credential Manager", "your Mac's Keychain".
    pub key_store: String,
    /// "Start Plenipo with Windows".
    pub start_at_sign_in: String,
    /// "When you sign in to Windows".
    pub when_you_sign_in: String,
    /// What the switch does, and where else to turn it off.
    pub start_at_sign_in_hint: String,
    /// "Starting with Windows" (is not available on this computer).
    pub starting_at_sign_in: String,
    /// "signing out", "logging out".
    pub signing_out: String,
    /// "the tray", "the menu bar".
    pub waits_in: String,
    /// "the tray icon", "the menu bar icon".
    pub waits_in_icon: String,
    /// "the tray menu", "its menu in the menu bar".
    pub waits_in_menu: String,
    /// "pop-up notices", "notifications".
    pub notices: String,
    /// "Windows Settings → System → Notifications".
    pub notice_settings: String,
    /// "File Explorer", "Finder", "your file manager".
    pub file_program: String,
    /// "Show in folder", "Show in Finder".
    pub show_file: String,
    /// "Windows' OpenSSH Authentication Agent, or Pageant", "ssh-agent".
    pub ssh_agent: String,
}

impl From<&Words> for SystemWords {
    fn from(w: &Words) -> Self {
        Self {
            system: w.system,
            system_name: w.system_name.to_owned(),
            this_computer: w.this_computer.to_owned(),
            the_system: w.the_system.to_owned(),
            key_store: w.key_store.to_owned(),
            start_at_sign_in: w.start_at_sign_in.to_owned(),
            when_you_sign_in: w.when_you_sign_in.to_owned(),
            start_at_sign_in_hint: w.start_at_sign_in_hint.to_owned(),
            starting_at_sign_in: w.starting_at_sign_in.to_owned(),
            signing_out: w.signing_out.to_owned(),
            waits_in: w.waits_in.to_owned(),
            waits_in_icon: w.waits_in_icon.to_owned(),
            waits_in_menu: w.waits_in_menu.to_owned(),
            notices: w.notices.to_owned(),
            notice_settings: w.notice_settings.to_owned(),
            file_program: w.file_program.to_owned(),
            show_file: w.show_file.to_owned(),
            ssh_agent: w.ssh_agent.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_system_gets_its_own_words() {
        assert_eq!(WORDS.system, System::current());
        assert_eq!(words_for(System::Windows).this_computer, "this PC");
        assert_eq!(words_for(System::Mac).this_computer, "this Mac");
        assert_eq!(words_for(System::Linux).this_computer, "this computer");
        // No Windows word on a Mac or Linux.
        for w in [&MAC, &LINUX] {
            let all = format!("{w:?}");
            assert!(!all.contains("Windows"), "{all}");
            assert!(!all.contains("PC"), "{all}");
        }
    }

    #[test]
    fn the_system_starts_a_sentence() {
        assert_eq!(sentence_start(WINDOWS.the_system), "Windows");
        assert_eq!(sentence_start(MAC.the_system), "Your Mac");
        assert_eq!(sentence_start(""), "");
    }

    /// `pnpm bindings` also writes every system's words for the screens
    /// (`everySystemsWords.ts`): their tests read each system's words from there, so the two
    /// cannot drift apart.
    #[test]
    fn export_bindings_system_words() {
        let Some(dir) = std::env::var_os("TS_RS_EXPORT_DIR") else {
            return;
        };
        let all = serde_json::json!({
            "windows": SystemWords::from(&WINDOWS),
            "mac": SystemWords::from(&MAC),
            "linux": SystemWords::from(&LINUX),
        });
        let text = format!(
            "// Each system's own words on screen (Phase 23, ADR-155), written by `pnpm bindings` \
             from crates/core/src/words.rs. Do not edit by hand.\n\
             import type {{ System }} from \"./System\";\n\
             import type {{ SystemWords }} from \"./SystemWords\";\n\n\
             export const SYSTEM_WORDS: Record<System, SystemWords> = {};\n",
            serde_json::to_string_pretty(&all).unwrap()
        );
        std::fs::write(
            std::path::Path::new(&dir).join("everySystemsWords.ts"),
            text,
        )
        .unwrap();
    }

    #[test]
    fn the_screens_get_the_same_words() {
        let screen = SystemWords::from(&MAC);
        assert_eq!(screen.system, System::Mac);
        assert_eq!(screen.show_file, "Show in Finder");
        let json = serde_json::to_value(&screen).unwrap();
        assert_eq!(json["system"], "mac");
        assert_eq!(json["thisComputer"], "this Mac");
    }
}
