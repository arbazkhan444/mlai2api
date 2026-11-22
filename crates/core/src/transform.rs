//! Model aliases and OpenAI wire normalization.
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

pub fn rewrite_request(body: &Value, cfg: &Config, real: &str) -> Value {
    let mut out = body.clone();
    if let Some(obj) = out.as_object_mut() {
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
        if let Some(effort) = &cfg.reasoning_effort {
            if !obj.contains_key("reasoning_effort") {
                obj.insert("reasoning_effort".into(), json!(effort));
            }
        }
        if cfg.max_tokens_floor > 0 {
            if let Some(current) = obj.get("max_tokens").and_then(|v| v.as_u64()) {
                if current < cfg.max_tokens_floor {
                    obj.insert("max_tokens".into(), json!(cfg.max_tokens_floor));
                }
            }
        }
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

pub fn normalize_chunk(chunk: &Value, ctx: &mut StreamCtx) -> Value {
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
        if let Some(choices) = obj.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for (i, choice) in choices.iter_mut().enumerate() {
                let Some(cobj) = choice.as_object_mut() else { continue };
                if !cobj.get("index").map(|v| v.is_number()).unwrap_or(false) {
                    cobj.insert("index".into(), json!(i));
                }
                let Some(delta) = cobj.get_mut("delta").and_then(|d| d.as_object_mut()) else { continue };
                if let Some(calls) = delta.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                    for (n, call) in calls.iter_mut().enumerate() {
                        if let Some(cobj) = call.as_object_mut() {
                            if !cobj.get("index").map(|v| v.is_number()).unwrap_or(false) {
                                cobj.insert("index".into(), json!(n));
                            }
                            cobj.entry("type".to_string()).or_insert_with(|| json!("function"));
                        }
                    }
                }
                apply_reasoning(delta, ctx.reasoning);
            }
        }
    }
    ctx.chunks += 1;
    out
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
