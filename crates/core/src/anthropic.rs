//! Anthropic Messages (`/v1/messages`) translated onto the internal OpenAI shape.
//!
//! The gateway speaks OpenAI chat completions. This module is the edge adapter: it decodes an
//! Anthropic request into that shape, and encodes the reply (and its stream) back into the
//! event sequence an Anthropic client expects.
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(text_of).collect::<Vec<_>>().join(""),
        Value::Object(map) => map
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn blocks(message: &Value) -> Vec<&Value> {
    match message.get("content") {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(other) => vec![other],
        None => Vec::new(),
    }
}

fn block_kind(block: &Value) -> &str {
    if block.is_string() {
        "text"
    } else {
        block.get("type").and_then(|v| v.as_str()).unwrap_or("")
    }
}

fn image_part(block: &Value) -> Option<Value> {
    let source = block.get("source")?;
    match source.get("type").and_then(|v| v.as_str()) {
        Some("base64") => {
            let media = source
                .get("media_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image/png");
            let data = source.get("data").and_then(|v| v.as_str())?;
            Some(json!({"type": "image_url",
                "image_url": {"url": format!("data:{media};base64,{data}")}}))
        }
        Some("url") => {
            let url = source.get("url").and_then(|v| v.as_str())?;
            Some(json!({"type": "image_url", "image_url": {"url": url}}))
        }
        _ => None,
    }
}

fn map_tool_choice(choice: &Value) -> Value {
    if let Some(kind) = choice.as_str() {
        return json!(kind);
    }
    match choice.get("type").and_then(|v| v.as_str()) {
        Some("auto") => json!("auto"),
        Some("any") => json!("required"),
        Some("none") => json!("none"),
        Some("tool") => json!({"type": "function",
            "function": {"name": choice.get("name").and_then(|v| v.as_str()).unwrap_or("")}}),
        _ => json!("auto"),
    }
}

fn assistant_message(message: &Value) -> Value {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for block in blocks(message) {
        match block_kind(block) {
            "text" => text.push_str(&text_of(block)),
            "tool_use" => {
                let arguments = block
                    .get("input")
                    .map(|input| serde_json::to_string(input).unwrap_or_else(|_| "{}".into()))
                    .unwrap_or_else(|| "{}".into());
                tool_calls.push(json!({
                    "id": block.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "type": "function",
                    "function": {
                        "name": block.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "arguments": arguments
                    }
                }));
            }
            _ => {}
        }
    }
    let mut out = json!({
        "role": "assistant",
        "content": if text.is_empty() && !tool_calls.is_empty() { Value::Null } else { json!(text) }
    });
    if !tool_calls.is_empty() {
        out["tool_calls"] = json!(tool_calls);
    }
    out
}

fn user_messages(message: &Value) -> Vec<Value> {
    let mut parts: Vec<Value> = Vec::new();
    let mut tool_messages: Vec<Value> = Vec::new();
    for block in blocks(message) {
        match block_kind(block) {
            "tool_result" => {
                let id = block
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                tool_messages.push(json!({
                    "role": "tool",
                    "tool_call_id": id,
                    "content": text_of(block.get("content").unwrap_or(&Value::Null))
                }));
            }
            "image" => {
                if let Some(part) = image_part(block) {
                    parts.push(part);
                }
            }
            _ => {
                let text = text_of(block);
                if !text.is_empty() {
                    parts.push(json!({"type": "text", "text": text}));
                }
            }
        }
    }
    let mut out = tool_messages;
    if !parts.is_empty() {
        let only_text = parts.len() == 1 && parts[0].get("type").and_then(|v| v.as_str()) == Some("text");
        let content = if only_text {
            parts[0].get("text").cloned().unwrap_or_else(|| json!(""))
        } else {
            json!(parts)
        };
        out.push(json!({"role": "user", "content": content}));
    }
    out
}

