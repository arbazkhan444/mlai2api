# mlai2api

A local, OpenAI-compatible bridge to **ML AI Code**. Point Cline, Roo Code, opencode or any
OpenAI-compatible client at it, and it relays each request through the ML AI Code cloud gateway
using your own account — with model aliases, multiple accounts, token/cache accounting, and a
desktop control panel.

> Unofficial community tool. Not affiliated with, endorsed by, or supported by ML AI Code.

---

## How it works

ML AI Code authorises a "run" per model by minting a short-lived run token instead of handing out a
long-lived API key. This bridge does that minting for you:

1. You add an ML AI Code account (device token) once.
2. On the first request for a model, the bridge calls `POST /api/laptop` with action `start` and keeps the
   run token (re-minting shortly before it expires).
3. It forwards the request to `https://code.ml.ai/api/model/v1/chat/completions` and streams the response
   back unchanged in OpenAI format.

The upstream already speaks OpenAI SSE, so the proxy mainly normalises model names, guarantees a usage
chunk, tracks tokens/cache, and keeps a local record of every request.

---

## Features

- **OpenAI-compatible** `/v1/chat/completions` (streaming and non-streaming), `/v1/models`, `/health`, `/usage`.
- **Model aliases** — expose your own names (`opus`, `glm`, `forge`, …) and map them to upstream models.
- **Multiple accounts** — add several ML AI Code accounts, switch the active one, per-request attribution.
  Tokens are encrypted with **Windows DPAPI**, never stored in plaintext.
- **Usage tracking** — input/output/cached/reasoning tokens, cache hit rate, credits per model and per account.
- **Request inspector** — per-request drawer with the full request/response bodies, timings (duration, first-token),
  reasoning effort, and copy-as-JSON. Bodies are persisted to disk with a bounded, self-pruning store.
- **Desktop panel** (Tauri) with Proxy / Usage / Accounts tabs, plus a headless `serve` mode.
- **Self-testing** — a black-box HTTP suite (`test-rust.mjs`) covering the whole proxy surface.

---

## Requirements

| need | why |
| --- | --- |
| Windows 10/11 x64 | the app and DPAPI account storage are Windows-first |
| WebView2 runtime | bundled in Windows 11; the installer can fetch it on Windows 10 |
| Rust (MSVC toolchain) + VS 2022 Build Tools (C++) | building from source |
| Node.js 18+ | only for the Tauri CLI used by `bundle.cmd` |
| An ML AI Code account | sign in with the `mlai` CLI, or paste a device token |

Run `rustup show` — the default toolchain must be the **MSVC** one (`stable-x86_64-pc-windows-msvc`),
not the standalone GNU toolchain. All build scripts load `vcvars64.bat` for you.

---

## Build

### Debug build (fast loop)

```bat
build.cmd build
```

`build.cmd` loads the Visual Studio environment, points `TEMP` at `%LOCALAPPDATA%\Temp`, and forwards
everything to `cargo`, so `build.cmd test`, `build.cmd run`, etc. also work.

Output: `target\debug\mlai-bridge.exe`.

### Release build + installer

```bat
npm install --save-dev @tauri-apps/cli@^2   :: once
bundle.cmd
```

`bundle.cmd` runs `npx tauri build --ci` with the MSVC environment and produces:

- `target\release\mlai-bridge.exe` — the app
- `target\release\bundle\nsis\MLAI Bridge_<version>_x64-setup.exe` — per-user installer (no admin required)

### Tests

```bat
build.cmd test -p mlai-bridge-core        :: unit tests (account store, DPAPI roundtrip)
build.cmd build -p mlai-bridge-app
.\target\debug\mlai-bridge.exe serve --port 8790
node test-rust.mjs                       :: set BRIDGE_PORT=8790 for a non-default port
```

`test-rust.mjs` exercises auth, model listing, streaming SSE framing, tool calls, usage accounting and cache hits.

---

## Run

### Desktop panel (default)

```bat
mlai-bridge.exe
```

The panel opens on the **Proxy** tab and starts the proxy automatically when a project is configured.
Use **Start / Stop** to control it, and copy the base URL + bridge key into your client.

### Headless proxy

```bat
mlai-bridge.exe serve --port 8787
```

Prints the account in use and listens on `http://127.0.0.1:8787/v1`.

### First-time setup

```bat
mlai-bridge.exe init            :: writes config.json with a new bridge key and discovered project
mlai-bridge.exe doctor          :: checks sign-in, config and project
mlai-bridge.exe self-test       :: drives the GUI's command path without opening a window
```

Config resolution order: `--config <path>` → `%LOCALAPPDATA%\mlai-bridge\config.json` → `./config.json`.

---

## Connect a client

In Cline / Roo Code choose provider **OpenAI Compatible** and fill in:

| field | value |
| --- | --- |
| Base URL | `http://127.0.0.1:8787/v1` |
| API key | the `apiKey` from `config.json` (also shown on the Proxy tab) |
| Model | any alias from `/v1/models` (`opus`, `sonnet`, `gpt-6`, `glm`, `kimi`, `grok`, `forge`, `mlai-code`) |

