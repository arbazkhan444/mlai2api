# Local / desktop tools close to MLAI Bridge: UX research

Scope: local AI proxies/gateways, local model servers, desktop proxy clients, HTTP inspectors.
Focus areas: tray, autostart, start/stop & port conflicts, live log/traffic view, account/token pools with health and quota, "copy config for client X", first-run, dark mode, updates.
Everything below is sourced; anything I guessed is marked **[INFERENCE]**.

---

## 1. One API (songquanpeng/one-api)

- **What it is:** self-hosted multi-provider OpenAI-compatible gateway with a web admin. Source: <https://github.com/songquanpeng/one-api>
- **Patterns**
  - **Channels** (one upstream key = one channel) with load balancing across channels, per-channel model list, model mapping, auto-retry on failure. (README feature list, same URL)
  - **Tokens** with expiry, quota, allowed IP ranges and allowed models: the client-facing key is separate from upstream keys. (same)
  - **Quota detail view** with amounts shown in USD. (same)
  - Theme switching through the `THEME` env var, not a UI toggle. (same)
- **Why it matters for us:** it set the pattern most later tools copy: channel/key list, test button, logs. Our "accounts" map onto channels and our "proxy API key" maps onto a token.
- **Complaint/limit:** model mapping rebuilds the request body instead of passing it through, so the README warns that fields the gateway doesn't know yet may be dropped. (README item 15)

## 2. New API (QuantumNous / Calcium-Ion/new-api)

- Repo/README: <https://github.com/Calcium-Ion/new-api>
- **Channel list** shows name, type, **status (green active / red disabled)**, **response time**, **used quota**. Each row has a **Test** button (dialog with latency + success/fail), plus **"Test All Channels"** for one-click batch tests. Batch enable/disable/tag appears in a toolbar once rows are checked. <https://docs.newapi.ai/en/docs/guide/feature-guide/admin/channel>
- **Auto-disable** after consecutive failures; **multi-key mode** in one channel (round-robin or weighted random). A failed key is skipped and comes back when it recovers. (same)
- **Logs page:** a collapsible Filter button with time range, username, model, channel and token name, a summary stats panel above the table, and a dashboard trend chart with hover detail. <https://docs.newapi.ai/en/docs/guide/feature-guide/admin/log>, <https://docs.newapi.ai/en/docs/guide/console/usage-log>
- **"Chat" one-click import:** in the key row, `… → Chat → <target app>` launches the installed desktop app (e.g. Cherry Studio) through its custom URL scheme and pre-fills base URL + key. A manual fallback table (Base URL / Key / Models) covers apps without a scheme. The FAQ admits the scheme can fail: "Desktop application not launching… confirm OS registered the custom protocol". <https://www.newapi.ai/en/docs/guide/feature-guide/user/chat-apps>
- Recent release added a **Sankey "traffic flow" chart** for token routing with persistent filters. <https://github.com/Calcium-Ion/new-api/releases/latest>
- **Takeaway:** per-account row = status dot + latency + used quota + Test. Add a Test-all button and a deep link or copy button per client.

## 3. CLIProxyAPI + Management Center (web UI) + EasyCLIProxyAPI (desktop)

### 3a. Management Center (web UI bundled at `/management.html`)
- <https://github.com/router-for-me/Cli-Proxy-API-Management-Center>
- Three pillars: **Configure / Connect / Observe**. Settings can be edited visually or as **YAML with a diff preview before saving**. **Quotas page** per provider (Claude, Antigravity, Codex, Kimi, xAI). **Logs** with auto-refresh, search, a **"hide management traffic"** toggle and **download request error logs**. **System** page: check for updates, list available models, clear local login data. (same)
- Clear separation of the **management key** (UI login) from the **client keys** that authorize proxy requests, called out as "not interchangeable". (same)
- **Troubleshooting written into the README:** the Logs page is missing until "Logging to file" is on, and `/v1/models` needs at least one proxy API key. (same)
- OAuth via the web UI works only for local instances. <https://help.router-for.me/hands-on/tutorial-6>

