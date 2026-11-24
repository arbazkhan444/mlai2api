//! OpenAI Responses (`/v1/responses`) translated onto the internal chat-completions shape.
//!
//! Codex-style agents speak this API. The proxy stays stateless: `previous_response_id` and
//! `store` are not honoured, so callers must send their full `input` each turn.
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

fn image_part(part: &Value) -> Option<Value> {
    let url = part
        .get("image_url")
        .or_else(|| part.get("url"))
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| v.get("url").and_then(|u| u.as_str()).map(str::to_string))
        })?;
    Some(json!({"type": "image_url", "image_url": {"url": url}}))
}

fn message_item(item: &Value) -> Value {
    let role = match item.get("role").and_then(|v| v.as_str()) {
        Some("assistant") => "assistant",
        Some("system") | Some("developer") => "system",
        _ => "user",
    };
    let content = match item.get("content") {
        Some(Value::String(text)) => json!(text),
        Some(Value::Array(parts)) => {
            let mut mapped = Vec::new();
            for part in parts {
                match part.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                    "input_image" | "image_url" => {
                        if let Some(image) = image_part(part) {
                            mapped.push(image);
                        }
                    }
                    _ => {
                        let text = text_of(part);
                        if !text.is_empty() {
                            mapped.push(json!({"type": "text", "text": text}));
                        }
                    }
                }
            }
            if mapped.len() == 1 && mapped[0].get("type").and_then(|v| v.as_str()) == Some("text") {
                mapped[0].get("text").cloned().unwrap_or_else(|| json!(""))
            } else {
                json!(mapped)
            }
        }
        _ => json!(""),
    };
    json!({"role": role, "content": content})
}

fn function_call(item: &Value) -> Value {
    let id = item
        .get("call_id")
        .or_else(|| item.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    json!({
        "id": id,
        "type": "function",
        "function": {
            "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "arguments": item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}")
        }
    })
}

fn map_tool_choice(choice: &Value) -> Value {
    if let Some(kind) = choice.as_str() {
        return json!(kind);
    }
    match choice.get("type").and_then(|v| v.as_str()) {
        Some("function") => json!({"type": "function",
            "function": {"name": choice.get("name").and_then(|v| v.as_str()).unwrap_or("")}}),
        _ => json!("auto"),
    }
}

/// Decode a Responses request into the OpenAI chat body the gateway accepts.
pub fn to_openai(input: &Value) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(instructions) = input.get("instructions").and_then(|v| v.as_str()) {
        if !instructions.is_empty() {
            messages.push(json!({"role": "system", "content": instructions}));
        }
    }
    let mut pending: Vec<Value> = Vec::new();
    let flush = |messages: &mut Vec<Value>, pending: &mut Vec<Value>| {
        if !pending.is_empty() {
            let calls = std::mem::take(pending);
            messages.push(json!({"role": "assistant", "content": Value::Null, "tool_calls": calls}));
        }
    };
    match input.get("input") {
        Some(Value::String(text)) => messages.push(json!({"role": "user", "content": text})),
        Some(Value::Array(items)) => {
            for item in items {
                match item.get("type").and_then(|v| v.as_str()).unwrap_or("message") {
                    "function_call" => pending.push(function_call(item)),
                    "function_call_output" => {
                        flush(&mut messages, &mut pending);
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": item.get("call_id").and_then(|v| v.as_str()).unwrap_or(""),
                            "content": text_of(item.get("output").unwrap_or(&Value::Null))
                        }));
                    }
                    _ => {
                        flush(&mut messages, &mut pending);
                        messages.push(message_item(item));
                    }
                }
            }
        }
        _ => {}
    }
    flush(&mut messages, &mut pending);

    let mut out = json!({"messages": messages});
    for key in ["model", "temperature", "top_p", "stream"] {
        if let Some(value) = input.get(key) {
            out[key] = value.clone();
        }
    }
    if let Some(max) = input.get("max_output_tokens") {
        out["max_tokens"] = max.clone();
    }
    if let Some(effort) = input.pointer("/reasoning/effort") {
        out["reasoning_effort"] = effort.clone();
    }
    if let Some(tools) = input.get("tools").and_then(|v| v.as_array()) {
        let mapped: Vec<Value> = tools
            .iter()
            .filter(|tool| tool.get("type").and_then(|v| v.as_str()) == Some("function"))
            .map(|tool| {
                json!({"type": "function", "function": {
                    "name": tool.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": tool.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                    "parameters": tool.get("parameters").cloned()
                        .unwrap_or_else(|| json!({"type": "object", "properties": {}}))
                }})
            })
            .collect();
        if !mapped.is_empty() {
            out["tools"] = json!(mapped);
        }
    }
    if let Some(choice) = input.get("tool_choice") {
        out["tool_choice"] = map_tool_choice(choice);
    }
    if let Some(format) = input.pointer("/text/format") {
        match format.get("type").and_then(|v| v.as_str()) {
            Some("json_object") => out["response_format"] = json!({"type": "json_object"}),
            Some("json_schema") => {
                let name = format.get("name").cloned().unwrap_or_else(|| json!("response"));
                let schema = format
                    .get("schema")
                    .cloned()
                    .unwrap_or_else(|| json!({"type": "object"}));
                let strict = format.get("strict").cloned().unwrap_or_else(|| json!(false));
                out["response_format"] = json!({"type": "json_schema", "json_schema": {
                    "name": name, "schema": schema, "strict": strict
                }});
            }
            _ => {}
        }
    }
    out
}

