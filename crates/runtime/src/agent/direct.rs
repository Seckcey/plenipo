//! Direct keys for AI companies (Phase 16 Wave 3, ADR-087): each company's own service, reached
//! with the owner's key through Plenipo's own helper, like OpenRouter (ADR-086).
//!
//! One adapter serves every company; each company is a row: its paid AI tool's ID (the helper's
//! service), its name, and the models Plenipo offers with the prices on the company's own pages
//! as of [`PRICES_CHECKED`]. The companies' own model lists carry no prices, so a model that is
//! not in the row is not priced, and never runs on a key (ADR-085 §3.4).
//!
//! Where a company charges more above some prompt size, a model's words are held under it
//! ([`Sold::max_input_bytes`]: a token is at least one byte), so the price in the row always
//! applies. Where it charges more at busy hours (DeepSeek), the row has the dearer price; where it
//! stores input for reuse by itself and charges more for that (OpenAI), every fresh input token is
//! counted at that price. Plenipo can count more than was spent, never less.

use std::path::PathBuf;

use serde_json::Value;

use crate::agent::adapter::{
    cap, model_name, ProbeOutput, ProviderSession, RuntimeAdapter, TurnParser, TurnRequest,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{makers, AuthStatus, Effort, KnownModel, Maker, RuntimeCapabilities};
use crate::agent::paid::{
    conversation_file, fitting, last_json, load_conversation, most_input_tokens, parse_check,
    sent_text, valid_conversation_id, PaidLimits, PaidParser, DEFAULT_OUTPUT_TOKENS, HELPER_ARG,
    MAX_INPUT_BYTES,
};
use crate::pricing::Price;

/// When the prices below were read from each company's own pricing page.
pub const PRICES_CHECKED: &str = "2026-09-30";

/// A model a company sells, with its price per million tokens in micros.
#[derive(Debug, Clone, Copy)]
pub struct Sold {
    pub name: &'static str,
    pub label: &'static str,
    pub price: Price,
    pub effort: &'static [Effort],
    /// The most words sent at once, in bytes: under a price step or the model's context.
    pub max_input_bytes: u64,
    /// The same model on other AI tools (ADR-036 §4).
    pub same: Option<&'static str>,
}

const fn sold(name: &'static str, label: &'static str, price: Price) -> Sold {
    Sold {
        name,
        label,
        price,
        effort: &[],
        max_input_bytes: MAX_INPUT_BYTES,
        same: None,
    }
}

impl Sold {
    const fn thinks(mut self, effort: &'static [Effort]) -> Self {
        self.effort = effort;
        self
    }
    const fn under(mut self, bytes: u64) -> Self {
        self.max_input_bytes = bytes;
        self
    }
    const fn same(mut self, same: &'static str) -> Self {
        self.same = Some(same);
        self
    }
}

const fn m(input: u64, cached: u64, output: u64) -> Price {
    Price::micros(input, Some(cached), output)
}

/// Below 200,000 tokens, where xAI and Google charge double for a larger prompt.
const UNDER_200K: u64 = 190_000;
/// Below 272,000 tokens, where OpenAI charges more for a larger prompt.
const UNDER_272K: u64 = 260_000;
/// Room for a 16,000-token answer in a 262,144-token context.
const IN_262K: u64 = 240_000;
/// Room for a 16,000-token answer in a 204,800-token context.
const IN_204K: u64 = 180_000;
/// Room for a 16,000-token answer in a 256,000-token context.
const IN_256K: u64 = 230_000;

const TO_MAX: &[Effort] = &[
    Effort::Low,
    Effort::Medium,
    Effort::High,
    Effort::XHigh,
    Effort::Max,
];
const LOW_TO_HIGH: &[Effort] = &[Effort::Low, Effort::Medium, Effort::High];
const LOW_HIGH_MAX: &[Effort] = &[Effort::Low, Effort::High, Effort::Max];

const ANTHROPIC_MODELS: &[Sold] = &[
    sold(
        "claude-sonnet-5-5",
        "Claude Sonnet 5.5",
        m(2_000_000, 200_000, 10_000_000),
    )
    .thinks(TO_MAX),
    sold(
        "claude-opus-5-5",
        "Claude Opus 5.5",
        m(4_000_000, 200_000, 20_000_000),
    )
    .thinks(TO_MAX),
    sold(
        "claude-fable-5-1",
        "Claude Fable 5.1",
        m(10_000_000, 250_000, 50_000_000),
    )
    .thinks(TO_MAX),
    sold(
        "claude-haiku-4-5",
        "Claude Haiku 4.5",
        m(1_000_000, 100_000, 5_000_000),
    ),
];

const OPENAI_MODELS: &[Sold] = &[
    sold(
        "gpt-6.1-sol",
        "GPT-6.1 Sol",
        m(2_000_000, 100_000, 10_000_000).with_cache_write(2_500_000),
    )
    .thinks(LOW_TO_HIGH)
    .under(UNDER_272K),
    sold(
        "gpt-6-astra",
        "GPT-6 Astra",
        m(10_000_000, 1_000_000, 50_000_000).with_cache_write(12_500_000),
    )
    .thinks(LOW_TO_HIGH)
    .under(UNDER_272K),
    sold(
        "gpt-6-luna",
        "GPT-6 Luna",
        m(100_000, 10_000, 500_000).with_cache_write(125_000),
    )
    .thinks(LOW_TO_HIGH)
    .under(UNDER_272K),
];

const XAI_MODELS: &[Sold] = &[
    sold("grok-4.7", "Grok 4.7", m(2_000_000, 500_000, 6_000_000)).under(UNDER_200K),
    sold("grok-4.3", "Grok 4.3", m(1_250_000, 200_000, 2_500_000)).under(UNDER_200K),
    sold(
        "grok-build-0.1",
        "Grok Build 0.1 (coding)",
        m(1_000_000, 200_000, 2_000_000),
    )
    .under(UNDER_200K),
];

const MOONSHOT_MODELS: &[Sold] = &[
    sold("kimi-k3", "Kimi K3", m(3_000_000, 300_000, 15_000_000))
        .thinks(LOW_HIGH_MAX)
        .same("kimi-k3"),
    sold(
        "kimi-k2.7-code",
        "Kimi K2.7 Code",
        m(950_000, 190_000, 4_000_000),
    )
    .under(IN_262K),
    sold("kimi-k2.6", "Kimi K2.6", m(950_000, 160_000, 4_000_000)).under(IN_262K),
];

const GOOGLE_MODELS: &[Sold] = &[
    // Google charges half this until 2026-12-31; Plenipo counts the regular price.
    sold(
        "gemini-3.8-flash",
        "Gemini 3.8 Flash",
        m(1_500_000, 150_000, 7_500_000),
    )
    .thinks(LOW_TO_HIGH),
    sold(
        "gemini-3.5-flash-lite",
        "Gemini 3.5 Flash-Lite",
        m(300_000, 30_000, 2_500_000),
    )
    .thinks(LOW_TO_HIGH),
    sold(
        "gemini-3.1-pro-preview",
        "Gemini 3.1 Pro (preview)",
        m(2_000_000, 200_000, 12_000_000),
    )
    .thinks(LOW_TO_HIGH)
    .under(UNDER_200K),
];

const DEEPSEEK_MODELS: &[Sold] = &[
    // The busy-hours prices (off-peak is half).
    sold(
        "deepseek-flash",
        "DeepSeek V4.1 Flash",
        m(300_000, 6_000, 1_200_000),
    )
    .thinks(LOW_HIGH_MAX)
    .same("deepseek-v4.1-flash"),
    sold(
        "deepseek-v4-pro",
        "DeepSeek V4 Pro",
        m(1_320_000, 44_000, 3_960_000),
    )
    .thinks(LOW_HIGH_MAX)
    .same("deepseek-v4-pro"),
];

const ZAI_MODELS: &[Sold] = &[
    sold("glm-5.3", "GLM-5.3", m(1_400_000, 260_000, 4_400_000))
        .thinks(LOW_HIGH_MAX)
        .same("glm-5.3"),
    sold(
        "glm-5.3-flash",
        "GLM-5.3 Flash",
        m(150_000, 30_000, 500_000),
    )
    .thinks(LOW_HIGH_MAX)
    .same("glm-5.3-flash"),
];

const MINIMAX_MODELS: &[Sold] = &[
    sold("MiniMax-M3", "MiniMax M3", m(300_000, 60_000, 1_200_000)).same("minimax-m3"),
    sold(
        "MiniMax-M2.7",
        "MiniMax M2.7",
        m(300_000, 60_000, 1_200_000).with_cache_write(375_000),
    )
    .under(IN_204K),
];

const MISTRAL_MODELS: &[Sold] = &[sold(
    "mistral-medium-3-5",
    "Mistral Medium 3.5",
    m(1_500_000, 150_000, 7_500_000),
)
.under(IN_256K)
.same("mistral-medium-3.5")];

const ALIBABA_MODELS: &[Sold] = &[
    sold(
        "qwen3.8-flash",
        "Qwen3.8 Flash",
        Price::micros(150_000, None, 470_000),
    )
    .same("qwen3.8-flash"),
    sold(
        "qwen3.8-max",
        "Qwen3.8 Max",
        Price::micros(2_000_000, None, 6_000_000),
    ),
];

/// One company's row.
#[derive(Debug)]
pub struct Company {
    /// The paid AI tool's ID, also the helper's service ("anthropic-key").
    pub id: &'static str,
    /// The card's name ("Anthropic (paid per use)").
    pub label: &'static str,
    /// The company's name in its own words ("Anthropic refused the key").
    pub name: &'static str,
    pub maker: (&'static str, &'static str),
    /// Where the owner makes a key, in a few words.
    pub where_keys: &'static str,
    pub models: &'static [Sold],
}

/// Every AI company whose own service takes a key (the owner's choice 14). NVIDIA publishes no
/// price per use (its keys come with trial credits), so it has no row (ADR-087 §6).
pub const COMPANIES: &[Company] = &[
    Company {
        id: "anthropic-key",
        label: "Anthropic (paid per use)",
        name: "Anthropic",
        maker: makers::ANTHROPIC,
        where_keys: "console.anthropic.com → API keys",
        models: ANTHROPIC_MODELS,
    },
    Company {
        id: "openai-key",
        label: "OpenAI (paid per use)",
        name: "OpenAI",
        maker: makers::OPENAI,
        where_keys: "platform.openai.com → API keys",
        models: OPENAI_MODELS,
    },
    Company {
        id: "xai-key",
        label: "xAI (paid per use)",
        name: "xAI",
        maker: makers::XAI,
        where_keys: "console.x.ai → API keys",
        models: XAI_MODELS,
    },
    Company {
        id: "moonshot-key",
        label: "Moonshot AI (paid per use)",
        name: "Moonshot AI",
        maker: makers::MOONSHOT,
        where_keys: "platform.kimi.ai → API keys",
        models: MOONSHOT_MODELS,
    },
    Company {
        id: "google-key",
        label: "Google (paid per use)",
        name: "Google",
        maker: makers::GOOGLE,
        where_keys: "aistudio.google.com → Get API key",
        models: GOOGLE_MODELS,
    },
    Company {
        id: "deepseek-key",
        label: "DeepSeek (paid per use)",
        name: "DeepSeek",
        maker: makers::DEEPSEEK,
        where_keys: "platform.deepseek.com → API keys",
        models: DEEPSEEK_MODELS,
    },
    Company {
        id: "zai-key",
        label: "Z.ai (paid per use)",
        name: "Z.ai",
        maker: makers::ZAI,
        where_keys: "z.ai → API keys",
        models: ZAI_MODELS,
    },
    Company {
        id: "minimax-key",
        label: "MiniMax (paid per use)",
        name: "MiniMax",
        maker: makers::MINIMAX,
        where_keys: "platform.minimax.io → API keys",
        models: MINIMAX_MODELS,
    },
    Company {
        id: "mistral-key",
        label: "Mistral (paid per use)",
        name: "Mistral",
        maker: makers::MISTRAL,
        where_keys: "console.mistral.ai → API keys",
        models: MISTRAL_MODELS,
    },
    Company {
        id: "alibaba-key",
        label: "Alibaba Cloud (paid per use)",
        name: "Alibaba Cloud",
        maker: makers::ALIBABA,
        where_keys: "Alibaba Cloud Model Studio (international) → API keys",
        models: ALIBABA_MODELS,
    },
];

/// A company's own service with the owner's key.
#[derive(Debug, Clone, Copy)]
pub struct Direct {
    pub company: &'static Company,
}

impl Direct {
    fn sold(&self, model: &str) -> Option<&'static Sold> {
        self.company.models.iter().find(|s| s.name == model)
    }

    fn known(&self, s: &Sold) -> KnownModel {
        let mut k = KnownModel::new(s.name, s.label, s.effort).by(self.company.maker);
        k.price = Some(s.price);
        k.same = s.same.map(str::to_owned);
        k
    }

    /// The most words a task on `model` may send.
    fn max_input(&self, model: Option<&str>) -> u64 {
        model
            .or_else(|| self.default_model())
            .and_then(|m| self.sold(m))
            .map_or(MAX_INPUT_BYTES, |s| s.max_input_bytes.min(MAX_INPUT_BYTES))
    }
}

