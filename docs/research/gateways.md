# LLM gateway & observability dashboards: UI patterns survey

Scope: request logs and inspectors, usage/cost dashboards, keys and provider management, routing/alias/fallback config, onboarding snippets, health/status. Every claim has a link. Statements marked **[INFERENCE]** are my reading, not a documented fact.

Date of research: 2026-11-24. Docs change often, so details may have moved.

---

## 1. OpenRouter (hosted): Logs, Activity, Keys, Model pages

### Logs page (request log + inspector)
- **Tabs by record type:** Generations (default), Upstream Requests (each routing attempt, including retries and fallbacks), Sessions (generations grouped, with primary/supporting models and total cost), Videos, Batches. Source: https://openrouter.ai/docs/guides/features/logs
- **Generations columns:** date, model, final provider, generation ID, status, attempts, API key, latency. Same source.
- **Filter bar** above the table: user, model, provider, status, region, API key, modality. You can also paste a generation ID to jump straight to that record. **Filters are stored in the URL**, so a filtered view can be bookmarked or shared. Same source.
- **Request histogram above the table** shows volume over the selected date range, "so you can spot spikes before drilling into individual rows". Same source.
- **Detail sheet (side panel)** opens on row click. Sections:
  - **Overview:** model, provider, cost, tokens, finish reason, streaming flag, latency/throughput, region, fallbacks.
  - **Provider Responses:** a *waterfall* of each upstream attempt, with latency, moderation time and overhead.
  - **Usage:** cost breakdown, including cache-read discounts.
  - **Prompt / Completion:** the content, only if I/O logging is on.
  - **Raw JSON.**
  - **Actions:** "open in Chatroom" or "replay in Playground with a different model". Same source.
- **Prompt detail view:** renders the messages array with a **per-message token flamegraph colored by role** (system/user/assistant/tool). **The cached prefix is shaded**, so you can see where the prompt cache broke. Source: https://openrouter.ai/blog/announcements/activity-dashboard/
- **Content is opt-in:** prompt and completion text is stored only after the user turns on "Input & Output Logging" under Observability settings, and only for later requests. The empty state says so. You can also include or exclude specific API keys from logging. Source: https://openrouter.ai/docs/guides/features/input-output-logging

### Activity page (usage/cost dashboard)
- **Overview headline cards:** spend, requests, token volume, cache hit rate, blended cost per 1M tokens. Each card has a **sparkline and a comparison against the previous period**. Source: https://openrouter.ai/blog/announcements/activity-dashboard/
- **Charts:** usage by model (stacked daily bars), token breakdown (prompt/completion/reasoning), cached vs uncached prompt tokens, credits vs BYOK spend. Top keys/apps are shown as ranked cards. Sources: https://openrouter.ai/docs/guides/features/activity and the blog above.
- **Trends tab:** sorts by movement instead of size, with "rising/falling" side panels (useful for spotting a runaway agent). Source: blog above.
- **Explore tab:** pick a metric, group by up to two dimensions, choose a rollup (minute through month, or none for a ranked table), and a chart type (bar/line/dot). Charts can be saved, and results exported as CSV/PDF. Source: https://openrouter.ai/docs/guides/features/activity
- **Shared filter state:** all tabs share the same date range and filter state, kept in the URL. Same source.
- **Chart → logs drill-down:** clicking a bar, a stacked segment or a ranked row opens the Logs page filtered to that time bucket and group. A popover offers "View logs" and "Open in Explorer". Source: blog above.
- **Clamped ranges are flagged:** high-resolution metrics are limited to about 31 days, and "Activity tells you when it has clamped your range". Source: blog FAQ.

### Keys
- **Key creation:** each key gets a name and an optional credit limit, which can reset daily, weekly or monthly. Once a key hits its limit, requests are rejected until the limit is raised. The docs warn that a key with no limit can spend your whole balance. Source: https://openrouter.ai/docs/api/reference/authentication
- **Actionable 401 messages:** a table maps exact error strings to causes, e.g. "this is not an OpenRouter API key" when the SDK silently fell back to `OPENAI_API_KEY`. Same source.

