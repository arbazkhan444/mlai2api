# Clean agent-facing layer — implementation plan

Target: `mlai2api` (this repo). Companion documents: `docs/model-layer-audit.md`
(the measured failures) and `docs/fix-run-budget-recovery.md` (the run-token fix).

## 0. Verdict and scope

The bridge is a correct *relay*: stateless per call, one small surface, a run pool that hides
minting. It is not yet an *agent-facing layer*, because a coding agent needs three things the
relay does not do: it must never receive an opaque rejection, it must never be handed a request
the model cannot serve, and it must be able to speak its own protocol.

This plan turns the relay into a **capability-aware adapter** without adding an agent loop,
session state, prompt injection, or summarisation.

Rules that keep it clean:

1. The proxy never owns the conversation. State stays in the client.
2. The proxy never injects ML AI Code's system prompts. The client owns its prompt.
3. The proxy never summarises. It bounds the context and says so.
4. Every silent repair is logged once, not per request.

## 1. Architecture

```
client protocol            edge adapter                 core (existing)              upstream
─────────────────         ──────────────────           ───────────────────           ─────────
/v1/chat/completions  ──▶ openai::decode ─┐
/v1/messages          ──▶ anthropic::decode ├─▶ Normalizer::request ──▶ rewrite_request ──▶ gateway
/v1/responses         ──▶ responses::decode ┘        │                              │
                                                     │ Capability                  │
                                        Normalizer::response ◀── normalize_chunk ◀──┘
                                                     │
                     ◀── openai::encode / anthropic::encode / responses::encode
```

The internal wire format stays **OpenAI chat completions**, because that is what the upstream
speaks. New protocols are decoded into it and encoded out of it at the edges. This keeps the
change surface small and reuses `rewrite_request`, `normalize_chunk`, `normalize_completion`.

## 2. Module layout

```
crates/core/src/
  protocol/
    mod.rs        // Meta, ClientKind, decode/encode traits
    openai.rs     // current behaviour, moved
    anthropic.rs  // /v1/messages
    responses.rs  // /v1/responses (phase 4)
  normalize/
    mod.rs        // request() and response() entry points
    capability.rs // Capability, effort table, learning
    effort.rs     // clamp rules
    limits.rs     // max_tokens, context budget
    vision.rs     // image gate
    tools.rs      // id/index repair, argument repair
    usage.rs      // weighted units, want_usage gating
    errors.rs     // taxonomy + recovery decision
  context.rs      // estimator + masking (phase 2)
```

`transform.rs` keeps alias resolution and the two normalisers, or is folded into
`normalize/`; either is fine as long as there is exactly one place that mutates a request.

## 3. Capability profile

One record per **real** model, persisted in `limits.cache.json` (extend the existing file;
all new fields optional so old files still parse).

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Capability {
    pub context: i64,          // from start.model.contextWindow, else 200_000
    pub output: i64,           // from start.model.responseReserve, else 32_768
    pub vision: bool,          // from start.model.vision, else true
    pub efforts: Vec<Effort>,  // accepted set; see table
    pub tools: bool,           // true
    pub parallel_tools: bool,  // true
    pub reasoning_field: ReasoningField, // ReasoningContent (default)
    pub seen: String,          // RFC3339
    pub source: Source,        // start | table | learned
}

