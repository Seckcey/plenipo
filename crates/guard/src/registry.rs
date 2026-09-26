//! The capability registry: every permission a worker can be given (the rollout plan's initial
//! capabilities), with the plain words the app shows and whether Plenipo has tools for it yet.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A permission a worker can hold. The wire form is the plan's dotted name, e.g.
/// `filesystem.read`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Capability {
    #[serde(rename = "filesystem.read")]
    FilesystemRead,
    #[serde(rename = "filesystem.write")]
    FilesystemWrite,
    #[serde(rename = "shell.exec")]
    ShellExec,
    #[serde(rename = "powershell.exec")]
    PowershellExec,
    #[serde(rename = "git.read")]
    GitRead,
    #[serde(rename = "git.write")]
    GitWrite,
    #[serde(rename = "github.read")]
    GithubRead,
    #[serde(rename = "github.write")]
    GithubWrite,
    #[serde(rename = "ssh.connect")]
    SshConnect,
    #[serde(rename = "browser.navigate")]
    BrowserNavigate,
    #[serde(rename = "browser.automate")]
    BrowserAutomate,
    #[serde(rename = "computer.observe")]
    ComputerObserve,
    #[serde(rename = "computer.control")]
    ComputerControl,
    #[serde(rename = "mcp.invoke")]
    McpInvoke,
    #[serde(rename = "network.local")]
    NetworkLocal,
    #[serde(rename = "process.manage")]
    ProcessManage,
}

/// Registry entry: what the app says about a capability.
pub struct Info {
    pub capability: Capability,
    pub id: &'static str,
    /// Short plain name ("Read files").
    pub label: &'static str,
    /// One sentence for Settings.
    pub description: &'static str,
    /// Plenipo has tools for it in this version.
    pub tools: bool,
    /// When it has no tools yet: the phase that brings them.
    pub arrives: Option<&'static str>,
}

const REGISTRY: [Info; 16] = [
    Info {
        capability: Capability::FilesystemRead,
        id: "filesystem.read",
        label: "Read files",
        description: "List, open, and search files in the project folder.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::FilesystemWrite,
        id: "filesystem.write",
        label: "Change files",
        description: "Create, edit, move, and delete files in the project folder.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::ShellExec,
        id: "shell.exec",
        label: "Run programs",
        description: "Run programs such as build and test commands in the project folder. \
                      Approved commands run without asking; others ask you.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::PowershellExec,
        id: "powershell.exec",
        label: "Run PowerShell scripts",
        description: "Run a PowerShell script in the project folder.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::GitRead,
        id: "git.read",
        label: "Read git history",
        description: "See the project's changes, history, and branches.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::GitWrite,
        id: "git.write",
        label: "Save to git",
        description: "Stage and commit changes and create or switch branches. Pushing to a \
                      server always asks you first.",
        tools: true,
        arrives: None,
    },
    Info {
        capability: Capability::GithubRead,
        id: "github.read",
        label: "Read GitHub",
        description: "Read issues, pull requests, and checks on GitHub.",
        tools: false,
        arrives: Some("Phase 8"),
    },
    Info {
        capability: Capability::GithubWrite,
        id: "github.write",
        label: "Change GitHub",
        description: "Open pull requests and comment on GitHub.",
        tools: false,
        arrives: Some("Phase 8"),
    },
    Info {
        capability: Capability::SshConnect,
        id: "ssh.connect",
        label: "Connect to servers",
        description: "Run commands on servers you set up (SSH).",
        tools: false,
        arrives: Some("Phase 11"),
    },
    Info {
        capability: Capability::BrowserNavigate,
        id: "browser.navigate",
        label: "Visit websites",
        description: "Open and read web pages.",
        tools: false,
        arrives: Some("Phase 10"),
    },
    Info {
        capability: Capability::BrowserAutomate,
        id: "browser.automate",
        label: "Use websites",
        description: "Click and type on web pages.",
        tools: false,
        arrives: Some("Phase 10"),
    },
    Info {
        capability: Capability::ComputerObserve,
        id: "computer.observe",
        label: "See the screen",
        description: "Take screenshots of this computer's screen.",
        tools: false,
        arrives: Some("Phase 10"),
    },
    Info {
        capability: Capability::ComputerControl,
        id: "computer.control",
        label: "Use the mouse and keyboard",
        description: "Control this computer's mouse and keyboard.",
        tools: false,
        arrives: Some("Phase 10"),
    },
    Info {
        capability: Capability::McpInvoke,
        id: "mcp.invoke",
        label: "Use add-on tools",
        description: "Use add-on tools (MCP servers) you set up.",
        tools: false,
        arrives: Some("a later phase"),
    },
    Info {
        capability: Capability::NetworkLocal,
        id: "network.local",
        label: "Reach local services",
        description: "Connect to services on this computer or your local network.",
        tools: false,
        arrives: Some("a later phase"),
    },
    Info {
        capability: Capability::ProcessManage,
        id: "process.manage",
        label: "Manage running programs",
        description: "See and stop programs it started, such as a development server.",
        tools: false,
        arrives: Some("a later phase"),
    },
];

impl Capability {
    pub const ALL: [Capability; 16] = [
        Self::FilesystemRead,
        Self::FilesystemWrite,
        Self::ShellExec,
        Self::PowershellExec,
        Self::GitRead,
        Self::GitWrite,
        Self::GithubRead,
        Self::GithubWrite,
        Self::SshConnect,
        Self::BrowserNavigate,
        Self::BrowserAutomate,
        Self::ComputerObserve,
        Self::ComputerControl,
        Self::McpInvoke,
        Self::NetworkLocal,
        Self::ProcessManage,
    ];

    pub fn info(self) -> &'static Info {
        REGISTRY
            .iter()
            .find(|i| i.capability == self)
            .unwrap_or(&REGISTRY[0])
    }

    /// The plan's dotted name, e.g. `filesystem.read`.
    pub fn id(self) -> &'static str {
        self.info().id
    }

    pub fn label(self) -> &'static str {
        self.info().label
    }

    pub fn parse(id: &str) -> Option<Self> {
        REGISTRY.iter().find(|i| i.id == id).map(|i| i.capability)
    }

    /// Plenipo has tools for it in this version.
    pub fn has_tools(self) -> bool {
        self.info().tools
    }

    /// Uses the project folder (so it needs one).
    pub fn needs_folder(self) -> bool {
        matches!(
            self,
            Self::FilesystemRead
                | Self::FilesystemWrite
                | Self::ShellExec
                | Self::PowershellExec
                | Self::GitRead
                | Self::GitWrite
        )
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_the_plans_capabilities_once() {
        let plan = [
            "filesystem.read",
            "filesystem.write",
            "shell.exec",
            "powershell.exec",
            "git.read",
            "git.write",
            "github.read",
            "github.write",
            "ssh.connect",
            "browser.navigate",
            "browser.automate",
            "computer.observe",
            "computer.control",
            "mcp.invoke",
            "network.local",
            "process.manage",
        ];
        assert_eq!(Capability::ALL.len(), plan.len());
        for (c, id) in Capability::ALL.iter().zip(plan) {
            assert_eq!(c.id(), id);
            assert_eq!(Capability::parse(id), Some(*c));
            // The wire form is the plan's name.
            assert_eq!(serde_json::to_value(c).unwrap(), serde_json::json!(id));
            let info = c.info();
            assert!(info.tools || info.arrives.is_some(), "{id}");
        }
        assert_eq!(Capability::parse("root.everything"), None);
    }
}
