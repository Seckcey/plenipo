//! Role templates seeded as data (ADR-009 §10). Purposes follow the rollout plan's initial role
//! templates (§5) and are written into each worker's instructions. The owner can add roles;
//! nothing about departments or projects is seeded.

use plenipo_ledger::{RoleTemplate, RoleType};
use plenipo_router::{CostPreference, CrossCompany, ModelFeature, RolePolicy};
use serde_json::json;

struct Template {
    name: &'static str,
    description: &'static str,
    role_type: RoleType,
    persistent: bool,
    glyph: &'static str,
    purpose: &'static [&'static str],
    capabilities: &'static [&'static str],
    /// Names it was seeded under before (see `RoleTemplate::formerly`).
    formerly: &'static [&'static str],
}

const TEMPLATES: &[Template] = &[
    Template {
        name: "VP",
        description: "Takes your objectives, picks the supervisor for each, tracks the big \
                      results, raises problems early, and reports back when work is done.",
        role_type: RoleType::Superintendent,
        persistent: true,
        glyph: "executive",
        purpose: &[
            "take the owner's objectives",
            "pick the supervisor for each piece of work",
            "track the big results",
            "raise problems early",
            "report back when work is done",
        ],
        capabilities: &[],
        formerly: &["Superintendent"],
    },
    Template {
        name: "Manager",
        description: "Runs a department: sets priorities, hands work to the supervisors of its \
                      projects, tracks progress, and raises problems.",
        role_type: RoleType::DepartmentManager,
        persistent: true,
        glyph: "manager",
        purpose: &[
            "set the department's priorities",
            "hand work to the supervisors of its projects",
            "track progress",
            "raise problems",
        ],
        capabilities: &[],
        formerly: &["Department Manager"],
    },
    Template {
        name: "Supervisor",
        description:
            "Leads a project: breaks objectives into tasks, hands them to the team, keeps \
                      the work in order, asks for reviews, checks the result, and puts it together.",
        role_type: RoleType::ProjectCoordinator,
        persistent: true,
        glyph: "coordinator",
        purpose: &[
            "break the project's objectives into clear, bounded tasks",
            "hand those tasks to the workers on your team",
            "keep work that depends on other work in order",
            "ask for reviews",
            "check the result against what was asked",
            "put the final result together",
        ],
        capabilities: &["git.read"],
        formerly: &["Project Coordinator"],
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
        formerly: &[],
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
        formerly: &[],
    },
    Template {
        name: "QA Engineer",
        description: "Tests, reproduction, and acceptance verification.",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "qa",
        purpose: &["tests", "reproduction", "acceptance verification"],
        capabilities: &["filesystem.read", "shell.exec"],
        formerly: &[],
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
        formerly: &[],
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
        formerly: &[],
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
        formerly: &[],
    },
    Template {
        name: "Designer",
        description: "Graphics, campaign visuals, and brand assets (needs an AI model that can \
                      work with images).",
        role_type: RoleType::Worker,
        persistent: false,
        glyph: "design",
        purpose: &["graphics", "campaign visuals", "brand assets"],
        capabilities: &["filesystem.read", "filesystem.write"],
        formerly: &[],
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
            formerly: t.formerly,
        })
        .collect()
}

/// Starting model policies for built-in roles, by template name (the plan's examples, Phase 6):
/// designers need a model that sees and makes images, reviewers and security auditors prefer
/// another AI company than the work they review, documentation prefers economical models and
/// development premium ones. No model names: which models exist is the owner's to say. Given
/// to a template role once, when it has no policy; the owner can change them.
pub fn template_policies() -> Vec<(&'static str, RolePolicy)> {
    vec![
        (
            "Senior Developer",
            RolePolicy {
                cost: CostPreference::Premium,
                ..RolePolicy::default()
            },
        ),
        (
            "Code Reviewer",
            RolePolicy {
                cross_company: CrossCompany::Prefer,
                ..RolePolicy::default()
            },
        ),
        (
            "Security Auditor",
            RolePolicy {
                cross_company: CrossCompany::Prefer,
                ..RolePolicy::default()
            },
        ),
        (
            "Documentation Writer",
            RolePolicy {
                cost: CostPreference::Economical,
                ..RolePolicy::default()
            },
        ),
        (
            "Designer",
            RolePolicy {
                needs: vec![ModelFeature::Vision, ModelFeature::ImageGeneration],
                ..RolePolicy::default()
            },
        ),
    ]
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
        // Leadership has plain titles (the chain of command: Worker, Supervisor, Manager, VP),
        // and a former name is never a current one.
        for (name, t) in [
            ("VP", RoleType::Superintendent),
            ("Manager", RoleType::DepartmentManager),
            ("Supervisor", RoleType::ProjectCoordinator),
        ] {
            assert!(
                all.iter().any(|r| r.name == name && r.role_type == t),
                "{name}"
            );
        }
        for r in &all {
            for old in r.formerly {
                assert!(all.iter().all(|o| o.name != *old), "{old}");
            }
        }
        // Every starting policy belongs to a template.
        for (name, _) in template_policies() {
            assert!(all.iter().any(|r| r.name == name), "{name}");
        }
        // Capability names are from the plan's list (what the role asks for; Guard grants
        // permissions from the owner's permission sets, Phase 7).
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
