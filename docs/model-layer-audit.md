# Model-layer audit — where the bridge mis-translates ML AI Code

Measured against the live gateway on 2026-11-24 through the new 0.1.0 binary
(`serve --port 8795`), plus a read of `crates/core/src/{transform,server,mlai}.rs`.
Every claim below is either a probe result or a code path named in brackets.

## 1. Measured compatibility matrix

`reasoning_effort` acceptance (`200` accepted, `400` rejected), `max_tokens: 16`:

| model (alias) | low | medium | high | max |
|---|---|---|---|---|
| `glm` → glm-5.3 | 200 | 200 | 200 | **400** |
| `kimi` → kimi-k3 | 200 | 200 | 200 | **400** |
| `grok` → grok-4.7 | 200 | 200 | 200 | **400** |
| `gpt-6` → gpt-6-astra | 200 | 200 | 200 | **400** |
| `mlai-code` → pixis-opus | 200 | 200 | 200 | 200 |
| `forge` → ml-ai-forge | 200 | 200 | 200 | 200 |

Other probes, all accepted (200): `temperature`, `n=2`, `response_format {type:json_object}`,
`max_completion_tokens`, `parallel_tool_calls` with `tools`.

Rejected (400): an `image_url` part sent to `glm` (a model whose start response says
`vision: false`).

The engine's comment that the GLM route accepts "low/high/max only" does not match the
gateway today: `medium` is accepted everywhere, and `max` is the rejected value on four of
six models.

## 2. Gotchas, ranked

### P0 — `reasoning_effort: "max"` fails on four models

`rewrite_request` passes the caller's value through untouched and only injects the config
default when the field is absent [transform.rs:51-55]. A client or user that picks `max`
gets `400 upstream_400: The model rejected the request.` — with no hint which parameter was
wrong. Coding agents often expose max as "highest".

Fix: a per-model effort set in the capability profile; clamp `max` to `high` (or drop it)
when unsupported, and log the substitution once.

### P0 — Images sent to a text-only model fail the request

The start response carries `model.vision`, and it is stored in `limits.cache.json`
[mlai.rs:294-300], but nothing reads it. `limits_for` is defined and never called
[mlai.rs:233]. Any agent that attaches a screenshot to `glm`/`forge` gets a bare 400.

Fix: before forwarding, if `vision == false` and any message part is `image_url`, replace it
with the engine's own note ("this model cannot see the image …") or return
`image_unsupported` naming a model that can. Never let the upstream 400 be the answer.

### P1 — The usage-only chunk is forwarded to clients that did not ask for it

`rewrite_request` forces `stream_options.include_usage = true` whenever `alwaysIncludeUsage`
is on (default) [transform.rs:42-50], and `stream_out` forwards every frame it parses.
Observed tail of a streamed tool call:

```
{"choices":[],"usage":{"completion_tokens":5,"prompt_tokens":139,"total_tokens":144}}
```

Clients that assume `choices[0]` exists crash or log errors. Fix: remember whether the
caller asked for usage; count the chunk internally; forward it only when requested.

### P1 — Context window is advertised but never enforced

`/v1/models` reports `context_length` and `max_completion_tokens` from the stored limits
[server.rs:126-158]. `rewrite_request` ignores both. A long agent session therefore runs
into an upstream 400 mid-conversation, which is the worst moment for an opaque failure.

