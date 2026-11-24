//! What the upstream will accept for one model: window, output cap, vision, effort set.
//!
//! The gateway has a capability envelope per model. Passing a value outside it back to the
//! caller as an opaque 400 is the failure this module exists to prevent: every gate here
//! either substitutes a value the model accepts or removes the field, and the request is
//! forwarded once.
use crate::config::{CapabilityOverride, Config};
use crate::mlai::Limits;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CONTEXT: i64 = 200_000;
pub const DEFAULT_OUTPUT: i64 = 32_768;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Effort::Low),
            "medium" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            "max" => Some(Effort::Max),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Max => "max",
        }
    }
}

/// Accepted effort values measured against the gateway on 2026-11-24: `max` is rejected by
/// glm-5.3, kimi-k3, grok-4.7 and gpt-6-astra, and accepted by pixis-opus and ml-ai-forge.
/// Unknown models get the conservative set, so a new model fails closed instead of guessing.
pub fn default_efforts(real: &str) -> Vec<Effort> {
    match real {
        "pixis-opus" | "ml-ai-forge" => vec![Effort::Low, Effort::Medium, Effort::High, Effort::Max],
        _ => vec![Effort::Low, Effort::Medium, Effort::High],
    }
}

/// Models that cannot see images, from the stored limits and the measured catalog.
/// A static fallback matters on the very first request, before any run has been minted.
pub fn default_vision(real: &str) -> bool {
    !matches!(real, "glm-5.3" | "ml-ai-forge")
}

/// Measured context windows for the models this bridge exposes. `None` means the model is
/// unknown, so the value learned from its start response (or the generic default) applies.
/// A known model's window wins over the stored cache: a stale or test-written entry must not
/// shrink a million-token model to 200k.
pub fn known_context(real: &str) -> Option<i64> {
    match real {
        "pixis-opus" | "pixis-sol" => Some(1_048_576),
        "ml-ai-forge" => Some(110_000),
        "glm-5.3" | "kimi-k3" | "grok-4.7" | "gpt-6-astra" | "claude-sonnet-5.5" => Some(200_000),
        _ => None,
    }
}

/// The per-model envelope used to normalise one request.
#[derive(Clone, Debug)]
pub struct Capability {
    pub real: String,
    pub context: i64,
    pub output: i64,
    pub vision: bool,
    pub efforts: Vec<Effort>,
    pub tools: bool,
    pub parallel_tools: bool,
}

impl Capability {
    /// Start from what the run token told us, then a config override, then the defaults.
    /// `limits` comes from `limits.cache.json` (written when a run is minted).
    pub fn resolve(real: &str, limits: Option<&Limits>, cfg: &Config) -> Self {
        let over: Option<&CapabilityOverride> = cfg.capabilities.get(real);
        let context = over
            .and_then(|o| o.context)
            .or_else(|| known_context(real))
            .or_else(|| limits.map(|l| l.context))
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_CONTEXT);
        let output = over
            .and_then(|o| o.output)
            .or_else(|| limits.and_then(|l| l.output))
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_OUTPUT);
        let vision = over
            .and_then(|o| o.vision)
            .or_else(|| limits.map(|l| l.vision))
            .unwrap_or_else(|| default_vision(real));
        let efforts = over
            .and_then(|o| o.efforts.as_ref())
            .map(|list| list.iter().filter_map(|value| Effort::parse(value)).collect())
            .filter(|list: &Vec<Effort>| !list.is_empty())
            .unwrap_or_else(|| default_efforts(real));
        Capability {
            real: real.to_string(),
            context,
            output,
            vision,
            efforts,
            tools: over.and_then(|o| o.tools).unwrap_or(true),
            parallel_tools: over.and_then(|o| o.parallel_tools).unwrap_or(true),
        }
    }

    /// The effort to send: the caller's choice when the model accepts it, otherwise the
    /// nearest stronger value it does accept. `None` means send no effort at all.
    pub fn effort(&self, requested: Option<&str>, fallback: Option<&str>) -> Option<Effort> {
        let wanted = requested
            .and_then(Effort::parse)
            .or_else(|| fallback.and_then(Effort::parse))?;
        if self.efforts.contains(&wanted) {
            return Some(wanted);
        }
        for candidate in [Effort::High, Effort::Medium, Effort::Low] {
            if self.efforts.contains(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    pub fn clamp_output(&self, requested: i64) -> i64 {
        requested.clamp(1, self.output.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn unknown_models_get_the_conservative_effort_set() {
        let efforts = default_efforts("some-new-model");
        assert!(!efforts.contains(&Effort::Max));
        assert!(efforts.contains(&Effort::High));
    }

    #[test]
    fn opus_accepts_max() {
        let cap = Capability::resolve("pixis-opus", None, &config());
        assert_eq!(cap.effort(Some("max"), None), Some(Effort::Max));
    }

    #[test]
    fn opus_has_a_million_token_window_before_any_run() {
        let cap = Capability::resolve("pixis-opus", None, &config());
        assert_eq!(cap.context, 1_048_576);
    }

    #[test]
    fn sol_also_has_a_million_token_window() {
        let cap = Capability::resolve("pixis-sol", None, &config());
        assert_eq!(cap.context, 1_048_576);
    }

    #[test]
    fn a_stale_cache_does_not_shrink_a_known_model() {
        let limits = Limits {
            context: 200_000,
            output: Some(32_768),
            vision: true,
            seen: String::new(),
        };
        let cap = Capability::resolve("pixis-sol", Some(&limits), &config());
        assert_eq!(cap.context, 1_048_576);
    }

    #[test]
    fn max_is_substituted_on_glm() {
        let cap = Capability::resolve("glm-5.3", None, &config());
        assert_eq!(cap.effort(Some("max"), None), Some(Effort::High));
    }

    #[test]
    fn unknown_effort_string_falls_back_to_the_default() {
        let cap = Capability::resolve("glm-5.3", None, &config());
        assert_eq!(cap.effort(Some("extreme"), Some("low")), Some(Effort::Low));
    }

    #[test]
    fn no_effort_configured_sends_none() {
        let cap = Capability::resolve("glm-5.3", None, &config());
        assert_eq!(cap.effort(None, None), None);
        assert_eq!(cap.effort(Some(""), None), None);
    }

    #[test]
    fn output_is_clamped_to_the_model_cap() {
        let limits = Limits {
            context: 200_000,
            output: Some(32_768),
            vision: true,
            seen: String::new(),
        };
        let cap = Capability::resolve("glm-5.3", Some(&limits), &config());
        assert_eq!(cap.clamp_output(100_000), 32_768);
        assert_eq!(cap.clamp_output(1_000), 1_000);
        assert_eq!(cap.clamp_output(0), 1);
    }

    #[test]
    fn a_config_override_wins_over_the_stored_limits() {
        let mut cfg = config();
        cfg.capabilities.insert(
            "glm-5.3".into(),
            CapabilityOverride {
                efforts: Some(vec!["low".into()]),
                vision: Some(true),
                ..Default::default()
            },
        );
        let limits = Limits {
            context: 200_000,
            output: Some(32_768),
            vision: false,
            seen: String::new(),
        };
        let cap = Capability::resolve("glm-5.3", Some(&limits), &cfg);
        assert_eq!(cap.efforts, vec![Effort::Low]);
        assert!(cap.vision);
    }
}
