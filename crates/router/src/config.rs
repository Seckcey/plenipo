//! The routing configuration: the owner's model registry, role policies, and options, kept in
//! the Ledger's `routing` setting as one document (ADR-011). Every change is validated here
//! before it is written.

use std::collections::{BTreeMap, HashSet};

use plenipo_ledger::workforce::{clean_line, clean_model, clean_runtime_id};
use plenipo_runtime::agent::{Effort, KnownModel};
use serde::{Deserialize, Serialize};

use crate::dto::*;
use crate::error::{Result, RouterError};

/// Most models in the registry, and in one role's list.
pub const MAX_MODELS: usize = 64;
pub const MAX_ROLE_MODELS: usize = 12;
/// Largest context size accepted, in tokens.
pub const MAX_CONTEXT_TOKENS: u32 = 100_000_000;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RoutingConfig {
    pub models: Vec<ModelInfo>,
    /// By role ID.
    pub policies: BTreeMap<String, RolePolicy>,
    /// The whole organization's rule (ADR-041).
    pub organization: ModelRule,
    /// By department ID.
    pub departments: BTreeMap<String, ModelRule>,
    /// Each agent's own rule, by position ID.
    pub positions: BTreeMap<String, ModelRule>,
    pub options: RoutingOptions,
    /// When the owner last asked to try each runtime again after a usage limit (ms).
    pub cleared_limits: BTreeMap<String, u64>,
    /// The owner's weekly budget of tokens for each AI tool that reports nothing of its plan,
    /// by runtime ID (Phase 25, item 4.6).
    pub budgets: BTreeMap<String, u64>,
}

/// What one AI tool takes: its name, its effort levels, and its own models' levels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolEfforts {
    pub label: String,
    pub levels: Vec<Effort>,
    pub models: Vec<KnownModel>,
}

impl ToolEfforts {
    /// The levels `model` takes: its own when the AI tool lists it, else the AI tool's.
    pub fn for_model(&self, model: Option<&str>) -> &[Effort] {
        model
            .and_then(|m| self.models.iter().find(|k| k.name == m))
            .map_or(&self.levels, |k| &k.effort_levels)
    }
}

/// The AI tools this build has (runtime IDs), with what each takes.
pub type ToolLevels = BTreeMap<String, ToolEfforts>;

fn invalid(message: impl Into<String>) -> RouterError {
    RouterError::Invalid(message.into())
}