For opencode use `@ai-sdk/openai-compatible` with `baseURL` set to the same URL and `limit.context` matching
the model (the panel shows real context windows once limits have been learned).

> **Reasoning tokens count against `max_tokens`.** A small `max_tokens` with high reasoning can produce an empty
> answer. The default `reasoningEffort` is `low` for that reason — raise it per request if you need deeper thinking.

---

## Accounts

Open the **Accounts** tab:

- **Import from mlai CLI** — reuses the account already signed in by `mlai login` on this machine.
- **Add account** — paste a device token (`mlaid_...`). It is validated with `whoami` and labelled with the
  account name.
- **Use** — makes an account active and restarts the proxy with its credential.
- **Remove** — deletes the account and its stored token.

Account metadata lives in `accounts.json`; each token is encrypted with **Windows DPAPI** into
`accounts.keys\<id>.bin`, so only a `mlaid_…abcd` hint is stored in plaintext. The Usage tab shows the
active account and can filter requests by account; every request row records the account that sent it.

---

## Usage and spend

The **Usage** tab shows totals and per-model breakdowns: input, output, cached input, reasoning, cache-hit rate and
credits. ML AI Code's unit is **1 credit = 1,000,000 tokens**, so spend is derived as `total_tokens / 1e6`
(the API does not publish per-model prices).

Clicking a row opens a drawer with timestamp, duration, first-token latency, effort, token counts, and the full
request/response bodies. Bodies live in `bodies/<id>.json` and are capped:

| setting | default | meaning |
| --- | --- | --- |
| `bodyKeep` | 200 | keep at most this many body files (oldest pruned first) |
| `bodyMaxBytes` | 67108864 | also prune oldest until the bodies folder fits 64 MB |

Old metadata rows are kept even after their bodies are pruned; the drawer says so instead of inventing data.
Rows written before account attribution existed are shown as **Unattributed**.

---

## Configuration reference

`config.json` (camelCase):

| key | default | meaning |
| --- | --- | --- |
| `host` | `127.0.0.1` | bind address; keep localhost unless you know why |
| `port` | `8787` | proxy port |
| `server` | `https://code.ml.ai` | ML AI Code control plane |
| `apiKey` | generated | bridge key clients must send as `Authorization: Bearer …` |
| `project` / `folder` | discovered | run context sent when minting a run token |
| `runTask` | `Local model bridge session` | task title on the run card |
| `defaultModel` | `mlai-code` | model used when a client omits `model` |
| `aliases` | see above | client model name → upstream model id |
| `reasoning` | `native` | how to surface reasoning: `native` / `reasoning` / `both` / `none` |
| `reasoningEffort` | `low` | sent upstream when the client does not specify one |
| `tokenSafetyMs` | `300000` | re-mint run tokens this long before expiry |
| `requestTimeoutMs` | `900000` | upstream timeout |
| `historyPath` | auto | `requests.jsonl` metadata log |
| `limitsCache` | auto | learned context/output limits |
| `accountsPath` | auto | `accounts.json` |

Data location: `%LOCALAPPDATA%\mlai-bridge` when writable, otherwise the working directory
(config, accounts, `requests.jsonl`, `bodies/`, limits cache).

### HTTP endpoints

| endpoint | purpose |
| --- | --- |
| `POST /v1/chat/completions` | OpenAI-compatible chat (alias `/chat/completions`) |
| `GET /v1/models` | alias list with learned `context_length` / `max_completion_tokens` |
| `GET /usage` | aggregate usage JSON (stats, per model, per account) |
| `GET /health` | status, run pool, limits known, version |

---

## Project layout

```
crates/core/      proxy engine: config, auth, accounts (DPAPI), mlai client/run pool,
                  request rewriting + SSE normalisation, storage
crates/app/       Tauri desktop panel + CLI entry point
ui/               static panel (no build step): index.html, app.js, styles.css
test-rust.mjs     black-box HTTP suite
build.cmd         cargo wrapper with the MSVC environment
bundle.cmd        release build + NSIS installer
```

---

## Troubleshooting

- **`401 invalid_api_key`** — the client is sending the wrong bridge key; copy it from the Proxy tab.
- **`404 model_not_found`** — the alias does not exist and raw model ids are disabled; check `/v1/models`.
- **Empty answers** — reasoning consumed `max_tokens`; lower effort or raise the limit.
- **`No active account`** — add one on the Accounts tab, or run `mlai login` and import it.
- **Linker error about `x86_64-w64-mingw32-clang`** — you are invoking `cargo` directly instead of `build.cmd`
  (wrong toolchain on PATH).
- **WebView2 "Access is denied" on launch** — the app creates its profile in a writable directory; ensure
  `%LOCALAPPDATA%` is writable for your user, or run from a writable folder.
- **Port already in use** — stop the other instance or run `serve --port <n>`.

---

## License

MIT — see [LICENSE](LICENSE).

Copyright (c) 2026 Arbaz Khan.

## Disclaimer

This project talks to ML AI Code's private endpoints on your behalf, using your own account and plan.
Use it only with an account you are authorised to use, and respect the provider's terms of service.
The protocol may change without notice.