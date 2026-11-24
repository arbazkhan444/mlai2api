//! Model aliases and OpenAI wire normalization.
use crate::capability::{Capability, Effort};
use crate::config::{Config, ReasoningMode};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct Resolved {
    pub alias: String,
    pub real: String,
}

pub fn resolve_model(requested: Option<&str>, cfg: &Config, catalog: &[String]) -> Result<Resolved, String> {
    let want = requested
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| cfg.default_model.clone());
    if want.is_empty() {
        return Err("No model in the request and no defaultModel in config.json".into());
    }
    if let Some(real) = cfg.aliases.get(&want) {
        return Ok(Resolved { alias: want, real: real.clone() });
    }
    if let Some((alias, real)) = cfg.aliases.iter().find(|(k, _)| k.eq_ignore_ascii_case(&want)) {
        return Ok(Resolved { alias: alias.clone(), real: real.clone() });
    }
    if catalog.iter().any(|id| id == &want) {
        return Ok(Resolved { alias: want.clone(), real: want });
    }
    if catalog.is_empty() && cfg.allow_raw_model_ids {
        return Ok(Resolved { alias: want.clone(), real: want });
    }
    Err(format!(
        "Unknown model \"{}\". Use one of the models this bridge exposes at GET /v1/models.",
        want
    ))
}

/// Text the engine uses when a model cannot see images; the caller is told, not failed.
pub const IMAGE_OMITTED: &str = "[image omitted: this model cannot see images. Tell the person in one sentence and suggest switching to a model that can.]";

fn strip_images(obj: &mut Map<String, Value>, note: &str) -> bool {
    let Some(messages) = obj.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    for message in messages.iter_mut() {
        let Some(content) = message.get_mut("content").and_then(|v| v.as_array_mut()) else {
            continue;
        };
        for part in content.iter_mut() {
            let kind = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(kind, "image_url" | "image" | "input_image") {
                *part = json!({"type": "text", "text": note});
                changed = true;
            }
        }
    }
    changed
}

pub fn rewrite_request(body: &Value, cfg: &Config, real: &str, cap: &Capability) -> Value {
    let mut out = body.clone();
    let Some(obj) = out.as_object_mut() else {
        return out;
    };
    obj.insert("model".into(), json!(real));
    let stream = obj.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    if stream && cfg.always_include_usage {
        let mut options = obj
            .get("stream_options")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        options.insert("include_usage".into(), json!(true));
        obj.insert("stream_options".into(), Value::Object(options));
    }
    // Effort: keep the caller's value when the model accepts it, otherwise substitute one it does.
    let requested = obj
        .get("reasoning_effort")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    match cap.effort(requested.as_deref(), cfg.reasoning_effort.as_deref()) {
        Some(effort) => {
            if let Some(from) = requested.as_deref() {
                if Effort::parse(from) != Some(effort) {
                    tracing::info!(model = real, from, to = effort.as_str(), "effort substituted");
                }
            }
            obj.insert("reasoning_effort".into(), json!(effort.as_str()));
        }
        None => {
            obj.remove("reasoning_effort");
        }
    }
    // Output tokens: one field, never above the model's cap.
    let requested_max = obj
        .get("max_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| obj.get("max_completion_tokens").and_then(|v| v.as_i64()));
    obj.remove("max_completion_tokens");
    if let Some(max) = requested_max {
        obj.insert("max_tokens".into(), json!(cap.clamp_output(max)));
    } else if cfg.max_tokens_floor > 0 {
        obj.insert(
            "max_tokens".into(),
            json!(cap.clamp_output(cfg.max_tokens_floor as i64)),
        );
    }
    // Vision: a text-only model is told an image was omitted, instead of rejecting the request.
    if !cap.vision && strip_images(obj, IMAGE_OMITTED) {
        tracing::info!(model = real, "image parts omitted for a text-only model");
    }
    // Tools the model cannot use are not sent.
    if !cap.tools {
        obj.remove("tools");
        obj.remove("tool_choice");
        obj.remove("parallel_tool_calls");
    } else if !cap.parallel_tools {
        obj.remove("parallel_tool_calls");
    }
    out
}

pub struct StreamCtx {
    pub alias: String,
    pub id: String,
    pub created: i64,
    pub reasoning: ReasoningMode,
    pub usage: Option<Value>,
    pub chunks: usize,
    /// The caller asked for usage in the stream, so the final usage-only chunk belongs to it.
    pub want_usage: bool,
    /// Stable id per tool-call index, so a synthesised id survives later fragments.
    pub tool_ids: std::collections::BTreeMap<usize, String>,
}

impl StreamCtx {
    pub fn new(alias: &str, reasoning: ReasoningMode) -> Self {
        let mut bytes = [0u8; 8];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        StreamCtx {
            alias: alias.to_string(),
            id: format!("chatcmpl-bridge-{}", hex::encode(bytes)),
            created: chrono::Utc::now().timestamp(),
            reasoning,
            usage: None,
            chunks: 0,
            want_usage: false,
            tool_ids: std::collections::BTreeMap::new(),
        }
    }
}

fn apply_reasoning(map: &mut Map<String, Value>, mode: ReasoningMode) {
    let text = map
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| map.get("reasoning").and_then(|v| v.as_str()).map(str::to_string));
    let Some(text) = text else { return };
    match mode {
        ReasoningMode::Native => {}
        ReasoningMode::None => {
            map.remove("reasoning_content");
            map.remove("reasoning");
        }
        ReasoningMode::Reasoning => {
            map.remove("reasoning_content");
            map.insert("reasoning".into(), json!(text));
        }
        ReasoningMode::Both => {
            map.insert("reasoning_content".into(), json!(text));
            map.insert("reasoning".into(), json!(text));
        }
    }
}

