# MLAI Bridge (desktop)

Rust + Tauri control panel for the local OpenAI-compatible bridge to ML AI Code.
The proxy, run-token pool, model aliases, SSE normalization and request history live in
`crates/core`; the Tauri shell only edits config and drives the proxy lifecycle.

## Build

The machine's PATH points at a standalone gnullvm Rust, so builds go through `build.cmd`,
which loads the VS 2022 BuildTools environment and uses rustup's MSVC toolchain:

```
build.cmd build -p mlai-bridge-app      # debug exe: target/debug/mlai-bridge.exe
build.cmd build -p mlai-bridge-app --release
```

## Run

```
target/debug/mlai-bridge.exe            # opens the control panel, proxy is started from the UI
target/debug/mlai-bridge.exe init       # write ~/.mlai-bridge/config.json (key + project id)
target/debug/mlai-bridge.exe doctor     # sign-in, project and catalog check
target/debug/mlai-bridge.exe serve      # headless proxy for scripts / CI
```

Credentials are the ones the `mlai` CLI already stored in `~/.mlai` (or `MLAI_TOKEN`).

## What the panel does

- Start/stop the proxy and copy the base URL + bridge key.
- Edit host, port, server, project, folder, run task, default model, reasoning effort.
- List model aliases with the real context window and output reserve (Read limits mints one run per model).
- Live request log with status, latency and token counts, persisted in `~/.mlai-bridge/history.redb`.

## Layout

```
crates/core/src/config.rs   config.json (same field names as the Node bridge)
crates/core/src/auth.rs     ~/.mlai device credential
crates/core/src/mlai.rs     control plane + run-token pool (one token per model)
crates/core/src/transform.rs model aliases + OpenAI wire normalization
crates/core/src/server.rs   axum routes, SSE passthrough
crates/core/src/store.rs    redb history store
crates/app/src/gui.rs       Tauri commands
ui/                         static panel (no build step)
```
## Tabs and usage

The panel has two tabs: **Proxy** (connection, config, model limits) and **Usage** (token accounting).
Usage is recorded per request in `requests.jsonl` and aggregated per model alias.

| field | source |
| --- | --- |
| input | `usage.prompt_tokens` |
| output | `usage.completion_tokens` (includes reasoning tokens) |
| cached input | `usage.prompt_tokens_details.cached_tokens` |
| reasoning | `usage.completion_tokens_details.reasoning_tokens` |
| spend | `total_tokens / 1_000_000` credits |

**Caching is real and reported**, but only by some models: `pixis`/`pixis-opus` return
`prompt_tokens_details.cached_tokens` (a repeated ~1.8k-token prompt reported 1812 cached of 1813
prompt tokens), while `glm-5.3` returns totals only. The Usage tab shows cached input, a per-model
hit rate, and treats missing details as unknown rather than zero.

Spend uses ML AI Code's own unit: **1 credit = 1,000,000 tokens** (`TOKENS_PER_CREDIT` in the app's
`tui/chat.mjs`). The API exposes no per-model USD price, so credit totals are derived from tokens.

`GET /usage` on the proxy returns the same aggregate as JSON for scripts.
## Request bodies and the detail drawer

Every proxied call stores the exact upstream request body and the response body, plus timings,
so the Usage tab can show a per-request drawer that survives restarts:

- metadata row in `requests.jsonl` (`effort`, `firstTokenMs`, `hasBody`, token breakdown)
- bodies in `<history dir>/bodies/<id>.json` as `{ request, response }`

For streamed calls the response is the assembled completion (`content`, `reasoning`, `toolCalls`,
`finishReason`, `usage`, `chunks`) plus the raw SSE up to 512 KB; JSON responses are stored as-is.
A single body is capped at 8 MB and replaced by a preview beyond that.

**Disk usage is bounded.** Two settings in `config.json` control it:

| key | default | meaning |
| --- | --- | --- |
| `bodyKeep` | 200 | keep at most this many body files, oldest pruned first |
| `bodyMaxBytes` | 67108864 (64 MB) | also prune oldest until the bodies folder fits |

Pruning runs at startup and after every write; metadata rows are kept separately (last 5000 in
memory, rotated on disk), so history still lists requests whose bodies were pruned — the drawer
says the body is no longer available instead of inventing one.

The table also filters by model and effort, and each row shows input/output/cached/reasoning tokens.
## Accounts

The Accounts tab manages several ML AI Code accounts (device tokens). Metadata lives in
`accounts.json`; each token is encrypted with **Windows DPAPI** into `accounts.keys/<id>.bin`,
so the token is never stored in plaintext (the config only holds the bridge key). Only a
`mlaid_…abcd` hint is kept for display.

- **Add account** validates the pasted token with `whoami` and uses the returned name as the label.
- **Import from mlai CLI** reuses the account already signed in on this machine.
- **Use** makes an account active and restarts the proxy with its credential.
- Requests are signed with the active account and every row records `accountId` / `accountLabel`.
- Rows written before attribution existed deserialize as **Unattributed** (serde defaults), so old
  history keeps working and can be filtered rather than disappearing.

The Usage tab shows the active account and can filter requests by account, model or effort.
## Build a release installer

```
npm install                # pulls @tauri-apps/cli
bundle.cmd                 # release build + NSIS installer
```

`bundle.cmd` loads the MSVC environment (vcvars64) and runs `npx tauri build --ci`.
Outputs:

- `target\release\mlai-bridge.exe` — the app
- `target\release\bundle\nsis\MLAI Bridge_<version>_x64-setup.exe` — the installer

The installer is per-machine and bundles the WebView2 bootstrapper. It installs
`MLAI Bridge.exe` plus an uninstaller; settings, accounts, history and request bodies live in
`%LOCALAPPDATA%\mlai-bridge\` (config, accounts, requests.jsonl, bodies/) when writable, and fall
back to the working directory otherwise.
