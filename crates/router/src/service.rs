//! The Router service: reads the routing configuration and the AI tools' live state, applies
//! the owner's changes, and makes decisions through the engine (ADR-011).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use plenipo_ledger::{Ledger, LedgerError};
use plenipo_runtime::agent::{AgentRuntime, AgentRuntimeInfo};
use serde_json::{json, Value};

use crate::config::RoutingConfig;
use crate::dto::*;
use crate::engine::{limit_words, not_ready, route, RouteInput, ToolState};
use crate::error::{Result, RouterError};
use crate::limits;

/// Ledger setting holding the routing configuration.
pub const SETTING: &str = "routing";
const OWNER: &str = "owner";
const PLENIPO: &str = "plenipo";

type Tools = Arc<dyn Fn() -> Vec<AgentRuntimeInfo> + Send + Sync>;

struct Inner {
    ledger: Arc<Ledger>,
    tools: Tools,
    notices: Mutex<Vec<String>>,
}

/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct Router {
    inner: Arc<Inner>,
}

/// What one decision is about.
#[derive(Debug, Clone, Copy, Default)]
pub struct RouteRequest<'a> {
    pub role_id: &'a str,
    /// When the work belongs to a project: its name and allowed runtimes.
    pub project: Option<(&'a str, &'a [String])>,
    /// Runtimes whose work the worker reviews (cross-company review).
    pub reviewed: &'a [String],
}

/// The configuration, AI tool state, and roles, read once for several decisions.
pub struct Planner {
    pub config: RoutingConfig,
    pub tools: Vec<ToolState>,
    roles: HashMap<String, String>,
    pub now: u64,
}

impl Planner {
    pub fn tool(&self, runtime_id: &str) -> Option<&ToolState> {
        self.tools.iter().find(|t| t.info.id == runtime_id)
    }

    /// The model the role's next worker would get, and why.
    pub fn route(&self, request: &RouteRequest<'_>) -> RouteDecision {
        let role = self
            .roles
            .get(request.role_id)
            .map_or("This role", String::as_str);
        let policy = self.config.policy(request.role_id);
        route(&RouteInput {
            role,
            policy: &policy,
            models: &self.config.models,
            tools: &self.tools,
            project: request.project,
            reviewed: request.reviewed,
            on_limit: self.config.options.on_usage_limit,
            now: self.now,
        })
    }

    /// Why `runtime_id` cannot take a task now (not ready, or at a usage limit), if it cannot.
    pub fn unavailable(&self, runtime_id: &str) -> Option<String> {
        let Some(t) = self.tool(runtime_id) else {
            return Some(format!(
                "The AI tool {runtime_id} is not available in this version of Plenipo"
            ));
        };
        not_ready(&t.info).or_else(|| {
            t.limit
                .as_ref()
                .map(|l| format!("{} {}", t.info.label, limit_words(l, self.now)))
        })
    }

    /// A decision the owner made on the position: its fixed AI tool and model.
    pub fn fixed(&self, title: &str, runtime_id: &str, model: Option<&str>) -> RouteDecision {
        let (runtime_label, company) = self.tool(runtime_id).map_or_else(
            || (runtime_id.to_owned(), String::new()),
            |t| (t.info.label.clone(), t.info.provider.clone()),
        );
        let label = match model {
            Some(m) => format!("{m} ({runtime_label})"),
            None => format!("{runtime_label} (default model)"),
        };
        let listed = self
            .config
            .models
            .iter()
            .find(|m| m.runtime_id == runtime_id && m.name.as_deref() == model);
        RouteDecision {
            reason: format!("You set {title} to always use {label}."),
            choice: Some(RouteChoice {
                model_id: listed.map_or_else(String::new, |m| m.id.clone()),
                runtime_id: runtime_id.to_owned(),
                runtime_label,
                company,
                model: model.map(str::to_owned),
                label,
            }),
            rank: None,
            candidates: Vec::new(),
            fixed: true,
        }
    }
}