fn new_call_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 6];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("call_{}", hex::encode(bytes))
}

/// Every tool call needs a stable id, a contiguous index and a type. A provider that omits the
/// id would hand the caller a call it cannot answer, so one is synthesised and remembered by
/// index for the fragments that follow.
fn normalize_tool_call(call: &mut Value, position: usize, ctx: &mut StreamCtx) {
    let Some(cobj) = call.as_object_mut() else {
        return;
    };
    let index = cobj
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(position);
    cobj.insert("index".into(), json!(index));
    cobj.entry("type".to_string())
        .or_insert_with(|| json!("function"));
    let incoming = cobj
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let id = if incoming.is_empty() {
        ctx.tool_ids.entry(index).or_insert_with(new_call_id).clone()
    } else {
        ctx.tool_ids.insert(index, incoming.clone());
        incoming
    };
    cobj.insert("id".into(), json!(id));
}

/// Returns `None` for a chunk the caller did not ask to receive (a usage-only frame when it did
/// not request stream usage). The usage is still counted first, so billing stays correct.
pub fn normalize_chunk(chunk: &Value, ctx: &mut StreamCtx) -> Option<Value> {
    let mut out = chunk.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("object".into(), json!("chat.completion.chunk"));
        obj.insert("id".into(), json!(ctx.id));
        obj.insert("created".into(), json!(ctx.created));
        obj.insert("model".into(), json!(ctx.alias));
        if let Some(usage) = obj.get("usage") {
            if !usage.is_null() {
                ctx.usage = Some(usage.clone());
            }
        }
        let empty_choices = obj
            .get("choices")
            .and_then(|c| c.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true);
        if empty_choices && !ctx.want_usage {
            return None;
        }
        if let Some(choices) = obj.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for (i, choice) in choices.iter_mut().enumerate() {
                let Some(cobj) = choice.as_object_mut() else {
                    continue;
                };
                if !cobj.get("index").map(|v| v.is_number()).unwrap_or(false) {
                    cobj.insert("index".into(), json!(i));
                }
                let Some(delta) = cobj.get_mut("delta").and_then(|d| d.as_object_mut()) else {
                    continue;
                };
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                    for (n, call) in calls.iter_mut().enumerate() {
                        normalize_tool_call(call, n, ctx);
                    }
                }
                apply_reasoning(delta, ctx.reasoning);
            }
        }
    }
    ctx.chunks += 1;
    Some(out)
}

pub fn normalize_completion(json_in: &Value, ctx: &mut StreamCtx) -> Value {
    let mut out = json_in.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("object".into(), json!("chat.completion"));
        obj.entry("id".to_string()).or_insert_with(|| json!(ctx.id));
        obj.insert("created".into(), json!(ctx.created));
        obj.insert("model".into(), json!(ctx.alias));
        if let Some(usage) = obj.get("usage") {
            if !usage.is_null() {
                ctx.usage = Some(usage.clone());
            }
        }
        if let Some(choices) = obj.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for (i, choice) in choices.iter_mut().enumerate() {
                let Some(cobj) = choice.as_object_mut() else { continue };
                if !cobj.get("index").map(|v| v.is_number()).unwrap_or(false) {
                    cobj.insert("index".into(), json!(i));
                }
                if let Some(msg) = cobj.get_mut("message").and_then(|m| m.as_object_mut()) {
                    apply_reasoning(msg, ctx.reasoning);
                    if let Some(calls) = msg.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                        for (n, call) in calls.iter_mut().enumerate() {
                            normalize_tool_call(call, n, ctx);
                        }
                    }
                    let has_tools = msg
                        .get("tool_calls")
                        .and_then(|t| t.as_array())
                        .map(|a| !a.is_empty())
                        .unwrap_or(false);
                    let empty_content = msg.get("content").map(|c| c.is_null()).unwrap_or(true);
                    if empty_content && !has_tools {
                        msg.insert("content".into(), json!(""));
                    }
                }
            }
        }
    }
    out
}

