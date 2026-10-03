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
//! 2. **Cross-company review,** by who made the models (ADR-081 §3): when the worker reviews
//!    work made by some AI companies and the role prefers another company, models by other
//!    companies go first; when it requires one, models by the same companies are skipped. A
//!    model whose maker is not known, or work by one, is never counted as another company's
//!    (ADR-081 §7).
//! 3. **Checks, per model:** its AI tool exists; neither its AI tool's company nor the company
//!    that made it is on a layer's never-use list (ADR-081 §5);
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

use plenipo_runtime::agent::{
    AgentRuntimeInfo, AuthState, Effort, InstallState, Maker, WorkDoneBy,
};

use crate::dto::*;
use crate::limits::duration_words;
use crate::makers::maker_of;

/// One AI tool as the router sees it: detection state and any usage limit.
#[derive(Debug, Clone)]
pub struct ToolState {
    pub info: AgentRuntimeInfo,
    pub limit: Option<UsageLimit>,
    /// A paid AI tool (ADR-085): paid per use with the owner's key, whatever its check says now
    /// (signed out, checking, or ready).
    pub paid: bool,
}

/// The paid AI tools this version has (ADR-085), by ID.
pub fn paid_tool_ids() -> std::collections::HashSet<String> {
    plenipo_runtime::agent::builtin_adapters()
        .iter()
        .filter(|a| a.paid())
        .map(|a| a.id().to_owned())
        .collect()
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
    /// The work this worker reviews: each AI tool and the model it ran (cross-company review).
    pub reviewed: &'a [WorkDoneBy],
    pub on_limit: LimitBehavior,
    pub now: u64,
    /// What is left this month under the spending caps covering this work (ADR-085): a paid
    /// route is skipped when nothing is. None: not known, or no paid route can run anyway.
    pub spending_room: Option<u64>,
}

/// The price of `model` on a paid AI tool, from what it reported, when it makes sense (the
/// runtime refuses any other). `Some(None)` for its default model, which Plenipo prices right
/// before the task (the runtime refuses it then if it has no price); None: not priced.
fn paid_price(
    info: &plenipo_runtime::agent::AgentRuntimeInfo,
    model: Option<&str>,
) -> Option<Option<plenipo_runtime::pricing::Price>> {
    let Some(name) = model else {
        return Some(None);
    };
    info.reported_models
        .as_ref()?
        .models
        .iter()
        .find(|m| m.name == name)
        .and_then(|m| m.price)
        .filter(plenipo_runtime::pricing::Price::is_sane)
        .map(Some)
}