### 3b. EasyCLIProxyAPI (formerly EasyCLI), a Tauri + React + Rust desktop app
- <https://github.com/router-for-me/EasyCLI>
- **Home dashboard:** start / stop / restart / refresh core; **install state, runtime state, PID, listening port**; **copy ready-to-use OpenAI / Claude / Gemini endpoints**; a local connectivity check; app + core version. (README "Home dashboard")
- **Agents page (very relevant):** detects installed clients (Claude Code, Codex, OpenCode, Claude Desktop, Kimi Code, …), syncs the model catalog, lets you pick a default model, **backs up the client's original config before applying, and can restore it**. Each client remembers its config directory, including WSL UNC paths. (README "Agent client configuration")
- **Usage page:** totals, tokens, success rate, throughput, **cache hit rate, estimated cost**; filter by time/model/provider/source/key/result; input/output/reasoning/cache breakdown; request details. (README "Usage history")
- **Updates:** checks once in the background at startup and again when you open Version Management. Checks never auto-download. Background download and install on next launch is opt-in (off by default) and **rolls back automatically if the new version fails to launch**. (README "Upgrading", <https://github.com/router-for-me/EasyCLI/blob/main/docs/automatic-updates.md> listed in repo)
- **Tray:** "Keep the application available from the macOS menu bar or Windows system tray." (README)
- **Complaints:**
  - Fixed OAuth callback port 1455 falls inside **Hyper-V reserved port ranges** on Windows, so login fails with "port in use"; users asked for a fallback random port. <https://github.com/router-for-me/CLIProxyAPI/issues/793>
  - "Address already in use" restart loop. <https://github.com/router-for-me/CLIProxyAPI/issues/1146>
  - Host setting in config ignored. <https://github.com/router-for-me/EasyCLIProxyAPI/issues/76>
- **Takeaway:** the EasyCLI home card (state + PID + port + copy endpoints + connectivity check) is almost exactly our Proxy tab. Its Agents page shows the safe way to write client configs: back up first, offer restore.

## 4. claude-code-router (CCR): desktop app + `ccr ui`

- <https://github.com/musistudio/claude-code-router>, CLI README <https://github.com/musistudio/claude-code-router/blob/main/packages/cli/README.md>
- **Onboarding sequence** stated in the UI/docs: Providers → API Keys → (Routing) → **Server: confirm gateway running** → point client at the URL shown. The desktop app offers **tray, desktop notifications, automatic app updates**; the CLI/web version does not. (CLI README)
- **Port conflict:** "If the preferred management port is occupied, CCR tries the next available ports and **prints the actual URL**." Its troubleshooting entry "The management URL changed ports" spells this out. (CLI README)
- **Agent Config page:** choose Claude Code / Codex / OpenCode / …, pick a model, **apply profile**. **Logs** show the resolved provider, model, status, tokens, latency and errors. (main README Quick Start)
- **Observability claims:** request & response details, resolved provider/model/credential, latency, tokens, estimated cost, tool calls. (main README "Core capabilities")
- **Complaints (useful anti-patterns):**
  - **Silent takeover of global client configs** (`~/.claude/settings.json`, `~/.codex/config.toml`) with no opt-out switch. Hand edits are overwritten on the next apply, and managed state is never shown in the UI. The maintainer agreed to "warn before enabling global, surface the managed state, provide release/restore". <https://github.com/musistudio/claude-code-router/issues/1575>; related <https://github.com/musistudio/claude-code-router/issues/1602>
  - **Auth failure is shown as the onboarding wizard.** Opening the UI without a token shows "Get Started", and saving from it can **wipe the existing config**. Every startup RPC swallows its errors. <https://github.com/musistudio/claude-code-router/issues/1832>
  - Saving from the UI overwrites env-variable interpolation in config. <https://github.com/musistudio/claude-code-router/issues/1168>
  - Blank, unresponsive window after an upgrade. <https://github.com/musistudio/claude-code-router/issues/1576>
- **Takeaway:** only show first-run when the backend *confirms* no config exists, and keep "can't reach backend" as its own error state. Never write client configs silently.

## 5. AIClient-2-API (justlovemaki)

- <https://github.com/justlovemaki/AIClient-2-API>
- Web UI management console (added 2025-11-11) for "**real-time configuration management and health status monitoring**". Account pool with "multi-account polling, intelligent failover, automatic degradation". A per-node health check action exists in the provider modal. Theme switching was added 2026-01-03. (README changelog; commit <http://forgejo-ulq90rfwbtp6njof007fkowr.91.98.39.120.sslip.io/admin-valentin/AIClient-2-API/commit/a497aaaf19dc6654cc5643868a51782881bdecd5> is a fork mirror, cited only as corroboration)
- Pool manager: async refresh queue, warm-up, automatic expiry detection of tokens. (README 2026.01.15)
- `GET /health` returns status, timestamp, provider. <https://aiproxy.justlikemaki.vip/en/docs/api-usage/health-check.html>
- Provider pool docs: <https://aiproxy.justlikemaki.vip/en/docs/advanced-config/provider-pools.html>
- **Takeaway:** check token expiry in the background and flag it on the account row before requests start failing.

## 6. Antigravity Manager / "Antigravity Tools" (lbjlaq), Tauri v2 + React

- <https://github.com/lbjlaq/Antigravity-Manager>, site <https://antigravitymanager.com/>
- **Account table:** avatar + email + plan badge (Pro/Ultra/Free), **per-model quota percentages** (Gemini 88%, Claude 100%), **last used**, a Select action; filter chips by plan; search. **Overview tiles:** saved accounts, average quota per model family, a **"low-quota accounts" count**, current account, and a **recommended account (most remaining quota)**. (site demo)
- **Quota semantics** done carefully: separate **5-hour vs weekly windows**, with "combined = the lower value"; **reset time** and **last refresh time** shown; **missing data must not be shown as 0%** ("Refresh the account and check any reported error before treating the quota as exhausted"). <https://antigravitymanager.com/guide/quotas/>
- **Health badges:** *Suspected Throttled → In QoS Cooldown → Probing Recovery → On Probation*, plus a **Release** button that clears the local penalty. (same)
- **Scheduling:** Balance / Cache-First / Performance modes; **Fixed Account Mode**; **Quota Protection** (keep a reserve %); spillover cap; "Clear Session Bindings". (same)
- **Which account served the request:** response headers `X-Account-Email`, `X-Mapped-Model`, also visible in the in-app monitor. (same)
- **Takeaway:** treat quota as data that goes stale. Show "updated 3 min ago · resets in 2h" and use an explicit "unknown" state instead of 0%. Show the serving account in each request-log row.

## 7. Kiro proxies (kiro2api, Kiro-Go, kiro-account-manager)

- xwteam/kiro2api usage guide: <https://github.com/xwteam/kiro2api/blob/main/docs/en/USAGE.md>
  - **Dashboard:** uptime live counter, version, **account pool active count + health**, **global credits aggregated**, current load-balancing mode.
  - **Update check:** silently checks GitHub on load; the button turns into **"Update to vX"**, and the dialog shows **release notes in the UI language** plus a copyable upgrade command. It "only informs, never runs the upgrade automatically".
  - **Batch account import with live per-row verification:** progress bar and "processing i/N", **success / duplicate / failed counts**, and each row goes pending → checking → verified. Dead accounts are rolled back automatically, accounts are **deduped by refresh token**, and the dialog can't be closed mid-import while verified rows save immediately.
  - Account actions: enable/disable/**reset cooldown**/edit priority & weight/**check balance**/remove; changes apply without restart.
  - **Logs:** **SSE live stream**, structured table, filter by direction (request/response/error), search, pagination, **download .txt**.
  - **Model Test page:** send a test through the relay with the user's own key.
  - **Integration snippets:** copy ready-made examples per protocol × language.
  - Top bar: restart service, **theme toggle**, language.
  - **Security self-criticism:** masking is browser-only; the endpoint returns the master key in plaintext. A warning not to paste it into issues.
- Kiro-Go: multi-account pooling, auto token refresh, web admin. <https://github.com/Quorinex/Kiro-Go>
- kiro-account-manager (desktop): one-click switch + quota monitoring. <https://github.com/hj01857655/kiro-account-manager>
- Complaint: a request for a Web Admin Panel on jwadow/kiro-gateway, i.e. users expect a GUI. <https://github.com/jwadow/kiro-gateway/issues/144>

## 8. LM Studio (Developer tab / local server)

- <https://lmstudio.ai/docs/developer/core/server>: a **"Start server" toggle** in the Developer tab; the same is available via `lms server start`.
- **Settings:** port, require authentication, serve on local network, CORS, JIT loading, auto-unload. <https://lmstudio.ai/docs/developer/core/server/settings>
- **Headless/autostart:** an app setting "run the LLM server on login". When it's on, **closing the app minimizes it to the tray** and the server keeps running. **The last server state is saved and restored on launch.** <https://lmstudio.ai/docs/developer/core/headless>
- **Logs:** model vs server log sources; `lms log stream --source server` shows "Received request: POST to /v1/chat/completions" with the body. <https://www.simplified.guide/lm-studio/log-view>
- **Complaints:**
  - 0.3.6 began **truncating request bodies in dev logs** ("… …"), breaking users' tooling; an opt-out was added later. <https://github.com/lmstudio-ai/lmstudio-bug-tracker/issues/300>
  - "Port 1234 already in use" is common. The port is sticky ("last used port"), and the guide tells users to run netstat/PowerShell themselves to find the owner, so the app doesn't name the conflicting process [INFERENCE from the guide's existence]. <https://automatalabs.ca/blog/lm-studio-port-1234-already-in-use-windows>
- **Takeaway:** keep request/response bodies untruncated in the detail drawer (with an optional "truncate in list" setting). When the port is busy, name the owning process and offer the next free port.

## 9. Ollama desktop app (Windows)

- Runs in the background as a tray app with the API on `localhost:11434`. Logs live in `%LOCALAPPDATA%\Ollama` (`app.log`, `server.log`, `upgrade.log`). <https://docs.ollama.com/windows>
- **`ollama launch claude|opencode|codex`** configures and starts the coding agent: "No environment variables or config files needed"; `--config` configures without launching. <https://ollama.com/blog/launch>, integrations index <https://docs.ollama.com/integrations>
- **Complaints (strong signal):**
  - **Auto-starts at login with no in-app toggle.** Users uninstall over it ("invasive", "user-hostile"), and killing the process makes it **respawn**, which then blocks `ollama serve` with a **port-in-use bind error**. <https://github.com/ollama/ollama/issues/8783>; feature request for a settings toggle <https://github.com/ollama/ollama/issues/14375>; older <https://github.com/ollama/ollama/issues/162>
  - Request to hide the tray icon. <https://github.com/ollama/ollama/issues/11063>
  - Tray icon not right-clickable on Windows. <https://github.com/ollama/ollama/issues/12925>
  - Settings that matter (context length, flash attention, parallel) aren't in the UI; integration settings are spread "across a SQLite DB, config.json, and the target app's own config". <https://github.com/ollama/ollama/issues/11310>
- **Takeaway:** autostart must be opt-in and visible in Settings, and Quit must actually quit.

## 10. Jan (local API server)

- <https://docs.jan.ai/desktop/api-server>
- Settings → Local API Server: choose a default model, click **Start Server**, and **Server Status** shows running. The base URL `http://127.0.0.1:1337/v1` is displayed. A **Configuration** button holds host (127.0.0.1 vs 0.0.0.0 with a warning), port, API prefix, **API key (empty = no auth, "not recommended when binding to 0.0.0.0")**, trusted hosts, request timeout, CORS, and **verbose server logs (on by default)** with a "Server Logs" view.
- The docs carry a **troubleshooting table keyed by HTTP symptom**: connection refused / 401 / 404 (wrong model id or prefix) / CORS. (same)
- **Takeaway:** a short "if your client says X, check Y" helper under the connect card.

## 11. Cherry Studio (provider settings)

- <https://cherryai.com/docs/en/pre-basic/settings/providers/>
- Per provider: **API Key** field with a **"Get API Key"** link; multiple comma-separated keys used round-robin. **API Host** (root only; the app appends `/v1/chat/completions`). **Sync models** fetches the model list; manual + exists for custom models. **Model Check** next to the key field tests connectivity.
- Cherry Studio has a **built-in New API provider**, where one address covers OpenAI Chat/Responses, Anthropic and Gemini. <https://docs.cherryai.com.cn/pre-basic/providers/newapi>
- **Complaints:**
  - Its local OpenAI-compatible API doesn't rotate comma-separated keys while the UI chat does, causing 403s. <https://github.com/CherryHQ/cherry-studio/issues/14344>
  - Users asked to retry automatically with the next key when one fails. <https://github.com/CherryHQ/cherry-studio/issues/9450>
  - A key-verification check passed, but chat still failed with "Authentication failed", so the health check didn't match real use. <https://github.com/CherryHQ/cherry-studio/issues/13592>
- **Takeaway:** the account "Check" should send a real minimal request through the same path the proxy uses, not a different endpoint.

## 12. Clash Verge Rev (Tauri 2, mihomo core)

- <https://github.com/clash-verge-rev/clash-verge-rev>: dark/light previews, **custom theme color**, **custom tray icon**, CSS injection.
- **Sidebar:** Proxies, Profiles, Connections, Rules, Logs, Test, Settings, with **core version + running status pinned at the bottom of the sidebar**. <https://clash-dev.com/en/blog/articles/clash-verge-rev-ui-tour.html>
- **Connections tab:** one row per live connection (process, src, dst, matched rule chain, live up/down speed, total traffic) with close-one / close-all. **Logs tab:** level dropdown, search box, **pause / clear**. (same)
- **Tray menu** (source): Dashboard, mode switches (Rule/Global/Direct), profiles, proxies, system proxy, TUN, close all connections, **lightweight mode**, **copy env**, open config/core/logs dirs, **app log / core log**, restart core, restart app, version, exit. Left-click behaviour is configurable (`tray_event`: system proxy / TUN / main window / tray menu). <https://github.com/clash-verge-rev/clash-verge-rev/blob/dev/src-tauri/src/core/tray/menu_def.rs>; `theme_mode: light|dark|system`, `env_type`, `start_page` in <https://raw.githubusercontent.com/clash-verge-rev/clash-verge-rev/v2.4.7/src-tauri/src/config/verge.rs>
- **"Copy environment variables" from the tray**, with the shell type (PowerShell / Bash / CMD) chosen in settings. <https://clashverge.lefly.cloud/faq/other.html>
- **Autostart + silent start** (no window on launch) + close-to-tray are separate toggles; guides stress that "close" ≠ "quit" and that the OS startup list must agree with the in-app switch. <https://clashhelp.com/en/blog/articles/clash-verge-rev-auto-start-tray-minimize-windows-macos-2026.html>
- **Complaints / FAQ:**
  - **Traffic graph caused high GPU** on high-refresh screens; fixed by capping it at 30 fps and offering a toggle to turn the graph off.
  - **Blank UI when the controller port is taken.**
  - **Logs filling disk**: fixed with a log level setting and auto-cleanup interval.
  - All three from <https://clashverge.lefly.cloud/faq/other.html>
- **Takeaway:** live charts need an fps cap and an off switch. A port-in-use error must not render as a blank page. Log retention needs a setting.

## 13. v2rayN

- <https://github.com/2dust/v2rayN>
- **Status bar** at the bottom of the main window: inbound ports, **live proxy and direct speeds**, current server, routing selection, system proxy mode, TUN toggle. **Tray** menu: show/hide window, import from clipboard, update subscriptions, **copy proxy command to clipboard**, system-proxy mode switch. <https://deepwiki.com/2dust/v2rayN/4.6-status-bar-and-system-tray>
- **Tray icon changes with system-proxy state**, and users can swap in custom icons (`NotifyIcon1-4.ico`). <https://github.com/2dust/v2rayN/wiki/Faq>
- FAQ: "run core failed" is usually caused by the **local listen port being taken by another app (e.g. VMs / Hyper-V)**. It also separates **GUI logs** from **core logs**. (same)
- **Takeaway:** state shown in the tray icon itself (running / stopped / error) is the pattern users rely on most. Keep a compact always-visible status strip.

## 14. HTTP Toolkit

- View page: <https://httptoolkit.com/docs/reference/view-page>
  - **Split pane:** exchange list on the left, detail "cards" on the right, with a draggable divider.
  - **Rows:** colour marker, method, status (spinner while pending, 🚫 aborted), source icon, host + path.
  - **List footer:** search with **structured filters** (`status!=200`, `bodySize>=1000`, `header[Authorization]^=Bearer`, `body*=error`, `completed/pending/errored`), autosuggest from captured values, "N shown of M", **pause/resume capture**, export/import HAR, clear, **scroll to bottom**.
  - Context menu: pin, copy URL, **"Filter traffic like this"**, copy as snippet (format remembered).
  - Body cards: Monaco viewer, **expand to full pane**, formatter dropdown (JSON, JSON records/NDJSON, …), byte count.
- Intercept page: <https://httptoolkit.com/docs/reference/intercept-page/>
  - **Grid of one-click "connect this client" tiles** that light up only when the tool is installed.
  - **"Existing Terminal"** shows a copyable one-liner for Bash/Fish/PowerShell.
  - **"Connected sources" panel** listing clients actually seen.
  - Note: "a client can be correctly configured but not appear until it sends a request".
- **Takeaway:** our "connect a client" card could be a tile grid (Cline, Roo, opencode, Claude Code, Cursor, curl). The request log gets structured filters, "pending" spinners, pause and scroll-to-bottom.

## 15. Proxyman

- **Filter bar** (⌘F): primary chips (HTTP/HTTPS/JSON/…), **status chips 2xx/4xx/5xx**, multi-select; secondary filter of field × operator (contains / regex / `>=`…) including **Duration (ms)** and **body size**. <https://proxyman.com/docs/basic-features/content-filter>
- **Copy as**: URL, **cURL (⇧⌘C)**, headers/body/cookies, Markdown table, CSV. <https://proxyman.com/docs/basic-features/copy-as>
- **Compose**: rebuild a request from cURL and resend. <https://proxyman.com/docs/advanced-features/compose>
- **Takeaway:** status-class chips plus a duration filter fit our request log, and "Copy as cURL" belongs in the request drawer next to "copy as JSON".

## 16. Charles

- SSL proxying, throttling, breakpoints, **JSON shown as a tree**, and **auto-configuring the system/browser proxy**. <https://www.charlesproxy.com/overview/features>, breakpoints <https://www.charlesproxy.com/documentation/proxying/breakpoints/>
- HN users praise its "really nice UI for looking at JSON responses… a collapsible tree is invaluable". <https://news.ycombinator.com/item?id=8778596>
- **Takeaway:** a collapsible JSON tree for request/response bodies in the drawer.

---

## Client config facts we would generate (for "copy config for X")

| Client | What it needs | Source |
|---|---|---|
| Cline | Provider "OpenAI Compatible", Base URL, API Key, Model ID, optional context window / max tokens / prices | <https://docs.cline.bot/provider-config/openai-compatible> |
| Roo Code | Same three fields (OpenAI Compatible); native tool calling required | <https://docs.roocode.com/providers/openai-compatible> |
| opencode | `opencode.jsonc` → `providers.<id>` with `package: @opencode/ai/providers/openai-compatible`, `settings.baseURL`, `env: [KEY]`, `models` map | <https://opencode.ai/docs/providers> |
| Claude Code | `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN` (Bearer) or `ANTHROPIC_API_KEY` (x-api-key); shell export (bash / PowerShell) or `~/.claude/settings.json` `env` block; VS Code uses `claudeCode.environmentVariables`; verify with a curl to `/v1/messages`; `/status` shows the base URL | <https://code.claude.com/docs/en/llm-gateway-connect> |
| Any shell | Copy env vars for PowerShell / bash / cmd (Clash Verge pattern) | <https://clashverge.lefly.cloud/faq/other.html> |

[INFERENCE] Claude Code speaks the Anthropic Messages API. If our proxy only serves `/v1/chat/completions`, the Claude Code tile should be hidden or marked unsupported rather than emit a config that 404s.

---

## Recurring user complaints (cross-product)

1. **Silent autostart / no off switch / respawn after kill.** Ollama #8783, #14375.
2. **Silent rewriting of client config files with no visible ownership or restore.** CCR #1575, #1602.
3. **Misleading empty/onboarding state when the backend is actually unreachable or auth failed; saving from it destroys config.** CCR #1832.
4. **Port conflicts with no guidance** (Hyper-V reserved ranges, a sticky last-used port, another instance). CLIProxyAPI #793/#1146, LM Studio guide, v2rayN FAQ, Clash FAQ.
5. **Truncated logs** that hide the very data devs need. LM Studio #300.
6. **Health checks that don't match real traffic** (key passes check, chat fails; rotation only in one code path). Cherry #13592/#14344.
7. **Live charts burning GPU; logs filling disk.** Clash Verge FAQ.
8. **Key masking only in the UI while plaintext is still exposed.** kiro2api USAGE warning.
9. **Settings that matter hidden outside the app** (env vars, sqlite). Ollama #11310.

---

## Top 15 patterns for MLAI Bridge (ranked by value / effort for our users)

1. **Status-in-tray + tray menu.** The tray icon changes for running / stopped / error (v2rayN, Clash). Menu: Start/Stop, Copy base URL, Copy API key, Copy env for PowerShell/bash, Open request log, Open logs folder, Quit. Close-to-tray stays separate from Quit (LM Studio, Clash).
2. **Explicit, opt-in autostart + "start proxy on launch" + "start hidden".** Three separate visible toggles; restore last running state on launch (LM Studio). Never silently register startup (Ollama backlash).
3. **One status header card** (EasyCLI home): state pill, `127.0.0.1:<port>`, uptime, active account, version, a **"Test connection"** button that sends one real tiny request through the proxy and shows latency/result, plus copy buttons.
4. **Port-conflict UX.** On bind failure show "Port 8080 is used by `<process.exe>` (PID n)", with buttons **Use next free port (8081)** / **Change port** / **Retry**. Detect Windows reserved ranges (Hyper-V) explicitly (CLIProxyAPI #793, LM Studio, CCR "tries the next port and prints actual URL").
5. **Client tiles: "Connect Cline / Roo Code / opencode / Claude Code / Cursor / curl".** Each tile opens a snippet with the real base URL, key and model filled in, plus copy (HTTP Toolkit intercept grid, kiro2api integration snippets, New API chat import). Copy only by default. Any "apply to config file" must **back up first, show exactly what it writes, and offer Restore** (EasyCLI agents vs CCR #1575).
6. **Request log = inspector-grade list.** Live-append with a pending spinner, status chips (2xx/4xx/5xx), filters for account/model/effort/duration, **pause/resume**, **"N of M shown"**, auto-scroll toggle / jump-to-latest, and **"filter like this"** from the context menu (HTTP Toolkit, Proxyman, kiro2api SSE logs, Clash logs pause/clear).
7. **Request drawer upgrades.** Collapsible JSON tree + raw toggle (Charles), **never truncate stored bodies** (LM Studio #300), expand body to full pane, **Copy as cURL** in addition to JSON (Proxyman), with the serving account and mapped upstream model shown at the top (Antigravity headers).
8. **Account rows with health + quota freshness.** Status dot, latency, credits/quota bar, **"updated 3 min ago · resets in …"**, an explicit **unknown** instead of 0% (Antigravity), and badges for rate-limited/cooldown with a Release/Reset action (Antigravity, kiro2api reset cooldown).
9. **"Check" per account and "Check all".** The check sends a minimal real request through the same code path (New API Test/Test All; Cherry #13592 shows why a different path misleads).
10. **Token expiry/refresh warnings.** Background check that flags "token expires in 2 days / expired: re-add" on the account row and tray before requests fail (AIClient-2-API expiry detection, Kiro auto refresh).
11. **Honest first-run onboarding.** Three steps (add account by device token or import from mlai CLI → start proxy → connect a client tile), shown **only when the backend confirms an empty config**. Backend unreachable gets its own error screen (CCR #1832).
12. **Usage overview tiles.** Tokens in/out/cached/reasoning, cache-hit %, credits, success rate, with one trend chart per time range (EasyCLI Usage, New API dashboard). **Cap the chart fps / allow disabling live charts** (Clash GPU FAQ).
13. **Update notification that only informs.** Silent check on launch → the header/sidebar shows "Update to vX", which opens a dialog with release notes and "Install & restart". Never install without asking (kiro2api, EasyCLI). Tauri's updater plugin covers this ([tauri.app/plugin/updater](https://tauri.app/plugin/updater/)).
14. **Dark / light / system theme.** A three-way setting (Clash `theme_mode: light|dark|system`); default to system.
15. **Symptom-based troubleshooting helper** under the connect card: "Connection refused → proxy stopped/port", "401 → wrong key/header", "404 model → use an alias from this list" (Jan troubleshooting table, Claude Code gateway docs), plus "Open logs folder" (Ollama/Clash put logs in known dirs).

Skipped as not fitting our scope: system-proxy/TUN modes, HAR import/export, breakpoints/rewrite rules, multi-user token issuance with spend limits (One/New API admin features). Add them only if the proxy becomes multi-user.
