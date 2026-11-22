//! OpenAI-compatible proxy: /v1/chat/completions, /v1/models, /usage, /health.
use crate::config::Config;
use crate::mlai::{MlaiClient, RunPool};
use crate::store::{BodyPair, NewRequest, Store};
use crate::transform::{
    error_payload, normalize_chunk, normalize_completion, resolve_model, rewrite_request, StreamCtx,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;

const MAX_RAW_SSE: usize = 512 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: Arc<MlaiClient>,
    pub pool: Arc<RunPool>,
    pub store: Arc<Store>,
    pub catalog_ids: Arc<Vec<String>>,
    pub catalog_labels: Arc<Vec<(String, String)>>,
    pub account_id: String,
    pub account_label: String,
    pub started: i64,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/usage", get(usage))
        .route("/v1/models", get(models))
        .route("/v1/models/:id", get(model_one))
        .route("/v1/chat/completions", post(chat))
        .route("/chat/completions", post(chat))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}

fn constant_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn authorized(config: &Config, headers: &HeaderMap) -> bool {
    if config.api_key.trim().is_empty() {
        return true;
    }
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(|v| v.trim().to_string())
        .unwrap_or_default();
    let supplied = if bearer.is_empty() {
        headers.get("x-api-key").and_then(|v| v.to_str().ok()).unwrap_or_default().trim().to_string()
    } else {
        bearer
    };
    !supplied.is_empty() && constant_eq(&supplied, &config.api_key)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

async fn health(State(state): State<AppState>) -> Response {
    let limits = state.pool.limits_table().await;
    json_response(
        StatusCode::OK,
        json!({
            "ok": true,
            "service": "mlai-bridge",
            "version": env!("CARGO_PKG_VERSION"),
            "server": state.config.server,
            "project": state.config.project,
            "models": models_list(&state).await.len(),
            "limits_known": limits.len(),
            "runs": state.pool.snapshot().await,
            "started": state.started,
            "stats": state.store.stats().ok(),
        }),
    )
}

async fn usage(State(state): State<AppState>) -> Response {
    json_response(
        StatusCode::OK,
        json!({
            "stats": state.store.stats().ok(),
            "byModel": state.store.by_model().unwrap_or_default(),
            "byAccount": state.store.by_account().unwrap_or_default(),
            "activeAccount": { "id": state.account_id, "label": state.account_label },
            "tokensPerCredit": crate::store::TOKENS_PER_CREDIT,
        }),
    )
}

async fn models(State(state): State<AppState>) -> Response {
    json_response(StatusCode::OK, json!({ "object": "list", "data": models_list(&state).await }))
}

async fn model_one(State(state): State<AppState>, AxumPath(id): AxumPath<String>) -> Response {
    let list = models_list(&state).await;
    match list.into_iter().find(|m| m.get("id").and_then(|v| v.as_str()) == Some(id.as_str())) {
        Some(model) => json_response(StatusCode::OK, model),
        None => json_response(StatusCode::NOT_FOUND, error_payload(404, &format!("Unknown model {}", id), "model_not_found")),
    }
}

async fn models_list(state: &AppState) -> Vec<Value> {
    let limits = state.pool.limits_table().await;
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (alias, real) in state.config.aliases.iter() {
        seen.push(alias.clone());
        let mut entry = json!({"id": alias, "object": "model", "owned_by": "mlai"});
        if alias != real {
            entry["bridged_to"] = json!(real);
        }
        if let Some(limit) = limits.get(real) {
            entry["context_length"] = json!(limit.context);
            if let Some(output) = limit.output {
                entry["max_completion_tokens"] = json!(output);
            }
        }
        out.push(entry);
    }
    for id in state.catalog_ids.iter() {
        if seen.contains(id) {
            continue;
        }
        seen.push(id.clone());
        let mut entry = json!({"id": id, "object": "model", "owned_by": "mlai"});
        if let Some(limit) = limits.get(id) {
            entry["context_length"] = json!(limit.context);
            if let Some(output) = limit.output {
                entry["max_completion_tokens"] = json!(output);
            }
        }
        out.push(entry);
    }
    out
}

struct RecordArgs<'a> {
    model: &'a str,
    real_model: &'a str,
    stream: bool,
    status: u16,
    ms: i64,
    effort: Option<String>,
    first_token_ms: Option<i64>,
    ctx: Option<&'a StreamCtx>,
    error: Option<String>,
    bodies: Option<BodyPair>,
}

