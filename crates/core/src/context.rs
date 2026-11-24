//! Context budget for one request: estimate, mask the oldest tool observations, or refuse
//! with a typed error.
//!
//! The proxy never summarises and never owns the conversation. When a prompt is too large it
//! either removes old tool output (the same trade the ML AI Code engine makes) or tells the
//! caller the exact numbers so its own compaction can act.
use crate::capability::Capability;
use crate::config::Config;
use serde_json::Value;

pub const MASK_PREFIX: &str = "[observation masked";
const IMAGE_TOKENS: i64 = 1_500;

#[derive(Clone, Copy, Debug)]
pub struct Overflow {
    pub estimate: i64,
    pub limit: i64,
}

fn count_chars(value: &Value) -> i64 {
    match value {
        Value::String(text) => text.chars().count() as i64,
        Value::Array(items) => items.iter().map(count_chars).sum(),
        Value::Object(map) => map.values().map(count_chars).sum(),
        _ => 0,
    }
}

fn is_image(part: &Value) -> bool {
    matches!(
        part.get("type").and_then(|value| value.as_str()),
        Some("image" | "image_url" | "input_image")
    )
}

fn message_units(message: &Value) -> (i64, i64) {
    let mut chars = 0;
    let mut images = 0;
    match message.get("content") {
        Some(Value::Array(parts)) => {
            for part in parts {
                if is_image(part) {
                    images += 1;
                } else {
                    chars += count_chars(part);
                }
            }
        }
        Some(Value::String(text)) => chars += text.chars().count() as i64,
        _ => {}
    }
    for key in ["role", "name", "tool_call_id", "tool_calls"] {
        if let Some(value) = message.get(key) {
            chars += count_chars(value);
        }
    }
    (chars, images)
}

/// Rough token count: four characters per token, a flat cost per image (so a base64 data URL
/// does not dominate the estimate), plus the tool schemas.
pub fn estimate_tokens(body: &Value) -> i64 {
    let mut chars = 0;
    let mut images = 0;
    if let Some(messages) = body.get("messages").and_then(|value| value.as_array()) {
        for message in messages {
            let (message_chars, message_images) = message_units(message);
            chars += message_chars;
            images += message_images;
        }
    }
    if let Some(tools) = body.get("tools") {
        chars += count_chars(tools);
    }
    chars / 4 + images * IMAGE_TOKENS
}

/// What one request may use: the window, minus room for the answer, minus a safety reserve.
pub fn limit_tokens(cap: &Capability, cfg: &Config) -> i64 {
    let reserve = cfg.context_guard.reserve.max(0);
    (cap.context - cap.output - reserve).max(1_024)
}

fn is_masked(content: &Value) -> bool {
    matches!(content, Value::String(text) if text.starts_with(MASK_PREFIX))
}

/// Masks the oldest tool observation that is not protected. Returns false when nothing can be
/// masked: system messages, the first user message and the last four messages are preserved,
/// and assistant messages that carry tool calls are left intact.
fn mask_oldest_observation(body: &mut Value) -> bool {
    let Some(messages) = body.get_mut("messages").and_then(|value| value.as_array_mut()) else {
        return false;
    };
    let total = messages.len();
    let keep_tail = 4.min(total);
    let first_user = messages
        .iter()
        .position(|message| message.get("role").and_then(|v| v.as_str()) == Some("user"));
    for index in 0..total.saturating_sub(keep_tail) {
        if Some(index) == first_user {
            continue;
        }
        let Some(message) = messages[index].as_object_mut() else {
            continue;
        };
        if message.get("role").and_then(|v| v.as_str()) != Some("tool") {
            continue;
        }
        let Some(content) = message.get("content") else {
            continue;
        };
        if is_masked(content) {
            continue;
        }
        let text_len = match content {
            Value::String(text) => text.chars().count(),
            Value::Array(parts) => parts.iter().map(count_chars).sum::<i64>() as usize,
            _ => 0,
        };
        message.insert(
            "content".into(),
            Value::String(format!("{MASK_PREFIX}: {text_len} chars]")),
        );
        return true;
    }
    false
}