fn usage_of(openai: &Value) -> Value {
    let usage = openai.get("usage").cloned().unwrap_or(Value::Null);
    let number = |path: &str| usage.pointer(path).and_then(|v| v.as_i64()).unwrap_or(0);
    json!({
        "input_tokens": number("/prompt_tokens"),
        "input_tokens_details": {"cached_tokens": number("/prompt_tokens_details/cached_tokens")},
        "output_tokens": number("/completion_tokens"),
        "output_tokens_details": {"reasoning_tokens": number("/completion_tokens_details/reasoning_tokens")},
        "total_tokens": number("/total_tokens")
    })
}

fn suffix(openai: &Value) -> String {
    openai
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("bridge")
        .trim_start_matches("chatcmpl-")
        .to_string()
}

/// Encode a normalised OpenAI completion as a Responses object.
pub fn from_completion(openai: &Value, alias: &str) -> Value {
    let choice = openai
        .get("choices")
        .and_then(|v| v.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let message = choice.get("message").cloned().unwrap_or(Value::Null);
    let id = suffix(openai);
    let mut output = Vec::new();
    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        if !text.is_empty() {
            output.push(json!({
                "type": "message",
                "id": format!("msg_{id}"),
                "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text, "annotations": []}]
            }));
        }
    }
    if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for (index, call) in calls.iter().enumerate() {
            let function = call.get("function").cloned().unwrap_or(Value::Null);
            output.push(json!({
                "type": "function_call",
                "id": format!("fc_{id}_{index}"),
                "call_id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "name": function.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "arguments": function.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}"),
                "status": "completed"
            }));
        }
    }
    json!({
        "id": format!("resp_{id}"),
        "object": "response",
        "created_at": openai.get("created").cloned().unwrap_or_else(|| json!(chrono::Utc::now().timestamp())),
        "status": "completed",
        "model": alias,
        "output": output,
        "usage": usage_of(openai),
        "error": Value::Null,
        "incomplete_details": Value::Null
    })
}

fn event(kind: &str, seq: &mut u64, mut data: Value) -> String {
    *seq += 1;
    data["type"] = json!(kind);
    data["sequence_number"] = json!(*seq);
    format!("event: {kind}\ndata: {}\n\n", serde_json::to_string(&data).unwrap_or_default())
}

struct MessageState {
    index: usize,
    id: String,
    text: String,
}

struct ToolState {
    index: usize,
    id: String,
    name: String,
    arguments: String,
}

/// Turns normalised OpenAI chunks into the Responses event sequence.
pub struct Stream {
    alias: String,
    id: String,
    created: i64,
    seq: u64,
    next_index: usize,
    started: bool,
    finished: bool,
    message: Option<MessageState>,
    tools: BTreeMap<usize, ToolState>,
}

