//! The Model Policy Engine: a pure function from a role's policy, the model registry, and the
//! AI tools' live state to a decision with its reasons (ADR-011).
//!
//! Order of work:
//!
//! 0. **Layers** (ADR-041): the agent's own rule, its role's choices, its department's rule, and
//!    the organization's, closest first. The closest layer that sets something wins; AI
//!    companies never to use add up.
//! 1. **Candidates.** The closest layer's models in its order (first choice, then the backups);
//!    without a list anywhere, every registered model, cheapest or dearest first as the role
//!    prefers.
//! 2. **Cross-company review.** When the worker reviews work done by some AI companies and the
//!    role prefers another company, models from other companies go first; when it requires
//!    one, models from the same companies are skipped.
//! 3. **Checks, per model:** its AI tool exists; its company is on no layer's never-use list;
//!    the project allows the tool; it can do what the role needs and takes enough
//!    context; the tool is installed and signed in with a subscription (API-key sign-ins are
//!    refused: pay-per-use billing is off); the tool is not at a usage limit.
//! 4. **Usage limits.** With [`LimitBehavior::Wait`], once a model is skipped for a usage limit,
//!    models from other companies are skipped too: work waits rather than switching company.
//! 5. The first model that passes is chosen. Every model gets a verdict and a note.
//! 6. **Effort:** from the closest layer that sets one the chosen model takes (its effort for
//!    that model, else its effort for any model); else the model's own setting; else the AI
//!    tool's default. The reason names the layer.

use std::collections::BTreeMap;

use plenipo_runtime::agent::{AgentRuntimeInfo, AuthState, Effort, InstallState};

use crate::dto::*;
use crate::limits::duration_words;

/// One AI tool as the router sees it: detection state and any usage limit.
#[derive(Debug, Clone)]
pub struct ToolState {
    pub info: AgentRuntimeInfo,
    pub limit: Option<UsageLimit>,
}

/// A department's rule, with what the reason calls it.
#[derive(Debug, Clone, Copy)]
pub struct DepartmentRule<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub rule: &'a ModelRule,
}

/// Everything one decision depends on.
#[derive(Debug, Clone, Copy)]
pub struct RouteInput<'a> {
    /// The role's name, for the explanation.
    pub role: &'a str,
    /// The role's ID (`""` when unknown).
    pub role_id: &'a str,
    pub policy: &'a RolePolicy,
    /// The agent's own rule, and its position's ID (ADR-041).
    pub agent: Option<(&'a str, &'a ModelRule)>,
    /// Its department's rule.
    pub department: Option<DepartmentRule<'a>>,
    /// The organization's rule.
    pub organization: Option<&'a ModelRule>,
    pub models: &'a [ModelInfo],
    pub tools: &'a [ToolState],
    /// When the work belongs to a project: its name and allowed runtimes.
    pub project: Option<(&'a str, &'a [String])>,
    /// Runtimes whose work this worker reviews (for cross-company review).
    pub reviewed: &'a [String],
    pub on_limit: LimitBehavior,
    pub now: u64,
}

struct Candidate<'a> {
    id: &'a str,
    /// Its place in the role's list (1-based), when the role has one.
    rank: Option<u32>,
    model: Option<&'a ModelInfo>,
}

/// "Opus (Claude Code)", or just the label when it already names the tool.
pub fn model_label(model: &ModelInfo, tools: &[ToolState]) -> String {
    let tool = tools
        .iter()
        .find(|t| t.info.id == model.runtime_id)
        .map_or(model.runtime_id.as_str(), |t| t.info.label.as_str());
    if model.label.to_lowercase().contains(&tool.to_lowercase()) {
        model.label.clone()
    } else {
        format!("{} ({tool})", model.label)
    }
}

/// Why a tool cannot take work, if it cannot (install and sign-in only).
pub fn not_ready(info: &AgentRuntimeInfo) -> Option<String> {
    if info.ready {
        return None;
    }
    let label = &info.label;
    Some(match info.installation.state {
        InstallState::Checking => format!("{label} is still being checked"),
        InstallState::NotInstalled => format!("{label} is not installed"),
        InstallState::Unsupported | InstallState::Broken => {
            format!("{label} is installed in a way Plenipo cannot run")
        }
        // Given no tasks after an update that left it not answering (ADR-059 §6).
        InstallState::Installed if info.installation.detail.is_some() => {
            format!("Plenipo is not giving {label} tasks for now")
        }
        InstallState::Installed => match info.auth.state {
            AuthState::Checking => format!("{label} is still being checked"),
            AuthState::SignedOut => format!("{label} is not signed in"),
            AuthState::ApiKey => {
                format!("{label} is signed in with an API key, and pay-per-use API billing is off")
            }
            AuthState::ThirdPartyCloud => {
                format!("{label} is set up for a third-party cloud, which Plenipo does not use")
            }
            AuthState::Subscription | AuthState::Unverified | AuthState::Unknown => {
                format!("Plenipo could not confirm {label}'s subscription sign-in")
            }
        },
    })
}