### Model pages
- **Provider table:** input/output price per 1M tokens, latency, throughput and uptime per provider, with a percentile toggle (P50–P99). Source: https://openrouter.ai/rekaai/reka-edge-2603
- **Uptime vs availability:** both shown for the last 3 days, with a 72-hour availability timeline strip and a plain-language definition of each term. Same source.
- **Pricing:** effective vs listed price, plus **cache hit rate per provider**. Same source.
- **API section:** "drop-in code" that is "OpenAI-compatible — most SDKs work by just swapping the base URL". Same source.

**Why good:** OpenRouter carries the same mental model through every screen: aggregate → click → filtered rows → detail sheet → raw JSON. The URL holds the state, so any view can be shared.

---

## 2. Vercel AI Gateway (hosted)

### Logs
- **Columns:** Time (hover for full timestamp), Status, Model, Provider, Usage, Cost, Duration, Authentication. Source: https://vercel.com/docs/ai-gateway/observability-and-spend/logs
- **Compact Usage cell:** shows `1.7K → 233 tokens`. Hovering expands it into input vs cache-read on the left and reasoning vs output on the right. Same source.
- **"Dash means nothing reported":** a request that failed before reaching a provider shows a dash in both Usage and Cost. Same source.
- **Filters:**
  - **Search:** matches model, provider or request ID.
  - **Multi-select dropdowns:** Status (`2xx`/`4xx`/`5xx` or an exact code like `429`), Model, Provider, Authentication, Routing, Modality, Latency or TTFT ranges, Tokens, Cost.
  - **Date presets:** from 5 minutes to 30 days.
  - **Reset:** clears search and filters.
  - **URL:** all filters live in the URL.

  Same source.
- **Chart above the table:** shows token volume under the current filters, and falls back to request count when there are no tokens. **Drag across the chart to zoom the time range.** Same source.
- **Live toggle:**
  - Tails logs with new rows at the top, refreshing every 5 seconds.
  - Only works with a *relative* range; turning it on from an absolute range switches to the last hour.
  - Scrolling back through history is disabled while live. Pause to load older rows.
  - With Live off, the list loads 50 more rows on each scroll to the bottom.

  Same source.
- **Detail panel:**
  - Opens beside the list and is **resizable**. The selected log ID goes into the URL.
  - Header shows the ID with a copy button.
  - A "Routed to [provider]" card for each attempt, with TTFT and timing spans colored **green (ok) / amber (4xx) / red (5xx or timeout)**.
  - A recovered request gets a refresh icon in the table's Status cell.
  - Usage and Cost are itemized line by line.

  Same source.
- **Export:** "Copy visible logs" to the clipboard, or Export to CSV / JSON. Same source.

### Overview
- **Four charts:** Requests by model, P50 TTFT, input/output tokens, Spend. Source: https://vercel.com/docs/ai-gateway/observability-and-spend/observability
- **Summaries per project and per API key:** request count, average tokens/sec, P50 duration, P50 TTFT, cost. Same source.

### Onboarding for coding agents
- **One CLI command:** `npx vercel ai-gateway setup` detects installed agents, **shows a diff of every planned config change before writing**, reuses an existing key, and stores the key in the macOS Keychain. Source: https://vercel.com/docs/ai-gateway/coding-agents
- **Per-agent base URL table:**
  - Generic: `/coding-agent/v1`.
  - Claude Code: `/claude-code`, with no `/v1` because the Anthropic SDK appends it.
  - Codex: `/codex/v1` with `wire_api = "responses"`.
  - Cline: provider dropdown + key.

  The coding-agent `/models` list hides models that can't produce text, so non-text models don't show up in agent pickers. Same source.
- **Virtual models:** a settings bundle (fallbacks, provider order) selected as `vmc/<slug>` in the agent's model picker. Same source.

**Why good:** this is the closest analog to MLAI Bridge's audience (Cline/opencode users). Vercel solves "which URL do I paste where" with a per-client table and a preview-diff installer.

---

## 3. Cloudflare AI Gateway (hosted)

### Analytics
- **Metrics:** requests, token usage, costs, errors, and cached-response percentage. They can be filtered by time, provider, model, cost, token counts and a custom metadata key/value. Source: https://developers.cloudflare.com/ai-gateway/observability/analytics/
- **Cost is labelled as an estimate**, calculated from sampled data. Same source.