fn to_ledger(e: RouterError) -> LedgerError {
    match e {
        RouterError::Ledger(e) => e,
        RouterError::Invalid(m) => LedgerError::InvalidInput(m),
    }
}

impl Router {
    /// The router for the desktop app: AI tool state comes from the agent runtime.
    pub fn new(ledger: Arc<Ledger>, runtime: AgentRuntime) -> Self {
        Self::with_tools(ledger, Arc::new(move || runtime.runtimes()))
    }

    /// A router with its own source of AI tool state (tests). Adds each AI tool's built-in
    /// "default model" entry when it is missing.
    pub fn with_tools(ledger: Arc<Ledger>, tools: Tools) -> Self {
        let this = Self {
            inner: Arc::new(Inner {
                ledger,
                tools,
                notices: Mutex::new(Vec::new()),
            }),
        };
        if let Err(e) = this.ensure_builtins() {
            this.notice(format!("Could not add the AI tools' default models: {e}"));
        }
        this
    }

    fn notice(&self, notice: String) {
        let mut notices = self.inner.notices.lock().unwrap_or_else(|p| p.into_inner());
        if !notices.contains(&notice) {
            notices.push(notice);
        }
    }

    fn ledger(&self) -> &Ledger {
        &self.inner.ledger
    }

    fn tools(&self) -> Vec<AgentRuntimeInfo> {
        (self.inner.tools)()
    }

    /// The stored configuration.
    pub fn config(&self) -> Result<RoutingConfig> {
        let value = self.ledger().setting(SETTING)?.unwrap_or(Value::Null);
        RoutingConfig::from_value(value).map_err(|e| {
            RouterError::Invalid(format!("the saved model settings could not be read: {e}"))
        })
    }