/// "low, medium, high, extra high, or max".
pub fn levels_words(levels: &[Effort]) -> String {
    let words: Vec<&str> = levels.iter().map(|e| e.label()).collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [a, b] => format!("{a} or {b}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

/// "Sonnet (Claude Code)": a model's name with its AI tool's.
fn model_words(label: &str, runtime_id: &str, tools: &ToolLevels) -> String {
    let tool = tools
        .get(runtime_id)
        .map_or(runtime_id, |t| t.label.as_str());
    if tool.is_empty() || label.to_lowercase().contains(&tool.to_lowercase()) {
        label.to_owned()
    } else {
        format!("{label} ({tool})")
    }
}

/// Refuse an effort level the model does not take, in plain words naming the levels it does
/// (ADR-041 §8).
pub fn check_model_effort(
    effort: Effort,
    runtime_id: &str,
    name: Option<&str>,
    label: &str,
    tools: &ToolLevels,
) -> Result<()> {
    let levels = tools.get(runtime_id).map_or(&[][..], |t| t.for_model(name));
    if levels.contains(&effort) {
        return Ok(());
    }
    let who = model_words(label, runtime_id, tools);
    Err(invalid(if levels.is_empty() {
        format!("{who} has no effort setting")
    } else {
        format!(
            "{who} does not take {} effort. It takes {}",
            effort.label(),
            levels_words(levels)
        )
    }))
}

/// Refuse an effort for any model that no AI tool takes.
fn check_any_effort(effort: Effort, tools: &ToolLevels) -> Result<()> {
    let taken = tools.values().any(|t| {
        t.levels.contains(&effort) || t.models.iter().any(|m| m.effort_levels.contains(&effort))
    });
    if taken {
        Ok(())
    } else {
        Err(invalid(format!(
            "none of your AI tools takes {} effort",
            effort.label()
        )))
    }
}

impl RoutingConfig {
    /// Read the stored document (`Null`: nothing stored yet).
    pub fn from_value(value: serde_json::Value) -> std::result::Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    pub fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }

    /// The role's policy (the default one when it has none).
    pub fn policy(&self, role_id: &str) -> RolePolicy {
        self.policies.get(role_id).cloned().unwrap_or_default()
    }

    /// Rename built-in entries still called by the old name ("Claude Code (default model)") to
    /// the new one ("Claude Code: its own choice", Phase 25, item 2.5). A name the owner gave is
    /// kept. Returns the new names.
    pub fn rename_old_builtins(&mut self, tools: &[(String, String)]) -> Vec<String> {
        let mut renamed = Vec::new();
        for (runtime_id, label) in tools {
            let old = format!("{label} (default model)");
            let Some(id) = self
                .models
                .iter()
                .find(|m| m.built_in && &m.runtime_id == runtime_id && m.label == old)
                .map(|m| m.id.clone())
            else {
                continue;
            };
            let new = unique_label(self, &own_choice_label(label), Some(&id));
            if let Some(m) = self.models.iter_mut().find(|m| m.id == id) {
                m.label.clone_from(&new);
                renamed.push(new);
            }
        }
        renamed
    }

    /// Add built-in entries — each AI tool's default model — that are missing. Returns them.
    pub fn add_builtins(&mut self, tools: &[(String, String)]) -> Vec<ModelInfo> {
        let mut added = Vec::new();
        for (runtime_id, label) in tools {
            let exists = self
                .models
                .iter()
                .any(|m| m.built_in && &m.runtime_id == runtime_id);
            if !exists {
                let m = ModelInfo {
                    id: uuid::Uuid::new_v4().to_string(),
                    runtime_id: runtime_id.clone(),
                    name: None,
                    label: unique_label(self, &own_choice_label(label), None),
                    features: Vec::new(),
                    context_tokens: None,
                    cost: CostClass::Standard,
                    effort: None,
                    built_in: true,
                    maker: None,
                };
                self.models.push(m.clone());
                added.push(m);
            }
        }
        added
    }

    /// Add or change a model. Returns the model.
    pub fn save_model(&mut self, input: &ModelInput, tools: &ToolLevels) -> Result<ModelInfo> {
        let runtime_id = clean_runtime_id(&input.runtime_id)?;
        if !tools.contains_key(&runtime_id) {
            return Err(invalid(format!("there is no AI tool named {runtime_id:?}")));
        }
        let name = input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(clean_model)
            .transpose()?;
        let label = clean_line("the model's name", &input.label, 80)?;
        if let Some(e) = input.effort {
            check_model_effort(e, &runtime_id, name.as_deref(), &label, tools)?;
        }
        let mut features = input.features.clone();
        features.sort();
        features.dedup();
        if let Some(c) = input.context_tokens {
            if !(1..=MAX_CONTEXT_TOKENS).contains(&c) {
                return Err(invalid(format!(
                    "the context size must be 1–{MAX_CONTEXT_TOKENS} tokens"
                )));
            }
        }
        let existing = match &input.id {
            Some(id) => Some(
                self.models
                    .iter()
                    .position(|m| &m.id == id)
                    .ok_or_else(|| invalid("that model is no longer in your list"))?,
            ),
            None => None,
        };
        if let Some(i) = existing {
            let m = &self.models[i];
            if m.built_in && (m.runtime_id != runtime_id || name.is_some()) {
                return Err(invalid(
                    "a built-in entry always stands for its AI tool's default model; add a new \
                     model instead",
                ));
            }
        }
        let others = || {
            self.models
                .iter()
                .enumerate()
                .filter(move |(i, _)| Some(*i) != existing)
                .map(|(_, m)| m)
        };
        if others().any(|m| m.runtime_id == runtime_id && m.name == name) {
            return Err(invalid(match &name {
                Some(n) => format!("{n} on that AI tool is already in your list"),
                None => "that AI tool's default model is already in your list".into(),
            }));
        }
        if others().any(|m| m.label.to_lowercase() == label.to_lowercase()) {
            return Err(invalid(format!(
                "a model named \"{label}\" is already in your list"
            )));
        }
        let model = ModelInfo {
            id: existing.map_or_else(
                || uuid::Uuid::new_v4().to_string(),
                |i| self.models[i].id.clone(),
            ),
            runtime_id,
            name,
            label,
            features,
            context_tokens: input.context_tokens,
            cost: input.cost,
            effort: input.effort,
            built_in: existing.is_some_and(|i| self.models[i].built_in),
            maker: None,
        };
        // A role's or a rule's effort for this model must still suit it (its AI tool may be new).
        let levels: Vec<Effort> = tools
            .get(&model.runtime_id)
            .map(|t| t.for_model(model.name.as_deref()).to_vec())
            .unwrap_or_default();
        let keep = |id: &String, e: &mut Effort| *id != model.id || levels.contains(e);
        for policy in self.policies.values_mut() {
            policy.efforts.retain(keep);
        }
        for rule in self.rules_mut() {
            rule.efforts.retain(keep);
        }
        self.drop_empty_rules();
        match existing {
            Some(i) => self.models[i] = model.clone(),
            None => {
                if self.models.len() >= MAX_MODELS {
                    return Err(invalid(format!("at most {MAX_MODELS} models")));
                }
                self.models.push(model.clone());
            }
        }
        Ok(model)
    }

    /// Remove a model the owner added, and take it out of every role's list. Returns the
    /// removed model and the roles whose lists changed.
    pub fn remove_model(&mut self, id: &str) -> Result<(ModelInfo, Vec<String>)> {
        let i = self
            .models
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| invalid("that model is no longer in your list"))?;
        if self.models[i].built_in {
            return Err(invalid(
                "the built-in entry for an AI tool's default model cannot be removed; leave it \
                 out of your roles' choices instead",
            ));
        }
        let removed = self.models.remove(i);
        let mut changed = Vec::new();
        for (role, policy) in &mut self.policies {
            let before = policy.models.len();
            policy.models.retain(|m| m != id);
            policy.efforts.remove(id);
            if policy.models.len() != before {
                changed.push(role.clone());
            }
        }
        for rule in self.rules_mut() {
            rule.models.retain(|m| m != id);
            rule.efforts.remove(id);
        }
        self.drop_empty_rules();
        Ok((removed, changed))
    }

    /// A department's or agent's rule left with nothing in it (its only model left the list, or
    /// no longer takes its effort) is no rule: forget it, as saving an empty rule does.
    fn drop_empty_rules(&mut self) {
        self.departments.retain(|_, r| !r.is_empty());
        self.positions.retain(|_, r| !r.is_empty());
    }

    /// Every rule that is not a role's: the organization's, each department's, each agent's.
    fn rules_mut(&mut self) -> impl Iterator<Item = &mut ModelRule> {
        std::iter::once(&mut self.organization)
            .chain(self.departments.values_mut())
            .chain(self.positions.values_mut())
    }

    /// A validated rule for the organization, a department, or an agent (ADR-041).
    pub fn check_rule(
        &self,
        rule: &ModelRule,
        companies: &[String],
        tools: &ToolLevels,
    ) -> Result<ModelRule> {
        let models = self.check_models(&rule.models)?;
        let efforts = self.check_efforts(&rule.efforts, tools)?;
        if let Some(e) = rule.effort {
            check_any_effort(e, tools)?;
        }
        Ok(ModelRule {
            models,
            efforts,
            effort: rule.effort,
            never_companies: check_companies(&rule.never_companies, companies)?,
        })
    }

    /// A model list: every model in the owner's list, once, at most [`MAX_ROLE_MODELS`].
    fn check_models(&self, models: &[String]) -> Result<Vec<String>> {
        if models.len() > MAX_ROLE_MODELS {
            return Err(invalid(format!(
                "a list has at most {MAX_ROLE_MODELS} models"
            )));
        }
        let mut seen = HashSet::new();
        for id in models {
            if self.model(id).is_none() {
                return Err(invalid("a chosen model is no longer in your list"));
            }
            if !seen.insert(id) {
                return Err(invalid("a model is listed twice"));
            }
        }
        Ok(models.to_vec())
    }

    /// Efforts by model: each model is in the owner's list and takes its level.
    fn check_efforts(
        &self,
        efforts: &BTreeMap<String, Effort>,
        tools: &ToolLevels,
    ) -> Result<BTreeMap<String, Effort>> {
        for (id, effort) in efforts {
            let model = self
                .model(id)
                .ok_or_else(|| invalid("a chosen model is no longer in your list"))?;
            check_model_effort(
                *effort,
                &model.runtime_id,
                model.name.as_deref(),
                &model.label,
                tools,
            )?;
        }
        Ok(efforts.clone())
    }

    /// A validated policy for a role; `companies` are the provider IDs this build knows.
    pub fn check_policy(
        &self,
        policy: &RolePolicy,
        companies: &[String],
        tools: &ToolLevels,
    ) -> Result<RolePolicy> {
        let models = self.check_models(&policy.models)?;
        let mut needs = policy.needs.clone();
        needs.sort();
        needs.dedup();
        if let Some(c) = policy.min_context_tokens {
            if !(1..=MAX_CONTEXT_TOKENS).contains(&c) {
                return Err(invalid(format!(
                    "the context size must be 1–{MAX_CONTEXT_TOKENS} tokens"
                )));
            }
        }
        // An effort equal to the model's own setting is kept: with rules above the role
        // (ADR-041), it says "this level for this role", not nothing.
        let efforts = self.check_efforts(&policy.efforts, tools)?;
        if let Some(e) = policy.effort {
            check_any_effort(e, tools)?;
        }
        Ok(RolePolicy {
            models,
            needs,
            min_context_tokens: policy.min_context_tokens,
            never_companies: check_companies(&policy.never_companies, companies)?,
            cost: policy.cost,
            cross_company: policy.cross_company,
            efforts,
            effort: policy.effort,
        })
    }
}