### Logs
- **Fields:** each log can include prompt, response, provider, timestamp, status, tokens, cost, duration and user agent. Source: https://developers.cloudflare.com/ai-gateway/observability/logging
- **User-agent filter:** equals / does not equal / contains, to tell `openai-python` apart from a custom app. Source: https://developers.cloudflare.com/changelog/post/2026-06-12-user-agent-logging/
- **Two-level privacy switch:** the gateway-wide setting "Logs on/off" can be overridden per request with `cf-aig-collect-log`. `cf-aig-collect-log-payload: false` keeps the metadata (tokens/cost/status) but drops request and response bodies. Source: logging page above.

### Caching
- **Cache status is visible:** an `cf-aig-cache-status: HIT|MISS` response header.
- **Controls:** Settings → "Cache Responses" toggle with a default TTL, plus per-request skip/TTL/custom-key headers.
- **Cache-key explanation:** the docs spell out exactly which inputs form the cache key (provider + endpoint + model + auth + body).

Source: https://developers.cloudflare.com/ai-gateway/features/caching

### Routing
- **Model aliases:** "Create alias" → name (the `custom/` prefix is added automatically) → add models → **drag to set fallback order** → choose an algorithm (Default order or Smart routing).
- **Validation in the UI:** naming rules, a limit of 1–50 models, and no alias-of-alias.
- **Error table:** `404 Model not found`, `400 has no available models`, `503`.

Source: https://developers.cloudflare.com/ai-gateway/features/routing/model-aliases/
- **Dynamic routes:** a **visual node editor** (Start, Conditional, Percentage, Model, Rate Limit, Budget Limit, End) with **versions: save a draft, deploy, instant rollback**. Source: https://developers.cloudflare.com/ai-gateway/features/routing/dynamic-routing

**Why good:** the alias editor (drag-to-order fallback list) is the right complexity for "alias → upstream model" mapping. The node editor is overkill for a single user **[INFERENCE]**.

---

## 4. LiteLLM Proxy Admin UI (self-hosted, served at `/ui`)

### Navigation
- **Sidebar:** Virtual Keys, Models + Endpoints, Internal Users, Teams & Organizations, Usage, Logs, Chat UI. Source: https://www.krishnaik.in/ai-tutorials/litellm/admin-ui (summarizing https://docs.litellm.ai/docs/proxy/ui)

### Logs
- **Per-request fields:** spend, token usage, key, team; also which tools were provided and which were called. Source: https://docs.litellm.ai/docs/proxy/ui_logs
- **Prompts are not stored by default.** Logs → Settings has a "Store Prompts in Spend Logs" toggle that takes effect without a restart, plus a Retention Period setting. Same source.
- **Token redaction:** "known token-shaped secrets are masked in the stored content by default". Same source.
- **Log drawer:** shows the cost breakdown and error details. Same source (metadata table).

### Usage
- **Endpoint Activity tab:** per-endpoint requests, success rate, tokens, spend, a success-vs-failed chart and daily trends. Source: https://docs.litellm.ai/docs/proxy/endpoint_activity
- **Two different counters, explained:** the gateway counts every request at the edge (including auth rejects), while the per-key/model panels come from spend logs. The docs explain why the two won't tie out. Same source.

### Models
- **All Models list:** public name, provider, underlying model and cost per token. A badge marks the source (DB vs config file); **config-file models are read-only in the UI**. Search plus provider filters. Source: https://docs.litellm.ai/docs/proxy/model_management
- **Model detail page:**
  - **Overview** and **Raw JSON** tabs.
  - **Edit Settings** to change the public name, upstream model or price override.
  - **Test Connection**, which re-verifies the stored credentials against the provider.
  - **Delete Model.**

  Same source.
- **Health Status tab:** a "Run All Checks" button; each model shows its status, error details, last check time and last success time. Source: https://litellm.vercel.app/docs/proxy/health
- **Probe endpoints:** `/health/liveliness` and `/health/readiness` for machines. Same source.
- **Reusable named credentials:** "LLM Credentials" tab, with "Re-use Credentials" from an existing model. Source: model_management page.