pub enum Effort { Low, Medium, High, Max }
```

Sources, in order:

1. The `start` response (`contextWindow`, `responseReserve`, `vision`) — already stored today.
2. A static effort table from the probe results (below).
3. A learned correction: an effort that produced a 400 once is removed for that model and the
   profile is rewritten.

```rust
pub fn default_efforts(real: &str) -> Vec<Effort> {
    match real {
        // measured 2026-11-24: max is rejected on these four
        "glm-5.3" | "kimi-k3" | "grok-4.7" | "gpt-6-astra" => vec![Low, Medium, High],
        "pixis-opus" | "ml-ai-forge" => vec![Low, Medium, High, Max],
        _ => vec![Low, Medium, High],
    }
}
```

Unknown models get the conservative set. A rejected effort is never retried blindly; see §8.

## 4. Request pipeline

Order matters. Each stage either mutates the OpenAI-shaped `Value` or returns a typed error.

```
1. auth                      (existing)
2. decode(protocol)          -> (Value openai, Meta)
3. resolve alias             -> real, Capability
4. effort          §4.1
5. max_tokens      §4.2
6. vision gate     §4.3
7. context budget  §4.4  (phase 2)
8. tool hygiene    §4.5
9. forward                   (existing chat call + retry loop)
```

`Meta` carries what the encoder needs and what the request must not lose:

```rust
pub struct Meta {
    pub client: ClientKind,   // OpenAi | Anthropic | Responses
    pub stream: bool,
    pub want_usage: bool,     // caller asked for stream usage
    pub requested_model: String,
    pub drop_on_400: Vec<String>, // fields we may drop for exactly one retry
}
```

### 4.1 Effort

- Alias `medium` stays `medium` (measured: accepted everywhere).
- If the requested effort is not in `cap.efforts`, replace with `High` and record
  `effort_substituted` in the request log.
- If the field is absent, inject the configured default **only if** it is in `cap.efforts`.
- Never send `reasoning_effort` to a model whose `efforts` list is empty.

### 4.2 Output tokens

- Unify `max_completion_tokens` into `max_tokens` (the gateway accepts both today; one field is
  easier to reason about).
- Clamp to `cap.output`. Log `max_tokens_clamped` with requested/actual when they differ.
- If absent, do not invent one unless a config floor is set (current behaviour).

### 4.3 Vision gate

If `!cap.vision` and any message contains an `image_url` part:

- Replace the part with a text part:
  `[image omitted: this model cannot see images. Tell the person in one sentence and suggest switching models.]`
  (the engine's own wording, `IMAGE_OMITTED`).
- Emit one `vision_unsupported` note in `/usage` counters; never fail the request.

### 4.4 Context budget (phase 2)

Estimator: `ceil(chars / 4)` for text, `1500` per image, plus `json_len(tools) / 4`. Conservative
is fine; the reserve absorbs the error.

```
estimate = estimate_tokens(messages, tools)
limit    = cap.context - cap.output - cfg.context_guard.reserve
```

If `estimate <= limit`, forward unchanged.

Otherwise mask in steps (max 8):

1. Never touch: system messages, the first user message, the last 4 messages, assistant
   messages that carry `tool_calls`.
2. Candidates: `role:"tool"` messages and assistant observations older than the tail.
3. Replace content with `[observation masked: N chars]` and continue.
4. Mask from oldest to newest so the prefix stays byte-stable for the prompt cache.

If still over budget, return:

```json
{"error":{"type":"invalid_request_error","code":"context_overflow",
  "message":"Prompt is about 214k tokens; this model accepts 200k with 32k reserved for the answer. Compact the conversation or start a new one.",
  "estimated":214000,"limit":168000}}