/// A worker on this AI tool answers in text only (its own words say so).
pub(crate) fn text_only(info: &plenipo_runtime::agent::AgentRuntimeInfo) -> bool {
    info.capabilities
        .tool_posture
        .starts_with("Conversation only")
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
            // A paid AI tool says why (ADR-085): paid keys switched off, no key, or the key
            // refused.
            AuthState::SignedOut => match info.auth.detail.as_deref().map(str::trim) {
                Some(why) if !why.is_empty() => {
                    format!(
                        "{label} cannot take work now ({})",
                        why.trim_end_matches('.')
                    )
                }
                _ => format!("{label} is not signed in"),
            },
            AuthState::PaidKey => format!("Plenipo could not confirm {label}'s paid key"),
            AuthState::ApiKey => {
                format!(
                    "{label} is signed in with its own API key, outside Plenipo's spending caps, \
                     so Plenipo does not use it"
                )
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

/// The closest layer that names an AI company never to use, other than `company` (a model's AI
/// tool's own, checked on its own): who sets it. A model whose maker is not known could have
/// been made by any of them.
pub fn never_other_than(layers: &[Layer<'_>], company: &str) -> Option<String> {
    layers
        .iter()
        .find(|l| l.never.iter().any(|c| c != company))
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

/// The same model on the same company's key, for a model whose subscription AI tool reached its
/// usage limit (Phase 25, item 4.4; ADR-204): the company's paid AI tool, its name for the model,
/// and the model's label there ("Claude Sonnet 5.5 (Anthropic)"), when the two are linked
/// (`KnownModel.same`; an alias through the exact model it points to). `None` without a link or
/// such a key.
fn key_twin<'a>(
    tools: &'a [ToolState],
    sub: &ToolState,
    name: Option<&str>,
) -> Option<(&'a ToolState, String, String)> {
    let name = name?;
    let known = &sub.info.capabilities.known_models;
    let entry = known.iter().find(|k| k.name == name)?;
    let entry = entry
        .points_to
        .as_ref()
        .and_then(|exact| known.iter().find(|k| &k.name == exact))
        .unwrap_or(entry);
    let link = entry.same.as_deref()?;
    let key = tools
        .iter()
        .find(|k| k.paid && k.info.id != sub.info.id && k.info.provider == sub.info.provider)?;
    let linked = |k: &&plenipo_runtime::agent::KnownModel| k.same.as_deref() == Some(link);
    let twin = key
        .info
        .reported_models
        .as_ref()
        .and_then(|r| r.models.iter().find(linked))
        .or_else(|| key.info.capabilities.known_models.iter().find(linked))?;
    let label = if twin
        .label
        .to_lowercase()
        .contains(&key.info.label.to_lowercase())
    {
        twin.label.clone()
    } else {
        format!("{} ({})", twin.label, key.info.label)
    };
    Some((key, twin.name.clone(), label))
}

/// Why the same company's key cannot take the model now, if it cannot: every check a paid route
/// the owner listed gets (ADR-085 §6), but "wait instead of moving to another AI company", since
/// it is the same company (ADR-204).
fn key_blocked(
    input: &RouteInput<'_>,
    layers: &[Layer<'_>],
    key: &ToolState,
    name: &str,
) -> Option<String> {
    let info = &key.info;
    if let Some(who) = never_by(layers, &info.provider) {
        return Some(format!("{who} never uses {}", info.provider_label));
    }
    if let Some((project, _)) = input
        .project
        .filter(|(_, allowed)| !allowed.contains(&info.id))
    {
        return Some(format!("{project} does not allow {}", info.label));
    }
    if let Some(limit) = &key.limit {
        return Some(format!("{} {}", info.label, limit_words(limit, input.now)));
    }
    let Some(price) = paid_price(info, Some(name)) else {
        return Some("it is not priced yet on your key".into());
    };
    if input.spending_room == Some(0) {
        return Some(
            "nothing is left this month under the spending caps covering this work".into(),
        );
    }
    if input.spending_room.is_some_and(|room| {
        price.is_some_and(|p| room < plenipo_runtime::agent::paid::smallest_step_cost(&p))
    }) {
        return Some(
            "too little is left this month under the spending caps covering this work".into(),
        );
    }
    None
}

pub fn route(input: &RouteInput<'_>) -> RouteDecision {
    let policy = input.policy;
    let layers = layers(input);
    let tool = |id: &str| input.tools.iter().find(|t| t.info.id == id);
    // Who made a model in the owner's list (`None`: not known).
    let maker =
        |m: &ModelInfo| tool(&m.runtime_id).and_then(|t| maker_of(&t.info, m.name.as_deref()));

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
        // A paid route is used only where the owner listed it (ADR-085 §8), never picked from
        // the whole list.
        all.into_iter()
            .filter(|m| tool(&m.runtime_id).is_none_or(|t| !t.paid))
            .map(|m| Candidate {
                id: &m.id,
                rank: None,
                model: Some(m),
            })
            .collect()
    };

    // 2. Cross-company review, by who made the models (ADR-081 §3).
    let mut reviewed: Vec<Maker> = Vec::new();
    let mut reviewed_unknown = false;
    for w in input.reviewed {
        let Some(t) = tool(&w.runtime_id) else {
            continue;
        };
        let made = if w.model_unread && t.info.capabilities.runs_other_makers {
            None
        } else {
            maker_of(&t.info, w.model.as_deref())
        };
        match made {
            Some(m) if !reviewed.iter().any(|r| r.id == m.id) => reviewed.push(m),
            Some(_) => {}
            None => reviewed_unknown = true,
        }
    }
    let cross = if reviewed.is_empty() && !reviewed_unknown {
        CrossCompany::Off
    } else {
        policy.cross_company
    };
    let mut names: Vec<String> = reviewed.iter().map(|m| m.label.clone()).collect();
    if reviewed_unknown {
        names.push("an AI company Plenipo doesn't know".into());
    }
    let reviewed_names = list(&names);
    // Could be the same company as the work it reviews. A model whose maker is not known, and
    // work by one, can never be told apart, so they count as the same (ADR-081 §7).
    let same_company = |m: &ModelInfo| match maker(m) {
        None => true,
        Some(made) => reviewed_unknown || reviewed.iter().any(|r| r.id == made.id),
    };
    if cross == CrossCompany::Prefer {
        // A paid route the owner listed after a subscription stays after it: preferring another
        // company never turns subscription use into paid use (ADR-036 §2.6, ADR-085 §8).
        let mut listed_subscription = false;
        let behind: std::collections::HashSet<&str> = candidates
            .iter()
            .filter_map(|c| {
                let paid = c
                    .model
                    .and_then(|m| tool(&m.runtime_id))
                    .is_some_and(|t| t.paid);
                if !paid {
                    listed_subscription = true;
                    None
                } else {
                    listed_subscription.then_some(c.id)
                }
            })
            .collect();
        // Then another company first, then the same one, and a model whose maker is not known
        // last (ADR-081 §7).
        candidates.sort_by_key(|c| {
            (
                behind.contains(c.id),
                match c.model {
                    Some(m) if maker(m).is_none() => 2,
                    Some(m) if same_company(m) => 1,
                    _ => 0,
                },
            )
        });
    }

    // 3–5. Checks.
    let mut notes: Vec<CandidateNote> = Vec::new();
    let mut chosen: Option<(RouteChoice, Option<u32>)> = None;
    let mut effort_from: Option<RuleSource> = None;
    let mut effort_passed: Option<(Effort, RuleSource)> = None;
    // The company whose usage limit work is waiting for (LimitBehavior::Wait).
    let mut waiting: Option<(&str, String)> = None;
    // The subscription whose usage limit moved the work to the same company's key, and its
    // limit in words (Phase 25, item 4.4; ADR-204).
    let mut on_key: Option<(String, String)> = None;
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
        let made_by = maker(m);
        // Skipped only because its subscription reached its usage limit.
        let mut limited_here = false;
        let skip = if let Some(who) = never_by(&layers, &info.provider) {
            Some(format!("{who} never uses {}", info.provider_label))
        } else if let Some((who, made)) = made_by
            .as_ref()
            .filter(|made| made.id != info.provider)
            .and_then(|made| never_by(&layers, &made.id).map(|who| (who, made)))
        {
            Some(format!("{who} never uses {}, which made it", made.label))
        } else if let Some(who) = made_by
            .is_none()
            .then(|| never_other_than(&layers, &info.provider))
            .flatten()
        {
            // It could have been made by a company on the list (ADR-081 §7: play it safe).
            Some(format!(
                "{who} has AI companies never to use, and who made this model is not known"
            ))
        } else if let Some((name, _)) = input
            .project
            .filter(|(_, allowed)| !allowed.contains(&info.id))
        {
            Some(format!("{name} does not allow {}", info.label))
        // What a model can do (`needs`) and its context size are kept in saved settings and no
        // longer rule a model out (Phase 25, items 2.3 and 2.4, amending ADR-011).
        } else if cross == CrossCompany::Require && same_company(m) {
            Some(if made_by.is_none() {
                format!(
                    "{} needs a different AI company than the work it reviews \
                     ({reviewed_names}), and who made this model is not known",
                    input.role
                )
            } else if reviewed_unknown {
                format!(
                    "{} needs a different AI company than the work it reviews \
                     ({reviewed_names}), and who made that work is not known",
                    input.role
                )
            } else {
                format!(
                    "{} needs a different AI company than the work it reviews ({reviewed_names})",
                    input.role
                )
            })
        } else if let Some(why) = not_ready(info) {
            Some(why)
        } else if let Some(limit) = &t.limit {
            if input.on_limit == LimitBehavior::Wait && waiting.is_none() {
                waiting = Some((info.provider.as_str(), info.label.clone()));
            }
            limited_here = true;
            Some(format!("{} {}", info.label, limit_words(limit, input.now)))
        } else if t.paid && paid_price(info, m.name.as_deref()).is_none() {
            Some(
                "it is not priced yet: Plenipo does not know what it costs, so it will not use a \
                 paid key for it"
                    .into(),
            )
        } else if t.paid && input.spending_room == Some(0) {
            Some(format!(
                "{} is paid per use, and nothing is left this month under the spending caps \
                 covering this work",
                info.label
            ))
        } else if t.paid
            && input.spending_room.is_some_and(|room| {
                paid_price(info, m.name.as_deref())
                    .flatten()
                    .is_some_and(|p| room < plenipo_runtime::agent::paid::smallest_step_cost(&p))
            })
        {
            Some(format!(
                "{} is paid per use, and too little is left this month under the spending caps \
                 covering this work for even its shortest task",
                info.label
            ))
        } else if let Some((_, tool_label)) = waiting.as_ref().filter(|(p, _)| *p != info.provider)
        {
            Some(format!(
                "work waits for {tool_label}'s usage limit to reset instead of moving to another AI \
                 company (Settings → AI models)"
            ))
        } else {
            None
        };
        // Your subscription first, then the same model on the same company's key (Phase 25, item
        // 4.4; ADR-204), only while its key can take it.
        let twin = (limited_here && !t.paid)
            .then(|| key_twin(input.tools, t, m.name.as_deref()))
            .flatten()
            .filter(|(key, _, _)| not_ready(&key.info).is_none());
        match (skip, twin) {
            (Some(why), Some((key, name, twin_label))) => {
                if let Some(blocked) = key_blocked(input, &layers, key, &name) {
                    note(
                        CandidateVerdict::Skipped,
                        format!(
                            "{why}, and your {} key can't take it: {blocked}",
                            key.info.label
                        ),
                    );
                    continue;
                }
                note(CandidateVerdict::Skipped, why);
                notes.push(CandidateNote {
                    model_id: m.id.clone(),
                    label: twin_label.clone(),
                    verdict: CandidateVerdict::Chosen,
                    note: String::new(),
                });
                let levels = key.info.capabilities.effort_levels_for(Some(&name));
                let (effort, from, passed) = effort_for(&layers, &m.id, m.effort, levels);
                chosen = Some((
                    RouteChoice {
                        model_id: m.id.clone(),
                        runtime_id: key.info.id.clone(),
                        runtime_label: key.info.label.clone(),
                        company: key.info.provider.clone(),
                        model: Some(name),
                        effort,
                        label: twin_label,
                        maker: made_by.clone(),
                        paid: true,
                    },
                    c.rank,
                ));
                effort_from = from;
                effort_passed = passed;
                on_key = t
                    .limit
                    .as_ref()
                    .map(|l| (info.label.clone(), limit_words(l, input.now)));
            }
            (Some(why), None) => note(CandidateVerdict::Skipped, why),
            (None, _) => {
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
                        maker: made_by.clone(),
                        paid: t.paid,
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
            let mut reason = match (rank, &on_key) {
                (Some(n), Some((sub, limited))) => format!(
                    "{} is {whose} {} choice, on your {} key: {sub} {limited}, so the same model \
                     runs on your key until then.",
                    choice.label,
                    ordinal(n),
                    choice.runtime_label
                ),
                (None, Some((sub, limited))) => format!(
                    "{} runs on your {} key: {sub} {limited}, so the same model runs on your key \
                     until then.",
                    choice.label, choice.runtime_label
                ),
                (Some(1), None) => {
                    format!("{} is {whose} first choice and is ready.", choice.label)
                }
                (Some(n), None) => format!(
                    "{} is {whose} {} choice: {}.",
                    choice.label,
                    ordinal(n),
                    first_skip.map_or_else(String::new, skipped)
                ),
                (None, None) => {
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
            // Whether it costs money, and what a worker on it can do (ADR-085).
            if let Some(t) = tool(&choice.runtime_id) {
                if choice.paid {
                    reason.push_str(
                        " It costs money: it is paid per use with your key, within your spending \
                         caps if you set any.",
                    );
                }
                if text_only(&t.info) {
                    reason.push_str(" A worker on it answers in text only.");
                }
            }
            if cross != CrossCompany::Off {
                let other = choice.maker.as_ref().is_some_and(|made| {
                    !reviewed_unknown && !reviewed.iter().any(|r| r.id == made.id)
                });
                if other {
                    reason.push_str(&format!(
                        " It comes from a different AI company than the work it reviews \
                         ({reviewed_names})."
                    ));
                } else if choice.maker.is_none() || reviewed_unknown {
                    reason.push_str(&format!(
                        " Plenipo cannot tell whether it comes from a different AI company than \
                         the work it reviews ({reviewed_names}), because who made {} is not \
                         known.",
                        if choice.maker.is_none() {
                            "it"
                        } else {
                            "that work"
                        }
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
                on_key_for: on_key.map(|(sub, _)| sub),
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
                on_key_for: None,
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
                    default_maker: None,
                    runs_other_makers: false,
                },
                install_hint: String::new(),
                login_hint: String::new(),
                ready: true,
                checked_at: None,
                checked_version: String::new(),
                account: Default::default(),
                reported_models: None,
                held: None,
            },
            limit: None,
            paid: false,
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
            maker: None,
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
        // The work under review, on each AI tool's default model.
        let reviewed: Vec<WorkDoneBy> = reviewed.iter().map(|r| WorkDoneBy::new(r, None)).collect();
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
            reviewed: &reviewed,
            on_limit,
            now: NOW,
            spending_room: None,
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

    /// What a role says a model must do, and the smallest context it takes, are kept in saved
    /// settings and no longer rule a model out (Phase 25, items 2.3 and 2.4): the first choice
    /// is chosen, marked or not.
    #[test]
    fn saved_needs_and_context_size_rule_nothing_out() {
        let mut w = world();
        let policy = RolePolicy {
            needs: vec![
                ModelFeature::Vision,
                ModelFeature::ImageGeneration,
                ModelFeature::ComputerUse,
            ],
            min_context_tokens: Some(200_000),
            ..prefer(&["opus", "gpt"])
        };
        w.models[1].context_tokens = Some(100_000);
        let d = decide(&w, &policy);
        assert_eq!(chosen(&d), Some("opus"), "{}", d.reason);
        assert!(d
            .candidates
            .iter()
            .all(|c| !c.note.contains("not marked as able") && !c.note.contains("context")));
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
            "Alpha Code is signed in with its own API key, outside Plenipo's spending caps, so \
             Plenipo does not use it"
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
        let mut m = model("x", "alpha", "Alpha Code: its own choice");
        assert_eq!(model_label(&m, &w.tools), "Alpha Code: its own choice");
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
            spending_room: None,
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

    // ---- Who made each model (ADR-081) -------------------------------------------------------

    /// Kimi (only Moonshot AI's models) and an Ollama-like AI tool that runs other companies'
    /// models: Kimi K3 (Moonshot AI), DeepSeek V4 Pro (DeepSeek), GLM-5.3 (Z.ai), gpt-oss
    /// (OpenAI, its default).
    fn makers_world() -> World {
        use plenipo_runtime::agent::makers;
        let mut kimi = tool("kimi", "moonshot");
        kimi.info.provider_label = "Moonshot AI".into();
        kimi.info.capabilities.known_models =
            vec![KnownModel::new("kimi-code/k3", "K3", &[]).by(makers::MOONSHOT)];
        let mut ollama = tool("ollama", "ollama");
        ollama.info.provider_label = "Ollama".into();
        ollama.info.capabilities.runs_other_makers = true;
        ollama.info.capabilities.default_maker =
            Some(Maker::new(makers::OPENAI.0, makers::OPENAI.1));
        ollama.info.capabilities.known_models = vec![
            KnownModel::new("gpt-oss", "gpt-oss", &[]).by(makers::OPENAI),
            KnownModel::new("kimi-k3:cloud", "Kimi K3", &[]).by(makers::MOONSHOT),
            KnownModel::new("deepseek-v4-pro:cloud", "DeepSeek V4 Pro", &[]).by(makers::DEEPSEEK),
            KnownModel::new("glm-5.3:cloud", "GLM-5.3", &[]).by(makers::ZAI),
        ];
        let m = |id: &str, runtime: &str, name: &str, label: &str| {
            let mut x = model(id, runtime, label);
            x.name = Some(name.into());
            x
        };
        World {
            models: vec![
                m("k3-on-ollama", "ollama", "kimi-k3:cloud", "Kimi K3"),
                m(
                    "deepseek",
                    "ollama",
                    "deepseek-v4-pro:cloud",
                    "DeepSeek V4 Pro",
                ),
                m("glm", "ollama", "glm-5.3:cloud", "GLM-5.3"),
                m("mystery", "ollama", "llama9:cloud", "Llama 9"),
                m("gpt-oss", "ollama", "gpt-oss", "gpt-oss"),
            ],
            tools: vec![kimi, ollama],
        }
    }

    fn review(w: &World, policy: &RolePolicy, reviewed: &[WorkDoneBy]) -> RouteDecision {
        route(&RouteInput {
            role: "Code Reviewer",
            role_id: "reviewer",
            policy,
            agent: None,
            department: None,
            organization: None,
            models: &w.models,
            tools: &w.tools,
            project: None,
            reviewed,
            on_limit: LimitBehavior::Wait,
            now: NOW,
            spending_room: None,
        })
    }

    fn requiring(ids: &[&str]) -> RolePolicy {
        RolePolicy {
            cross_company: CrossCompany::Require,
            ..prefer(ids)
        }
    }

    #[test]
    fn the_same_maker_on_two_ai_tools_counts_as_one_company() {
        let w = makers_world();
        // Work done by K3 on Kimi; Kimi K3 on Ollama is the same company (Moonshot AI).
        let reviewed = [WorkDoneBy::new("kimi", Some("kimi-code/k3"))];
        let d = review(&w, &requiring(&["k3-on-ollama", "deepseek"]), &reviewed);
        assert_eq!(chosen(&d), Some("deepseek"));
        assert!(d.candidates[0]
            .note
            .contains("needs a different AI company than the work it reviews (Moonshot AI)"));
        assert!(d.reason.ends_with(
            "It comes from a different AI company than the work it reviews (Moonshot AI)."
        ));
        assert_eq!(d.choice.unwrap().maker.unwrap().label, "DeepSeek");
        // Before ADR-081, both counted by their AI tool (Moonshot AI and Ollama), so Kimi K3 on
        // Ollama would have reviewed Moonshot AI's own work.
    }

    #[test]
    fn two_makers_inside_ollama_count_as_two() {
        let w = makers_world();
        // DeepSeek's work, reviewed by Z.ai's GLM: both on Ollama, two companies.
        let reviewed = [WorkDoneBy::new("ollama", Some("deepseek-v4-pro:cloud"))];
        let d = review(&w, &requiring(&["deepseek", "glm"]), &reviewed);
        assert_eq!(chosen(&d), Some("glm"));
        assert!(d.reason.contains("(DeepSeek)"));
        // Ollama's default (gpt-oss) is OpenAI's, so it may review DeepSeek's work too.
        let d = review(
            &w,
            &requiring(&["gpt-oss"]),
            &[WorkDoneBy::new("ollama", Some("deepseek-v4-pro:cloud"))],
        );
        assert_eq!(chosen(&d), Some("gpt-oss"));
    }

    #[test]
    fn a_model_whose_maker_is_not_known_plays_safe() {
        let w = makers_world();
        let deepseek_work = [WorkDoneBy::new("ollama", Some("deepseek-v4-pro:cloud"))];
        // Require: a model nobody is known to have made is never "another company".
        let d = review(&w, &requiring(&["mystery", "glm"]), &deepseek_work);
        assert_eq!(chosen(&d), Some("glm"));
        assert!(d.candidates[0]
            .note
            .contains("who made this model is not known"));
        // Work whose model Plenipo could not read: on Ollama, not known; on Kimi, still Moonshot
        // AI's (it runs only its own company's models).
        let d = review(&w, &requiring(&["glm"]), &[WorkDoneBy::unread("ollama")]);
        assert_eq!(chosen(&d), None);
        let d = review(&w, &requiring(&["glm"]), &[WorkDoneBy::unread("kimi")]);
        assert_eq!(chosen(&d), Some("glm"));
        assert!(d.reason.contains("(Moonshot AI)"), "{}", d.reason);
        // Work done by such a model: nothing can be told apart from it, so nobody reviews it,
        // and the reason says why.
        let mystery_work = [WorkDoneBy::new("ollama", Some("llama9:cloud"))];
        let d = review(&w, &requiring(&["glm", "deepseek"]), &mystery_work);
        assert_eq!(chosen(&d), None);
        assert_eq!(
            d.candidates[0].note,
            "Code Reviewer needs a different AI company than the work it reviews (an AI company \
             Plenipo doesn't know), and who made that work is not known"
        );
        // Known and unknown work together: the known company is still named.
        let both = [
            WorkDoneBy::new("ollama", Some("deepseek-v4-pro:cloud")),
            WorkDoneBy::new("ollama", Some("llama9:cloud")),
        ];
        let d = review(&w, &requiring(&["glm"]), &both);
        assert!(d.candidates[0]
            .note
            .contains("(DeepSeek and an AI company Plenipo doesn't know)"));
        // Prefer: known other companies first, then the same company, the unknown one last,
        // and the reason is honest.
        let preferring = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["mystery", "glm"])
        };
        let d = review(&w, &preferring, &deepseek_work);
        assert_eq!(chosen(&d), Some("glm"));
        let d = review(&w, &preferring, &mystery_work);
        assert_eq!(chosen(&d), Some("glm"));
        assert!(d
            .reason
            .contains("Plenipo cannot tell whether it comes from a different AI company"));
        // Moonshot AI's work: its own K3 on Ollama comes before a model nobody is known to have
        // made (ADR-081 §7: the unknown one last).
        let kimi_work = [WorkDoneBy::new("kimi", Some("kimi-code/k3"))];
        let preferring = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["mystery", "k3-on-ollama"])
        };
        let d = review(&w, &preferring, &kimi_work);
        assert_eq!(chosen(&d), Some("k3-on-ollama"));
        // The words on screen never say "maker".
        for d in [
            review(&w, &preferring, &kimi_work),
            review(&w, &requiring(&["mystery", "glm"]), &mystery_work),
            review(&w, &requiring(&["glm"]), &both),
        ] {
            let shown = std::iter::once(d.reason.clone())
                .chain(d.candidates.iter().map(|c| c.note.clone()))
                .collect::<Vec<_>>()
                .join(" ");
            assert!(!shown.to_lowercase().contains("maker"), "{shown}");
        }
    }

    /// GitHub Copilot as it ships (ADR-083): its default (Auto, which picks the model itself)
    /// says nobody made it, and the model Auto picked is not on its checked list, so its work
    /// plays safe in cross-company review (ADR-081 §7).
    #[test]
    fn copilots_work_counts_by_who_made_it_and_auto_plays_safe() {
        use plenipo_runtime::agent::{copilot::Copilot, RuntimeAdapter as _};
        let mut w = makers_world();
        let mut copilot = tool("copilot", "github");
        copilot.info.provider_label = "GitHub".into();
        copilot.info.capabilities = Copilot.capabilities();
        w.tools.push(copilot);
        let mut auto = model("copilot-auto", "copilot", "GitHub Copilot (its default)");
        auto.name = None;
        w.models.push(auto);
        // Reviewing Moonshot AI's work: Copilot's default could be anyone's, so a review that
        // must come from another company never picks it.
        let kimi_work = [WorkDoneBy::new("kimi", Some("kimi-code/k3"))];
        let d = review(&w, &requiring(&["copilot-auto", "glm"]), &kimi_work);
        assert_eq!(chosen(&d), Some("glm"));
        assert!(d.candidates[0]
            .note
            .contains("who made this model is not known"));
        // Work done on Copilot, by the model Auto picked or with no model read: never counted
        // as another company's, so nobody reviews it where a different company is required.
        for done in [
            WorkDoneBy::new("copilot", Some("mai-code-1.1-flash")),
            WorkDoneBy::unread("copilot"),
        ] {
            let d = review(&w, &requiring(&["glm"]), std::slice::from_ref(&done));
            assert_eq!(chosen(&d), None, "{done:?}");
        }
        // Preferring another company: Copilot's default comes last.
        let preferring = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["copilot-auto", "glm"])
        };
        assert_eq!(chosen(&review(&w, &preferring, &kimi_work)), Some("glm"));
        // A company never to use: its default could be that company's, so it is skipped; GitHub,
        // its own company, counts too.
        for never in ["anthropic", "github"] {
            let policy = RolePolicy {
                never_companies: vec![never.into()],
                ..prefer(&["copilot-auto", "glm"])
            };
            assert_eq!(chosen(&review(&w, &policy, &[])), Some("glm"), "{never}");
        }
    }

    #[test]
    fn a_company_never_to_use_is_skipped_as_the_maker_too() {
        let w = makers_world();
        let policy = RolePolicy {
            never_companies: vec!["openai".into()],
            ..prefer(&["gpt-oss", "glm"])
        };
        let d = review(&w, &policy, &[]);
        assert_eq!(chosen(&d), Some("glm"));
        assert_eq!(
            d.candidates[0].note,
            "Code Reviewer never uses OpenAI, which made it"
        );
        // The AI tool's company still counts on its own.
        let policy = RolePolicy {
            never_companies: vec!["ollama".into()],
            ..prefer(&["glm"])
        };
        assert_eq!(chosen(&review(&w, &policy, &[])), None);
        // A model whose maker is not known could be DeepSeek's: skipped too (play it safe).
        let policy = RolePolicy {
            never_companies: vec!["deepseek".into()],
            ..prefer(&["mystery", "glm"])
        };
        let d = review(&w, &policy, &[]);
        assert_eq!(chosen(&d), Some("glm"));
        assert_eq!(
            d.candidates[0].note,
            "Code Reviewer has AI companies never to use, and who made this model is not known"
        );
        // Without such a list it may be used.
        assert_eq!(
            chosen(&review(&w, &prefer(&["mystery"]), &[])),
            Some("mystery")
        );
    }

    // ---- Paid routes (Phase 16 Wave 3, ADR-085) ----

    /// Kimi K3 three ways (ADR-036 §4): the Kimi Code subscription, Ollama's plan, and an
    /// OpenRouter key, which is paid per use.
    fn three_ways() -> World {
        let mut paid = tool("openrouter", "openrouter");
        paid.paid = true;
        paid.info.label = "OpenRouter".into();
        paid.info.auth.state = AuthState::PaidKey;
        paid.info.capabilities.tool_posture =
            "Conversation only: an OpenRouter worker can answer, write, and review text.".into();
        let priced = |name: &str, label: &str| KnownModel {
            price: Some(plenipo_runtime::pricing::Price::per_million_dollars(3, 15)),
            ..KnownModel::new(name, label, &[])
        };
        paid.info.reported_models = Some(plenipo_runtime::agent::ReportedModels {
            models: vec![
                priced("moonshotai/kimi-k3", "Kimi K3"),
                KnownModel::new("someco/unpriced", "Unpriced", &[]),
            ],
            complete: true,
            checked_at: NOW,
        });
        let mut ollama = tool("ollama", "ollama");
        ollama.info.label = "Ollama".into();
        ollama.info.capabilities.tool_posture = "Conversation only: an Ollama worker …".into();
        World {
            models: vec![
                model("k3-kimi", "kimi", "Kimi K3"),
                model("k3-ollama", "ollama", "Kimi K3"),
                ModelInfo {
                    name: Some("moonshotai/kimi-k3".into()),
                    ..model("k3-openrouter", "openrouter", "Kimi K3")
                },
                ModelInfo {
                    name: Some("someco/unpriced".into()),
                    ..model("unpriced", "openrouter", "Unpriced")
                },
            ],
            tools: vec![tool("kimi", "moonshot"), ollama, paid],
        }
    }

    fn with_room(w: &World, policy: &RolePolicy, room: Option<u64>) -> RouteDecision {
        route(&RouteInput {
            role: "Senior Developer",
            role_id: "dev",
            policy,
            agent: None,
            department: None,
            organization: None,
            models: &w.models,
            tools: &w.tools,
            project: None,
            reviewed: &[],
            on_limit: LimitBehavior::NextChoice,
            now: NOW,
            spending_room: room,
        })
    }

    /// Claude Code (a subscription) and the Anthropic key, with Sonnet linked across both
    /// (Phase 25, item 4.4).
    fn subscription_and_key() -> World {
        let mut claude = tool("claude-code", "anthropic");
        claude.info.label = "Claude Code".into();
        claude.info.capabilities.known_models = vec![
            KnownModel::new("sonnet", "Sonnet", &[]).now("claude-sonnet-5-5"),
            KnownModel::new("claude-sonnet-5-5", "Sonnet 5.5", &[]).same("claude-sonnet-5-5"),
            KnownModel::new("unlinked", "Unlinked", &[]),
        ];
        let mut key = tool("anthropic-key", "anthropic");
        key.paid = true;
        key.info.label = "Anthropic".into();
        key.info.auth.state = AuthState::PaidKey;
        key.info.reported_models = Some(plenipo_runtime::agent::ReportedModels {
            models: vec![KnownModel {
                price: Some(plenipo_runtime::pricing::Price::per_million_dollars(2, 10)),
                ..KnownModel::new("claude-sonnet-5-5", "Claude Sonnet 5.5", &[])
                    .same("claude-sonnet-5-5")
            }],
            complete: true,
            checked_at: NOW,
        });
        let mut other = tool("codex", "openai");
        other.info.label = "Codex".into();
        World {
            models: vec![
                ModelInfo {
                    name: Some("sonnet".into()),
                    ..model("sonnet-cc", "claude-code", "Sonnet")
                },
                model("unlinked", "claude-code", "Unlinked"),
                model("gpt", "codex", "GPT"),
            ],
            tools: vec![claude, key, other],
        }
    }

    fn waiting_for_limits(w: &World, policy: &RolePolicy, room: Option<u64>) -> RouteDecision {
        route(&RouteInput {
            role: "Senior Developer",
            role_id: "dev",
            policy,
            agent: None,
            department: None,
            organization: None,
            models: &w.models,
            tools: &w.tools,
            project: None,
            reviewed: &[],
            on_limit: LimitBehavior::Wait,
            now: NOW,
            spending_room: room,
        })
    }

    /// Phase 25, item 4.4 (ADR-204): your subscription first, then the same model on the same
    /// company's key, only while the key can take it, and never silently.
    #[test]
    fn a_subscription_limit_moves_the_same_model_to_the_same_companys_key() {
        let mut w = subscription_and_key();
        let policy = prefer(&["sonnet-cc", "gpt"]);
        // Not limited: the subscription, as always.
        let d = waiting_for_limits(&w, &policy, None);
        let c = d.choice.as_ref().unwrap();
        assert_eq!((c.runtime_id.as_str(), c.paid), ("claude-code", false));
        assert_eq!(d.on_key_for, None);

        // Claude Code reaches its limit: Sonnet runs on the Anthropic key (even though the role
        // waits rather than moving to another AI company), and the reason says so.
        w.tools[0].limit = limited();
        let d = waiting_for_limits(&w, &policy, None);
        let c = d.choice.as_ref().unwrap();
        assert_eq!(c.runtime_id, "anthropic-key");
        assert_eq!(c.model.as_deref(), Some("claude-sonnet-5-5"));
        assert_eq!(c.model_id, "sonnet-cc", "still the owner's chosen model");
        assert!(c.paid);
        assert_eq!(d.rank, Some(1));
        assert_eq!(d.on_key_for.as_deref(), Some("Claude Code"));
        assert!(
            d.reason.starts_with(
                "Claude Sonnet 5.5 (Anthropic) is Senior Developer's first choice, on your \
                 Anthropic key: Claude Code reached its usage limit (resets in about an hour), so \
                 the same model runs on your key until then."
            ),
            "{}",
            d.reason
        );
        assert!(d.reason.contains("It costs money"), "{}", d.reason);
        assert_eq!(
            verdicts(&d),
            [
                CandidateVerdict::Skipped,
                CandidateVerdict::Chosen,
                CandidateVerdict::NotNeeded
            ]
        );

        // The spending caps have no room: it waits, and says why the key could not take it.
        let d = waiting_for_limits(&w, &policy, Some(0));
        assert!(d.choice.is_none(), "{d:?}");
        assert!(
            d.candidates[0].note.ends_with(
                "and your Anthropic key can't take it: nothing is left this month \
                            under the spending caps covering this work"
            ),
            "{}",
            d.candidates[0].note
        );

        // Paid keys switched off (or no key): the key is not ready, and nothing moves.
        let mut off = w.tools.clone();
        off[1].info.ready = false;
        off[1].info.auth = AuthStatus {
            state: AuthState::SignedOut,
            method: None,
            detail: Some("Paid AI keys are switched off".into()),
        };
        let switched_off = World {
            models: w.models.clone(),
            tools: off,
        };
        let d = waiting_for_limits(&switched_off, &policy, None);
        assert!(d.choice.is_none());
        assert!(d.reason.contains("waits for it"), "{}", d.reason);

        // A model with no link to the key, and "its own choice", never move.
        let d = waiting_for_limits(&w, &prefer(&["unlinked"]), None);
        assert!(d.choice.is_none(), "{d:?}");
        let own = World {
            models: vec![ModelInfo {
                name: None,
                ..model("own", "claude-code", "Claude Code: its own choice")
            }],
            tools: w.tools.clone(),
        };
        assert!(waiting_for_limits(&own, &prefer(&["own"]), None)
            .choice
            .is_none());
    }

    fn limited() -> Option<UsageLimit> {
        Some(UsageLimit {
            model: None,
            since: NOW - 1000,
            resets_at: Some(NOW + 3_600_000),
            until: NOW + 3_600_000,
            detail: "usage limit reached".into(),
        })
    }

    #[test]
    fn the_owners_example_kimi_runs_out_and_the_ollama_plan_carries_the_work() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        let d = with_room(
            &w,
            &prefer(&["k3-kimi", "k3-ollama", "k3-openrouter"]),
            None,
        );
        assert_eq!(chosen(&d), Some("k3-ollama"), "{}", d.reason);
        assert!(!d.choice.as_ref().unwrap().paid);
        assert!(d.reason.contains("second choice"), "{}", d.reason);
        assert!(d.reason.contains("usage limit"), "{}", d.reason);
        assert!(!d.reason.contains("costs money"), "{}", d.reason);
        assert!(d.reason.contains("answers in text only"), "{}", d.reason);
    }

    #[test]
    fn a_paid_route_runs_next_and_the_reason_says_it_costs_money() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        let d = with_room(
            &w,
            &prefer(&["k3-kimi", "k3-ollama", "k3-openrouter"]),
            Some(5_000_000),
        );
        assert_eq!(chosen(&d), Some("k3-openrouter"), "{}", d.reason);
        assert!(d.choice.as_ref().unwrap().paid);
        assert!(d.reason.contains("third choice"), "{}", d.reason);
        assert!(
            d.reason
                .contains("It costs money: it is paid per use with your key"),
            "{}",
            d.reason
        );
        assert!(d.reason.contains("Kimi K3 (OpenRouter)"), "{}", d.reason);
    }

    #[test]
    fn a_paid_route_over_its_cap_or_not_priced_is_skipped() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        let d = with_room(&w, &prefer(&["k3-openrouter", "k3-kimi"]), Some(0));
        assert_eq!(chosen(&d), None, "{}", d.reason);
        let note = &d.candidates[0];
        assert_eq!(note.verdict, CandidateVerdict::Skipped);
        assert!(
            note.note.contains("nothing is left this month"),
            "{}",
            note.note
        );
        let d = with_room(&w, &prefer(&["unpriced"]), Some(5_000_000));
        assert_eq!(chosen(&d), None, "{}", d.reason);
        assert!(
            d.candidates[0].note.contains("not priced yet"),
            "{}",
            d.candidates[0].note
        );
    }

    #[test]
    fn a_paid_route_is_used_only_where_the_owner_listed_it() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        // No list: the whole registry, but never a paid route, however it sorts.
        let d = with_room(&w, &RolePolicy::default(), Some(5_000_000));
        assert_eq!(chosen(&d), None, "{}", d.reason);
        assert!(d
            .candidates
            .iter()
            .all(|c| !c.model_id.contains("openrouter") && c.model_id != "unpriced"));
    }

    #[test]
    fn a_paid_route_without_its_key_is_skipped_and_the_reason_says_why() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        let paid = &mut w.tools[2].info;
        paid.ready = false;
        paid.auth.state = AuthState::SignedOut;
        paid.auth.detail = Some(
            "Paid AI keys are switched off: turn on Settings → Switches → Let workers use paid \
             AI keys."
                .into(),
        );
        let d = with_room(&w, &prefer(&["k3-openrouter"]), Some(5_000_000));
        assert_eq!(chosen(&d), None, "{}", d.reason);
        let note = &d.candidates[0].note;
        assert!(note.contains("OpenRouter cannot take work now"), "{note}");
        assert!(note.contains("Paid AI keys are switched off"), "{note}");
    }

    #[test]
    fn a_paid_route_with_less_left_than_its_shortest_task_is_skipped() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        // A tenth of a cent left; Kimi K3's shortest task sets aside about $0.24.
        let d = with_room(&w, &prefer(&["k3-openrouter"]), Some(1_000));
        assert_eq!(chosen(&d), None, "{}", d.reason);
        let note = &d.candidates[0].note;
        assert!(note.contains("too little is left this month"), "{note}");
        // With enough left, it runs.
        let d = with_room(&w, &prefer(&["k3-openrouter"]), Some(5_000_000));
        assert_eq!(chosen(&d), Some("k3-openrouter"), "{}", d.reason);
    }

    #[test]
    fn a_signed_out_paid_tool_is_never_among_the_whole_lists_choices() {
        let mut w = three_ways();
        w.tools[0].limit = limited();
        w.tools[1].limit = limited();
        let paid = &mut w.tools[2].info;
        paid.ready = false;
        paid.auth.state = AuthState::SignedOut;
        paid.auth.detail = Some("Paid AI keys are switched off.".into());
        let d = with_room(&w, &RolePolicy::default(), None);
        assert!(
            d.candidates
                .iter()
                .all(|c| !c.model_id.contains("openrouter") && c.model_id != "unpriced"),
            "{:?}",
            d.candidates
        );
        assert!(!d.reason.contains("switched off"), "{}", d.reason);
    }

    #[test]
    fn preferring_another_company_never_puts_a_listed_paid_route_before_a_subscription() {
        let mut w = three_ways();
        w.models.push(ModelInfo {
            name: Some("qwen/qwen3.8-flash".into()),
            maker: Some(Maker::new("alibaba", "Alibaba (Qwen)")),
            ..model("qwen-openrouter", "openrouter", "Qwen3.8 Flash")
        });
        w.tools[2]
            .info
            .reported_models
            .as_mut()
            .unwrap()
            .models
            .push(KnownModel {
                price: Some(plenipo_runtime::pricing::Price::per_million_dollars(1, 2)),
                ..KnownModel::new("qwen/qwen3.8-flash", "Qwen3.8 Flash", &[])
            });
        let kimi_work = [WorkDoneBy::new("kimi", Some("kimi-code/k3"))];
        // The owner listed Kimi's subscription first: reviewing Kimi's own work, another
        // company is preferred, but not by turning to a paid route listed after it.
        let preferring = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["k3-kimi", "qwen-openrouter"])
        };
        let d = review(&w, &preferring, &kimi_work);
        assert_eq!(chosen(&d), Some("k3-kimi"), "{}", d.reason);
        // Listed first by the owner, the paid route goes first.
        let preferring = RolePolicy {
            cross_company: CrossCompany::Prefer,
            ..prefer(&["qwen-openrouter", "k3-kimi"])
        };
        let d = review(&w, &preferring, &kimi_work);
        assert_eq!(chosen(&d), Some("qwen-openrouter"), "{}", d.reason);
    }
}