Fix: estimate the prompt (chars/4 is enough for a guard), keep a reserve, and either trim
old tool observations (the engine's `obs_mask` recipe) or return `context_overflow` with the
limit and the current estimate.

### P1 — `max_tokens` is never clamped to the model's output cap

`max_tokens_floor` only raises a value [transform.rs:56-62]; a caller asking for 100k passes
through even though `/v1/models` says 32768. Same fix site as the context guard.

### P2 — Every finished run reports empty usage

`finish_run` is only ever called with `json!({})` [mlai.rs:289,324,339]. The ML AI Code
engine sends `{tokens, calls}`. The bridge's `/usage` is its own local count, but the
server-side dashboard sees zero for bridge runs. Accumulate weighted units per run and pass
them on `finish`.

### P2 — `409 run_finished` is not recovered

The engine treats `409 run_finished` as "this run is closed" and starts a new one. The
bridge has no `409` branch (grep: none), so a closed run is forwarded as an error. Add
`CONFLICT` to the invalidate-and-replay set next to `401`/`402`.

### P2 — Reasoning is exposed in whichever field the config picked

`ReasoningMode::Native` keeps `reasoning_content`; `Reasoning` renames it to `reasoning`;
`Both` duplicates it [transform.rs:92-114]. Most OpenAI-compatible agents ignore unknown
delta keys, so the plan is silently dropped. Pick one canonical field
(`reasoning_content` is the most widely recognised) and document it; keep `None` for clients
that echo deltas into content.

### P2 — Changing effort between requests cools the prompt cache

The engine notes that the GLM endpoint keeps a separate prompt cache per effort level
(`kimi_bridge.py:111`). Because the bridge injects the config default only when absent, two
clients with different defaults (or one client that varies effort) pay full input each
time. Pin effort per (client, model) session.

### Minor

- `title` is the constant `"ML AI Code bridge"` and `project`/`folder` come from config, so
  every run is attributed to one project on the dashboard.
- `mlai-code` is a bridge-only alias: the server rejects it as a run model
  (`model_unknown`), which is correct as long as the alias layer always resolves first.
- Tool-call ids arrive populated in the observed gateway; the bridge does not synthesise
  one if a provider omits it.

## 3. A better translation layer

The pattern behind every finding above: the bridge is a **pass-through** with a few
renames, while the gateway has a capability envelope and its own error vocabulary. Move
from "pass-through" to "capability-aware adapter".

### 3.1 Capability profile per real model

Extend the existing `limits` cache into one record:

```jsonc
{
  "pixis-opus": {
    "context": 1048576, "output": 32768, "vision": true,
    "efforts": ["low", "medium", "high", "max"],
    "reasoning_field": "reasoning_content",
    "tools": true
  },
  "glm-5.3": { "context": 200000, "output": 32768, "vision": false,
               "efforts": ["low", "medium", "high"], "tools": true }
}
```

Fill it from the `start` response (`contextWindow`, `responseReserve`, `vision`) and, for
`efforts`, from a small static table corrected by the probe results above; a rejected
effort can be learned once and memoised.

### 3.2 Request normaliser (one place, ordered)

1. Resolve alias → real model, load its profile.
2. Clamp `reasoning_effort` to `profile.efforts` (default `low`).
3. Clamp `max_tokens` / `max_completion_tokens` to `profile.output`.
4. Vision gate: replace `image_url` parts when `profile.vision == false`.
5. Context guard: estimate prompt; trim oldest tool observations to a budget
   (`context - output - reserve`); if still over, return `context_overflow`.
6. Keep the prefix stable: never rewrite system messages, never reorder `tools`.

### 3.3 Response normaliser

- One canonical reasoning field; never merged into `content`.
- Tool calls: guarantee `id`, `index`, `type: function`; keep arguments as deltas.
- `finish_reason`: pass `tool_calls` / `stop` / `length` through; map provider-specific
  values to those three.
- Usage chunk: forward only when requested; always count internally.
- Error taxonomy: map upstream 400 bodies to `unsupported_effort`, `image_unsupported`,
  `context_overflow`, `max_tokens_too_large`; auto-recover `401`, `402`, `409`; preserve the
  upstream message instead of replacing it.

### 3.4 Protocol adapters (the big unlock)

The bridge only speaks OpenAI Chat Completions. Many coding agents do not:

| agent | wire format | today |
|---|---|---|
| Cline, Roo, Continue, Aider, opencode | OpenAI `/v1/chat/completions` | works |
| Claude Code, some Zed/VS Code agents | Anthropic `/v1/messages` | not supported |
| Codex-style agents | OpenAI `/v1/responses` | not supported |

Add adapters that translate to the internal OpenAI shape:

- **`POST /v1/messages`** — `system` → system message; content blocks
  (`text`, `image`) → parts; `tool_use` → `tool_calls`; `tool_result` → `role:"tool"`
  with `tool_call_id`; map `stop_reason` (`end_turn`/`tool_use`/`max_tokens`); stream as
  `message_start` → `content_block_start` → `content_block_delta` → `content_block_stop` →
  `message_delta` → `message_stop`, with `input_json_delta` for tool arguments.
- **`POST /v1/responses`** — map `input` items and `function_call`/`function_call_output`
  to messages and `tool_calls`, and stream `response.output_text.delta` events.

That is what "translation layer" should mean here: one capability-aware core, several
front doors, and no agent ever seeing a run-level error.

## 4. Suggested order of work

1. Capability profile + effort/max_tokens/vision gates (small, removes every P0).
2. Usage-chunk gating and the `409` retry (small, removes the remaining client-visible
   errors).
3. Context guard (medium; the main long-session failure).
4. `finish` usage reporting (small).
5. Anthropic `/v1/messages` adapter (largest, biggest reach).