/// Decode an Anthropic Messages request into the OpenAI body the gateway accepts.
pub fn to_openai(input: &Value) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = input.get("system") {
        let text = text_of(system);
        if !text.is_empty() {
            messages.push(json!({"role": "system", "content": text}));
        }
    }
    if let Some(list) = input.get("messages").and_then(|v| v.as_array()) {
        for message in list {
            match message.get("role").and_then(|v| v.as_str()).unwrap_or("user") {
                "assistant" => messages.push(assistant_message(message)),
                _ => messages.extend(user_messages(message)),
            }
        }
    }
    let mut out = json!({"messages": messages});
    for key in ["model", "max_tokens", "temperature", "top_p", "stream"] {
        if let Some(value) = input.get(key) {
            out[key] = value.clone();
        }
    }
    if let Some(stop) = input.get("stop_sequences") {
        out["stop"] = stop.clone();
    }
    if let Some(tools) = input.get("tools").and_then(|v| v.as_array()) {
        let mapped: Vec<Value> = tools
            .iter()
            .map(|tool| {
                json!({"type": "function", "function": {
                    "name": tool.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": tool.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    "parameters": tool.get("input_schema").cloned()
                        .unwrap_or_else(|| json!({"type": "object", "properties": {}}))
                }})
            })
            .collect();
        out["tools"] = json!(mapped);
    }
    if let Some(choice) = input.get("tool_choice") {
        out["tool_choice"] = map_tool_choice(choice);
    }
    // Extended thinking has no OpenAI equivalent; the nearest is the strongest effort.
    if input.pointer("/thinking/type").and_then(|v| v.as_str()) == Some("enabled") {
        out["reasoning_effort"] = json!("high");
    }
    out
}

fn map_finish(reason: &str) -> &'static str {
    match reason {
        "length" => "max_tokens",
        "tool_calls" => "tool_use",
        _ => "end_turn",
    }
}

fn usage_of(openai: &Value) -> Value {
    let usage = openai.get("usage").cloned().unwrap_or(Value::Null);
    let number = |path: &str| usage.pointer(path).and_then(|v| v.as_i64()).unwrap_or(0);
    let prompt = number("/prompt_tokens");
    let cached = number("/prompt_tokens_details/cached_tokens");
    json!({
        "input_tokens": (prompt - cached).max(0),
        "output_tokens": number("/completion_tokens"),
        "cache_read_input_tokens": cached
    })
}

/// Encode a normalised OpenAI completion as an Anthropic message.
pub fn from_completion(openai: &Value, alias: &str) -> Value {
    let choice = openai
        .get("choices")
        .and_then(|v| v.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let message = choice.get("message").cloned().unwrap_or(Value::Null);
    let mut content = Vec::new();
    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            content.push(json!({"type": "text", "text": text}));
        }
    }
    if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for call in calls {
            let function = call.get("function").cloned().unwrap_or(Value::Null);
            let input = function
                .get("arguments")
                .and_then(|v| v.as_str())
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .unwrap_or_else(|| json!({}));
            content.push(json!({
                "type": "tool_use",
                "id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "name": function.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "input": input
            }));
        }
    }
    let raw_id = openai.get("id").and_then(|v| v.as_str()).unwrap_or("bridge");
    json!({
        "id": format!("msg_{}", raw_id.trim_start_matches("chatcmpl-")),
        "type": "message",
        "role": "assistant",
        "model": alias,
        "content": content,
        "stop_reason": map_finish(choice.get("finish_reason").and_then(|v| v.as_str()).unwrap_or("stop")),
        "stop_sequence": Value::Null,
        "usage": usage_of(openai)
    })
}

fn event(name: &str, data: Value) -> String {
    format!("event: {name}\ndata: {}\n\n", serde_json::to_string(&data).unwrap_or_default())
}

/// Turns normalised OpenAI chunks into the Anthropic event sequence.
pub struct Stream {
    alias: String,
    id: String,
    input_tokens: i64,
    next_index: usize,
    text: Option<usize>,
    tools: BTreeMap<usize, usize>,
    started: bool,
    finished: bool,
    stop: Option<String>,
}

impl Stream {
    pub fn new(alias: &str, input_tokens: i64) -> Self {
        let mut bytes = [0u8; 8];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Stream {
            alias: alias.to_string(),
            id: format!("msg_bridge-{}", hex::encode(bytes)),
            input_tokens,
            next_index: 0,
            text: None,
            tools: BTreeMap::new(),
            started: false,
            finished: false,
            stop: None,
        }
    }