/// "reached its usage limit (resets in about 3 hours)".
pub fn limit_words(limit: &UsageLimit, now: u64) -> String {
    let left = duration_words(limit.until.saturating_sub(now));
    if limit.resets_at.is_some() {
        format!("reached its usage limit (resets in {left})")
    } else {
        format!("reached its usage limit (Plenipo tries it again in {left})")
    }
}

fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

fn ordinal(n: u32) -> String {
    match n {
        1 => "first".into(),
        2 => "second".into(),
        3 => "third".into(),
        _ => format!("#{n}"),
    }
}

/// One layer of model and effort choices, as the engine reads it (ADR-041).
#[derive(Debug, Clone)]
pub struct Layer<'a> {
    pub source: RuleSource,
    pub models: &'a [String],
    pub efforts: &'a BTreeMap<String, Effort>,
    pub effort: Option<Effort>,
    pub never: &'a [String],
}

/// The layers of `input`, closest first: the agent, its role, its department, the organization.
pub fn layers<'a>(input: &RouteInput<'a>) -> Vec<Layer<'a>> {
    let mut out = Vec::new();
    if let Some((position_id, rule)) = input.agent {
        out.push(Layer {
            source: RuleSource {
                layer: RuleLayer::Agent,
                name: "this agent".into(),
                id: Some(position_id.to_owned()),
            },
            models: &rule.models,
            efforts: &rule.efforts,
            effort: rule.effort,
            never: &rule.never_companies,
        });
    }
    out.push(Layer {
        source: RuleSource {
            layer: RuleLayer::Role,
            name: input.role.to_owned(),
            id: (!input.role_id.is_empty()).then(|| input.role_id.to_owned()),
        },
        models: &input.policy.models,
        efforts: &input.policy.efforts,
        effort: input.policy.effort,
        never: &input.policy.never_companies,
    });
    if let Some(d) = input.department {
        out.push(Layer {
            source: RuleSource {
                layer: RuleLayer::Department,
                name: format!("the {} department", d.name),
                id: Some(d.id.to_owned()),
            },
            models: &d.rule.models,
            efforts: &d.rule.efforts,
            effort: d.rule.effort,
            never: &d.rule.never_companies,
        });
    }
    if let Some(rule) = input.organization {
        out.push(Layer {
            source: RuleSource {
                layer: RuleLayer::Organization,
                name: "the organization".into(),
                id: None,
            },
            models: &rule.models,
            efforts: &rule.efforts,
            effort: rule.effort,
            never: &rule.never_companies,
        });
    }
    out
}

/// "the Development department's", "this agent's", "Senior Developer's".
fn possessive(source: &RuleSource) -> String {
    format!("{}'s", source.name)
}