pub fn error_payload(status: u16, message: &str, code: &str) -> Value {
    let kind = if status == 401 {
        "authentication_error"
    } else if status >= 500 {
        "upstream_error"
    } else {
        "invalid_request_error"
    };
    json!({
        "error": {
            "message": message.chars().take(2000).collect::<String>(),
            "type": kind,
            "code": code
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::config::{CapabilityOverride, Config, ReasoningMode};

    fn cap(real: &str) -> Capability {
        Capability::resolve(real, None, &Config::default())
    }

    #[test]
    fn effort_is_substituted_when_the_model_rejects_it() {
        let mut cfg = Config::default();
        cfg.reasoning_effort = None;
        let body = json!({"model": "x", "reasoning_effort": "max", "messages": []});
        let out = rewrite_request(&body, &cfg, "glm-5.3", &cap("glm-5.3"));
        assert_eq!(out["reasoning_effort"], "high");
    }

    #[test]
    fn accepted_effort_is_kept() {
        let mut cfg = Config::default();
        cfg.reasoning_effort = None;
        let body = json!({"model": "x", "reasoning_effort": "max", "messages": []});
        let out = rewrite_request(&body, &cfg, "pixis-opus", &cap("pixis-opus"));
        assert_eq!(out["reasoning_effort"], "max");
    }

    #[test]
    fn output_tokens_are_unified_and_clamped() {
        let cfg = Config::default();
        let body = json!({"model": "x", "max_completion_tokens": 100000, "messages": []});
        let out = rewrite_request(&body, &cfg, "glm-5.3", &cap("glm-5.3"));
        assert_eq!(out["max_tokens"], 32_768);
        assert!(out.get("max_completion_tokens").is_none());
    }

    #[test]
    fn images_are_replaced_for_text_only_models() {
        let cfg = Config::default();
        let body = json!({"model": "x", "messages": [{"role": "user", "content": [
            {"type": "text", "text": "what is this"},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AA"}}
        ]}]});
        let out = rewrite_request(&body, &cfg, "glm-5.3", &cap("glm-5.3"));
        let parts = out["messages"][0]["content"].as_array().unwrap();
        assert_eq!(parts[1]["type"], "text");
        assert!(parts[1]["text"].as_str().unwrap().contains("cannot see images"));
    }

    #[test]
    fn tools_are_removed_when_the_model_has_none() {
        let mut cfg = Config::default();
        cfg.capabilities.insert(
            "no-tools".into(),
            CapabilityOverride {
                tools: Some(false),
                ..Default::default()
            },
        );
        let cap = Capability::resolve("no-tools", None, &cfg);
        let body = json!({"model": "x", "messages": [], "tools": [{"type": "function"}],
                          "tool_choice": "auto", "parallel_tool_calls": true});
        let out = rewrite_request(&body, &cfg, "no-tools", &cap);
        assert!(out.get("tools").is_none());
        assert!(out.get("tool_choice").is_none());
        assert!(out.get("parallel_tool_calls").is_none());
    }

    #[test]
    fn usage_only_chunk_is_swallowed_unless_requested() {
        let chunk = json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 1, "total_tokens": 11}});
        let mut ctx = StreamCtx::new("glm", ReasoningMode::Native);
        assert!(normalize_chunk(&chunk, &mut ctx).is_none());
        assert!(ctx.usage.is_some(), "usage is still counted");
        ctx.want_usage = true;
        assert!(normalize_chunk(&chunk, &mut ctx).is_some());
    }

    #[test]
    fn tool_call_id_is_synthesised_and_stable_across_fragments() {
        let mut ctx = StreamCtx::new("glm", ReasoningMode::Native);
        let first = normalize_chunk(
            &json!({"choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "function": {"name": "ping", "arguments": "{}"}}
            ]}}]}),
            &mut ctx,
        )
        .unwrap();
        let id = first["choices"][0]["delta"]["tool_calls"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(id.starts_with("call_"), "{id}");
        let second = normalize_chunk(
            &json!({"choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "function": {"arguments": "{}"}}
            ]}}]}),
            &mut ctx,
        )
        .unwrap();
        assert_eq!(second["choices"][0]["delta"]["tool_calls"][0]["id"], id);
    }

    #[test]
    fn provider_tool_call_id_is_kept() {
        let mut ctx = StreamCtx::new("glm", ReasoningMode::Native);
        let out = normalize_chunk(
            &json!({"choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "id": "call_provided", "type": "function",
                 "function": {"name": "ping", "arguments": "{}"}}
            ]}}]}),
            &mut ctx,
        )
        .unwrap();
        assert_eq!(
            out["choices"][0]["delta"]["tool_calls"][0]["id"],
            "call_provided"
        );
    }

    #[test]
    fn non_streamed_tool_call_gets_an_id_and_type() {
        let mut ctx = StreamCtx::new("glm", ReasoningMode::Native);
        let out = normalize_completion(
            &json!({"choices": [{"message": {"role": "assistant", "content": null,
                "tool_calls": [{"function": {"name": "ping", "arguments": "{}"}}]}}]}),
            &mut ctx,
        );
        let call = &out["choices"][0]["message"]["tool_calls"][0];
        assert!(call["id"].as_str().unwrap().starts_with("call_"));
        assert_eq!(call["type"], "function");
        assert_eq!(call["index"], 0);
    }
}