fn record(state: &AppState, args: RecordArgs<'_>) {
    let usage = args.ctx.and_then(|ctx| ctx.usage.as_ref());
    let row = NewRequest {
        model: args.model.to_string(),
        real_model: args.real_model.to_string(),
        stream: args.stream,
        status: args.status,
        ms: args.ms,
        account_id: state.account_id.clone(),
        account_label: state.account_label.clone(),
        effort: args.effort,
        first_token_ms: args.first_token_ms,
        prompt_tokens: usage.and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_i64()),
        completion_tokens: usage.and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_i64()),
        cached_tokens: usage.and_then(|u| u.pointer("/prompt_tokens_details/cached_tokens")).and_then(|v| v.as_i64()),
        reasoning_tokens: usage.and_then(|u| u.pointer("/completion_tokens_details/reasoning_tokens")).and_then(|v| v.as_i64()),
        total_tokens: usage.and_then(|u| u.get("total_tokens")).and_then(|v| v.as_i64()),
        error: args.error,
    };
    if let Err(err) = state.store.record(&row, args.bodies) {
        tracing::warn!("history insert failed: {}", err);
    }
}

fn effort_of(body: &Value) -> Option<String> {
    body.get("reasoning_effort").and_then(|v| v.as_str()).map(|s| s.to_string())
}