/// Brings the request under the model's budget, or reports what is left. Mutates only masked
/// tool observations, oldest first, so the prefix of the conversation stays byte-stable.
pub fn enforce(body: &mut Value, cap: &Capability, cfg: &Config) -> Result<(), Overflow> {
    let limit = limit_tokens(cap, cfg);
    let mut estimate = estimate_tokens(body);
    if estimate <= limit {
        return Ok(());
    }
    let guard = &cfg.context_guard;
    if guard.enabled && guard.mask {
        for _ in 0..guard.max_mask_steps {
            if !mask_oldest_observation(body) {
                break;
            }
            estimate = estimate_tokens(body);
            if estimate <= limit {
                tracing::info!(estimate, limit, "context budget met by masking old observations");
                return Ok(());
            }
        }
    }
    Err(Overflow { estimate, limit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::config::Config;
    use serde_json::json;

    fn capability(context: i64, output: i64) -> Capability {
        Capability {
            real: "test".into(),
            context,
            output,
            vision: true,
            efforts: vec![],
            tools: true,
            parallel_tools: true,
        }
    }

    fn body(tool_messages: usize, chars: usize) -> Value {
        let mut messages = vec![
            json!({"role": "system", "content": "rules"}),
            json!({"role": "user", "content": "fix the bug"}),
        ];
        for index in 0..tool_messages {
            messages.push(json!({
                "role": "tool",
                "tool_call_id": format!("call_{index}"),
                "content": "x".repeat(chars)
            }));
        }
        messages.push(json!({"role": "assistant", "content": "done"}));
        messages.push(json!({"role": "user", "content": "thanks"}));
        json!({"model": "test", "messages": messages})
    }

    #[test]
    fn small_prompts_pass_untouched() {
        let mut request = body(1, 40);
        let before = request.clone();
        assert!(enforce(&mut request, &capability(200_000, 32_768), &Config::default()).is_ok());
        assert_eq!(request, before);
    }

    #[test]
    fn old_observations_are_masked_oldest_first() {
        let mut request = body(12, 4_000);
        let cfg = Config::default();
        enforce(&mut request, &capability(10_000, 1_000), &cfg).expect("masking should fit");
        let messages = request["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["content"], "fix the bug");
        assert!(messages[2]["content"].as_str().unwrap().starts_with(MASK_PREFIX));
        for message in &messages[messages.len() - 4..] {
            assert!(!message["content"]
                .as_str()
                .unwrap_or("")
                .starts_with(MASK_PREFIX));
        }
    }

    #[test]
    fn tail_and_first_user_are_never_masked() {
        let mut request = body(2, 60_000);
        let cfg = Config {
            context_guard: crate::config::ContextGuard {
                max_mask_steps: 8,
                ..Default::default()
            },
            ..Config::default()
        };
        let _ = enforce(&mut request, &capability(10_000, 1_000), &cfg);
        let messages = request["messages"].as_array().unwrap();
        assert_eq!(messages[1]["content"], "fix the bug");
        let tail = messages.len() - 4;
        assert!(!messages[tail]["content"].as_str().unwrap().starts_with(MASK_PREFIX));
    }

    #[test]
    fn an_impossible_prompt_reports_the_numbers() {
        let mut request = body(10, 40_000);
        let cfg = Config {
            context_guard: crate::config::ContextGuard {
                max_mask_steps: 2,
                ..Default::default()
            },
            ..Config::default()
        };
        let overflow = enforce(&mut request, &capability(10_000, 1_000), &cfg).unwrap_err();
        assert!(overflow.estimate > overflow.limit);
        assert_eq!(overflow.limit, 10_000 - 1_000 - 4_096);
    }

    #[test]
    fn images_cost_a_flat_amount_not_their_base64_length() {
        let request = json!({"messages": [{"role": "user", "content": [
            {"type": "text", "text": "what is this"},
            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", "A".repeat(200_000))}}
        ]}]});
        let estimate = estimate_tokens(&request);
        assert!(estimate < IMAGE_TOKENS + 100, "estimate was {estimate}");
    }

    #[test]
    fn a_disabled_guard_refuses_without_masking() {
        let mut request = body(12, 4_000);
        let cfg = Config {
            context_guard: crate::config::ContextGuard {
                enabled: false,
                ..Default::default()
            },
            ..Config::default()
        };
        let before = request.clone();
        assert!(enforce(&mut request, &capability(10_000, 1_000), &cfg).is_err());
        assert_eq!(request, before);
    }
}