impl Stream {
    pub fn new(alias: &str, created: i64) -> Self {
        let mut bytes = [0u8; 8];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        Stream {
            alias: alias.to_string(),
            id: format!("resp_bridge-{}", hex::encode(bytes)),
            created,
            seq: 0,
            next_index: 0,
            started: false,
            finished: false,
            message: None,
            tools: BTreeMap::new(),
        }
    }

    fn response(&self, status: &str, output: Vec<Value>, usage: Option<Value>) -> Value {
        json!({
            "id": self.id,
            "object": "response",
            "created_at": self.created,
            "status": status,
            "model": self.alias,
            "output": output,
            "usage": usage,
            "error": Value::Null,
            "incomplete_details": Value::Null
        })
    }

    pub fn start(&mut self) -> Vec<String> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let response = self.response("in_progress", Vec::new(), None);
        vec![event("response.created", &mut self.seq, json!({"response": response}))]
    }

    pub fn feed(&mut self, chunk: &Value) -> Vec<String> {
        let mut events = self.start();
        let Some(choice) = chunk.get("choices").and_then(|v| v.get(0)) else {
            return events;
        };
        if let Some(delta) = choice.get("delta") {
            if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    if self.message.is_none() {
                        let index = self.next_index;
                        self.next_index += 1;
                        let id = format!("msg_{}", self.id.trim_start_matches("resp_"));
                        events.push(event(
                            "response.output_item.added",
                            &mut self.seq,
                            json!({"output_index": index, "item": {
                                "type": "message", "id": id, "status": "in_progress",
                                "role": "assistant", "content": []
                            }}),
                        ));
                        events.push(event(
                            "response.content_part.added",
                            &mut self.seq,
                            json!({"item_id": id, "output_index": index, "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []}}),
                        ));
                        self.message = Some(MessageState { index, id, text: String::new() });
                    }
                    if let Some(message) = self.message.as_mut() {
                        message.text.push_str(text);
                        events.push(event(
                            "response.output_text.delta",
                            &mut self.seq,
                            json!({"item_id": message.id, "output_index": message.index,
                                "content_index": 0, "delta": text}),
                        ));
                    }
                }
            }
            if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for call in calls {
                    let openai_index = call.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if !self.tools.contains_key(&openai_index) {
                        let index = self.next_index;
                        self.next_index += 1;
                        let id = format!("fc_{}_{}", self.id.trim_start_matches("resp_"), openai_index);
                        let name = call
                            .pointer("/function/name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        events.push(event(
                            "response.output_item.added",
                            &mut self.seq,
                            json!({"output_index": index, "item": {
                                "type": "function_call", "id": id,
                                "call_id": call.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                "name": name, "arguments": "", "status": "in_progress"
                            }}),
                        ));
                        self.tools.insert(
                            openai_index,
                            ToolState { index, id, name, arguments: String::new() },
                        );
                    }
                    if let Some(arguments) = call.pointer("/function/arguments").and_then(|v| v.as_str()) {
                        if !arguments.is_empty() {
                            if let Some(tool) = self.tools.get_mut(&openai_index) {
                                tool.arguments.push_str(arguments);
                                events.push(event(
                                    "response.function_call_arguments.delta",
                                    &mut self.seq,
                                    json!({"item_id": tool.id, "output_index": tool.index, "delta": arguments}),
                                ));
                            }
                        }
                    }
                }
            }
        }
        events
    }

    pub fn finish(&mut self, usage: Option<&Value>) -> Vec<String> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;
        let mut events = self.start();
        let mut output = Vec::new();
        if let Some(message) = self.message.take() {
            events.push(event(
                "response.output_text.done",
                &mut self.seq,
                json!({"item_id": message.id, "output_index": message.index,
                    "content_index": 0, "text": message.text}),
            ));
            events.push(event(
                "response.content_part.done",
                &mut self.seq,
                json!({"item_id": message.id, "output_index": message.index, "content_index": 0,
                    "part": {"type": "output_text", "text": message.text, "annotations": []}}),
            ));
            let item = json!({
                "type": "message", "id": message.id, "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": message.text, "annotations": []}]
            });
            events.push(event(
                "response.output_item.done",
                &mut self.seq,
                json!({"output_index": message.index, "item": item.clone()}),
            ));
            output.push(item);
        }
        let mut tools: Vec<&ToolState> = self.tools.values().collect();
        tools.sort_by_key(|tool| tool.index);
        for tool in tools {
            events.push(event(
                "response.function_call_arguments.done",
                &mut self.seq,
                json!({"item_id": tool.id, "output_index": tool.index, "arguments": tool.arguments}),
            ));
            let item = json!({
                "type": "function_call", "id": tool.id, "call_id": tool.id,
                "name": tool.name, "arguments": tool.arguments, "status": "completed"
            });
            events.push(event(
                "response.output_item.done",
                &mut self.seq,
                json!({"output_index": tool.index, "item": item.clone()}),
            ));
            output.push(item);
        }
        let usage = usage.map(|value| usage_of(&json!({"usage": value.clone()})));
        let response = self.response("completed", output, usage);
        events.push(event(
            "response.completed",
            &mut self.seq,
            json!({"response": response}),
        ));
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_instructions_and_string_input() {
        let out = to_openai(&json!({
            "model": "pixis-sol", "instructions": "be brief", "input": "hi",
            "max_output_tokens": 64, "reasoning": {"effort": "low"}
        }));
        assert_eq!(out["messages"][0], json!({"role": "system", "content": "be brief"}));
        assert_eq!(out["messages"][1], json!({"role": "user", "content": "hi"}));
        assert_eq!(out["max_tokens"], 64);
        assert_eq!(out["reasoning_effort"], "low");
    }

    #[test]
    fn decodes_function_call_history() {
        let out = to_openai(&json!({"input": [
            {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "check"}]},
            {"type": "function_call", "call_id": "call_1", "name": "ping", "arguments": "{\"x\":1}"},
            {"type": "function_call_output", "call_id": "call_1", "output": "pong"}
        ]}));
        let messages = out["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[2]["content"], "pong");
    }

    #[test]
    fn decodes_flat_function_tools() {
        let out = to_openai(&json!({
            "input": "hi",
            "tools": [
                {"type": "function", "name": "ping", "description": "ping",
                 "parameters": {"type": "object"}},
                {"type": "web_search"}
            ],
            "tool_choice": {"type": "function", "name": "ping"}
        }));
        assert_eq!(out["tools"].as_array().unwrap().len(), 1);
        assert_eq!(out["tools"][0]["function"]["name"], "ping");
        assert_eq!(out["tool_choice"]["function"]["name"], "ping");
    }

    #[test]
    fn encodes_text_and_tool_calls() {
        let openai = json!({
            "id": "chatcmpl-bridge-abc",
            "created": 1,
            "choices": [{"message": {"role": "assistant", "content": "done", "tool_calls": [
                {"id": "call_1", "type": "function",
                 "function": {"name": "ping", "arguments": "{\"x\":1}"}}
            ]}, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        });
        let out = from_completion(&openai, "pixis-sol");
        assert_eq!(out["object"], "response");
        assert_eq!(out["output"].as_array().unwrap().len(), 2);
        assert_eq!(out["output"][0]["content"][0]["text"], "done");
        assert_eq!(out["output"][1]["type"], "function_call");
        assert_eq!(out["usage"]["input_tokens"], 10);
    }

    #[test]
    fn stream_emits_items_and_completes() {
        let mut stream = Stream::new("pixis-sol", 1);
        let first = stream.feed(&json!({"choices": [{"delta": {"content": "Hel"}}]}));
        assert!(first[0].contains("response.created"));
        assert!(first.iter().any(|e| e.contains("output_text.delta")));
        let second = stream.feed(&json!({"choices": [{"delta": {"tool_calls": [
            {"index": 0, "id": "call_1", "function": {"name": "ping", "arguments": "{\"x\""}}
        ]}}]}));
        assert!(second.iter().any(|e| e.contains("function_call_arguments.delta")));
        let end = stream.finish(Some(&json!({"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15})));
        let all: Vec<String> = second.into_iter().chain(end).collect();
        assert!(all.iter().any(|e| e.contains("response.output_text.done")));
        assert!(all.iter().any(|e| e.contains("response.completed")));
        assert!(all.last().unwrap().contains("\"status\":\"completed\""));
        assert_eq!(all.iter().filter(|e| e.contains("response.completed")).count(), 1);
    }
}
