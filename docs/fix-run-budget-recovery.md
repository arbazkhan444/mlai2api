# Fix plan — recover a spent run without restarting the client

Status: implemented in `fix/run-budget-recovery`.

## Symptom

Coding agents pointed at the bridge worked for a while, then every request returned:

```
HTTP 402
{"error":{"code":"billing_limit","message":"Your plan allowance for this run is used up. Upgrade or wait for the allowance to reset.","type":"billing_limit"}}
```

The same model worked without errors in the ML AI Code CLI on the same account.

## Cause (measured, not inferred)

1. `POST /api/laptop {action:"start"}` returns a per-run cap. On this account it is
   `budget: 40000000` (40 credits; 1 credit = 1,000,000 weighted units). The account itself is
   uncapped: `accountTokensLeft: null`.
2. The gateway enforces that cap for the run and answers `402 billing_limit` once it is reached.
   The engine counts fresh input at 1x, cached input at 0.2x and output at 3x.
3. The bridge cached one token per model and only replaced it when `expires - now < tokenSafetyMs`.
   Run tokens live about four hours, so a run that spent its budget in minutes was replayed for the
   rest of its lifetime.
4. There was no `402` handling anywhere in `crates/core` (grep). Only `401 run_token_invalid` caused
   a re-mint.
5. The CLI never hits this because one message is one fresh run, capped at 120 steps, with compaction
   (observed: ~0.32 credits per run).

Observed bridge traffic when this was diagnosed: 2,305 requests, 396,146,365 raw tokens,
134,291,785 weighted units — roughly 3.4 run budgets, spread over ~18 refusals.

## Change

`crates/core/src/mlai.rs`

- `weighted_units(&usage)` — the same weights as the engine, so the local count matches the number the
  server enforces.
- `needs_refresh(&RunToken, used)` — true once `used >= budget - reserve`, where reserve is 10% of the
  budget capped at 4,000,000. A run with no budget is never refreshed.
- `RunPool::consumed` counts weighted units per model since the run was minted; it is reset on mint and
  on invalidate.
- `RunPool::ensure` now treats a run as reusable only while it is unexpired **and** has budget left.
- `/health` reports `consumed` next to `budget` so the state is observable.

`crates/core/src/server.rs`

- `record()` feeds each response's usage into `note_usage`, which is the input to the proactive check.
- The chat handler now retries once. A `401 run_token_invalid` **or** `402 billing_limit` invalidates the
  run, mints a new one, and replays the identical upstream body. Any other error is forwarded unchanged.

## Why the client session survives

The proxy is stateless per call: the coding agent holds the conversation and sends the full message
array every time. The retry happens on the status line, before any byte of the body is forwarded, so a
stream is never partially delivered twice. The client sees one normal `200`; nothing restarts, no
history is lost, and the model/provider/prompt-cache prefix are unchanged.

## Verification

Unit tests (`cargo test -p mlai-bridge-core`, 8 passing):

- weights: 1000 prompt / 800 cached / 100 output = 660 units;
- cached tokens clamped to the prompt;
- absent usage counts nothing;
- uncapped runs never refresh;
- 40M budget refreshes at 36,000,000;
- 1M budget refreshes at 900,000.

End-to-end against a local mock control plane and gateway (no real tokens spent):

| Scenario | Result |
|---|---|
| mock answers `402 billing_limit`, then `200` | bridge minted a second run and replayed; client got `200 MOCK-OK`; mock saw 2 starts, 2 chats |
| mock budget set to 1,000, two client calls | bridge minted before the second call; 2 starts, 2 chats, both `200` |
| `/health` after a 1000/50 response | `consumed: 1150` (= 1000 + 50×3), then reset to 0 on the next mint |

## Operational notes

- `/health` → `runs[]` now shows `budget` and `consumed`; a refresh is visible as a new `runId`.
- A refresh costs one extra `/api/laptop start` and may cool the provider's prompt cache once; the
  client is unaffected.
- If a fresh run is refused too, the error is forwarded — that is a real account or plan state, not a
  stale token.

## Follow-ups (not in this change)

- The `/health` and `/usage` `credits` figure is raw tokens ÷ 1M, while the gateway bills weighted
  units. Reporting weighted units would match the server.
- `tokenSafetyMs` remains the only expiry knob; a `budgetReserveRatio` setting could make the reserve
  tunable if a plan ships a much smaller per-run cap.