    /// Change the configuration in one Ledger transaction, recording `event` with the payload
    /// `change` returns. `None` from `change`: nothing to record, nothing written.
    fn update(
        &self,
        event: &str,
        actor: &str,
        change: impl FnOnce(&mut RoutingConfig) -> Result<Option<Value>>,
    ) -> Result<()> {
        let mut skipped = false;
        let result = self
            .ledger()
            .update_setting(SETTING, event, actor, |value| {
                let mut config = RoutingConfig::from_value(value).map_err(|e| {
                    LedgerError::InvalidInput(format!(
                        "the saved model settings could not be read: {e}"
                    ))
                })?;
                match change(&mut config).map_err(to_ledger)? {
                    Some(payload) => Ok((config.to_value(), payload)),
                    None => {
                        skipped = true;
                        Err(LedgerError::InvalidInput(String::new()))
                    }
                }
            });
        match result {
            Ok(_) => Ok(()),
            Err(_) if skipped => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn ensure_builtins(&self) -> Result<()> {
        let tools: Vec<(String, String)> =
            self.tools().into_iter().map(|t| (t.id, t.label)).collect();
        let config = self.config()?;
        let missing = tools.iter().any(|(id, _)| {
            !config
                .models
                .iter()
                .any(|m| m.built_in && &m.runtime_id == id)
        });
        if !missing {
            return Ok(());
        }
        self.update("router.models_added", PLENIPO, |c| {
            let added = c.add_builtins(&tools);
            Ok((!added.is_empty()).then(|| json!({ "models": added })))
        })
    }

    /// Give roles that have no policy yet the ones in `defaults` (role ID → policy). Used once
    /// for the built-in role templates; the owner's policies are never replaced.
    pub fn seed_policies(&self, defaults: &[(String, RolePolicy)]) -> Result<()> {
        let config = self.config()?;
        if defaults
            .iter()
            .all(|(role, _)| config.policies.contains_key(role))
        {
            return Ok(());
        }
        self.update("router.policies_added", PLENIPO, |c| {
            let mut added = serde_json::Map::new();
            for (role, policy) in defaults {
                if !c.policies.contains_key(role) {
                    c.policies.insert(role.clone(), policy.clone());
                    added.insert(role.clone(), json!(policy));
                }
            }
            Ok((!added.is_empty()).then(|| json!({ "policies": added })))
        })
    }

    /// Read everything one or more decisions need.
    pub fn planner(&self) -> Result<Planner> {
        let config = self.config()?;
        let now = plenipo_ledger::now_ms();
        let outcomes = self
            .ledger()
            .recent_turn_outcomes(now.saturating_sub(limits::LOOKBACK_MS))?;
        let mut active = limits::active(&outcomes, &config.cleared_limits, now);
        let tools = self
            .tools()
            .into_iter()
            .map(|info| ToolState {
                limit: active.remove(&info.id),
                info,
            })
            .collect();
        let roles = self
            .ledger()
            .list_roles()?
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect();
        Ok(Planner {
            config,
            tools,
            roles,
            now,
        })
    }

    /// Everything the Settings page shows.
    pub fn snapshot(&self) -> Result<RoutingSnapshot> {
        let planner = self.planner()?;
        let tools: Vec<ToolInfo> = planner
            .tools
            .iter()
            .map(|t| {
                let not_ready = not_ready(&t.info);
                let status = match (&not_ready, &t.limit) {
                    (Some(why), _) => why.clone(),
                    (None, Some(limit)) => {
                        format!("{} {}", t.info.label, limit_words(limit, planner.now))
                    }
                    (None, None) => "Ready: signed in with a subscription".into(),
                };
                ToolInfo {
                    runtime_id: t.info.id.clone(),
                    label: t.info.label.clone(),
                    company: t.info.provider.clone(),
                    company_label: t.info.provider_label.clone(),
                    ready: t.info.ready,
                    auth: t.info.auth.state,
                    status,
                    usage_limit: t.limit.clone(),
                    available: not_ready.is_none() && t.limit.is_none(),
                }
            })
            .collect();
        let roles = self
            .ledger()
            .list_roles()?
            .into_iter()
            .map(|r| RolePolicyView {
                next: planner.route(&RouteRequest {
                    role_id: &r.id,
                    ..RouteRequest::default()
                }),
                policy: planner.config.policy(&r.id),
                role_id: r.id,
                role_name: r.name,
                full_time: r.persistent,
            })
            .collect();
        let seen =
            self.ledger()
                .models_seen(50)?
                .into_iter()
                .filter_map(|s| {
                    let tool = planner.tool(&s.runtime)?;
                    Some(ModelSeen {
                        listed: planner.config.models.iter().any(|m| {
                            m.runtime_id == s.runtime && m.name.as_deref() == Some(&s.model)
                        }),
                        runtime_label: tool.info.label.clone(),
                        runtime_id: s.runtime,
                        name: s.model,
                        runs: s.runs,
                        last_used: s.last_used,
                    })
                })
                .collect();
        let models = planner.config.models.clone();
        let notices = self
            .inner
            .notices
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        Ok(RoutingSnapshot {
            models,
            tools,
            roles,
            seen,
            options: planner.config.options,
            api_billing: false,
            notices,
            generated_at: planner.now,
        })
    }

    // ---- The owner's changes ------------------------------------------------------------

    pub fn save_model(&self, input: &ModelInput) -> Result<RoutingSnapshot> {
        let tools: Vec<String> = self.tools().into_iter().map(|t| t.id).collect();
        self.update("router.model_saved", OWNER, |c| {
            let model = c.save_model(input, &tools)?;
            Ok(Some(json!({ "model": model })))
        })?;
        self.snapshot()
    }

    pub fn remove_model(&self, id: &str) -> Result<RoutingSnapshot> {
        self.update("router.model_removed", OWNER, |c| {
            let (model, roles) = c.remove_model(id)?;
            Ok(Some(json!({ "model": model, "roles": roles })))
        })?;
        self.snapshot()
    }

    pub fn set_policy(&self, role_id: &str, policy: &RolePolicy) -> Result<RoutingSnapshot> {
        let role = self
            .ledger()
            .role(role_id)?
            .ok_or_else(|| RouterError::Invalid("that role no longer exists".into()))?;
        let mut companies: Vec<String> = self.tools().into_iter().map(|t| t.provider).collect();
        companies.dedup();
        self.update("router.policy_changed", OWNER, |c| {
            let policy = c.check_policy(policy, &companies)?;
            c.policies.insert(role.id.clone(), policy.clone());
            Ok(Some(
                json!({ "roleId": role.id, "role": role.name, "policy": policy }),
            ))
        })?;
        self.snapshot()
    }

    pub fn set_options(&self, options: RoutingOptions) -> Result<RoutingSnapshot> {
        self.update("router.options_changed", OWNER, |c| {
            c.options = options;
            Ok(Some(json!({ "options": options })))
        })?;
        self.snapshot()
    }

    /// Try an AI tool again now, although it reported a usage limit.
    pub fn clear_limit(&self, runtime_id: &str) -> Result<RoutingSnapshot> {
        let tool = self
            .tools()
            .into_iter()
            .find(|t| t.id == runtime_id)
            .ok_or_else(|| {
                RouterError::Invalid(format!("there is no AI tool named {runtime_id:?}"))
            })?;
        let now = plenipo_ledger::now_ms();
        self.update("router.limit_cleared", OWNER, |c| {
            c.cleared_limits.insert(tool.id.clone(), now);
            Ok(Some(json!({ "runtimeId": tool.id, "label": tool.label })))
        })?;
        self.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_ledger::{ExecutionRow, NewEvent, RoleTemplate, RoleType};
    use plenipo_runtime::agent::{
        AuthState, AuthStatus, InstallState, Installation, RuntimeCapabilities,
    };

    fn info(id: &str, label: &str, company: &str, ready: bool) -> AgentRuntimeInfo {
        AgentRuntimeInfo {
            id: id.into(),
            label: label.into(),
            provider: company.into(),
            provider_label: company.to_uppercase(),
            installation: Installation {
                state: InstallState::Installed,
                executable: None,
                version: None,
                detail: None,
            },
            auth: AuthStatus {
                state: if ready {
                    AuthState::Subscription
                } else {
                    AuthState::SignedOut
                },
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
            },
            install_hint: String::new(),
            login_hint: String::new(),
            ready,
            checked_at: None,
        }
    }

    fn setup() -> (Arc<Ledger>, Router, String) {
        let ledger = Arc::new(Ledger::open_in_memory().unwrap());
        let roles = ledger
            .ensure_roles(
                &[RoleTemplate {
                    name: "Senior Developer",
                    description: "",
                    role_type: RoleType::Worker,
                    persistent: false,
                    metadata: Value::Null,
                    formerly: &[],
                }],
                "plenipo",
            )
            .unwrap();
        let router = Router::with_tools(
            Arc::clone(&ledger),
            Arc::new(|| {
                vec![
                    info("alpha", "Alpha Code", "acme", true),
                    info("beta", "Beta CLI", "bolt", true),
                ]
            }),
        );
        (ledger, router, roles[0].id.clone())
    }

    fn model(runtime: &str, name: &str, label: &str) -> ModelInput {
        ModelInput {
            id: None,
            runtime_id: runtime.into(),
            name: Some(name.into()),
            label: label.into(),
            features: vec![],
            context_tokens: None,
            cost: CostClass::Standard,
        }
    }

    fn next(s: &RoutingSnapshot, role: &str) -> Option<String> {
        s.roles
            .iter()
            .find(|r| r.role_id == role)
            .and_then(|r| r.next.choice.as_ref())
            .map(|c| c.model_id.clone())
    }

    #[test]
    fn each_tool_gets_its_default_model_once() {
        let (ledger, router, _) = setup();
        let s = router.snapshot().unwrap();
        let labels: Vec<&str> = s.models.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Alpha Code (default model)", "Beta CLI (default model)"]
        );
        assert!(s.models.iter().all(|m| m.built_in && m.name.is_none()));
        let seeded = ledger.recent_events(1).unwrap().remove(0);
        assert_eq!(seeded.event_type, "router.models_added");
        // A second router on the same Ledger adds nothing.
        let again = Router::with_tools(Arc::clone(&ledger), Arc::clone(&router.inner.tools));
        assert_eq!(again.config().unwrap().models.len(), 2);
        assert_eq!(ledger.recent_events(1).unwrap()[0].seq, seeded.seq);
        assert!(!s.api_billing);
    }

    #[test]
    fn changing_a_roles_policy_changes_its_next_worker() {
        let (ledger, router, dev) = setup();
        let s = router.save_model(&model("alpha", "opus", "Opus")).unwrap();
        let opus = s
            .models
            .iter()
            .find(|m| m.label == "Opus")
            .unwrap()
            .id
            .clone();
        let s = router.save_model(&model("beta", "gpt", "GPT")).unwrap();
        let gpt = s
            .models
            .iter()
            .find(|m| m.label == "GPT")
            .unwrap()
            .id
            .clone();
        // No list: the first ready model in the registry (Alpha's default).
        let first = s.models[0].id.clone();
        assert_eq!(next(&s, &dev), Some(first));
        let set = |ids: &[&String]| {
            router
                .set_policy(
                    &dev,
                    &RolePolicy {
                        models: ids.iter().map(|s| (*s).clone()).collect(),
                        ..RolePolicy::default()
                    },
                )
                .unwrap()
        };
        let s = set(&[&gpt, &opus]);
        assert_eq!(next(&s, &dev), Some(gpt.clone()));
        let event = ledger.recent_events(1).unwrap().remove(0);
        assert_eq!(event.event_type, "router.policy_changed");
        assert_eq!(event.payload["role"], "Senior Developer");
        assert_eq!(event.payload["policy"]["models"], json!([gpt, opus]));
        let s = set(&[&opus, &gpt]);
        assert_eq!(next(&s, &dev), Some(opus.clone()));
        let view = s.roles.iter().find(|r| r.role_id == dev).unwrap();
        assert_eq!(
            view.next.reason,
            "Opus (Alpha Code) is Senior Developer's first choice and is ready."
        );
        // Removing a model takes it out of the list; the next choice takes over.
        let s = router.remove_model(&opus).unwrap();
        assert_eq!(next(&s, &dev), Some(gpt.clone()));
        let removed = ledger.recent_events(1).unwrap().remove(0);
        assert_eq!(removed.payload["roles"], json!([dev]));
        // Refusals change nothing.
        let before = router.config().unwrap();
        assert!(router
            .set_policy("no-such-role", &RolePolicy::default())
            .is_err());
        assert!(router
            .set_policy(
                &dev,
                &RolePolicy {
                    never_companies: vec!["nobody".into()],
                    ..RolePolicy::default()
                }
            )
            .is_err());
        assert!(router.save_model(&model("gamma", "x", "X")).is_err());
        assert_eq!(router.config().unwrap(), before);
    }

    #[test]
    fn usage_limits_come_from_the_ledger_and_can_be_cleared() {
        let (ledger, router, dev) = setup();
        let s = router.save_model(&model("beta", "gpt", "GPT")).unwrap();
        let alpha_default = s.models[0].id.clone();
        let gpt = s
            .models
            .iter()
            .find(|m| m.label == "GPT")
            .unwrap()
            .id
            .clone();
        router
            .set_policy(
                &dev,
                &RolePolicy {
                    models: vec![alpha_default.clone(), gpt.clone()],
                    ..RolePolicy::default()
                },
            )
            .unwrap();
        let now = plenipo_ledger::now_ms();
        let turn = |id: &str, outcome: &str, error: &str| {
            ledger
                .upsert_execution(
                    &ExecutionRow {
                        id: id.into(),
                        task_id: None,
                        runtime: "alpha".into(),
                        provider: Some("acme".into()),
                        model: None,
                        session_id: None,
                        process_id: None,
                        profile_id: None,
                        label: "turn".into(),
                        executable: None,
                        args: vec![],
                        working_dir: None,
                        state: "succeeded".into(),
                        exit_code: Some(0),
                        detail: None,
                        started_at: now,
                        ended_at: Some(now),
                        usage_metadata: json!({}),
                    },
                    "test",
                )
                .unwrap();
            ledger
                .append_event(NewEvent {
                    execution_id: Some(id.into()),
                    source: "agent:alpha".into(),
                    event_type: "agent.result".into(),
                    payload: json!({ "outcome": outcome, "error": error }),
                    ..NewEvent::default()
                })
                .unwrap();
        };
        let reset = now / 1000 + 2 * 3600;
        turn(
            "e1",
            "usageLimited",
            &format!("usage limit reached|{reset}"),
        );
        let s = router.snapshot().unwrap();
        let alpha = &s.tools[0];
        assert!(alpha.ready && !alpha.available);
        assert_eq!(
            alpha.usage_limit.as_ref().unwrap().resets_at,
            Some(reset * 1000)
        );
        assert!(alpha
            .status
            .starts_with("Alpha Code reached its usage limit (resets in"));
        // Wait (the default): no move to another company.
        assert_eq!(next(&s, &dev), None);
        // Next choice: the owner allowed moving on.
        let s = router
            .set_options(RoutingOptions {
                on_usage_limit: LimitBehavior::NextChoice,
            })
            .unwrap();
        assert_eq!(next(&s, &dev), Some(gpt.clone()));
        // The owner tries it again now: the limit is set aside (and stays so after a restart).
        let s = router.clear_limit("alpha").unwrap();
        assert!(s.tools[0].available);
        assert_eq!(next(&s, &dev), Some(alpha_default.clone()));
        assert_eq!(
            ledger.recent_events(1).unwrap()[0].event_type,
            "router.limit_cleared"
        );
        assert!(router.clear_limit("gamma").is_err());
        // A new limit after that counts again; a later success lifts it.
        std::thread::sleep(std::time::Duration::from_millis(5));
        turn("e2", "usageLimited", "rate limit");
        assert!(!router.snapshot().unwrap().tools[0].available);
        turn("e3", "completed", "");
        assert!(router.snapshot().unwrap().tools[0].available);
    }

    #[test]
    fn template_policies_are_seeded_once_and_never_replace_the_owners() {
        let (ledger, router, dev) = setup();
        let designer = RolePolicy {
            needs: vec![ModelFeature::Vision],
            ..RolePolicy::default()
        };
        router
            .seed_policies(&[(dev.clone(), designer.clone())])
            .unwrap();
        assert_eq!(router.config().unwrap().policies[&dev], designer);
        assert_eq!(
            ledger.recent_events(1).unwrap()[0].event_type,
            "router.policies_added"
        );
        router.set_policy(&dev, &RolePolicy::default()).unwrap();
        let seq = ledger.recent_events(1).unwrap()[0].seq;
        router.seed_policies(&[(dev.clone(), designer)]).unwrap();
        assert_eq!(
            router.config().unwrap().policies[&dev],
            RolePolicy::default()
        );
        assert_eq!(ledger.recent_events(1).unwrap()[0].seq, seq);
    }

    #[test]
    fn fixed_positions_and_unavailable_tools_explain_themselves() {
        let (_, router, _) = setup();
        let planner = router.planner().unwrap();
        let d = planner.fixed("Code Reviewer", "beta", Some("gpt-x"));
        assert!(d.fixed);
        assert_eq!(
            d.reason,
            "You set Code Reviewer to always use gpt-x (Beta CLI)."
        );
        assert_eq!(d.choice.unwrap().company, "bolt");
        assert_eq!(planner.unavailable("alpha"), None);
        assert!(planner
            .unavailable("gamma")
            .unwrap()
            .contains("not available"));
    }
}