/// The closest layer that never uses `company`, by name.
pub fn never_by(layers: &[Layer<'_>], company: &str) -> Option<String> {
    layers
        .iter()
        .find(|l| l.never.iter().any(|c| c == company))
        .map(|l| l.source.name.clone())
}

/// The effort for model `model_id` (`""` for a model not in the owner's list): from the closest
/// layer that sets one it takes, else its own setting. Also the first level set for it that it
/// does not take, and by whom, to say so.
pub fn effort_for(
    layers: &[Layer<'_>],
    model_id: &str,
    model_effort: Option<Effort>,
    levels: &[Effort],
) -> (
    Option<Effort>,
    Option<RuleSource>,
    Option<(Effort, RuleSource)>,
) {
    let mut passed = None;
    for l in layers {
        let own = (!model_id.is_empty())
            .then(|| l.efforts.get(model_id).copied())
            .flatten();
        for e in [own, l.effort].into_iter().flatten() {
            if levels.contains(&e) {
                return (Some(e), Some(l.source.clone()), passed);
            }
            if passed.is_none() {
                passed = Some((e, l.source.clone()));
            }
        }
    }
    match model_effort.filter(|e| levels.contains(e)) {
        Some(e) => (
            Some(e),
            Some(RuleSource {
                layer: RuleLayer::Model,
                name: "the model".into(),
                id: (!model_id.is_empty()).then(|| model_id.to_owned()),
            }),
            passed,
        ),
        None => (None, None, passed),
    }
}

/// " It runs at high effort, from the Development department's rule." and, when a closer level
/// did not work with the model, why it was passed over.
pub fn effort_words(
    effort: Option<Effort>,
    from: Option<&RuleSource>,
    passed: Option<&(Effort, RuleSource)>,
    label: &str,
) -> String {
    let mut out = String::new();
    if let (Some(e), Some(src)) = (effort, from) {
        let whose = match src.layer {
            RuleLayer::Agent => "this agent's own setting".to_owned(),
            RuleLayer::Model | RuleLayer::Fixed => "the model's own setting".to_owned(),
            _ => format!("{} rule", possessive(src)),
        };
        out.push_str(&format!(" It runs at {} effort, from {whose}.", e.label()));
    }
    if let Some((e, src)) = passed {
        let set_by = match src.layer {
            RuleLayer::Agent => "this agent's own setting".to_owned(),
            _ => format!("{} rule", possessive(src)),
        };
        let mut level = e.label().to_owned();
        if let Some(first) = level.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        out.push_str(&format!(
            " {level} effort, from {set_by}, does not work with {label}."
        ));
    }
    out
}

pub fn route(input: &RouteInput<'_>) -> RouteDecision {
    let policy = input.policy;
    let layers = layers(input);
    let tool = |id: &str| input.tools.iter().find(|t| t.info.id == id);
    let company = |m: &ModelInfo| tool(&m.runtime_id).map(|t| t.info.provider.as_str());

    // 1. Candidates: the closest layer's list, else the whole registry.
    let listing = layers.iter().find(|l| !l.models.is_empty());
    let mut candidates: Vec<Candidate<'_>> = if let Some(l) = listing {
        l.models
            .iter()
            .zip(1..)
            .map(|(id, rank)| Candidate {
                id,
                rank: Some(rank),
                model: input.models.iter().find(|m| &m.id == id),
            })
            .collect()
    } else {
        let mut all: Vec<&ModelInfo> = input.models.iter().collect();
        match policy.cost {
            CostPreference::Any => {}
            CostPreference::Economical => all.sort_by_key(|m| m.cost),
            CostPreference::Premium => all.sort_by_key(|m| std::cmp::Reverse(m.cost)),
        }
        all.into_iter()
            .map(|m| Candidate {
                id: &m.id,
                rank: None,
                model: Some(m),
            })
            .collect()
    };

    // 2. Cross-company review.
    let mut reviewed: Vec<(&str, &str)> = Vec::new();
    for t in input.reviewed.iter().filter_map(|r| tool(r)) {
        let c = (t.info.provider.as_str(), t.info.provider_label.as_str());
        if !reviewed.contains(&c) {
            reviewed.push(c);
        }
    }
    let cross = if reviewed.is_empty() {
        CrossCompany::Off
    } else {
        policy.cross_company
    };
    let reviewed_names = list(
        &reviewed
            .iter()
            .map(|(_, l)| (*l).to_owned())
            .collect::<Vec<_>>(),
    );
    let same_company =
        |m: &ModelInfo| company(m).is_some_and(|c| reviewed.iter().any(|(r, _)| *r == c));
    if cross == CrossCompany::Prefer {
        candidates.sort_by_key(|c| c.model.is_some_and(same_company));
    }

    // 3–5. Checks.
    let mut notes: Vec<CandidateNote> = Vec::new();
    let mut chosen: Option<(RouteChoice, Option<u32>)> = None;
    let mut effort_from: Option<RuleSource> = None;
    let mut effort_passed: Option<(Effort, RuleSource)> = None;
    // The company whose usage limit work is waiting for (LimitBehavior::Wait).
    let mut waiting: Option<(&str, String)> = None;
    for c in &candidates {
        let Some(m) = c.model else {
            notes.push(CandidateNote {
                model_id: c.id.to_owned(),
                label: "A removed model".into(),
                verdict: CandidateVerdict::Skipped,
                note: "it is no longer in your list of models".into(),
            });
            continue;
        };
        let label = model_label(m, input.tools);
        let mut note = |verdict, text: String| {
            notes.push(CandidateNote {
                model_id: m.id.clone(),
                label: label.clone(),
                verdict,
                note: text,
            });
        };
        if chosen.is_some() {
            note(CandidateVerdict::NotNeeded, String::new());
            continue;
        }
        let Some(t) = tool(&m.runtime_id) else {
            note(
                CandidateVerdict::Skipped,
                format!(
                    "its AI tool ({}) is not available in this version of Plenipo",
                    m.runtime_id
                ),
            );
            continue;
        };
        let info = &t.info;
        let skip = if let Some(who) = never_by(&layers, &info.provider) {
            Some(format!("{who} never uses {}", info.provider_label))
        } else if let Some((name, _)) = input
            .project
            .filter(|(_, allowed)| !allowed.contains(&info.id))
        {
            Some(format!("{name} does not allow {}", info.label))
        } else if let Some(f) = policy.needs.iter().find(|f| !m.features.contains(f)) {
            Some(format!("it is not marked as able to {}", f.words()))
        } else if let Some(min) = policy
            .min_context_tokens
            .filter(|min| m.context_tokens.is_none_or(|have| have < *min))
        {
            Some(match m.context_tokens {
                None => format!(
                    "its context size is not recorded ({} needs {min} tokens)",
                    input.role
                ),
                Some(have) => format!(
                    "it takes {have} tokens of context and {} needs {min}",
                    input.role
                ),
            })
        } else if cross == CrossCompany::Require && same_company(m) {
            Some(format!(
                "{} needs a different AI company than the work it reviews ({reviewed_names})",
                input.role
            ))
        } else if let Some(why) = not_ready(info) {
            Some(why)
        } else if let Some(limit) = &t.limit {
            if input.on_limit == LimitBehavior::Wait && waiting.is_none() {
                waiting = Some((info.provider.as_str(), info.label.clone()));
            }
            Some(format!("{} {}", info.label, limit_words(limit, input.now)))
        } else if let Some((_, tool_label)) = waiting.as_ref().filter(|(p, _)| *p != info.provider)
        {
            Some(format!(
                "work waits for {tool_label}'s usage limit to reset instead of moving to another AI \
                 company (Settings → AI models)"
            ))
        } else {
            None
        };
        match skip {
            Some(why) => note(CandidateVerdict::Skipped, why),
            None => {
                note(CandidateVerdict::Chosen, String::new());
                // Only a level the model (or, for a model the AI tool does not list, the AI
                // tool) takes.
                let levels = info.capabilities.effort_levels_for(m.name.as_deref());
                let (effort, from, passed) = effort_for(&layers, &m.id, m.effort, levels);
                chosen = Some((
                    RouteChoice {
                        model_id: m.id.clone(),
                        runtime_id: info.id.clone(),
                        runtime_label: info.label.clone(),
                        company: info.provider.clone(),
                        model: m.name.clone(),
                        effort,
                        label: label.clone(),
                    },
                    c.rank,
                ));
                effort_from = from;
                effort_passed = passed;
            }
        }
    }

    // Explanation.
    let skipped = |n: &CandidateNote| format!("{} was skipped because {}", n.label, n.note);
    let first_skip = notes
        .iter()
        .find(|n| n.verdict == CandidateVerdict::Skipped);
    match chosen {
        Some((choice, rank)) => {
            let whose =
                listing.map_or_else(|| format!("{}'s", input.role), |l| possessive(&l.source));
            let mut reason = match rank {
                Some(1) => format!("{} is {whose} first choice and is ready.", choice.label),
                Some(n) => format!(
                    "{} is {whose} {} choice: {}.",
                    choice.label,
                    ordinal(n),
                    first_skip.map_or_else(String::new, skipped)
                ),
                None => {
                    let order = match policy.cost {
                        CostPreference::Any => "",
                        CostPreference::Economical => ", economical models first",
                        CostPreference::Premium => ", premium models first",
                    };
                    let mut s = format!(
                        "{} has no preferred models, so Plenipo uses {}, the first ready model in \
                         your list{order}.",
                        input.role, choice.label
                    );
                    if let Some(n) = first_skip {
                        s.push_str(&format!(" {}.", skipped(n)));
                    }
                    s
                }
            };
            reason.push_str(&effort_words(
                choice.effort,
                effort_from.as_ref(),
                effort_passed.as_ref(),
                &choice.label,
            ));
            if cross != CrossCompany::Off {
                let other = !reviewed.iter().any(|(c, _)| *c == choice.company);
                if other {
                    reason.push_str(&format!(
                        " It comes from a different AI company than the work it reviews \
                         ({reviewed_names})."
                    ));
                } else {
                    reason.push_str(&format!(
                        " No ready model from another AI company than the work it reviews \
                         ({reviewed_names}), so it comes from the same one."
                    ));
                }
            }
            RouteDecision {
                choice: Some(choice),
                reason: fix_sentence(&reason),
                rank,
                candidates: notes,
                fixed: false,
                model_from: listing.map(|l| l.source.clone()),
                effort_from,
            }
        }
        None => {
            let reason = if notes.is_empty() {
                format!(
                    "No models are set up for {}: add one in Settings → AI models.",
                    input.role
                )
            } else if let Some((_, tool_label)) = &waiting {
                format!(
                    "{tool_label} reached its usage limit, and {} waits for it rather than moving \
                     work to another AI company (Settings → AI models).",
                    input.role
                )
            } else {
                let reasons: Vec<String> = notes
                    .iter()
                    .take(3)
                    .map(|n| format!("{}: {}", n.label, n.note))
                    .collect();
                let more = notes.len().saturating_sub(3);
                let tail = if more > 0 {
                    format!("; and {more} more")
                } else {
                    String::new()
                };
                format!(
                    "No model can take {}'s work now — {}{tail}.",
                    input.role,
                    reasons.join("; ")
                )
            };
            RouteDecision {
                choice: None,
                reason,
                rank: None,
                candidates: notes,
                fixed: false,
                model_from: None,
                effort_from: None,
            }
        }
    }
}

/// Collapse doubled full stops left by notes that end with one.
fn fix_sentence(s: &str) -> String {
    s.replace("..", ".").replace(": .", ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_runtime::agent::{AuthStatus, Installation, KnownModel, RuntimeCapabilities};

    const NOW: u64 = 1_760_000_000_000;

    fn tool(id: &str, company: &str) -> ToolState {
        let label = match id {
            "alpha" => "Alpha Code",
            "beta" => "Beta CLI",
            other => other,
        };
        ToolState {
            info: AgentRuntimeInfo {
                id: id.into(),
                label: label.into(),
                provider: company.into(),
                provider_label: company.to_uppercase(),
                installation: Installation {
                    state: InstallState::Installed,
                    executable: None,
                    version: Some("1.0".into()),
                    detail: None,
                },
                auth: AuthStatus {
                    state: AuthState::Subscription,
                    method: None,
                    detail: None,
                },
                capabilities: RuntimeCapabilities {
                    streaming_text: true,
                    resume: true,
                    cancel: true,
                    structured_results: true,
                    billing_checked_per_turn: false,
                    tool_posture: String::new(),
                    effort_levels: vec![Effort::Low, Effort::High],
                    known_models: vec![KnownModel::new("sonnet", "Sonnet", &[])],
                },
                install_hint: String::new(),
                login_hint: String::new(),
                ready: true,
                checked_at: None,
                checked_version: String::new(),
                account: Default::default(),
                reported_models: None,
            },
            limit: None,
        }
    }

    fn model(id: &str, runtime: &str, label: &str) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            runtime_id: runtime.into(),
            name: Some(id.into()),
            label: label.into(),
            features: vec![],
            context_tokens: None,
            cost: CostClass::Standard,
            effort: None,
            built_in: false,
        }
    }

    struct World {
        models: Vec<ModelInfo>,
        tools: Vec<ToolState>,
    }

    fn world() -> World {
        World {
            models: vec![
                model("opus", "alpha", "Opus"),
                model("sonnet", "alpha", "Sonnet"),
                model("gpt", "beta", "GPT"),
            ],
            tools: vec![tool("alpha", "acme"), tool("beta", "bolt")],
        }
    }

    fn decide(w: &World, policy: &RolePolicy) -> RouteDecision {
        decide_with(w, policy, None, &[], LimitBehavior::Wait)
    }

    fn decide_with(
        w: &World,
        policy: &RolePolicy,
        project: Option<(&str, &[String])>,
        reviewed: &[String],
        on_limit: LimitBehavior,
    ) -> RouteDecision {
        route(&RouteInput {
            role: "Senior Developer",
            role_id: "dev",
            policy,
            agent: None,
            department: None,
            organization: None,
            models: &w.models,
            tools: &w.tools,
            project,
            reviewed,
            on_limit,
            now: NOW,
        })
    }

    fn chosen(d: &RouteDecision) -> Option<&str> {
        d.choice.as_ref().map(|c| c.model_id.as_str())
    }

    fn prefer(ids: &[&str]) -> RolePolicy {
        RolePolicy {
            models: ids.iter().map(|s| (*s).to_owned()).collect(),
            ..RolePolicy::default()
        }
    }

    fn verdicts(d: &RouteDecision) -> Vec<CandidateVerdict> {
        d.candidates.iter().map(|c| c.verdict).collect()
    }

    // ---- The plan's tests ------------------------------------------------------------------

    #[test]
    fn preferred_model_available() {
        let w = world();
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(chosen(&d), Some("opus"));
        let c = d.choice.as_ref().unwrap();
        assert_eq!(
            (
                c.runtime_id.as_str(),
                c.model.as_deref(),
                c.company.as_str()
            ),
            ("alpha", Some("opus"), "acme")
        );
        assert_eq!(d.rank, Some(1));
        assert_eq!(
            d.reason,
            "Opus (Alpha Code) is Senior Developer's first choice and is ready."
        );
        use CandidateVerdict::*;
        assert_eq!(verdicts(&d), [Chosen, NotNeeded]);
    }

    #[test]
    fn effort_comes_from_the_role_then_the_model() {
        let mut w = world();
        // No effort anywhere: the AI tool's default, and the reason says nothing about it.
        let d = decide(&w, &prefer(&["opus"]));
        assert_eq!(d.choice.as_ref().unwrap().effort, None);
        assert!(!d.reason.contains("effort"));
        // The model's own setting.
        w.models[0].effort = Some(Effort::Low);
        let d = decide(&w, &prefer(&["opus"]));
        assert_eq!(d.choice.as_ref().unwrap().effort, Some(Effort::Low));
        assert!(
            d.reason
                .ends_with("It runs at low effort, from the model's own setting."),
            "{}",
            d.reason
        );
        assert_eq!(d.effort_from.as_ref().unwrap().layer, RuleLayer::Model);
        // The role's setting for that model wins.
        let mut policy = prefer(&["opus"]);
        policy.efforts.insert("opus".into(), Effort::High);
        let d = decide(&w, &policy);
        assert_eq!(d.choice.as_ref().unwrap().effort, Some(Effort::High));
        assert!(
            d.reason
                .ends_with("It runs at high effort, from Senior Developer's rule."),
            "{}",
            d.reason
        );
        assert_eq!(d.effort_from.as_ref().unwrap().layer, RuleLayer::Role);
        // A level the AI tool does not accept is never passed on, and the reason says so.
        policy.efforts.insert("opus".into(), Effort::Max);
        let d = decide(&w, &policy);
        assert_eq!(d.choice.as_ref().unwrap().effort, Some(Effort::Low));
        assert!(
            d.reason.ends_with(
                "It runs at low effort, from the model's own setting. Max effort, from Senior \
                 Developer's rule, does not work with Opus (Alpha Code)."
            ),
            "{}",
            d.reason
        );
        // Nor one the model has no setting for (the AI tool lists Sonnet without effort levels).
        let mut sonnet = prefer(&["sonnet"]);
        sonnet.efforts.insert("sonnet".into(), Effort::High);
        let d = decide(&w, &sonnet);
        assert_eq!(d.choice.as_ref().unwrap().effort, None);
        assert!(d.effort_from.is_none());
        assert!(
            d.reason
                .contains("High effort, from Senior Developer's rule, does not work"),
            "{}",
            d.reason
        );
    }

    #[test]
    fn preferred_unavailable_falls_back() {
        let mut w = world();
        w.tools[0].info.installation.state = InstallState::NotInstalled;
        w.tools[0].info.ready = false;
        let d = decide(&w, &prefer(&["opus", "sonnet", "gpt"]));
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(d.rank, Some(3));
        assert_eq!(
            d.reason,
            "GPT (Beta CLI) is Senior Developer's third choice: Opus (Alpha Code) was skipped \
             because Alpha Code is not installed."
        );
        assert_eq!(d.candidates[1].note, "Alpha Code is not installed");
    }

    #[test]
    fn provider_unauthenticated() {
        let mut w = world();
        w.tools[0].info.auth.state = AuthState::SignedOut;
        w.tools[0].info.ready = false;
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(chosen(&d), Some("gpt"));
        assert!(
            d.reason.contains("Alpha Code is not signed in"),
            "{}",
            d.reason
        );
        // Still being checked (startup) reads differently, and nothing else is ready.
        w.tools[1].info.installation.state = InstallState::Checking;
        w.tools[1].info.ready = false;
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(chosen(&d), None);
        assert!(
            d.reason.contains("Beta CLI is still being checked"),
            "{}",
            d.reason
        );
    }

    #[test]
    fn usage_cap_reached_waits_by_default_and_can_move_on() {
        let mut w = world();
        w.tools[0].limit = Some(UsageLimit {
            model: Some("opus".into()),
            since: NOW - 1000,
            resets_at: Some(NOW + 3 * 3_600_000),
            until: NOW + 3 * 3_600_000,
            detail: "usage limit reached".into(),
        });
        let policy = prefer(&["opus", "sonnet", "gpt"]);
        // Wait: no switching company because of a usage limit.
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), None);
        assert_eq!(
            d.reason,
            "Alpha Code reached its usage limit, and Senior Developer waits for it rather than \
             moving work to another AI company (Settings → AI models)."
        );
        assert_eq!(
            d.candidates[0].note,
            "Alpha Code reached its usage limit (resets in about 3 hours)"
        );
        assert!(d.candidates[2]
            .note
            .contains("waits for Alpha Code's usage limit"));
        // Next choice: the owner allowed moving on.
        let d = decide_with(&w, &policy, None, &[], LimitBehavior::NextChoice);
        assert_eq!(chosen(&d), Some("gpt"));
        assert!(d.reason.contains("reached its usage limit"), "{}", d.reason);
        // Another model of the same (unlimited) company is not held back by waiting.
        w.models.push(model("gpt2", "beta", "GPT 2"));
        let d = decide(&w, &prefer(&["gpt", "opus", "gpt2"]));
        assert_eq!(chosen(&d), Some("gpt"));
    }

    #[test]
    fn capability_requirement_mismatch() {
        let mut w = world();
        let policy = RolePolicy {
            needs: vec![ModelFeature::Vision, ModelFeature::ImageGeneration],
            ..prefer(&["opus", "gpt"])
        };
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), None);
        assert!(d
            .reason
            .starts_with("No model can take Senior Developer's work now — "));
        assert!(d.candidates[0]
            .note
            .contains("not marked as able to see images"));
        // A model marked with both is chosen, even as a later choice.
        w.models[2].features = vec![ModelFeature::ImageGeneration, ModelFeature::Vision];
        assert_eq!(chosen(&decide(&w, &policy)), Some("gpt"));
        // Context: unknown or too small is skipped.
        let policy = RolePolicy {
            min_context_tokens: Some(200_000),
            ..prefer(&["opus", "sonnet", "gpt"])
        };
        w.models[1].context_tokens = Some(100_000);
        w.models[2].context_tokens = Some(400_000);
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(
            d.candidates[0].note,
            "its context size is not recorded (Senior Developer needs 200000 tokens)"
        );
        assert_eq!(
            d.candidates[1].note,
            "it takes 100000 tokens of context and Senior Developer needs 200000"
        );
    }

    #[test]
    fn api_fallback_disabled() {
        let mut w = world();
        // The preferred tool is signed in with an API key: never used, whatever the list says.
        w.tools[0].info.auth.state = AuthState::ApiKey;
        w.tools[0].info.ready = false;
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(
            d.candidates[0].note,
            "Alpha Code is signed in with an API key, and pay-per-use API billing is off"
        );
        // With no subscription tool left, nothing falls back to API billing.
        w.tools[1].info.auth.state = AuthState::ThirdPartyCloud;
        w.tools[1].info.ready = false;
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(chosen(&d), None);
        assert!(d.reason.contains("third-party cloud"), "{}", d.reason);
    }

    #[test]
    fn no_eligible_model() {
        let w = World {
            models: vec![],
            tools: world().tools,
        };
        let d = decide(&w, &RolePolicy::default());
        assert_eq!(chosen(&d), None);
        assert_eq!(
            d.reason,
            "No models are set up for Senior Developer: add one in Settings → AI models."
        );
        // Every choice ruled out: the reasons are listed.
        let w = world();
        let policy = RolePolicy {
            never_companies: vec!["acme".into(), "bolt".into()],
            ..prefer(&["opus", "sonnet", "gpt", "missing"])
        };
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), None);
        assert_eq!(
            d.reason,
            "No model can take Senior Developer's work now — Opus (Alpha Code): Senior Developer \
             never uses ACME; Sonnet (Alpha Code): Senior Developer never uses ACME; GPT (Beta \
             CLI): Senior Developer never uses BOLT; and 1 more."
        );
        assert_eq!(
            d.candidates[3].note,
            "it is no longer in your list of models"
        );
    }

    #[test]
    fn cross_provider_reviewer_rule() {
        let w = world();
        let reviewed = ["alpha".to_owned()];
        // Prefer: another company first, even though Opus is listed first.
        let policy = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["opus", "gpt"])
        };
        let d = decide_with(&w, &policy, None, &reviewed, LimitBehavior::Wait);
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(d.rank, Some(2));
        assert!(
            d.reason
                .ends_with("It comes from a different AI company than the work it reviews (ACME)."),
            "{}",
            d.reason
        );
        // Prefer, but nothing from another company is ready: the same company, said plainly.
        let mut limited = world();
        limited.tools[1].info.ready = false;
        limited.tools[1].info.auth.state = AuthState::SignedOut;
        let d = decide_with(&limited, &policy, None, &reviewed, LimitBehavior::Wait);
        assert_eq!(chosen(&d), Some("opus"));
        assert!(
            d.reason.contains("so it comes from the same one"),
            "{}",
            d.reason
        );
        // Require: the same company is never used.
        let policy = RolePolicy {
            cross_company: CrossCompany::Require,
            ..prefer(&["opus", "gpt"])
        };
        let d = decide_with(&limited, &policy, None, &reviewed, LimitBehavior::Wait);
        assert_eq!(chosen(&d), None);
        assert!(d.candidates[0]
            .note
            .contains("needs a different AI company than the work it reviews (ACME)"));
        // Nothing reviewed (or the rule off): the list order stands.
        assert_eq!(chosen(&decide(&w, &policy)), Some("opus"));
    }

    // ---- More rules ------------------------------------------------------------------------

    #[test]
    fn project_rules_and_never_used_companies() {
        let w = world();
        let allowed = ["beta".to_owned()];
        let d = decide_with(
            &w,
            &prefer(&["opus", "gpt"]),
            Some(("Website", &allowed)),
            &[],
            LimitBehavior::Wait,
        );
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(d.candidates[0].note, "Website does not allow Alpha Code");
        let policy = RolePolicy {
            never_companies: vec!["acme".into()],
            ..prefer(&["opus", "gpt"])
        };
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(d.candidates[0].note, "Senior Developer never uses ACME");
        // A model whose tool this version does not have.
        let mut w = world();
        w.models[0].runtime_id = "gamma".into();
        let d = decide(&w, &prefer(&["opus", "gpt"]));
        assert_eq!(
            d.candidates[0].note,
            "its AI tool (gamma) is not available in this version of Plenipo"
        );
        assert_eq!(d.candidates[0].label, "Opus (gamma)");
    }

    #[test]
    fn without_a_list_the_cost_preference_orders_the_registry() {
        let mut w = world();
        w.models[0].cost = CostClass::Premium;
        w.models[2].cost = CostClass::Economical;
        let any = decide(&w, &RolePolicy::default());
        assert_eq!(chosen(&any), Some("opus"));
        assert_eq!(any.rank, None);
        assert_eq!(
            any.reason,
            "Senior Developer has no preferred models, so Plenipo uses Opus (Alpha Code), the \
             first ready model in your list."
        );
        let cheap = decide(
            &w,
            &RolePolicy {
                cost: CostPreference::Economical,
                ..RolePolicy::default()
            },
        );
        assert_eq!(chosen(&cheap), Some("gpt"));
        assert!(
            cheap.reason.ends_with("economical models first."),
            "{}",
            cheap.reason
        );
        let dear = decide(
            &w,
            &RolePolicy {
                cost: CostPreference::Premium,
                ..RolePolicy::default()
            },
        );
        assert_eq!(chosen(&dear), Some("opus"));
        // Standard before economical when premium is preferred.
        w.models[0].cost = CostClass::Economical;
        let dear = decide(
            &w,
            &RolePolicy {
                cost: CostPreference::Premium,
                ..RolePolicy::default()
            },
        );
        assert_eq!(chosen(&dear), Some("sonnet"));
    }

    #[test]
    fn labels_do_not_repeat_the_tool() {
        let w = world();
        let mut m = model("x", "alpha", "Alpha Code (default model)");
        assert_eq!(model_label(&m, &w.tools), "Alpha Code (default model)");
        m.label = "Fast".into();
        assert_eq!(model_label(&m, &w.tools), "Fast (Alpha Code)");
    }

    fn decide_layers(
        w: &World,
        agent: Option<&ModelRule>,
        policy: &RolePolicy,
        department: Option<&ModelRule>,
        organization: Option<&ModelRule>,
    ) -> RouteDecision {
        route(&RouteInput {
            role: "Senior Developer",
            role_id: "dev",
            policy,
            agent: agent.map(|r| ("pos-1", r)),
            department: department.map(|rule| DepartmentRule {
                id: "d1",
                name: "Development",
                rule,
            }),
            organization,
            models: &w.models,
            tools: &w.tools,
            project: None,
            reviewed: &[],
            on_limit: LimitBehavior::Wait,
            now: NOW,
        })
    }

    fn rule(models: &[&str], effort: Option<Effort>) -> ModelRule {
        ModelRule {
            models: models.iter().map(|m| (*m).to_owned()).collect(),
            effort,
            ..ModelRule::default()
        }
    }

    /// ADR-041: the organization, a department, a role, and one agent each set model and effort;
    /// the closest wins, and the reason names the layer that decided.
    #[test]
    fn each_layer_sets_model_and_effort_and_the_closest_wins() {
        let w = world();
        let none = RolePolicy::default();
        let org = rule(&["opus"], Some(Effort::High));
        let d = decide_layers(&w, None, &none, None, Some(&org));
        assert_eq!(chosen(&d), Some("opus"));
        assert_eq!(
            d.reason,
            "Opus (Alpha Code) is the organization's first choice and is ready. It runs at high \
             effort, from the organization's rule."
        );
        assert_eq!(
            d.model_from.as_ref().unwrap().layer,
            RuleLayer::Organization
        );
        assert_eq!(
            d.effort_from.as_ref().unwrap().layer,
            RuleLayer::Organization
        );
        // The department's rule wins over the organization's.
        let dept = rule(&["gpt"], Some(Effort::Low));
        let d = decide_layers(&w, None, &none, Some(&dept), Some(&org));
        assert_eq!(chosen(&d), Some("gpt"));
        assert_eq!(
            d.reason,
            "GPT (Beta CLI) is the Development department's first choice and is ready. It runs at \
             low effort, from the Development department's rule."
        );
        assert_eq!(
            d.model_from.as_ref().map(|s| (s.layer, s.id.as_deref())),
            Some((RuleLayer::Department, Some("d1")))
        );
        // A role's own list is closer than the department's; the department still sets effort.
        let role = prefer(&["opus"]);
        let d = decide_layers(&w, None, &role, Some(&dept), Some(&org));
        assert_eq!(chosen(&d), Some("opus"));
        assert!(
            d.reason
                .starts_with("Opus (Alpha Code) is Senior Developer's first choice")
                && d.reason
                    .ends_with("It runs at low effort, from the Development department's rule."),
            "{}",
            d.reason
        );
        // One agent's own effort wins over every layer.
        let agent = rule(&[], Some(Effort::High));
        let d = decide_layers(&w, Some(&agent), &role, Some(&dept), Some(&org));
        assert_eq!(d.choice.as_ref().unwrap().effort, Some(Effort::High));
        assert!(
            d.reason
                .ends_with("It runs at high effort, from this agent's own setting."),
            "{}",
            d.reason
        );
        assert_eq!(d.effort_from.as_ref().unwrap().layer, RuleLayer::Agent);
        // And its own list wins over its role's.
        let agent = rule(&["gpt"], None);
        let d = decide_layers(&w, Some(&agent), &role, Some(&dept), Some(&org));
        assert_eq!(chosen(&d), Some("gpt"));
        assert!(d
            .reason
            .starts_with("GPT (Beta CLI) is this agent's first choice"));
        // A layer's effort for one model wins over its effort for any model.
        let mut org_both = rule(&["opus"], Some(Effort::High));
        org_both.efforts.insert("opus".into(), Effort::Low);
        let d = decide_layers(&w, None, &none, None, Some(&org_both));
        assert_eq!(d.choice.unwrap().effort, Some(Effort::Low));
    }

    /// ADR-041 §4: AI companies never to use add up; a closer layer cannot lift a ban.
    #[test]
    fn never_used_companies_add_up_across_layers() {
        let w = world();
        let org = ModelRule {
            never_companies: vec!["bolt".into()],
            ..ModelRule::default()
        };
        let role = prefer(&["gpt", "opus"]);
        let d = decide_layers(&w, None, &role, None, Some(&org));
        assert_eq!(chosen(&d), Some("opus"));
        assert_eq!(d.candidates[0].note, "the organization never uses BOLT");
        // An agent whose own rule never mentions BOLT still cannot use it.
        let agent = rule(&["gpt"], None);
        let d = decide_layers(&w, Some(&agent), &role, None, Some(&org));
        assert_eq!(chosen(&d), None);
        assert!(
            d.reason.contains("the organization never uses BOLT"),
            "{}",
            d.reason
        );
        // Bans from several layers all hold.
        let dept = ModelRule {
            never_companies: vec!["acme".into()],
            ..ModelRule::default()
        };
        let d = decide_layers(&w, None, &role, Some(&dept), Some(&org));
        assert_eq!(chosen(&d), None);
        assert_eq!(
            d.candidates[1].note,
            "the Development department never uses ACME"
        );
    }
}
