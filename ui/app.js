// Browser preview: when opened outside Tauri, stub the commands with sample data.
if (!window.__TAURI__ || !window.__TAURI__.core) {
  const sampleModels = [
    { id: "mlai-code", real: "pixis-opus", context: 1048576, output: 32768 },
    { id: "opus", real: "pixis-opus", context: 1048576, output: 32768 },
    { id: "glm", real: "glm-5.3", context: 200000, output: 32768 }
  ];
  const sampleRows = [
    { id: 12, ts: new Date().toISOString(), accountId: "acc-work", accountLabel: "Work", model: "opus", realModel: "pixis-opus", stream: true, status: 200, ms: 4820, effort: "low", firstTokenMs: 610, hasBody: true, promptTokens: 1831, completionTokens: 128, cachedTokens: 1792, reasoningTokens: 96, totalTokens: 1959 },
    { id: 11, ts: new Date(Date.now() - 61000).toISOString(), accountId: "unattributed", accountLabel: "Unattributed", model: "glm", realModel: "glm-5.3", stream: false, status: 200, ms: 1220, effort: "inherit", firstTokenMs: null, hasBody: false, promptTokens: 402, completionTokens: 10, cachedTokens: null, reasoningTokens: null, totalTokens: 412 }
  ];
  const sampleUsage = {
    stats: { requests: 2, errors: 0, promptTokens: 2233, completionTokens: 138, cachedTokens: 1792, reasoningTokens: 96, totalTokens: 2371, cacheHits: 1, credits: 0.002371 },
    byModel: [{ model: "opus", realModel: "pixis-opus", requests: 1, promptTokens: 1831, completionTokens: 128, cachedTokens: 1792, reasoningTokens: 96, totalTokens: 1959, cacheHits: 1, credits: 0.001959 }],
    byAccount: [
      { accountId: "acc-work", accountLabel: "Work", requests: 1, totalTokens: 1959, credits: 0.001959 },
      { accountId: "unattributed", accountLabel: "Unattributed", requests: 1, totalTokens: 412, credits: 0.000412 }
    ],
    tokensPerCredit: 1000000
  };
  const sampleAccounts = {
    accounts: [
      { id: "acc-work", label: "Work", server: "https://code.ml.ai", tokenHint: "mlaid_eyJ…", createdAt: new Date().toISOString(), lastUsed: new Date().toISOString(), active: true },
      { id: "acc-home", label: "Home", server: "https://code.ml.ai", tokenHint: "mlaid_9f2…", createdAt: new Date().toISOString(), lastUsed: null, active: false }
    ],
    active: "acc-work",
    path: "C:\\Users\\Lenovo\\AppData\\Local\\mlai-bridge\\accounts.json"
  };
  let runningState = { running: true, baseUrl: "http://127.0.0.1:8787", v1: "http://127.0.0.1:8787/v1", key: "mlba_preview_key", addr: "127.0.0.1:8787" };
  const config = {
    host: "127.0.0.1", port: 8787, server: "https://code.ml.ai", apiKey: "mlba_preview_key",
    project: "a6be2c98-de50-42b1-bb1f-4fd6bc00646e", folder: "Jambo-Card-Frontend",
    runTask: "Local model bridge session", defaultModel: "mlai-code",
    aliases: { "mlai-code": "pixis-opus", opus: "pixis-opus", glm: "glm-5.3" },
    reasoning: "native", reasoningEffort: "low", alwaysIncludeUsage: true, allowRawModelIds: true,
    tokenSafetyMs: 300000, requestTimeoutMs: 900000, maxTokensFloor: 0,
    limitsCache: "limits.cache.json", historyPath: "requests.jsonl", bodyKeep: 200, bodyMaxBytes: 67108864,
    accountsPath: "accounts.json"
  };
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    if (cmd === "bootstrap") return { configPath: "C:\\Users\\Lenovo\\AppData\\Local\\mlai-bridge\\config.json", configExists: true, config: config, credential: { store: "account", server: "https://code.ml.ai", label: "Work", tokenPrefix: "mlaid_" }, running: runningState, models: sampleModels, stats: sampleUsage.stats, accounts: sampleAccounts };
    if (cmd === "history") return sampleRows;
    if (cmd === "usage") return sampleUsage;
    if (cmd === "accounts") return sampleAccounts;
    if (cmd === "accounts_add") { return sampleAccounts; }
    if (cmd === "accounts_import") { return sampleAccounts; }
    if (cmd === "accounts_activate") { return { ok: true, restarted: true, running: runningState }; }
    if (cmd === "accounts_remove") { return sampleAccounts; }
    if (cmd === "request_detail") {
      const row = sampleRows.find(r => r.id === Number(args && args.id)) || sampleRows[0];
      return { row: row, hasBody: row.hasBody, request: { model: row.realModel, messages: [{ role: "user", content: "preview request" }], reasoning_effort: row.effort, max_tokens: 256, stream: row.stream }, response: { stream: true, assembled: true, content: "Preview response text.", reasoning: "Preview reasoning.", toolCalls: [], finishReason: "stop", usage: { prompt_tokens: row.promptTokens, completion_tokens: row.completionTokens, total_tokens: row.totalTokens }, chunks: 12 } };
    }
    if (cmd === "start") { runningState = { running: true, baseUrl: "http://127.0.0.1:8787", v1: "http://127.0.0.1:8787/v1", key: config.apiKey, addr: "127.0.0.1:8787" }; return runningState; }
    if (cmd === "stop") { runningState = { running: false }; return runningState; }
    if (cmd === "save_config") return { ok: true, restartRequired: true };
    if (cmd === "mint_limits") return sampleModels.map((m) => ({ model: m.real, context: m.context, output: m.output }));
    if (cmd === "clear_history") return { ok: true };
    return { ok: true };
  } } };
}