```

No summarisation in the proxy. That is the client's job, and it is stateful.

### 4.5 Tool hygiene

- Drop `tools` entirely when `cap.tools == false`.
- Drop `parallel_tool_calls` when `cap.parallel_tools == false`.
- Leave unknown fields alone (measured: `temperature`, `n`, `response_format` are accepted).

## 5. Response pipeline

Applies to both streamed deltas and non-streamed messages.

```rust
pub fn response(out: &mut Value, ctx: &mut StreamCtx, meta: &Meta) { /* ... */ }
```

1. **Reasoning** — canonical field is `reasoning_content`. `Reasoning::Both` mirrors to
   `reasoning`; `None` drops both; `Native` leaves the provider field. Never merged into
   `content`.
2. **Tool calls** (§6) — id, index, type, argument repair.
3. **finish_reason** — map to exactly `stop | length | tool_calls | content_filter`;
   unknown values become `stop` and are logged.
4. **Usage** — always captured; forwarded as a chunk only when `meta.want_usage`, or on a
   non-streamed response (where usage belongs on the message).
5. **Empty content** — `content: ""` when there are tool calls (already done).

## 6. Tool-call normalisation

```rust
pub fn normalize_tool_delta(delta: &mut Map<String, Value>, seen: &mut ToolSeen)
```

- `index`: assign `0..n` if absent or duplicated.
- `id`: if empty, synthesize `call_<12 hex>`; remember it by index so the arguments that follow
  keep the same id.
- `type`: default `"function"`.
- `function.name`: required on the first fragment; if a later fragment repeats it, drop the
  duplicate.
- `function.arguments`: if the provider sends an object instead of a string, stringify it.
- **XML leak repair** (GLM): if the whole arguments string contains
  `<arg_key>…</arg_key><arg_value>…</arg_value>`, parse it into a JSON object once the call is
  complete. Port `runner.GLM_PAIR`; do not repair partial fragments.
- Deduplicate calls with the same `id` in one message.

Nothing here executes or validates against a schema; that is the client's job.

## 7. Streaming state machine

`StreamNormalizer` owns per-response state:

```rust
pub struct StreamState {
    calls: BTreeMap<usize, CallAccum>, // index -> id/name/args
    finished: bool,
    done_sent: bool,
    saw_usage: bool,
}
```

Invariants:

1. Every frame forwarded has `id`, `created`, `model` (alias), `object`.
2. Exactly one `[DONE]` per response, emitted by us; upstream `[DONE]` is swallowed.
3. The usage chunk is never forwarded unless `meta.want_usage`.
4. On upstream error mid-stream, emit one `{"error":…}` frame, then `[DONE]`. Do not retry.
5. A 401/402/409 is decided **before** any byte is forwarded, so the replay in
   `chat()` stays valid. That invariant must not be broken by the normaliser.

## 8. Error taxonomy and recovery

| upstream | detected by | action | client sees |
|---|---|---|---|
| `401` + `run_token_invalid` | status + body | re-mint, replay once | nothing |
| `402` + `billing_limit` | status + body | re-mint, replay once | nothing |
| `409` + `run_finished` | status + body | re-mint, replay once | nothing |
| `400` + suspect field | heuristic | drop the field in `meta.drop_on_400`, replay once | retried result |
| `400` context | estimate near limit | `context_overflow` | typed error |
| `429` | status | pass through with `Retry-After` | typed error |
| `5xx` / stream drop | status / read error | one retry only if no bytes were sent | typed error |

`drop_on_400` is deliberately narrow: only fields this layer wrote or could have written
(`reasoning_effort`, `max_tokens`, `parallel_tool_calls`). A 400 bills nothing, so one retry is
safe; cap it at one.

Codes to standardise: `run_token_invalid`, `billing_limit`, `run_finished`, `rate_limited`,
`unsupported_effort`, `image_unsupported`, `context_overflow`, `max_tokens_too_large`,
`upstream_error`.

## 9. Protocol adapters

### 9.1 `/v1/messages` (Anthropic) — phase 3

Request mapping:

| Anthropic | internal |
|---|---|
| `system` (string or blocks) | one `system` message |
| `messages[].content` `text` block | `text` part |
| `image` block (`source.base64`) | `image_url` data URL |
| assistant `tool_use` block | assistant `tool_calls[]` |
| user `tool_result` block | `role:"tool"` + `tool_call_id` |
| `tools[].input_schema` | `tools[].function.parameters` |
| `tool_choice` | `tool_choice` (`auto`/`any`/`tool` → `auto`/`required`/named) |
| `max_tokens` | `max_tokens` (required in Anthropic; default 4096 if absent) |
| `stop_sequences` | `stop` |
| `metadata.user_id` | dropped |

Response mapping:

- Non-stream: `{id, type:"message", role:"assistant", content:[{type:"text"|"tool_use"}], stop_reason, usage}`.
- `stop_reason`: `tool_calls → tool_use`, `length → max_tokens`, `stop → end_turn`.
- Usage: `input_tokens` / `output_tokens` plus `cache_read_input_tokens` when cached.

Stream events, in order:

```
message_start
  content_block_start (text)   -> content_block_delta(text_delta)* -> content_block_stop
  content_block_start (tool_use) -> content_block_delta(input_json_delta)* -> content_block_stop