    pub fn start(&mut self) -> Vec<String> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        vec![event(
            "message_start",
            json!({"type": "message_start", "message": {
                "id": self.id,
                "type": "message",
                "role": "assistant",
                "model": self.alias,
                "content": [],
                "stop_reason": Value::Null,
                "stop_sequence": Value::Null,
                "usage": {"input_tokens": self.input_tokens, "output_tokens": 0}
            }}),
        )]
    }

    pub fn feed(&mut self, chunk: &Value) -> Vec<String> {
        let mut events = self.start();
        let Some(choice) = chunk.get("choices").and_then(|v| v.get(0)) else {
            return events;
        };
        if let Some(delta) = choice.get("delta") {
            if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    let index = match self.text {
                        Some(index) => index,
                        None => {
                            let index = self.next_index;
                            self.next_index += 1;
                            self.text = Some(index);
                            events.push(event(
                                "content_block_start",
                                json!({"type": "content_block_start", "index": index,
                                    "content_block": {"type": "text", "text": ""}}),
                            ));
                            index
                        }
                    };
                    events.push(event(
                        "content_block_delta",
                        json!({"type": "content_block_delta", "index": index,
                            "delta": {"type": "text_delta", "text": text}}),
                    ));
                }
            }
            if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for call in calls {
                    let openai_index = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let block = match self.tools.get(&openai_index) {
                        Some(index) => *index,
                        None => {
                            if let Some(text) = self.text.take() {
                                events.push(event(
                                    "content_block_stop",
                                    json!({"type": "content_block_stop", "index": text}),
                                ));
                            }
                            let index = self.next_index;
                            self.next_index += 1;
                            self.tools.insert(openai_index, index);
                            events.push(event(
                                "content_block_start",
                                json!({"type": "content_block_start", "index": index,
                                    "content_block": {
                                        "type": "tool_use",
                                        "id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                        "name": call.pointer("/function/name").and_then(|v| v.as_str()).unwrap_or(""),
                                        "input": {}
                                    }}),
                            ));
                            index
                        }
                    };
                    if let Some(arguments) = call.pointer("/function/arguments").and_then(|v| v.as_str()) {
                        if !arguments.is_empty() {
                            events.push(event(
                                "content_block_delta",
                                json!({"type": "content_block_delta", "index": block,
                                    "delta": {"type": "input_json_delta", "partial_json": arguments}}),
                            ));
                        }
                    }
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            self.stop = Some(map_finish(reason).to_string());
        }
        events
    }

    pub fn finish(&mut self, usage: Option<&Value>) -> Vec<String> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut events = self.start();
        if let Some(text) = self.text.take() {
            events.push(event(
                "content_block_stop",
                json!({"type": "content_block_stop", "index": text}),
            ));
        }
        let mut blocks: Vec<usize> = self.tools.values().copied().collect();
        blocks.sort_unstable();
        for index in blocks {
            events.push(event(
                "content_block_stop",
                json!({"type": "content_block_stop", "index": index}),
            ));
        }
        let output = usage
            .and_then(|value| value.get("completion_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        events.push(event(
            "message_delta",
            json!({"type": "message_delta",
                "delta": {"stop_reason": self.stop.clone().unwrap_or_else(|| "end_turn".into()),
                          "stop_sequence": Value::Null},
                "usage": {"output_tokens": output}}),
        ));
        events.push(event("message_stop", json!({"type": "message_stop"})));
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_text_system_and_tools() {
        let input = json!({
            "model": "pixis-sol",
            "max_tokens": 64,
            "system": [{"type": "text", "text": "be brief"}],
            "messages": [{"role": "user", "content": "hi"}],
            "tools": [{"name": "ping", "description": "ping", "input_schema": {"type": "object"}}],
            "tool_choice": {"type": "any"},
            "stop_sequences": ["END"]
        });
        let out = to_openai(&input);
        assert_eq!(out["messages"][0]["role"], "system");
        assert_eq!(out["messages"][0]["content"], "be brief");
        assert_eq!(out["messages"][1]["content"], "hi");
        assert_eq!(out["max_tokens"], 64);
        assert_eq!(out["stop"], json!(["END"]));
        assert_eq!(out["tools"][0]["function"]["name"], "ping");
        assert_eq!(out["tool_choice"], "required");
    }

    #[test]
    fn decodes_tool_use_and_tool_result() {
        let input = json!({"messages": [
            {"role": "assistant", "content": [
                {"type": "text", "text": "checking"},
                {"type": "tool_use", "id": "toolu_1", "name": "ping", "input": {"x": 1}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "pong"}
            ]}
        ]});
        let out = to_openai(&input);
        let assistant = &out["messages"][0];
        assert_eq!(assistant["tool_calls"][0]["id"], "toolu_1");
        assert_eq!(assistant["tool_calls"][0]["function"]["arguments"], "{\"x\":1}");
        let tool = &out["messages"][1];
        assert_eq!(tool["role"], "tool");
        assert_eq!(tool["tool_call_id"], "toolu_1");
        assert_eq!(tool["content"], "pong");
    }

    #[test]
    fn decodes_images_from_base64() {
        let input = json!({"messages": [{"role": "user", "content": [
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AA"}}
        ]}]});
        let out = to_openai(&input);
        assert_eq!(
            out["messages"][0]["content"][0]["image_url"]["url"],
            "data:image/png;base64,AA"
        );
    }

    #[test]
    fn encodes_a_text_completion() {
        let openai = json!({
            "id": "chatcmpl-bridge-abc",
            "choices": [{"message": {"role": "assistant", "content": "hello"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 2,
                      "prompt_tokens_details": {"cached_tokens": 4}}
        });
        let out = from_completion(&openai, "claude-test");
        assert_eq!(out["id"], "msg_bridge-abc");
        assert_eq!(out["content"][0], json!({"type": "text", "text": "hello"}));
        assert_eq!(out["stop_reason"], "end_turn");
        assert_eq!(out["usage"], json!({"input_tokens": 6, "output_tokens": 2, "cache_read_input_tokens": 4}));
    }

    #[test]
    fn encodes_a_tool_call() {
        let openai = json!({
            "id": "chatcmpl-bridge-abc",
            "choices": [{"message": {"role": "assistant", "content": null, "tool_calls": [
                {"id": "call_1", "type": "function",
                 "function": {"name": "ping", "arguments": "{\"x\":1}"}}
            ]}, "finish_reason": "tool_calls"}]
        });
        let out = from_completion(&openai, "claude-test");
        assert_eq!(out["stop_reason"], "tool_use");
        assert_eq!(out["content"][0]["type"], "tool_use");
        assert_eq!(out["content"][0]["input"], json!({"x": 1}));
    }

    #[test]
    fn stream_emits_blocks_then_delta_then_stop() {
        let mut stream = Stream::new("claude-test", 12);
        let first = stream.feed(&json!({"choices": [{"delta": {"content": "Hel"}}]}));
        assert!(first[0].contains("message_start"));
        assert!(first.iter().any(|e| e.contains("content_block_start")));
        assert!(first.iter().any(|e| e.contains("text_delta")));
        let second = stream.feed(&json!({"choices": [{"delta": {"tool_calls": [
            {"index": 0, "id": "call_1", "function": {"name": "ping", "arguments": "{\"x\""}}
        ]}}]}));
        assert!(second.iter().any(|e| e.contains("tool_use")));
        assert!(second.iter().any(|e| e.contains("input_json_delta")));
        let third = stream.feed(&json!({"choices": [{"delta": {}, "finish_reason": "tool_calls"}]}));
        let end = stream.finish(Some(&json!({"completion_tokens": 7})));
        let all: Vec<String> = second.into_iter().chain(third).chain(end).collect();
        assert!(all.iter().any(|e| e.contains("\"stop_reason\":\"tool_use\"")));
        assert!(all.iter().any(|e| e.contains("\"output_tokens\":7")));
        assert_eq!(all.iter().filter(|e| e.contains("message_stop")).count(), 1);
    }
}