async fn chat(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let started = std::time::Instant::now();
    if !authorized(&state.config, &headers) {
        return json_response(
            StatusCode::UNAUTHORIZED,
            error_payload(401, "Invalid bridge API key. Use the apiKey from config.json as the API key in your client.", "invalid_api_key"),
        );
    }
    let body: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return json_response(StatusCode::BAD_REQUEST, error_payload(400, "The request body must be a JSON object", "bad_request")),
    };
    if !body.is_object() {
        return json_response(StatusCode::BAD_REQUEST, error_payload(400, "The request body must be a JSON object", "bad_request"));
    }
    let requested = body.get("model").and_then(|m| m.as_str());
    let resolved = match resolve_model(requested, &state.config, &state.catalog_ids) {
        Ok(resolved) => resolved,
        Err(message) => return json_response(StatusCode::NOT_FOUND, error_payload(404, &message, "model_not_found")),
    };
    let stream = body.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let upstream_body = rewrite_request(&body, &state.config, &resolved.real);
    let effort = effort_of(&upstream_body);

    let entry = match state.pool.ensure(&resolved.real).await {
        Ok(entry) => entry,
        Err(error) => {
            record(&state, RecordArgs {
                model: &resolved.alias, real_model: &resolved.real, stream, status: 502,
                ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
                ctx: None, error: Some(error.to_string()),
                bodies: Some(BodyPair { request: upstream_body.clone(), response: error_payload(502, &error.to_string(), "run_token") }),
            });
            return json_response(StatusCode::BAD_GATEWAY, error_payload(502, &error.to_string(), "run_token"));
        }
    };
    let mut resp = match state.client.chat(&entry.gateway, &upstream_body, &entry.token, stream).await {
        Ok(resp) => resp,
        Err(error) => {
            record(&state, RecordArgs {
                model: &resolved.alias, real_model: &resolved.real, stream, status: 502,
                ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
                ctx: None, error: Some(error.to_string()),
                bodies: Some(BodyPair { request: upstream_body.clone(), response: error_payload(502, &error.to_string(), "upstream_unreachable") }),
            });
            return json_response(StatusCode::BAD_GATEWAY, error_payload(502, &error.to_string(), "upstream_unreachable"));
        }
    };
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        let text = resp.text().await.unwrap_or_default();
        if text.contains("run_token_invalid") {
            state.pool.invalidate(&resolved.real).await;
            match state.pool.ensure(&resolved.real).await {
                Ok(fresh) => {
                    resp = match state.client.chat(&fresh.gateway, &upstream_body, &fresh.token, stream).await {
                        Ok(resp) => resp,
                        Err(error) => {
                            record(&state, RecordArgs {
                                model: &resolved.alias, real_model: &resolved.real, stream, status: 502,
                                ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
                                ctx: None, error: Some(error.to_string()),
                                bodies: Some(BodyPair { request: upstream_body.clone(), response: error_payload(502, &error.to_string(), "upstream_unreachable") }),
                            });
                            return json_response(StatusCode::BAD_GATEWAY, error_payload(502, &error.to_string(), "upstream_unreachable"));
                        }
                    };
                }
                Err(error) => {
                    return json_response(StatusCode::BAD_GATEWAY, error_payload(502, &error.to_string(), "run_token"));
                }
            }
        } else {
            let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| error_payload(401, &text, "unauthorized"));
            record(&state, RecordArgs {
                model: &resolved.alias, real_model: &resolved.real, stream, status: 401,
                ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
                ctx: None, error: Some(text.clone()),
                bodies: Some(BodyPair { request: upstream_body.clone(), response: value.clone() }),
            });
            return json_response(StatusCode::UNAUTHORIZED, value);
        }
    }

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let value: Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| error_payload(status.as_u16(), &text.chars().take(800).collect::<String>(), "upstream_error"));
        record(&state, RecordArgs {
            model: &resolved.alias, real_model: &resolved.real, stream, status: status.as_u16(),
            ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
            ctx: None, error: Some(text.chars().take(500).collect::<String>()),
            bodies: Some(BodyPair { request: upstream_body.clone(), response: value.clone() }),
        });
        let code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        return json_response(code, value);
    }

    let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if stream && content_type.contains("text/event-stream") {
        return stream_out(state, resp, resolved, started, upstream_body, effort);
    }

    let value: Value = match resp.json::<Value>().await {
        Ok(value) => value,
        Err(error) => {
            record(&state, RecordArgs {
                model: &resolved.alias, real_model: &resolved.real, stream: false, status: 502,
                ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
                ctx: None, error: Some(error.to_string()),
                bodies: Some(BodyPair { request: upstream_body.clone(), response: json!({"error": "unreadable upstream body"}) }),
            });
            return json_response(StatusCode::BAD_GATEWAY, error_payload(502, "The model gateway returned an unreadable body", "invalid_response"));
        }
    };
    let mut ctx = StreamCtx::new(&resolved.alias, state.config.reasoning);
    let out = normalize_completion(&value, &mut ctx);
    record(&state, RecordArgs {
        model: &resolved.alias, real_model: &resolved.real, stream: false, status: 200,
        ms: started.elapsed().as_millis() as i64, effort, first_token_ms: None,
        ctx: Some(&ctx), error: None,
        bodies: Some(BodyPair { request: upstream_body, response: value }),
    });
    json_response(StatusCode::OK, out)
}

fn merge_tool_call(acc: &mut Vec<Value>, delta: &Value) {
    let index = delta.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    while acc.len() <= index {
        acc.push(json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}}));
    }
    let slot = &mut acc[index];
    if let Some(id) = delta.get("id").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            slot["id"] = json!(id);
        }
    }
    if let Some(name) = delta.pointer("/function/name").and_then(|v| v.as_str()) {
        if !name.is_empty() {
            slot["function"]["name"] = json!(name);
        }
    }
    if let Some(args) = delta.pointer("/function/arguments").and_then(|v| v.as_str()) {
        let current = slot.pointer("/function/arguments").and_then(|v| v.as_str()).unwrap_or("").to_string();
        slot["function"]["arguments"] = json!(format!("{}{}", current, args));
    }
}