### Complaints
- **Missing pagination:** the keys table showed 49 of 99 keys with no pagination control (a layout bug, fixed in v1.101). Source: https://github.com/BerriAI/litellm/issues/49213
- **Other UI bugs:** login redirect loops and asset 404s. Sources: https://github.com/BerriAI/litellm/issues/9027, https://github.com/BerriAI/litellm/issues/39599
- **Config sprawl:** users complain about YAML config at team scale. A commenter replied "why not use the UI? Then there's no config file". Source: https://www.reddit.com/r/LocalLLM/comments/1uf8qhi/we_moved_off_litellm_after_8_months_sharing_what

**Why good / bad:** "Test Connection" and "Health Status with last success time" map directly onto a single-account proxy. The admin UI is dense and multi-tenant-first, and has regressions in basic table plumbing. **Lesson: get the table basics right** (pagination, counts).

---

## 5. Bifrost (self-hosted, local web UI at `localhost:8080`; closest architectural analog)

- **Zero-config start:** `npx -y @maximhq/bifrost`, then open `http://localhost:8080`. The UI offers visual provider setup, real-time config ("changes apply immediately") and live monitoring. The app dir holds `config.db` and `logs.db` (SQLite); on Windows that is `%APPDATA%\bifrost`. Source: https://docs.getbifrost.ai/quickstart/gateway/setting-up
- **Security nudge:** a fresh instance has no password, and the docs warn to add one before exposing it beyond localhost. Same source.
- **Live log stream** over WebSocket; logging is async with "zero impact on request latency". Source: https://docs.getbifrost.ai/features/observability/default
- **Logs toolbar:**
  - Column selector (e.g. hidden-by-default **Stop Reason** column), with the choice **persisted across refreshes**.
  - **Group** toggle folds requests into `2 turns` (by session) or `1 fallback` (by parent request), with colored left borders. Stored in the URL.
  - **Pasting a request ID into search switches to exact lookup** and ignores the time range.

  Same source.
- **Filters:** provider, model, status (success/error/processing), latency range, token range, cost range, content full-text search, tool-call names. Same source.
- **Response stats header:** total requests, success rate, average latency, total tokens, total cost. Same source.
- **Attempt trail:** each failed attempt records key name, fail reason class (`rate_limit_error`, `authentication_error`…), status code, and whether it rotated to another key. Same source.
- **Streams:** cancelled or timed-out streams get *estimated* usage, flagged with `usage.estimation`. Same source.
- **Model Providers page:** per-provider keys with a model allowlist and weights (e.g. 0.7/0.3). Secrets are redacted in the UI. Source: https://docs.getbifrost.ai/quickstart/gateway/provider-configuration
- **Content logging control:** "Enable Logs" and "disable content logging" toggles keep metadata but drop bodies. Source: https://docs.getbifrost.ai/features/observability/content-logging

**Why good:** a single-binary local gateway with a SQLite log store and a browser UI is structurally what MLAI Bridge is. The **attempt trail and multi-key rotation** pattern maps to "multiple accounts, set active".

---

## 6. Portkey (hosted, now "Prisma AIRS AI Gateway")

- **Logs:** chronological list with timestamp, request type, model, tokens, thinking tokens and cost. Click → side panel with raw request and response. Each log has a **unique shareable URL** (`?logId=`). Source: https://docs.portkey.ai/docs/product/observability/logs
- **Status column with gateway feature states:** e.g. *Cache Hit / Miss / Semantic Hit / Disabled*, *Retry Success on N Tries / Failed*, *Fallback Active*. This answers "is the cache working?" at a glance. Same source.
- **Replay button:** in log details, opens the request in a playground to edit and rerun. It is disabled (with an explanation) for unsupported endpoints. Same source.
- **"Do not track":** keeps tokens, cost and latency but omits bodies; a side-by-side comparison shows how the log looks. Same source.
- **15 filter dimensions:** including Cache Status, Trace ID, API Key and Cost (cents). **Saved Filters** can be reused from the search bar. Source: https://docs.portkey.ai/docs/product/observability/filters
- **Configs:** JSON routing objects (fallback/loadbalance targets, `override_params`, `drop_params`) created on a Configs page. A **default config can be attached to an API key** for clients that can't send custom headers (Open WebUI, LibreChat). Source: https://docs.portkey.ai/docs/product/ai-gateway/configs