const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
let state = null;
let usageData = null;
let rows = [];
let filters = { account: "", model: "", effort: "" };
let currentDetail = null;
let busy = false;
let logTimer = null;
let activeTab = (() => {
  const saved = localStorage.getItem("mlaiTab");
  return saved === "usage" || saved === "accounts" ? saved : "proxy";
})();

function fmt(n) {
  if (n === null || n === undefined || n === "") return "\u2014";
  return Number(n).toLocaleString();
}

function credits(tokens) {
  const c = (Number(tokens) || 0) / 1000000;
  if (!c) return "0";
  if (c >= 1000) return Math.round(c).toLocaleString();
  if (c >= 100) return String(Math.round(c));
  if (c >= 1) return String(Math.round(c * 10) / 10);
  if (c >= 0.01) return String(Math.round(c * 100) / 100);
  if (c >= 0.0001) return String(Math.round(c * 10000) / 10000);
  return "<0.0001";
}

function percent(part, whole) {
  if (!whole) return "\u2014";
  return (Math.round(((Number(part) || 0) / Number(whole)) * 1000) / 10) + "%";
}

function timeOf(iso) {
  try {
    const d = new Date(iso);
    if (isNaN(d.getTime())) return String(iso);
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch (e) { return String(iso); }
}

function note(id, text, ms) {
  const el = $(id);
  if (!el) return;
  el.textContent = text;
  if (ms) setTimeout(() => { if (el.textContent === text) el.textContent = ""; }, ms);
}

function running() {
  return !!(state && state.running && state.running.running);
}

function activeAccount() {
  const data = (state && state.accounts) || {};
  const id = data.active;
  const account = (data.accounts || []).find((a) => a.id === id);
  return { id: id || null, label: account ? account.label : null, hint: account ? account.tokenHint : "" };
}

function setTab(tab) {
  activeTab = tab;
  localStorage.setItem("mlaiTab", tab);
  $("tab-proxy").hidden = tab !== "proxy";
  $("tab-usage").hidden = tab !== "usage";
  $("tab-accounts").hidden = tab !== "accounts";
  for (const name of ["proxy", "usage", "accounts"]) {
    const btn = $("tab-" + name + "-btn");
    btn.classList.toggle("active", tab === name);
    btn.setAttribute("aria-selected", String(tab === name));
  }
  if (tab === "usage") { loadUsage(); refreshLog(); }
  if (tab === "accounts") renderAccounts();
}

function render() {
  const on = running();
  $("status-pill").className = "status" + (on ? " running" : "") + (busy ? " starting" : "");
  $("status-text").textContent = busy ? "working" : (on ? "running" : "stopped");
  $("btn-start").disabled = on || busy;
  $("btn-stop").disabled = !on || busy;
  $("base-url").value = on ? state.running.v1 : "start the proxy to get a URL";
  const key = state && state.config ? state.config.apiKey : "";
  $("api-key").value = key || "(no key set)";
  $("config-path").textContent = (state && state.configPath) || "";
  fillForm();
  renderModels();
  renderAccounts();
}

function fillForm() {
  const c = (state && state.config) || {};
  $("f-host").value = c.host || "127.0.0.1";
  $("f-port").value = c.port || 8787;
  $("f-server").value = c.server || "https://code.ml.ai";
  $("f-project").value = c.project || "";
  $("f-folder").value = c.folder || "";
  $("f-runtask").value = c.runTask || "";
  $("f-default").value = c.defaultModel || "mlai-code";
  $("f-effort").value = c.reasoningEffort || "";
}

function formConfig() {
  const c = Object.assign({}, state.config);
  c.host = $("f-host").value.trim() || "127.0.0.1";
  c.port = Number($("f-port").value) || 8787;
  c.server = $("f-server").value.trim();
  c.project = $("f-project").value.trim();
  c.folder = $("f-folder").value.trim();
  c.runTask = $("f-runtask").value.trim();
  c.defaultModel = $("f-default").value.trim();
  c.reasoningEffort = $("f-effort").value || null;
  return c;
}

function renderModels() {
  const body = $("models-table").querySelector("tbody");
  const models = (state && state.models) || [];
  $("model-count").textContent = models.length ? String(models.length) : "";
  if (!models.length) {
    body.innerHTML = '<tr><td colspan="4" class="empty">Start the proxy to load the model menu.</td></tr>';
    return;
  }
  body.innerHTML = models.map((m) =>
    "<tr><td class=\"mono\">" + escapeHtml(m.id) + "</td><td class=\"mono\">" + escapeHtml(m.real || m.id) +
    "</td><td class=\"num\">" + fmt(m.context) + "</td><td class=\"num\">" + fmt(m.output) + "</td></tr>"
  ).join("");
}

function renderAccounts() {
  const data = (state && state.accounts) || { accounts: [], active: null };
  const list = data.accounts || [];
  const active = activeAccount();
  $("acc-count").textContent = list.length ? String(list.length) : "";
  $("acc-active-label").textContent = active.label || "(no account selected)";
  const account = list.find((a) => a.id === data.active);
  $("acc-active-server").textContent = account ? account.server : "";
  $("acc-active-hint").textContent = account ? account.tokenHint : "";

  const panel = $("u-account");
  panel.textContent = active.label || "Unattributed";
  $("u-account-hint").textContent = active.hint || "";

  const body = $("accounts-table").querySelector("tbody");
  if (!list.length) {
    body.innerHTML = '<tr><td colspan="6" class="empty">No accounts yet. Add one, or import the mlai CLI sign-in.</td></tr>';
    return;
  }
  body.innerHTML = list.map((a) => {
    const isActive = a.active;
    const actions = isActive
      ? '<span class="mine">active</span>'
      : '<button class="ghost small-btn" data-activate="' + escapeAttr(a.id) + '">Use</button> ' +
        '<button class="ghost small-btn" data-remove="' + escapeAttr(a.id) + '">Remove</button>';
    return "<tr class=\"" + (isActive ? "active-account-row" : "") + "\"><td><b>" + escapeHtml(a.label) + "</b></td>" +
      "<td class=\"mono\">" + escapeHtml(a.server) + "</td><td class=\"mono\">" + escapeHtml(a.tokenHint) + "</td>" +
      "<td class=\"mono\">" + escapeHtml((a.createdAt || "").slice(0, 10)) + "</td>" +
      "<td class=\"mono\">" + escapeHtml(a.lastUsed ? String(a.lastUsed).slice(0, 19).replace("T", " ") : "\u2014") + "</td>" +
      "<td class=\"num\">" + actions + "</td></tr>";
  }).join("");
}

function renderUsage() {
  const data = usageData || {};
  const s = data.stats || {};
  $("u-requests").textContent = fmt(s.requests);
  $("u-input").textContent = fmt(s.promptTokens);
  $("u-output").textContent = fmt(s.completionTokens);
  $("u-cached").textContent = fmt(s.cachedTokens);
  $("u-spend").textContent = credits(s.totalTokens);
  const hitRate = s.requests ? Math.round(((s.cacheHits || 0) / s.requests) * 1000) / 10 + "% of requests had cache hits" : "";
  $("usage-note").textContent = "Cached input is a subset of input tokens (" + (hitRate || "no hits yet") +
    "). Spend uses ML AI Code's unit: 1 credit = 1,000,000 tokens. Output includes reasoning tokens.";

  const list = data.byModel || [];
  const body = $("usage-table").querySelector("tbody");
  if (!list.length) {
    body.innerHTML = '<tr><td colspan="8" class="empty">No usage recorded yet. Send a request through the proxy.</td></tr>';
    return;
  }
  body.innerHTML = list.map((r) =>
    "<tr><td>" + escapeHtml(r.model) + "</td><td class=\"mono\">" + escapeHtml(r.realModel || "") +
    "</td><td class=\"num\">" + fmt(r.requests) + "</td><td class=\"num\">" + fmt(r.promptTokens) +
    "</td><td class=\"num\">" + fmt(r.completionTokens) + "</td><td class=\"num\">" + fmt(r.cachedTokens) +
    "</td><td class=\"num\">" + percent(r.cachedTokens, r.promptTokens) + "</td><td class=\"num\">" +
    credits(r.totalTokens) + "</td></tr>"
  ).join("");
}

function renderLogStats() {
  const s = (usageData && usageData.stats) || (state && state.stats);
  const el = $("stats");
  if (!el) return;
  if (!s) { el.textContent = ""; return; }
  el.textContent = fmt(s.requests) + " requests \u00b7 " + fmt(s.errors) + " errors \u00b7 " + fmt(s.totalTokens) + " tokens";
}

function renderFilters() {
  const accounts = Array.from(new Set(rows.map((r) => r.accountId || "unattributed")));
  const labels = new Map();
  for (const r of rows) labels.set(r.accountId || "unattributed", r.accountLabel || "Unattributed");
  const known = ((state && state.accounts && state.accounts.accounts) || []).map((a) => {
    labels.set(a.id, a.label);
    return a.id;
  });
  const accountIds = Array.from(new Set([...accounts, ...known])).sort();
  const models = Array.from(new Set(rows.map((r) => r.model))).sort();
  const efforts = Array.from(new Set(rows.map((r) => r.effort || "inherit"))).sort();

  const a = $("filter-account");
  if (a.options.length !== accountIds.length + 1) {
    a.innerHTML = '<option value="">All accounts</option>' + accountIds.map((id) => {
      const label = labels.get(id) || id;
      return '<option value="' + escapeAttr(id) + '">' + escapeHtml(label) + "</option>";
    }).join("");
    a.value = accountIds.includes(filters.account) ? filters.account : "";
  }
  const m = $("filter-model");
  if (m.options.length !== models.length + 1) {
    m.innerHTML = '<option value="">All models</option>' + models.map((x) => '<option value="' + escapeAttr(x) + '">' + escapeHtml(x) + "</option>").join("");
    m.value = models.includes(filters.model) ? filters.model : "";
  }
  const e = $("filter-effort");
  if (e.options.length !== efforts.length + 1) {
    e.innerHTML = '<option value="">All efforts</option>' + efforts.map((x) => '<option value="' + escapeAttr(x) + '">' + escapeHtml(x) + "</option>").join("");
    e.value = efforts.includes(filters.effort) ? filters.effort : "";
  }
}

function filteredRows() {
  return rows.filter((r) => {
    const accountId = r.accountId || "unattributed";
    if (filters.account && accountId !== filters.account) return false;
    if (filters.model && r.model !== filters.model) return false;
    if (filters.effort && (r.effort || "inherit") !== filters.effort) return false;
    return true;
  });
}

function renderLog() {
  const body = $("log-table").querySelector("tbody");
  const list = filteredRows();
  note("filter-note", list.length + " of " + rows.length + " requests");
  if (!list.length) {
    body.innerHTML = '<tr><td colspan="10" class="empty">No requests match. Clear the filters or send a request.</td></tr>';
    return;
  }
  body.innerHTML = list.map((r) => {
    const statusClass = r.status >= 400 ? "pill err" : "pill ok";
    return "<tr data-id=\"" + r.id + "\"><td class=\"mono\">" + timeOf(r.ts) + "</td><td>" +
      escapeHtml(r.accountLabel || "Unattributed") + "</td><td class=\"mono\">" + escapeHtml(r.model) +
      "</td><td><span class=\"effort-pill\">" + escapeHtml(r.effort || "inherit") + "</span></td>" +
      "<td class=\"num\">" + fmt(r.promptTokens) + "</td><td class=\"num\">" + fmt(r.completionTokens) +
      "</td><td class=\"num\">" + fmt(r.cachedTokens) + "</td><td class=\"num\">" + fmt(r.reasoningTokens) +
      "</td><td class=\"num\"><span class=\"" + statusClass + "\">" + r.status + "</span></td><td class=\"num\">" + r.ms + "</td></tr>";
  }).join("");
}

async function loadUsage() {
  try {
    usageData = await invoke("usage");
    renderUsage();
    renderLogStats();
  } catch (e) {
    note("usage-refresh-note", String(e), 5000);
  }
}

async function refreshLog() {
  if (!state) return;
  try {
    rows = await invoke("history", { limit: 500 });
    renderFilters();
    renderLog();
  } catch (e) {
    note("log-note", String(e), 4000);
  }
}

function cell(label, value) {
  return '<div class="cell"><b>' + escapeHtml(value) + "</b><span>" + escapeHtml(label) + "</span></div>";
}

function pretty(value) {
  const text = JSON.stringify(value, null, 2);
  if (!text) return "(empty)";
  if (text.length > 160000) return text.slice(0, 160000) + "\n\u2026 truncated for display (" + text.length + " chars)";
  return text;
}

async function openDetail(id) {
  try {
    const detail = await invoke("request_detail", { id });
    currentDetail = detail;
    const r = detail.row;
    $("drawer-title").textContent = "Request #" + r.id + " \u00b7 " + r.model;
    $("drawer-sub").textContent = new Date(r.ts).toLocaleString() + " \u00b7 " + r.ms + " ms" + (r.stream ? " \u00b7 streamed" : "");
    const cells = [
      cell("Timestamp", new Date(r.ts).toLocaleString()),
      cell("Duration", r.ms + " ms"),
      cell("First token", r.firstTokenMs === null || r.firstTokenMs === undefined ? "\u2014" : r.firstTokenMs + " ms"),
      cell("Status", r.status),
      cell("Account", r.accountLabel || "Unattributed"),
      cell("Model", r.model + " \u2192 " + r.realModel),
      cell("Effort", r.effort || "inherit"),
      cell("Input", fmt(r.promptTokens)),
      cell("Output", fmt(r.completionTokens)),
      cell("Cached", fmt(r.cachedTokens)),
      cell("Reasoning", fmt(r.reasoningTokens)),
      cell("Spend", credits(r.totalTokens) + " cr")
    ];
    $("drawer-meta").innerHTML = cells.join("");
    $("drawer-request").textContent = detail.request ? pretty(detail.request) : "(body not available \u2014 pruned or never written)";
    const responseText = detail.response ? pretty(detail.response) : (detail.hasBody ? "(response missing)" : "(body not available \u2014 pruned or never written)");
    $("drawer-response").textContent = responseText;
    $("drawer").hidden = false;
    $("drawer-backdrop").hidden = false;
  } catch (e) {
    note("log-note", String(e), 5000);
  }
}

function closeDrawer() {
  $("drawer").hidden = true;
  $("drawer-backdrop").hidden = true;
  currentDetail = null;
}

async function copyDetail() {
  if (!currentDetail) return;
  const payload = { meta: currentDetail.row, request: currentDetail.request || null, response: currentDetail.response || null };
  try {
    await navigator.clipboard.writeText(JSON.stringify(payload, null, 2));
    note("log-note", "copied request #" + currentDetail.row.id + " as JSON", 3000);
  } catch (e) {
    note("log-note", "clipboard blocked: " + String(e), 4000);
  }
}

function escapeHtml(value) {
  return String(value === null || value === undefined ? "" : value)
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function escapeAttr(value) {
  return escapeHtml(value).replace(/"/g, "&quot;");
}

async function load() {
  state = await invoke("bootstrap");
  render();
  await refreshLog();
  if (activeTab === "usage") await loadUsage();
  if (running() && !logTimer) {
    logTimer = setInterval(() => { refreshLog(); if (activeTab === "usage") loadUsage(); }, 3000);
  }
  if (!running() && logTimer) { clearInterval(logTimer); logTimer = null; }
}

async function withBusy(fn, errTarget) {
  if (busy) return;
  busy = true;
  render();
  try { await fn(); }
  catch (e) { note(errTarget || "save-note", String(e), 6000); }
  finally { busy = false; render(); }
}

function wire() {
  $("tab-proxy-btn").addEventListener("click", () => setTab("proxy"));
  $("tab-usage-btn").addEventListener("click", () => setTab("usage"));
  $("tab-accounts-btn").addEventListener("click", () => setTab("accounts"));

  $("btn-start").addEventListener("click", () => withBusy(async () => { await invoke("start"); await load(); }, "save-note"));
  $("btn-stop").addEventListener("click", () => withBusy(async () => { await invoke("stop"); await load(); }, "log-note"));

  $("btn-save").addEventListener("click", () => withBusy(async () => {
    const result = await invoke("save_config", { config: formConfig() });
    note("save-note", result.restartRequired ? "saved \u2014 restart the proxy to apply" : "saved", 5000);
    await load();
  }, "save-note"));

  $("btn-regen").addEventListener("click", () => withBusy(async () => {
    const bytes = new Uint8Array(24);
    crypto.getRandomValues(bytes);
    const key = "mlba_" + Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
    await invoke("save_config", { config: Object.assign({}, state.config, { apiKey: key }) });
    note("save-note", "new key saved", 5000);
    await load();
  }, "save-note"));

  $("btn-limits").addEventListener("click", () => withBusy(async () => {
    note("limits-note", "reading limits (one run per model)\u2026");
    const learned = await invoke("mint_limits");
    const ok = learned.filter((l) => !l.error).length;
    note("limits-note", "learned " + ok + " model(s)" + (learned.length - ok ? ", " + (learned.length - ok) + " failed" : ""), 6000);
    await load();
  }, "limits-note"));

  $("btn-usage-refresh").addEventListener("click", () => withBusy(async () => {
    await loadUsage();
    await refreshLog();
    note("usage-refresh-note", "refreshed", 2500);
  }, "usage-refresh-note"));

  $("btn-acc-add").addEventListener("click", () => withBusy(async () => {
    const token = $("acc-token").value.trim();
    if (!token) { note("acc-note", "paste the account's device token", 4000); return; }
    note("acc-note", "checking token\u2026");
    const result = await invoke("accounts_add", {
      token: token,
      label: $("acc-label").value.trim() || null,
      server: $("acc-server").value.trim() || null
    });
    $("acc-token").value = "";
    $("acc-label").value = "";
    note("acc-note", "account added", 5000);
    state = await invoke("bootstrap");
    render();
    renderAccounts();
    void result;
  }, "acc-note"));

  $("btn-acc-import").addEventListener("click", () => withBusy(async () => {
    note("acc-import-note", "importing\u2026");
    await invoke("accounts_import");
    note("acc-import-note", "imported from mlai CLI", 5000);
    await load();
  }, "acc-import-note"));

  $("accounts-table").addEventListener("click", (ev) => {
    const activate = ev.target.closest("[data-activate]");
    const remove = ev.target.closest("[data-remove]");
    if (activate) {
      withBusy(async () => {
        const result = await invoke("accounts_activate", { id: activate.getAttribute("data-activate"), restart: running() });
        note("accounts-note", result && result.restarted ? "account switched \u2014 proxy restarted" : "account switched", 5000);
        await load();
      }, "accounts-note");
    } else if (remove) {
      if (!confirm("Remove this account? Its stored token is deleted.")) return;
      withBusy(async () => {
        await invoke("accounts_remove", { id: remove.getAttribute("data-remove") });
        note("accounts-note", "account removed", 4000);
        await load();
      }, "accounts-note");
    }
  });

  $("btn-clear").addEventListener("click", () => withBusy(async () => {
    await invoke("clear_history");
    note("log-note", "history cleared", 4000);
    await load();
    if (activeTab === "usage") await loadUsage();
  }, "log-note"));

  $("filter-account").addEventListener("change", (e) => { filters.account = e.target.value; renderLog(); });
  $("filter-model").addEventListener("change", (e) => { filters.model = e.target.value; renderLog(); });
  $("filter-effort").addEventListener("change", (e) => { filters.effort = e.target.value; renderLog(); });
  $("btn-filter-clear").addEventListener("click", () => {
    filters = { account: "", model: "", effort: "" };
    $("filter-account").value = "";
    $("filter-model").value = "";
    $("filter-effort").value = "";
    renderLog();
  });

  $("log-table").querySelector("tbody").addEventListener("click", (e) => {
    const tr = e.target.closest("tr[data-id]");
    if (tr) openDetail(Number(tr.getAttribute("data-id")));
  });

  $("btn-drawer-close").addEventListener("click", closeDrawer);
  $("drawer-backdrop").addEventListener("click", closeDrawer);
  $("btn-copy-json").addEventListener("click", copyDetail);
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && !$("drawer").hidden) closeDrawer(); });

  document.querySelectorAll("button.copy").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const input = $(btn.getAttribute("data-copy"));
      try {
        await navigator.clipboard.writeText(input.value);
        const old = btn.textContent;
        btn.textContent = "Copied";
        setTimeout(() => { btn.textContent = old; }, 1200);
      } catch (e) { input.select(); }
    });
  });
}

let bootTries = 0;

async function boot() {
  wire();
  try {
    const forced = await invoke("startup_tab");
    if (forced === "usage" || forced === "proxy" || forced === "accounts") activeTab = forced;
  } catch (e) { /* older builds */ }
  setTab(activeTab);
  try { await load(); }
  catch (e) { note("save-note", String(e), 8000); return; }
  if (!running() && bootTries < 4) {
    bootTries += 1;
    setTimeout(() => { load().catch(() => {}); }, 1500);
  }
}

window.addEventListener("DOMContentLoaded", boot);
