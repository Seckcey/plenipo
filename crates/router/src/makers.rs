//! Who made a model (ADR-081, who made each model): the AI company that made it, apart from the
//! AI tool that runs it. Worked out each time from the AI tool's own list, never saved.
//!
//! 1. A model the AI tool's checked list names says who made it (`KnownModel::maker`). What a
//!    tool reports on its own never says who made a model.
//! 2. "Its default" (no model name): who made the AI tool's default model.
//! 3. A model it does not list: its company, for an AI tool that runs only its own company's
//!    models; **not known** for one that runs other companies' models (Ollama).

use plenipo_runtime::agent::{AgentRuntimeInfo, Maker};

/// The AI tool's own company, as a maker.
pub fn company_of(info: &AgentRuntimeInfo) -> Maker {
    Maker::new(&info.provider, &info.provider_label)
}

/// Who made `model` (`None`: the AI tool's default) when `info`'s AI tool runs it; `None`: not
/// known (ADR-081 §2).
pub fn maker_of(info: &AgentRuntimeInfo, model: Option<&str>) -> Option<Maker> {
    let caps = &info.capabilities;
    let Some(name) = model else {
        return caps
            .default_maker
            .clone()
            .or_else(|| (!caps.runs_other_makers).then(|| company_of(info)));
    };
    if let Some(maker) = caps
        .known_models
        .iter()
        .filter(|k| k.name == name)
        .find_map(|k| k.maker.clone())
    {
        return Some(maker);
    }
    (!caps.runs_other_makers).then(|| company_of(info))
}

/// Whether a model sees images, worked out from who made it (Phase 25, item 2.4, amending
/// ADR-011): every current model from Anthropic and Google does, and OpenAI's do apart from its
/// open-weight `gpt-oss` models. `None`: not known, and the worker is told nothing.
pub fn sees_images(maker: Option<&str>, model: Option<&str>) -> Option<bool> {
    match maker? {
        "anthropic" | "google" => Some(true),
        "openai" if !model.is_some_and(|m| m.to_lowercase().contains("gpt-oss")) => Some(true),
        _ => None,
    }
}

/// Every AI company this version of Plenipo knows: the AI tools' own companies and every maker
/// their lists name, by name. The list "AI companies never to use" offers and accepts
/// (ADR-081 §5).
pub fn companies(tools: &[AgentRuntimeInfo]) -> Vec<Maker> {
    let mut out: Vec<Maker> = Vec::new();
    for t in tools {
        let company = company_of(t);
        let listed = t
            .capabilities
            .known_models
            .iter()
            .filter_map(|k| k.maker.clone())
            .chain(t.capabilities.default_maker.clone());
        for m in std::iter::once(company).chain(listed) {
            if !out.iter().any(|o| o.id == m.id) {
                out.push(m);
            }
        }
    }
    out.sort_by_key(|m| m.label.to_lowercase());
    out
}

#[cfg(test)]
mod sees_tests {
    use super::sees_images;

    #[test]
    fn sees_images_comes_from_who_made_it() {
        assert_eq!(sees_images(Some("anthropic"), Some("fable")), Some(true));
        assert_eq!(sees_images(Some("google"), None), Some(true));
        assert_eq!(sees_images(Some("openai"), Some("gpt-6-sol")), Some(true));
        assert_eq!(sees_images(Some("openai"), Some("gpt-oss:120b")), None);
        assert_eq!(sees_images(Some("moonshot"), Some("kimi-code/k3")), None);
        assert_eq!(sees_images(None, Some("auto")), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plenipo_runtime::agent::{
        makers, AccountCommands, AuthState, AuthStatus, InstallState, Installation, KnownModel,
        ReportedModels, RuntimeCapabilities,
    };

    pub fn tool(
        id: &str,
        company: (&str, &str),
        models: Vec<KnownModel>,
        other: bool,
    ) -> AgentRuntimeInfo {
        AgentRuntimeInfo {
            id: id.into(),
            label: id.into(),
            provider: company.0.into(),
            provider_label: company.1.into(),
            installation: Installation {
                state: InstallState::Installed,
                executable: None,
                version: Some("1.0.0".into()),
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
                effort_levels: vec![],
                known_models: models,
                default_maker: other.then(|| Maker::new(makers::OPENAI.0, makers::OPENAI.1)),
                runs_other_makers: other,
            },
            install_hint: String::new(),
            login_hint: String::new(),
            ready: true,
            checked_at: None,
            checked_version: "1.0.0".into(),
            account: AccountCommands::default(),
            reported_models: None,
            held: None,
            uses_tools: true,
        }
    }

    #[test]
    fn a_listed_model_says_who_made_it_and_an_unlisted_one_follows_its_tool() {
        let ollama = tool(
            "ollama",
            ("ollama", "Ollama"),
            vec![
                KnownModel::new("deepseek-v4-pro:cloud", "DeepSeek V4 Pro", &[])
                    .by(makers::DEEPSEEK),
                KnownModel::new("glm-5.3:cloud", "GLM-5.3", &[]).by(makers::ZAI),
            ],
            true,
        );
        let codex = tool(
            "codex",
            makers::OPENAI,
            vec![KnownModel::new("gpt-6-sol", "GPT-6-Sol", &[]).by(makers::OPENAI)],
            false,
        );
        assert_eq!(maker_of(&ollama, Some("glm-5.3:cloud")).unwrap().id, "zai");
        // Ollama's default is OpenAI's gpt-oss.
        assert_eq!(maker_of(&ollama, None).unwrap().id, "openai");
        // A model Ollama runs that Plenipo does not list: not known.
        assert_eq!(maker_of(&ollama, Some("llama9:cloud")), None);
        // Codex runs only OpenAI's models, listed or not.
        assert_eq!(maker_of(&codex, Some("gpt-7")).unwrap().id, "openai");
        assert_eq!(maker_of(&codex, None).unwrap().id, "openai");
        // What a tool reports never says who made a model: only Plenipo's checked list does.
        let mut reported = ollama.clone();
        reported.reported_models = Some(ReportedModels {
            models: vec![KnownModel::new("qwen9:cloud", "Qwen 9", &[]).by(("alibaba", "Alibaba"))],
            complete: false,
            checked_at: 1,
        });
        assert_eq!(maker_of(&reported, Some("qwen9:cloud")), None);
        // Every company once, by name: the AI tools' own and their lists' makers.
        let names: Vec<String> = companies(&[ollama, codex])
            .into_iter()
            .map(|m| m.label)
            .collect();
        assert_eq!(names, ["DeepSeek", "Ollama", "OpenAI", "Z.ai"]);
    }
}