message_delta (stop_reason, usage)
message_stop
```

`ping` is optional; send one every 15 s while waiting for the first upstream token.

### 9.2 `/v1/responses` (Codex) — phase 4

Map `instructions` → system, `input` items (`message`, `function_call`,
`function_call_output`) → messages/tool messages, `tools` (function) → OpenAI tools. Stream
`response.created`, `response.output_item.added`, `response.output_text.delta`,
`response.function_call_arguments.delta`, `response.completed`.

## 10. Config additions

All optional, defaults preserve today's behaviour:

```jsonc
{
  "capabilities": { "glm-5.3": { "efforts": ["low", "medium", "high"] } },
  "contextGuard": { "enabled": true, "reserve": 4096, "maxMaskSteps": 8, "mask": true },
  "protocols": { "anthropic": true, "responses": false },
  "recovery": { "dropEffortOn400": true, "retryUpstream5xx": true }
}
```

## 11. Testing

Unit (pure functions, no network):

- `effort` clamp for every model in the table.
- `max_tokens` clamp at, below and above the cap.
- vision gate: text-only model with one image → one text part, no image.
- context estimator and masking: never masks system/first user/tail; stops at 8 steps.
- tool normaliser: missing id, duplicate index, object arguments, GLM XML leak.
- finish_reason mapping and error taxonomy.

Golden fixtures:

- `bodies/*.json` already stores `rawSse` and `request` for real traffic. Use them as the
  corpus: feed `rawSse` through the normaliser and snapshot the output. Add fixtures for
  tool-call deltas and a usage chunk.

Mock gateway:

- Extend `mlai-re/mock_upstream.py` (built for the run-token fix) with a scenario matrix:
  `402`, `401`, `409`, effort `400`, vision `400`, context `400`, slow stream, mid-stream drop.
  Each scenario is one integration test with a fixed expectation.

Protocol conformance:

- Run the official Anthropic SDK against `/v1/messages` in both modes and assert event order;
  same for the OpenAI SDK against `/v1/chat/completions` (regression).
- `test-rust.mjs` gains: tool-repair case, `context_overflow` case, "no usage chunk unless
  requested".

Live matrix (opt-in, costs tokens): the probe script used for the audit, kept as
`tools/probe-matrix.mjs`, prints the same table. Run it after any capability-table change.

## 12. Phases and acceptance

| Phase | Work | Acceptance |
|---|---|---|
| P0 | capability profile; effort clamp; max_tokens clamp; vision gate | audit probe matrix shows 200 everywhere except deliberately unsupported cases; no request carries an effort the model rejects |
| P1 | response normaliser; usage gating; tool id/index/argument repair; 409 recovery | golden fixtures pass; a streamed tool call with no id is delivered with a synthesised one; `choices:[]` appears only when asked |
| P2 | context estimator + masking + `context_overflow` | a 210k prompt on a 200k model is masked under budget or refused with the typed error; never an opaque 400 |
| P3 | `/v1/messages` adapter | Anthropic SDK streams a tool call end to end; Claude-Code-style client connects |
| P4 | `/v1/responses` adapter | Codex-style request/stream completes |
| P5 | `finish` reports real usage; per-model rejection counters in `/usage` | dashboard shows non-zero usage for bridge runs |

## 13. Risks

- **Masking invalidates the prompt cache** for the masked suffix. Mask only near the limit,
  once per request, and never rewrite the prefix.
- **Adapters widen the attack surface.** Reuse the body cap and auth; decode strictly; never
  echo upstream URLs.
- **A 400 retry could double a request.** 400 bills nothing and the retry is capped at one;
  never retry after any byte has been forwarded.
- **The upstream can change.** Capability is learned at runtime; the static table is only a
  fallback, and a learned correction wins.
- **Token estimation is approximate.** Keep the reserve; prefer refusing with a clear number
  over sending a request that will 400.

## 14. Open questions

1. Does `POST /api/laptop finish` expect the client-reported usage to match the gateway's
   count, or is it informational? The engine sends `{tokens, calls}`; the bridge sends `{}`.
2. Is `run_finished` always `409`, or can it arrive as `401`/`403` for an older run?
3. Does the gateway ever return Anthropic-shaped usage (`cache_read_input_tokens`)? If yes,
   the weighted counter needs to accept both shapes (it partly does).
4. Should the proxy pin one run per (client, model) across restarts, or is a per-process pool
   enough? Today it is per-process only.