**Why good:** the status-chip vocabulary for cache, retry and fallback is a compact way to show per-request gateway behavior. "Default config on a key" solves the same problem MLAI Bridge's aliases solve: clients that only take base URL + key + model.

---

## 7. Helicone (OSS + hosted; note the maintenance-mode banner)

- **Product status:** the live dashboard says "Helicone has joined Mintlify — Transitioning to maintenance mode"; the EU instance says "now part of Experiential Labs". Sources: https://us.helicone.ai/dashboard, https://eu.helicone.ai/dashboard
- **Dashboard tiles** (read from the empty-state page):
  - Averages per request: cost, prompt tokens, completion tokens, total tokens.
  - Requests; All Errors as a % with an error-type breakdown.
  - Top models by requests and by cost; top providers.
  - Costs, latency quantiles, TTFT average, tokens/minute.
  - **"Start Live" toggle** and a date picker.
  - **Empty state renders zeros**, not a blank page.

  Source: https://eu.helicone.ai/dashboard
- **Requests page redesign:**
  - **Switch between requests without closing the drawer** (prev/next while the drawer stays open).
  - Compact rows.
  - Better time selector with quick-select presets.
  - The chat widget no longer covers pagination.

  Source: https://www.helicone.ai/changelog/20241003-request-redesign
- **Custom Properties:** `Helicone-Property-*` headers become **columns in the Requests table** and filter/group dimensions. Source: https://docs.helicone.ai/features/advanced-usage/custom-properties
- **Sessions:** a tree built from `Session-Path` with a duration distribution chart, colored by path. Source: https://docs.helicone.ai/features/sessions
- **Alerts:** metric (error rate, cost, latency, tokens, cache read/write, count) + threshold + window. A **Min Requests** setting avoids false positives. Source: https://docs.helicone.ai/features/alerts
- **Replay:** documented as an API-driven script; there is no one-click button in that cookbook. Source: https://docs.helicone.ai/guides/cookbooks/replay-session

**Why good:** prev/next inside the drawer is cheap and high-value. Showing zeros as the empty state teaches the layout before data arrives.

---

## 8. Langfuse (tracing/observability, OSS + cloud)

- **Filter search bar:**
  - One-line `field:value` queries with autocomplete of observed values.
  - Operators (`latency:>2`), wildcards (`name:*checkout*`), negation (`-env:prod`), `has:`, and full-text search scoped with `input:` / `output:`.
  - **The bar and the sidebar edit the same filter state.** The query serializes to the URL.
  - **Facet counts** next to each sidebar value respect the active filters.

  Source: https://langfuse.com/docs/observability/features/filter-search-bar
- **Trace view:** tree/timeline toggle, search inside a trace, show/hide metrics, color coding. Source: https://langfuse.com/changelog/2025-03-19-new-trace-view
- **2026 redesign:** the same layout in the full page, the **side peek** and on small screens; less clutter around input, output and metadata. Source: https://langfuse.com/changelog/2026-11-20-redesigned-trace-view

**Why good:** facet counts and query↔chip parity are the gold standard for filtering. Full trace trees are irrelevant for a one-hop proxy **[INFERENCE]**.

---

## 9. Kong Konnect AI Gateway (enterprise UI)

- **Overview filters:** All Providers / All models / All agents; the default range is 7 days. Source: https://developer.konghq.com/ai-management/observability
- **Key-metrics strip:** p95 LLM latency, request count, total tokens, **cached token count**, error rate, costs. **Each is compared with the previous period.** Same source.
- **Performance section:** TTFT and **TPOT (time per output token)** charts at p50/p95/p99. Section names at the top work as jump links. Same source.
- **Explorer query sentence:** "From [source] Show [chart] with [metric] per [interval] by [dimension]". It separates **Request model vs Response model**, which is useful for alias → upstream mapping. Same source.

