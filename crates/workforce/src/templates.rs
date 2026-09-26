//! Role templates seeded as data (ADR-009 §10). Purposes follow the rollout plan's initial role
//! templates (§5) and are written into each worker's instructions. The owner can add roles;
//! nothing about departments or projects is seeded.

use plenipo_ledger::{RoleTemplate, RoleType};
use serde_json::json;

struct Template {
    name: &'static str,
    description: &'static str,
    role_type: RoleType,
    persistent: bool,
    glyph: &'static str,
    purpose: &'static [&'static str],
    capabilities: &'static [&'static str],
}

const TEMPLATES: &[Template] = &[
    Template {
        name: "Superintendent",
        description: "Accepts owner objectives, selects the project coordinator, tracks major \
                      outcomes, escalates blockers, and summarizes completion.",
        role_type: RoleType::Superintendent,
        persistent: true,
        glyph: "executive",
        purpose: &[
            "accept the owner's objectives",
            "select the project coordinator",
            "track major outcomes",
            "escalate blockers",
            "summarize completion",
        ],
        capabilities: &[],
    },
    Template {
        name: "Department Manager",
        description: "Runs a department: sets priorities, delegates outcomes to project \
                      coordinators, tracks progress, and escalates blockers.",
        role_type: RoleType::DepartmentManager,
        persistent: true,
        glyph: "manager",
        purpose: &[
            "set the department's priorities",
            "delegate outcomes to project coordinators",
            "track progress",
            "escalate blockers",
        ],
        capabilities: &[],
    },
    Template {
        name: "Project Coordinator",
        description: "Decomposes project objectives, spawns workers, coordinates dependencies, \
                      requests reviews, judges acceptance criteria, and synthesizes the result.",
        role_type: RoleType::ProjectCoordinator,
        persistent: true,
        glyph: "coordinator",
        purpose: &[
            "decompose project objectives into bounded tasks",
            "hand those tasks to the workers on your team",
            "coordinate dependencies",
            "request reviews",
            "judge acceptance criteria",
            "synthesize the result",
        ],
        capabilities: &["git.read"],
    },
    Template {
        name: "Senior Developer",
        description: "Implementation, debugging, and refactoring.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "code",
        purpose: &["implementation", "debugging", "refactoring"],
        capabilities: &[
            "filesystem.read",
            "filesystem.write",
            "git.read",
            "shell.exec",
        ],
    },
    Template {
        name: "Code Reviewer",
        description: "Independent review for correctness, maintainability, and architecture.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "review",
        purpose: &[
            "independent review",
            "correctness",
            "maintainability",
            "architectural findings",
        ],
        capabilities: &["filesystem.read", "git.read"],
    },
    Template {
        name: "QA Engineer",
        description: "Tests, reproduction, and acceptance verification.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "qa",
        purpose: &["tests", "reproduction", "acceptance verification"],
        capabilities: &["filesystem.read", "shell.exec"],
    },
    Template {
        name: "Security Auditor",
        description: "Security review: threats, secrets handling, permissions, and dependency \
                      risks.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "shield",
        purpose: &[
            "threat review",
            "secrets and credential handling",
            "permission and privilege risks",
            "dependency risks",
        ],
        capabilities: &["filesystem.read", "git.read"],
    },
    Template {
        name: "Documentation Writer",
        description: "README, architecture docs, release notes, and user and admin \
                      documentation.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "docs",
        purpose: &[
            "README",
            "architecture documentation",
            "release notes",
            "user and administrator documentation",
        ],
        capabilities: &["filesystem.read", "filesystem.write"],
    },
    Template {
        name: "Researcher",
        description: "Investigates options, gathers sources, and summarizes findings.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "research",
        purpose: &[
            "investigate options",
            "gather sources",
            "summarize findings",
        ],
        capabilities: &["browser.navigate"],
    },
    Template {
        name: "Designer",
        description: "Graphics, campaign visuals, and brand assets (needs a vision- and \
                      image-capable model).",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "design",
        purpose: &["graphics", "campaign visuals", "brand assets"],
        capabilities: &["filesystem.read", "filesystem.write"],
    },
];

/// Every built-in template, as the Ledger seeds them.
pub fn role_templates() -> Vec<RoleTemplate> {
    TEMPLATES
        .iter()
        .map(|t| RoleTemplate {
            name: t.name,
            description: t.description,
            role_type: t.role_type,
            persistent: t.persistent,
            metadata: json!({
                "template": true,
                "glyph": t.glyph,
                "purpose": t.purpose,
                "defaultCapabilities": t.capabilities,
            }),
        })
        .collect()
}

/// The glyph for a role without one (custom roles).
pub fn default_glyph(role_type: RoleType) -> &'static str {
    match role_type {
        RoleType::Superintendent => "executive",
        RoleType::DepartmentManager => "manager",
        RoleType::ProjectCoordinator => "coordinator",
        RoleType::Worker => "worker",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_cover_every_class_and_the_owners_oversight_roles() {
        let all = role_templates();
        for t in [
            RoleType::Superintendent,
            RoleType::DepartmentManager,
            RoleType::ProjectCoordinator,
            RoleType::Worker,
        ] {
            assert!(all.iter().any(|r| r.role_type == t), "{t:?}");
        }
        for name in ["Code Reviewer", "QA Engineer", "Security Auditor"] {
            let r = all.iter().find(|r| r.name == name).unwrap();
            assert!(!r.persistent, "{name} is on demand");
        }
        // Leadership is persistent; names are unique.
        assert!(all
            .iter()
            .filter(|r| r.role_type != RoleType::Worker)
            .all(|r| r.persistent));
        let mut names: Vec<_> = all.iter().map(|r| r.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        // Capability names are from the plan's list (nothing is granted before Guard).
        for r in &all {
            for c in r.metadata["defaultCapabilities"].as_array().unwrap() {
                assert!(
                    plenipo_liaison::protocol::CAPABILITIES.contains(&c.as_str().unwrap()),
                    "{c}"
                );
            }
        }
    }
}