/// The paid AI tools for each company, in the row's order.
pub fn adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    COMPANIES
        .iter()
        .map(|company| {
            std::sync::Arc::new(Direct { company }) as std::sync::Arc<dyn RuntimeAdapter>
        })
        .collect()
}

impl RuntimeAdapter for Direct {
    fn id(&self) -> &'static str {
        self.company.id
    }

    fn label(&self) -> &'static str {
        self.company.label
    }

    fn provider(&self) -> &'static str {
        self.company.maker.0
    }

    fn provider_label(&self) -> &'static str {
        self.company.maker.1
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        let mut effort: Vec<Effort> = Vec::new();
        for s in self.company.models {
            for e in s.effort {
                if !effort.contains(e) {
                    effort.push(*e);
                }
            }
        }
        effort.sort();
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            billing_checked_per_turn: false,
            tool_posture: format!(
                "Conversation only: a worker on {} can answer, write, and review text, but \
                 cannot read files or run programs yet.",
                self.company.label
            ),
            effort_levels: effort,
            known_models: self.company.models.iter().map(|s| self.known(s)).collect(),
            default_maker: Some(Maker::new(self.company.maker.0, self.company.maker.1)),
            runs_other_makers: false,
        }
    }

    fn checked_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn install_hint(&self) -> &'static str {
        "Built into Plenipo: nothing to install."
    }

    fn login_hint(&self) -> &'static str {
        "Turn on Settings → Switches → Let workers use paid AI keys, set the business's spending \
         cap, then add your key on its card. Plenipo keeps it in Windows Credential Manager and \
         never shows it again."
    }

    fn executable_name(&self) -> &'static str {
        self.company.id
    }

    fn known_locations(&self, _host: &HostEnv) -> Vec<PathBuf> {
        Vec::new()
    }

    fn built_in(&self) -> bool {
        true
    }

    fn bridged(&self) -> bool {
        true
    }

    fn bridge_args(&self) -> Option<Vec<String>> {
        Some(vec![HELPER_ARG.into(), self.company.id.into()])
    }

    fn paid(&self) -> bool {
        true
    }

    fn paid_note(&self) -> Option<String> {
        Some(format!(
            "Plenipo has not checked {name}'s service with a real key yet. Its models and \
             prices are from {name}'s own pages on {PRICES_CHECKED}; make a key at {keys}.",
            name = self.company.name,
            keys = self.company.where_keys,
        ))
    }

    fn default_model(&self) -> Option<&'static str> {
        self.company.models.first().map(|s| s.name)
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["check".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_check(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        // The helper reaches only this company, and its key comes on its standard input.
        Vec::new()
    }

    fn preassigns_session_id(&self) -> bool {
        true
    }

    fn accepts_tools(&self) -> bool {
        false
    }

    fn reports_memory_shortened(&self) -> bool {
        true
    }

    fn leaves_out(&self, request: &TurnRequest, prompt_bytes: usize) -> bool {
        let ProviderSession::Resume { id } = &request.session else {
            return false;
        };
        if !valid_conversation_id(id) {
            return false;
        }
        let max = self.max_input(request.model.as_deref());
        let file = conversation_file(&request.working_dir, self.company.id, id);
        load_conversation(&file).is_some_and(|history| {
            let prompt =
                " ".repeat(prompt_bytes.min(usize::try_from(max + 1).unwrap_or(usize::MAX)));
            fitting(&history, &prompt, max).is_none_or(|(_, left)| left > 0)
        })
    }

    /// The models the company's own list names: each one Plenipo prices, as its row says; any
    /// other, not priced.
    fn parse_models(&self, out: &ProbeOutput) -> Option<Vec<KnownModel>> {
        let answer = last_json(out)?;
        let list = answer.get("models")?.as_array()?;
        Some(
            list.iter()
                .filter_map(|m| {
                    let name = m.get("id").and_then(Value::as_str).and_then(model_name)?;
                    Some(match self.sold(&name) {
                        Some(s) => self.known(s),
                        None => {
                            let label = m
                                .get("name")
                                .and_then(Value::as_str)
                                .map_or_else(|| name.clone(), |n| cap(n, 120));
                            KnownModel::new(&name, &label, &[]).by(self.company.maker)
                        }
                    })
                })
                .collect(),
        )
    }

    fn turn_args(&self, request: &TurnRequest) -> Vec<String> {
        let model = request
            .model
            .clone()
            .or_else(|| self.default_model().map(str::to_owned))
            .unwrap_or_default();
        let mut args = vec!["chat".into(), "--model".into(), model];
        match &request.session {
            ProviderSession::New { preassigned } => {
                let id = preassigned
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                args.extend(["--session".into(), id]);
            }
            ProviderSession::Resume { id } => {
                args.extend(["--session".into(), id.clone(), "--resume".into()]);
            }
        }
        if let Some(effort) = request.effort {
            args.extend(["--effort".into(), effort.as_str().into()]);
        }
        // The step's limits and price follow, added by the runtime once it has set aside what
        // the step could cost.
        args
    }

    /// The price in the company's row: its own list names no prices.
    fn price_of(&self, model: &str, _reported: Option<&[KnownModel]>) -> Option<Price> {
        self.sold(model).map(|s| s.price).filter(Price::is_sane)
    }

    fn paid_limits(&self, request: &TurnRequest, prompt_bytes: usize) -> PaidLimits {
        let id = match &request.session {
            ProviderSession::Resume { id } => Some(id.as_str()),
            ProviderSession::New { .. } => None,
        };
        let max = self.max_input(request.model.as_deref());
        let (bytes, messages) =
            sent_text(&request.working_dir, self.company.id, id, prompt_bytes, max);
        PaidLimits {
            input_bytes: bytes,
            input_tokens: most_input_tokens(bytes, messages),
            output_tokens: DEFAULT_OUTPUT_TOKENS,
        }
    }

    fn parser(&self, _request: &TurnRequest) -> Box<dyn TurnParser> {
        Box::new(PaidParser::new(self.company.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_company_has_its_own_id_and_every_model_a_sane_price() {
        let mut ids = std::collections::HashSet::new();
        for c in COMPANIES {
            assert!(ids.insert(c.id), "{}", c.id);
            assert!(c.id.ends_with("-key"), "{}", c.id);
            assert!(!c.models.is_empty(), "{}", c.id);
            for s in c.models {
                assert!(s.price.is_sane(), "{}", s.name);
                assert!(s.price.input > 0 && s.price.output > 0, "{}", s.name);
                assert!(s.max_input_bytes <= MAX_INPUT_BYTES, "{}", s.name);
                assert!(model_name(s.name).is_some(), "{}", s.name);
            }
        }
    }

    #[test]
    fn every_same_model_link_joins_two_ways_or_more() {
        let mut ways: std::collections::HashMap<String, Vec<&str>> = Default::default();
        for a in crate::agent::builtin_adapters() {
            for m in a.capabilities().known_models {
                if let Some(same) = m.same {
                    ways.entry(same).or_default().push(a.id());
                }
            }
        }
        for (same, tools) in &ways {
            assert!(tools.len() >= 2, "{same}: {tools:?}");
        }
        assert_eq!(ways["glm-5.3"], ["ollama", "zai-key"]);
        assert_eq!(ways["qwen3.8-flash"], ["openrouter", "alibaba-key"]);
    }

    #[test]
    fn a_model_held_under_a_price_step_can_never_reach_it() {
        let xai = Direct {
            company: &COMPANIES[2],
        };
        assert_eq!(xai.company.id, "xai-key");
        // Grok 4.7 costs double from 200,000 prompt tokens: its words stay under that, with the
        // wrapping on top.
        let most_tokens = most_input_tokens(xai.max_input(Some("grok-4.7")), 1_000);
        assert!(most_tokens < 200_000, "{most_tokens}");
        // A model the row does not name is not priced.
        assert_eq!(xai.price_of("grok-unknown", None), None);
        assert!(xai.price_of("grok-4.7", None).is_some());
    }

    #[test]
    fn the_companys_own_list_is_priced_only_from_the_row() {
        let direct = Direct {
            company: &COMPANIES[0],
        };
        let out = ProbeOutput {
            exit_code: Some(0),
            stdout: serde_json::json!({ "signedIn": true, "models": [
                { "id": "claude-opus-5-5", "name": "Claude Opus 5.5", "price": null },
                { "id": "claude-new-6", "name": "Claude New 6", "price": null },
            ] })
            .to_string(),
            ..ProbeOutput::default()
        };
        let models = direct.parse_models(&out).unwrap();
        assert_eq!(models[0].price.unwrap().output, 20_000_000);
        assert_eq!(models[0].maker.as_ref().unwrap().id, "anthropic");
        assert_eq!(models[1].price, None);
        assert_eq!(models[1].maker.as_ref().unwrap().id, "anthropic");
    }
}
