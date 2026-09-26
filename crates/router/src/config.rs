//! The routing configuration: the owner's model registry, role policies, and options, kept in
//! the Ledger's `routing` setting as one document (ADR-011). Every change is validated here
//! before it is written.

use std::collections::{BTreeMap, HashSet};

use plenipo_ledger::workforce::{clean_line, clean_model, clean_runtime_id};
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
    pub options: RoutingOptions,
    /// When the owner last asked to try each runtime again after a usage limit (ms).
    pub cleared_limits: BTreeMap<String, u64>,
}

fn invalid(message: impl Into<String>) -> RouterError {
    RouterError::Invalid(message.into())
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
                    label: unique_label(self, &format!("{label} (default model)"), None),
                    features: Vec::new(),
                    context_tokens: None,
                    cost: CostClass::Standard,
                    built_in: true,
                };
                self.models.push(m.clone());
                added.push(m);
            }
        }
        added
    }

    /// Add or change a model; `tools` are the runtime IDs this build has. Returns the model.
    pub fn save_model(&mut self, input: &ModelInput, tools: &[String]) -> Result<ModelInfo> {
        let runtime_id = clean_runtime_id(&input.runtime_id)?;
        if !tools.contains(&runtime_id) {
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
            built_in: existing.is_some_and(|i| self.models[i].built_in),
        };
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
            if policy.models.len() != before {
                changed.push(role.clone());
            }
        }
        Ok((removed, changed))
    }

    /// A validated policy for a role; `companies` are the provider IDs this build knows.
    pub fn check_policy(&self, policy: &RolePolicy, companies: &[String]) -> Result<RolePolicy> {
        if policy.models.len() > MAX_ROLE_MODELS {
            return Err(invalid(format!(
                "a role lists at most {MAX_ROLE_MODELS} models"
            )));
        }
        let mut seen = HashSet::new();
        for id in &policy.models {
            if self.model(id).is_none() {
                return Err(invalid("a chosen model is no longer in your list"));
            }
            if !seen.insert(id) {
                return Err(invalid("a model is listed twice"));
            }
        }
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
        let mut never = Vec::new();
        for c in &policy.never_companies {
            if !companies.contains(c) {
                return Err(invalid(format!("there is no AI company named {c:?}")));
            }
            if !never.contains(c) {
                never.push(c.clone());
            }
        }
        Ok(RolePolicy {
            models: policy.models.clone(),
            needs,
            min_context_tokens: policy.min_context_tokens,
            never_companies: never,
            cost: policy.cost,
            cross_company: policy.cross_company,
        })
    }
}

/// `label`, or `label 2`, `label 3`, … when taken by another model.
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

    fn tools() -> Vec<String> {
        vec!["alpha".into(), "beta".into()]
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
        }
    }

    #[test]
    fn builtins_are_added_once_and_stay() {
        let mut c = RoutingConfig::default();
        let builtins = [("alpha".to_owned(), "Alpha Code".to_owned())];
        let added = c.add_builtins(&builtins);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].label, "Alpha Code (default model)");
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
            assert!(c.check_policy(&bad, &companies).is_err(), "{bad:?}");
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
}