**APISIX:** I found no AI-specific dashboard. Its AI plugins are config-only, and I did not verify a UI, so I omitted it.

---

## 10. LM Studio (local desktop server, for the desktop-app analog)

- **Developer tab:** a single "Start server" toggle; the server can also be started with `lms server start`. Server settings, serve-on-network and authentication live on sibling pages. Source: https://lmstudio.ai/docs/developer/core/server
- **Same server from CLI:** `lms server status` and `lms log stream` (CLI log tailing) exist. Sources: https://lmstudio.ai/docs/cli/serve/server-status, https://lmstudio.ai/docs/cli/serve/log-stream

**Why relevant:** this is the reference "local desktop app with an on/off server" pattern that developers using Cline already know **[INFERENCE]**.

---

## Cross-cutting user complaints / pitfalls
- **Missing pagination:** truncated tables that look complete (LiteLLM keys bug). https://github.com/BerriAI/litellm/issues/49213
- **Counts that don't reconcile:** gateway counts vs spend-log counts. LiteLLM had to write a whole section explaining the drift. https://docs.litellm.ai/docs/proxy/endpoint_activity
- **Prompt storage is a privacy decision:** every serious gateway defaults bodies off or makes them a separate toggle. Sources: LiteLLM ui_logs, OpenRouter I/O logging, Cloudflare logging, Portkey DO NOT TRACK, Bifrost content logging (links above).
- **Live mode vs history:** live tailing conflicts with scrolling through history; Vercel disables scroll-back while live. https://vercel.com/docs/ai-gateway/observability-and-spend/logs
- **Base-URL confusion:** Anthropic-protocol clients append `/v1` themselves, so they need the URL without it. https://vercel.com/docs/ai-gateway/coding-agents

---

## Top 15 patterns to copy for a local single-user desktop proxy (ranked)

1. **"Connect a client" panel with a per-client tab (Cline / Roo Code / opencode / curl / Python)**
   - Each tab shows the exact field names that client uses, the base URL, a masked key and copy buttons.
   - Call out the `/v1` trap for Anthropic-style clients.
   - *Vercel coding-agents base-URL table.* https://vercel.com/docs/ai-gateway/coding-agents

2. **Log table → resizable side drawer, with prev/next navigation inside the drawer**
   - The drawer's ID goes in the URL/route so it survives a reload.
   - *Vercel resizable panel; Helicone "toggle between requests without closing the drawer".* https://vercel.com/docs/ai-gateway/observability-and-spend/logs, https://www.helicone.ai/changelog/20241003-request-redesign

3. **Compact Usage cell: `in → out` tokens, with a hover breakdown**
   - The hover splits input vs cache-read and reasoning vs output.
   - Use a dash for "not reported" instead of `0`.
   - This fits the existing input/output/cached/reasoning fields directly. *Vercel.* https://vercel.com/docs/ai-gateway/observability-and-spend/logs

4. **Headline metric cards with a sparkline and a "vs previous period" delta**
   - Cards: requests, tokens, cache hit rate, credits.
   - *OpenRouter Activity; Kong key metrics.* https://openrouter.ai/blog/announcements/activity-dashboard/, https://developer.konghq.com/ai-management/observability

5. **Click any chart bar or segment → log table filtered to that bucket and group**
   - Clicking a per-model or per-account bar opens filtered logs.
   - *OpenRouter "Click a chart, land in the logs".* https://openrouter.ai/blog/announcements/activity-dashboard/

6. **Volume mini-chart above the log table, with drag-to-zoom on the time range**
   - *Vercel (drag to zoom); OpenRouter histogram.* https://vercel.com/docs/ai-gateway/observability-and-spend/logs, https://openrouter.ai/docs/guides/features/logs

7. **Live toggle on the log table**
   - New rows prepend at the top.
   - Pause live when the user scrolls or opens a row.
   - Show a "N new" pill instead of shifting rows under the cursor **[INFERENCE: pill is my addition]**.
   - This replaces the current "auto-refresh" checkbox. *Vercel Live; Helicone "Start Live"; Bifrost WebSocket stream.* https://vercel.com/docs/ai-gateway/observability-and-spend/logs, https://docs.getbifrost.ai/features/observability/default