fn stream_out(
    state: AppState,
    resp: reqwest::Response,
    resolved: crate::transform::Resolved,
    started: std::time::Instant,
    request_body: Value,
    effort: Option<String>,
) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    let alias = resolved.alias.clone();
    let real = resolved.real.clone();
    tokio::spawn(async move {
        let mut ctx = StreamCtx::new(&alias, state.config.reasoning);
        let mut buffer = String::new();
        let mut upstream = resp.bytes_stream();
        let mut failed: Option<String> = None;
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        let mut finish: Option<String> = None;
        let mut raw = String::new();
        let mut raw_truncated = false;
        let mut first_token_ms: Option<i64> = None;
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(bytes) => {
                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                    loop {
                        let Some(index) = buffer.find("\n\n") else { break };
                        let frame: String = buffer[..index].to_string();
                        buffer.drain(..index + 2);
                        for line in frame.split('\n') {
                            let line = line.trim();
                            let Some(payload) = line.strip_prefix("data:") else { continue };
                            let payload = payload.trim();
                            if payload.is_empty() || payload == "[DONE]" {
                                continue;
                            }
                            let Ok(value) = serde_json::from_str::<Value>(payload) else { continue };
                            let out = normalize_chunk(&value, &mut ctx);
                            if let Some(choices) = out.get("choices").and_then(|c| c.as_array()) {
                                for choice in choices {
                                    if let Some(delta) = choice.get("delta") {
                                        let text = delta.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                        let thought = delta.get("reasoning_content").and_then(|v| v.as_str()).unwrap_or("");
                                        if first_token_ms.is_none() && (!text.is_empty() || !thought.is_empty()) {
                                            first_token_ms = Some(started.elapsed().as_millis() as i64);
                                        }
                                        content.push_str(text);
                                        reasoning.push_str(thought);
                                        if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                            for call in calls {
                                                merge_tool_call(&mut tool_calls, call);
                                            }
                                        }
                                    }
                                    if let Some(reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                                        finish = Some(reason.to_string());
                                    }
                                }
                            }
                            if raw.len() < MAX_RAW_SSE {
                                raw.push_str(&format!("data: {}\n\n", out));
                            } else {
                                raw_truncated = true;
                            }
                            let frame = format!("data: {}\n\n", out);
                            if tx.send(Ok(Bytes::from(frame))).await.is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    failed = Some(error.to_string());
                    let payload = json!({"error": {"message": format!("The model stream ended early: {}", error), "type": "upstream_error", "code": "stream_interrupted"}});
                    let frame = format!("data: {}\n\n", payload);
                    let _ = tx.send(Ok(Bytes::from(frame))).await;
                    break;
                }
            }
        }
        let _ = tx.send(Ok(Bytes::from("data: [DONE]\n\n"))).await;
        let detail = json!({
            "stream": true,
            "assembled": true,
            "content": content,
            "reasoning": reasoning,
            "toolCalls": tool_calls,
            "finishReason": finish,
            "usage": ctx.usage,
            "chunks": ctx.chunks,
            "rawSse": raw,
            "rawTruncated": raw_truncated,
        });
        record(&state, RecordArgs {
            model: &alias,
            real_model: &real,
            stream: true,
            status: if failed.is_some() { 502 } else { 200 },
            ms: started.elapsed().as_millis() as i64,
            effort,
            first_token_ms,
            ctx: Some(&ctx),
            error: failed,
            bodies: Some(BodyPair { request: request_body, response: detail }),
        });
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream; charset=utf-8")
        .header("Cache-Control", "no-cache, no-transform")
        .header("Connection", "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}