/// AI companies never to use: each one this build knows, once.
fn check_companies(list: &[String], companies: &[String]) -> Result<Vec<String>> {
    let mut never = Vec::new();
    for c in list {
        if !companies.contains(c) {
            return Err(invalid(format!("there is no AI company named {c:?}")));
        }
        if !never.contains(c) {
            never.push(c.clone());
        }
    }
    Ok(never)
}

/// `label`, or `label 2`, `label 3`, … when taken by another model.
/// How an AI tool's own-choice entry in Your models is named (Phase 25, item 2.5): the AI tool
/// picks the model itself. "Claude Code: its own choice".
pub fn own_choice_label(tool: &str) -> String {
    format!("{tool}: its own choice")
}

fn unique_label(config: &RoutingConfig, label: &str, except: Option<&str>) -> String {
    let taken = |l: &str| {
        config
            .models
            .iter()
            .any(|m| Some(m.id.as_str()) != except && m.label.eq_ignore_ascii_case(l))
    };
    if !taken(label) {
        return label.to_owned();
    }
    (2..)
        .map(|n| format!("{label} {n}"))
        .find(|l| !taken(l))
        .unwrap_or_else(|| label.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> ToolLevels {
        let tool = |label: &str, levels: &[Effort]| ToolEfforts {
            label: label.into(),
            levels: levels.to_vec(),
            models: vec![KnownModel::new("mini", "Mini", &[])],
        };
        ToolLevels::from([
            (
                "alpha".into(),
                tool("Alpha Code", &[Effort::Low, Effort::High]),
            ),
            ("beta".into(), tool("Beta CLI", &[])),
        ])
    }

    fn input(runtime: &str, name: Option<&str>, label: &str) -> ModelInput {
        ModelInput {
            id: None,
            runtime_id: runtime.into(),
            name: name.map(str::to_owned),
            label: label.into(),
            features: vec![ModelFeature::Vision, ModelFeature::Vision],
            context_tokens: Some(200_000),
            cost: CostClass::Premium,
            effort: None,
        }
    }

    /// An install from before Phase 25 called them "Alpha Code (default model)": renamed once,
    /// and a name the owner gave is kept (Phase 25, item 2.5).
    #[test]
    fn old_builtin_names_become_its_own_choice() {
        let mut c = RoutingConfig::default();
        let builtins = [
            ("alpha".to_owned(), "Alpha Code".to_owned()),
            ("beta".to_owned(), "Beta CLI".to_owned()),
        ];
        c.add_builtins(&builtins);
        c.models[0].label = "Alpha Code (default model)".into();
        c.models[1].label = "My Beta".into();
        assert_eq!(
            c.rename_old_builtins(&builtins),
            ["Alpha Code: its own choice"]
        );
        assert_eq!(c.models[0].label, "Alpha Code: its own choice");
        assert_eq!(c.models[1].label, "My Beta");
        assert!(c.rename_old_builtins(&builtins).is_empty());
    }

    #[test]
    fn builtins_are_added_once_and_stay() {
        let mut c = RoutingConfig::default();
        let builtins = [("alpha".to_owned(), "Alpha Code".to_owned())];
        let added = c.add_builtins(&builtins);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].label, "Alpha Code: its own choice");
        assert!(added[0].built_in && added[0].name.is_none());
        assert!(c.add_builtins(&builtins).is_empty());
        // It can be relabelled and described, not repointed or removed.
        let id = added[0].id.clone();
        let edit = ModelInput {
            id: Some(id.clone()),
            name: None,
            ..input("alpha", None, "Alpha default")
        };
        let saved = c.save_model(&edit, &tools()).unwrap();
        assert_eq!(
            (saved.label.as_str(), saved.built_in),
            ("Alpha default", true)
        );
        assert_eq!(saved.features, [ModelFeature::Vision]);
        let repoint = ModelInput {
            name: Some("opus".into()),
            ..edit.clone()
        };
        assert!(c.save_model(&repoint, &tools()).is_err());
        assert!(c.remove_model(&id).is_err());
    }

    #[test]
    fn models_are_validated() {
        let mut c = RoutingConfig::default();
        let opus = c
            .save_model(&input("alpha", Some("opus"), "Opus"), &tools())
            .unwrap();
        assert_eq!(opus.name.as_deref(), Some("opus"));
        for bad in [
            input("gamma", Some("x"), "X"),
            input("alpha", Some("--danger"), "Danger"),
            input("alpha", Some("../x"), "Path"),
            input("alpha", Some("opus"), "Opus again"),
            input("alpha", Some("sonnet"), "opus"),
            input("alpha", Some("sonnet"), ""),
            ModelInput {
                context_tokens: Some(0),
                ..input("alpha", Some("sonnet"), "Sonnet")
            },
            ModelInput {
                effort: Some(Effort::Max),
                ..input("alpha", Some("sonnet"), "Sonnet")
            },
            ModelInput {
                effort: Some(Effort::Low),
                ..input("beta", Some("gpt"), "GPT")
            },
        ] {
            assert!(c.save_model(&bad, &tools()).is_err(), "{bad:?}");
        }
        // Blank name: the tool's default.
        let d = c
            .save_model(&input("beta", Some("  "), "Beta"), &tools())
            .unwrap();
        assert_eq!(d.name, None);
        // Editing keeps the ID; unknown IDs are refused.
        let edited = c
            .save_model(
                &ModelInput {
                    id: Some(opus.id.clone()),
                    ..input("alpha", Some("opus-5"), "Opus 5")
                },
                &tools(),
            )
            .unwrap();
        assert_eq!(edited.id, opus.id);
        assert_eq!(c.models.len(), 2);
        let ghost = ModelInput {
            id: Some("nope".into()),
            ..input("alpha", Some("x"), "X")
        };
        assert!(c.save_model(&ghost, &tools()).is_err());
        // An effort level the AI tool accepts is kept with the model.
        let high = c
            .save_model(
                &ModelInput {
                    effort: Some(Effort::High),
                    ..input("alpha", Some("haiku"), "Haiku")
                },
                &tools(),
            )
            .unwrap();
        assert_eq!(high.effort, Some(Effort::High));
    }

    #[test]
    fn role_efforts_are_validated_and_follow_their_model() {
        let mut c = RoutingConfig::default();
        let a = c
            .save_model(
                &ModelInput {
                    effort: Some(Effort::High),
                    ..input("alpha", Some("a"), "A")
                },
                &tools(),
            )
            .unwrap();
        let b = c
            .save_model(&input("alpha", Some("b"), "B"), &tools())
            .unwrap();
        let companies = ["acme".to_owned()];
        let policy = |efforts: &[(&String, Effort)]| RolePolicy {
            models: vec![a.id.clone(), b.id.clone()],
            efforts: efforts.iter().map(|(id, e)| ((*id).clone(), *e)).collect(),
            ..RolePolicy::default()
        };
        let ok = c
            .check_policy(
                &policy(&[(&a.id, Effort::High), (&b.id, Effort::Low)]),
                &companies,
                &tools(),
            )
            .unwrap();
        // Both are kept, A's too although it equals A's own setting: with rules above the role
        // (ADR-041), it still says something.
        assert_eq!(
            ok.efforts,
            BTreeMap::from([(a.id.clone(), Effort::High), (b.id.clone(), Effort::Low)])
        );
        for bad in [
            policy(&[(&b.id, Effort::Max)]),
            policy(&[(&"nope".to_owned(), Effort::Low)]),
        ] {
            assert!(
                c.check_policy(&bad, &companies, &tools()).is_err(),
                "{bad:?}"
            );
        }
        // Moving B to an AI tool without effort levels drops the role's effort for it; removing
        // a model drops it too.
        let mut ok = ok;
        ok.efforts.remove(&a.id);
        c.policies.insert("dev".into(), ok);
        c.save_model(
            &ModelInput {
                id: Some(b.id.clone()),
                ..input("beta", Some("b"), "B")
            },
            &tools(),
        )
        .unwrap();
        assert!(c.policies["dev"].efforts.is_empty());
        c.policies
            .get_mut("dev")
            .unwrap()
            .efforts
            .insert(a.id.clone(), Effort::Low);
        c.remove_model(&a.id).unwrap();
        assert!(c.policies["dev"].efforts.is_empty());
    }

    #[test]
    fn removing_a_model_takes_it_out_of_every_list() {
        let mut c = RoutingConfig::default();
        let a = c
            .save_model(&input("alpha", Some("a"), "A"), &tools())
            .unwrap();
        let b = c
            .save_model(&input("beta", Some("b"), "B"), &tools())
            .unwrap();
        c.policies.insert(
            "dev".into(),
            RolePolicy {
                models: vec![a.id.clone(), b.id.clone()],
                ..RolePolicy::default()
            },
        );
        c.policies.insert("qa".into(), RolePolicy::default());
        let (removed, roles) = c.remove_model(&a.id).unwrap();
        assert_eq!(removed.id, a.id);
        assert_eq!(roles, ["dev"]);
        assert_eq!(c.policies["dev"].models, std::slice::from_ref(&b.id));
        assert!(c.remove_model(&a.id).is_err());
    }

    #[test]
    fn a_rule_left_with_nothing_in_it_is_forgotten() {
        let mut c = RoutingConfig::default();
        let a = c
            .save_model(&input("alpha", Some("a"), "A"), &tools())
            .unwrap();
        let b = c
            .save_model(&input("alpha", Some("b"), "B"), &tools())
            .unwrap();
        let only = |id: &str| ModelRule {
            efforts: BTreeMap::from([(id.to_owned(), Effort::High)]),
            ..ModelRule::default()
        };
        c.positions.insert("agent".into(), only(&a.id));
        c.departments.insert("sales".into(), only(&a.id));
        c.departments.insert(
            "ops".into(),
            ModelRule {
                effort: Some(Effort::Low),
                ..only(&a.id)
            },
        );
        c.remove_model(&a.id).unwrap();
        assert!(c.positions.is_empty());
        assert_eq!(c.departments.keys().collect::<Vec<_>>(), ["ops"]);
        // Moved to an AI tool that takes no effort level: its effort, and so the rule, goes.
        c.positions.insert("agent".into(), only(&b.id));
        c.save_model(
            &ModelInput {
                id: Some(b.id.clone()),
                ..input("beta", Some("b"), "B")
            },
            &tools(),
        )
        .unwrap();
        assert!(c.positions.is_empty());
    }

    #[test]
    fn policies_are_validated() {
        let mut c = RoutingConfig::default();
        let a = c
            .save_model(&input("alpha", Some("a"), "A"), &tools())
            .unwrap();
        let companies = ["acme".to_owned()];
        let ok = c
            .check_policy(
                &RolePolicy {
                    models: vec![a.id.clone()],
                    needs: vec![ModelFeature::ComputerUse, ModelFeature::ComputerUse],
                    never_companies: vec!["acme".into(), "acme".into()],
                    ..RolePolicy::default()
                },
                &companies,
                &tools(),
            )
            .unwrap();
        assert_eq!(ok.needs, [ModelFeature::ComputerUse]);
        assert_eq!(ok.never_companies, ["acme"]);
        for bad in [
            RolePolicy {
                models: vec!["nope".into()],
                ..RolePolicy::default()
            },
            RolePolicy {
                models: vec![a.id.clone(), a.id.clone()],
                ..RolePolicy::default()
            },
            RolePolicy {
                never_companies: vec!["mystery".into()],
                ..RolePolicy::default()
            },
            RolePolicy {
                min_context_tokens: Some(0),
                ..RolePolicy::default()
            },
        ] {
            assert!(
                c.check_policy(&bad, &companies, &tools()).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_stored_document_round_trips_and_tolerates_absence() {
        assert_eq!(
            RoutingConfig::from_value(serde_json::Value::Null).unwrap(),
            RoutingConfig::default()
        );
        let mut c = RoutingConfig::default();
        c.save_model(&input("alpha", Some("a"), "A"), &tools())
            .unwrap();
        c.options.on_usage_limit = LimitBehavior::NextChoice;
        c.cleared_limits.insert("alpha".into(), 5);
        let back = RoutingConfig::from_value(c.to_value()).unwrap();
        assert_eq!(back, c);
        assert!(RoutingConfig::from_value(serde_json::json!({ "models": 3 })).is_err());
    }

    /// ADR-041 §8: an effort a model does not take is refused, naming the levels it takes.
    #[test]
    fn efforts_are_refused_in_plain_words_naming_what_the_model_takes() {
        let mut c = RoutingConfig::default();
        let opus = c
            .save_model(&input("alpha", Some("opus"), "Opus"), &tools())
            .unwrap();
        let mini = c
            .save_model(&input("alpha", Some("mini"), "Mini"), &tools())
            .unwrap();
        let companies = ["acme".to_owned()];
        let rule = |id: &str, e: Effort| ModelRule {
            efforts: BTreeMap::from([(id.to_owned(), e)]),
            ..ModelRule::default()
        };
        let refused = c
            .check_rule(&rule(&opus.id, Effort::Ultra), &companies, &tools())
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("Opus (Alpha Code) does not take ultra effort. It takes low or high"),
            "{refused}"
        );
        let none = c
            .check_rule(&rule(&mini.id, Effort::Low), &companies, &tools())
            .unwrap_err()
            .to_string();
        assert!(
            none.contains("Mini (Alpha Code) has no effort setting"),
            "{none}"
        );
        // An effort for any model must be one some AI tool takes.
        let any = |e| ModelRule {
            effort: Some(e),
            ..ModelRule::default()
        };
        assert!(c
            .check_rule(&any(Effort::High), &companies, &tools())
            .is_ok());
        assert!(c
            .check_rule(&any(Effort::Max), &companies, &tools())
            .unwrap_err()
            .to_string()
            .contains("none of your AI tools takes max effort"));
        // Lists and companies are checked as a role's are.
        let listed = ModelRule {
            models: vec![opus.id.clone(), opus.id.clone()],
            ..ModelRule::default()
        };
        assert!(c.check_rule(&listed, &companies, &tools()).is_err());
        let never = ModelRule {
            never_companies: vec!["acme".into(), "acme".into()],
            ..ModelRule::default()
        };
        assert_eq!(
            c.check_rule(&never, &companies, &tools())
                .unwrap()
                .never_companies,
            ["acme"]
        );
        // Removing a model takes it out of every rule.
        c.organization = ModelRule {
            models: vec![opus.id.clone()],
            efforts: BTreeMap::from([(opus.id.clone(), Effort::High)]),
            ..ModelRule::default()
        };
        c.departments.insert("dev".into(), c.organization.clone());
        c.remove_model(&opus.id).unwrap();
        assert!(c.organization.models.is_empty() && c.organization.efforts.is_empty());
        assert!(
            !c.departments.contains_key("dev"),
            "left empty, it is forgotten"
        );
    }
}