8. **Filter bar with multi-select dropdowns plus a search box**
   - Dropdowns: account, model, effort, status class `2xx/4xx/5xx`.
   - The search box also accepts a pasted request ID and jumps to exact lookup.
   - One "Reset" button.
   - *Vercel; Bifrost ID lookup.* https://vercel.com/docs/ai-gateway/observability-and-spend/logs, https://docs.getbifrost.ai/features/observability/default

9. **Detail drawer sections in a fixed order: Overview → Timing → Usage/Cost → Request → Response → Raw JSON**
   - **Overview:** model (requested alias → upstream), account, effort, status, finish reason, stream flag.
   - **Timing:** a bar from request start to first token to end. Color it green, amber (4xx) or red (5xx/timeout).
   - **Raw JSON:** with copy.
   - *OpenRouter detail sheet; Vercel timing colors.* https://openrouter.ai/docs/guides/features/logs, https://vercel.com/docs/ai-gateway/observability-and-spend/logs

10. **Show "requested model vs served model" everywhere aliases exist**
    - The alias row expands to show the upstream model, in both the log and usage breakdowns.
    - *Kong Request model vs Response model; Cloudflare `custom/` aliases.* https://developer.konghq.com/ai-management/observability, https://developers.cloudflare.com/ai-gateway/features/routing/model-aliases/

11. **Alias editor as a list: alias name → upstream model picker, with inline validation**
    - Validate: name charset, no duplicates, upstream must exist in the model list.
    - Add a per-row "Test" button.
    - Skip dynamic routing graphs (YAGNI for a single user).
    - *Cloudflare alias form; LiteLLM Test Connection.* https://developers.cloudflare.com/ai-gateway/features/routing/model-aliases/, https://docs.litellm.ai/docs/proxy/model_management

12. **Account cards with health: status dot, "last success X ago", last error text and a Test button**
    - The active account is visually pinned.
    - Show which account served each request in the logs.
    - *LiteLLM Health Status tab (status / error / last check / last success); Bifrost per-key attempt trail.* https://litellm.vercel.app/docs/proxy/health, https://docs.getbifrost.ai/features/observability/default

13. **Status pill that distinguishes the proxy server state from upstream reachability**
    - Server states: Stopped / Starting / Running on :port / Port in use.
    - The upstream check runs separately.
    - Use one big toggle, like LM Studio's "Start server" switch.
    - *LM Studio Developer tab; LiteLLM liveliness vs model health split.* https://lmstudio.ai/docs/developer/core/server, https://litellm.vercel.app/docs/proxy/health

14. **Explicit privacy control for stored bodies**
    - A "Store request/response bodies" toggle; metadata (tokens, credits, timing) is always kept.
    - Add a retention setting.
    - Mask token-shaped secrets in stored bodies.
    - In the drawer, the empty state explains why a body is missing.
    - *LiteLLM, OpenRouter, Cloudflare, Portkey, Bifrost all do this.* https://docs.litellm.ai/docs/proxy/ui_logs, https://openrouter.ai/docs/guides/features/input-output-logging

15. **Copy/export affordances**
    - Copy as JSON (already present), copy as cURL for replay **[INFERENCE: cURL is my suggestion; Portkey/OpenRouter use an in-app playground]**.
    - Export the visible rows as CSV/JSON.
    - A copy button on every ID.
    - *Vercel export; Portkey Replay; OpenRouter "replay in Playground".* https://vercel.com/docs/ai-gateway/observability-and-spend/logs, https://docs.portkey.ai/docs/product/observability/logs

**Honorable mentions:**
- **Filters in the URL and the last-used filter persisted:** nearly every product does this. For a Tauri app, persisting the last filter is enough.
- **Column chooser persisted across restarts:** Bifrost.
- **Zero-valued empty dashboard:** Helicone.
- **Per-message token flamegraph with the cached prefix shaded:** OpenRouter. A strong idea for explaining cache hit rate, but a large build.

**Skip (multi-tenant SaaS only):**
- Teams, orgs and RBAC.
- Saved shared charts.
- Budget alerts to Slack.
- Dynamic route node graphs.
- Per-key spend limits.
- Guardrails/DLP.
