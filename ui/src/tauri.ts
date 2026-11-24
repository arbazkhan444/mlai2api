// Typed bridge to the Rust commands in crates/app/src/gui.rs.
// Outside Tauri (plain browser preview) the commands are stubbed with sample data.

export interface RunningState { running: boolean; baseUrl?: string; v1?: string; key?: string; addr?: string }
export interface Config {
  [key: string]: unknown;
  host?: string; port?: number; server?: string; apiKey?: string; project?: string; folder?: string;
  runTask?: string; defaultModel?: string; reasoningEffort?: string | null;
}
export interface Model { id: string; real?: string; context?: number | null; output?: number | null }
export interface Account {
  id: string; label: string; server: string; tokenHint: string;
  createdAt?: string; lastUsed?: string | null; active: boolean;
  expires?: number | null; lastCheckAt?: string | null; lastCheckOk?: boolean | null; lastError?: string | null;
}
export interface CheckResult { id: string; ok: boolean; ms: number; error?: string | null }
export interface Accounts { accounts: Account[]; active: string | null; path?: string }
export interface Stats {
  requests: number; errors: number; promptTokens: number; completionTokens: number; cachedTokens: number;
  reasoningTokens: number; totalTokens: number; cacheHits: number; credits: number;
}
export interface Bootstrap {
  configPath: string; configExists: boolean; config: Config; credential: unknown;
  running: RunningState; models: Model[]; stats: Stats; accounts: Accounts;
}
export interface Row {
  id: number; ts: string; accountId?: string | null; accountLabel?: string | null; model: string; realModel: string;
  stream: boolean; status: number; ms: number; effort?: string | null; firstTokenMs?: number | null; hasBody: boolean;
  promptTokens?: number | null; completionTokens?: number | null; cachedTokens?: number | null;
  reasoningTokens?: number | null; totalTokens?: number | null;
}
export interface ModelUsage {
  model: string; realModel?: string; requests: number; promptTokens: number; completionTokens: number;
  cachedTokens: number; reasoningTokens: number; totalTokens: number; cacheHits: number; credits: number;
}
export interface Usage {
  stats: Stats; byModel: ModelUsage[];
  byAccount: { accountId: string; accountLabel: string; requests: number; totalTokens: number; credits: number }[];
  tokensPerCredit: number;
}
export interface UsageBucket { start: number; requests: number; tokens: number }
export interface UsageWindow extends Usage { from: number; to: number; buckets: UsageBucket[] }
export interface Detail { row: Row; hasBody: boolean; request?: unknown; response?: unknown }
export interface MintLimit { model: string; context?: number; output?: number; error?: string }
export interface TestResult { ok: boolean; ms: number; status: number; model: string; body?: string; error?: string }
export interface DesktopSettings { autostart: boolean; startHidden: boolean; closeToTray: boolean }

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

declare global {
  interface Window { __TAURI__?: { core: { invoke: InvokeFn } } }
}

function previewStub(): InvokeFn {
  const sampleModels: Model[] = [
    { id: "mlai-code", real: "pixis-opus", context: 1048576, output: 32768 },
    { id: "opus", real: "pixis-opus", context: 1048576, output: 32768 },
    { id: "glm", real: "glm-5.3", context: 200000, output: 32768 }
  ];
  // Deterministic sample log so virtualization, filters and the drawer are testable in a plain browser.
  const MODEL_PAIRS: [string, string][] = [["opus", "pixis-opus"], ["glm", "glm-5.3"], ["mlai-code", "pixis-opus"], ["sonnet", "pixis-sonnet"]];
  const ACCOUNT_PAIRS: [string, string][] = [["acc-work", "Work"], ["acc-home", "Home"], ["unattributed", "Unattributed"]];
  const EFFORTS = ["low", "inherit", "high", "medium"];
  const STATUSES = [200, 200, 200, 200, 200, 200, 200, 400, 429, 500];
  const base = Date.now();
  const sampleRows: Row[] = Array.from({ length: 300 }, (_, i) => {
    const [model, realModel] = MODEL_PAIRS[i % MODEL_PAIRS.length];
    const [accountId, accountLabel] = ACCOUNT_PAIRS[i % ACCOUNT_PAIRS.length];
    const noTokens = i % 19 === 7;
    const promptTokens = noTokens ? null : 300 + ((i * 137) % 8000);
    const completionTokens = noTokens ? null : 10 + ((i * 53) % 900);
    return {
      id: 300 - i,
      ts: new Date(base - i * 47000).toISOString(),
      accountId,
      accountLabel,
      model,
      realModel,
      stream: i % 3 !== 0,
      status: STATUSES[i % STATUSES.length],
      ms: 300 + ((i * 911) % 9000),
      effort: i % 11 === 0 ? null : EFFORTS[i % EFFORTS.length],
      firstTokenMs: i % 13 === 0 ? null : 120 + ((i * 71) % 2000),
      hasBody: i % 23 !== 0,
      promptTokens,
      completionTokens,
      cachedTokens: promptTokens === null ? null : i % 5 === 0 ? null : Math.round(promptTokens * 0.7),
      reasoningTokens: noTokens ? null : i % 4 === 0 ? null : 20 + (i % 80),
      totalTokens: promptTokens === null || completionTokens === null ? null : promptTokens + completionTokens,
    };
  });
  const sum = (pick: (r: Row) => number | null | undefined) => sampleRows.reduce((n, r) => n + (pick(r) ?? 0), 0);
  const sampleUsage: Usage = {
    stats: {
      requests: sampleRows.length,
      errors: sampleRows.filter((r) => r.status >= 400).length,
      promptTokens: sum((r) => r.promptTokens),
      completionTokens: sum((r) => r.completionTokens),
      cachedTokens: sum((r) => r.cachedTokens),
      reasoningTokens: sum((r) => r.reasoningTokens),
      totalTokens: sum((r) => r.totalTokens),
      cacheHits: sampleRows.filter((r) => (r.cachedTokens ?? 0) > 0).length,
      credits: sum((r) => r.totalTokens) / 1000000,
    },
    byModel: [{ model: "opus", realModel: "pixis-opus", requests: 150, promptTokens: 1831, completionTokens: 128, cachedTokens: 1792, reasoningTokens: 96, totalTokens: 1959, cacheHits: 1, credits: 0.001959 }],
    byAccount: [
      { accountId: "acc-work", accountLabel: "Work", requests: 100, totalTokens: 1959, credits: 0.001959 },
      { accountId: "unattributed", accountLabel: "Unattributed", requests: 100, totalTokens: 412, credits: 0.000412 }
    ],
    tokensPerCredit: 1000000
  };
  const sampleAccounts: Accounts = {
    accounts: [
      {
        id: "acc-work", label: "Work", server: "https://code.ml.ai", tokenHint: "mlaid_eyJ…",
        createdAt: new Date(Date.now() - 12 * 864e5).toISOString(), lastUsed: new Date().toISOString(), active: true,
        expires: Date.now() + 90 * 864e5, lastCheckAt: new Date(Date.now() - 4 * 60000).toISOString(), lastCheckOk: true, lastError: null,
      },
      {
        id: "acc-home", label: "Home", server: "https://code.ml.ai", tokenHint: "mlaid_9f2…",
        createdAt: new Date(Date.now() - 3 * 864e5).toISOString(), lastUsed: null, active: false,
        expires: Date.now() + 4 * 864e5, lastCheckAt: null, lastCheckOk: null, lastError: null,
      },
    ],
    active: "acc-work",
    path: "C:\\Users\\Lenovo\\AppData\\Local\\mlai-bridge\\accounts.json"
  };
  let runningState: RunningState = { running: true, baseUrl: "http://127.0.0.1:8787", v1: "http://127.0.0.1:8787/v1", key: "mlba_preview_key", addr: "127.0.0.1:8787" };
  const config: Config = {
    host: "127.0.0.1", port: 8787, server: "https://code.ml.ai", apiKey: "mlba_preview_key",
    project: "a6be2c98-de50-42b1-bb1f-4fd6bc00646e", folder: "Jambo-Card-Frontend",
    runTask: "Local model bridge session", defaultModel: "mlai-code",
    aliases: { "mlai-code": "pixis-opus", opus: "pixis-opus", glm: "glm-5.3" },
    reasoning: "native", reasoningEffort: "low", alwaysIncludeUsage: true, allowRawModelIds: true,
    tokenSafetyMs: 300000, requestTimeoutMs: 900000, maxTokensFloor: 0,
    limitsCache: "limits.cache.json", historyPath: "requests.jsonl", bodyKeep: 200, bodyMaxBytes: 67108864,
    accountsPath: "accounts.json"
  };
  const desktop: DesktopSettings = { autostart: false, startHidden: false, closeToTray: false };
  let nextId = 301;
  let portConflictShown = false;
  let historyCalls = 0;
  return async (cmd, args) => {
    if (cmd === "bootstrap") {
      // `?preview-empty` shows the first-run state without touching the sample data.
      const empty = location.search.includes("preview-empty");
      return {
        configPath: "C:\\Users\\Lenovo\\AppData\\Local\\mlai-bridge\\config.json",
        configExists: !empty,
        config,
        credential: empty ? null : { store: "account", server: "https://code.ml.ai", label: "Work", tokenPrefix: "mlaid_" },
        running: empty ? { running: false } : runningState,
        models: sampleModels,
        stats: empty ? undefined : sampleUsage.stats,
        accounts: empty ? { accounts: [], active: null } : sampleAccounts,
      };
    }
    if (cmd === "history") {
      if (location.search.includes("preview-empty")) return [];
      historyCalls += 1;
      // Every third poll adds a row, so live/paused behaviour is visible in the preview.
      if (historyCalls % 3 === 0) {
        const [model, realModel] = MODEL_PAIRS[nextId % MODEL_PAIRS.length];
        sampleRows.unshift({
          id: nextId, ts: new Date().toISOString(), accountId: "acc-work", accountLabel: "Work", model, realModel,
          stream: true, status: 200, ms: 900 + (nextId % 3000), effort: "low", firstTokenMs: 180, hasBody: true,
          promptTokens: 1200 + (nextId % 400), completionTokens: 64, cachedTokens: 900, reasoningTokens: 12,
          totalTokens: 1264 + (nextId % 400),
        });
        nextId += 1;
        if (sampleRows.length > 400) sampleRows.pop();
      }
      return sampleRows.slice();
    }
    if (cmd === "usage") return location.search.includes("preview-empty") ? { stats: undefined, byModel: [], byAccount: [], tokensPerCredit: 1000000 } : sampleUsage;
    if (cmd === "usage_window") {
      const from = Number(args?.fromMs ?? 0);
      const to = Number(args?.toMs ?? Date.now());
      const count = Math.max(1, Number(args?.buckets ?? 24));
      const rows = sampleRows.filter((r) => {
        const ts = new Date(r.ts).getTime();
        return ts >= from && ts <= to;
      });
      const tokens = (r: Row) => r.totalTokens ?? (r.promptTokens ?? 0) + (r.completionTokens ?? 0);
      const span = Math.max(1, to - from);
      const buckets = Array.from({ length: count }, (_, i) => ({ start: Math.round(from + (span * i) / count), requests: 0, tokens: 0 }));
      for (const r of rows) {
        const index = Math.min(count - 1, Math.max(0, Math.floor(((new Date(r.ts).getTime() - from) / span) * count)));
        buckets[index].requests += 1;
        buckets[index].tokens += tokens(r);
      }
      const models = Array.from(new Set(rows.map((r) => r.model)));
      return {
        from,
        to,
        stats: {
          requests: rows.length,
          errors: rows.filter((r) => r.status >= 400).length,
          promptTokens: rows.reduce((n, r) => n + (r.promptTokens ?? 0), 0),
          completionTokens: rows.reduce((n, r) => n + (r.completionTokens ?? 0), 0),
          cachedTokens: rows.reduce((n, r) => n + (r.cachedTokens ?? 0), 0),
          reasoningTokens: rows.reduce((n, r) => n + (r.reasoningTokens ?? 0), 0),
          totalTokens: rows.reduce((n, r) => n + tokens(r), 0),
          cacheHits: rows.filter((r) => (r.cachedTokens ?? 0) > 0).length,
          credits: rows.reduce((n, r) => n + tokens(r), 0) / 1000000,
        },
        byModel: models.map((model) => {
          const mine = rows.filter((r) => r.model === model);
          return {
            model,
            realModel: mine[0]?.realModel,
            requests: mine.length,
            promptTokens: mine.reduce((n, r) => n + (r.promptTokens ?? 0), 0),
            completionTokens: mine.reduce((n, r) => n + (r.completionTokens ?? 0), 0),
            cachedTokens: mine.reduce((n, r) => n + (r.cachedTokens ?? 0), 0),
            reasoningTokens: mine.reduce((n, r) => n + (r.reasoningTokens ?? 0), 0),
            totalTokens: mine.reduce((n, r) => n + tokens(r), 0),
            cacheHits: mine.filter((r) => (r.cachedTokens ?? 0) > 0).length,
            credits: mine.reduce((n, r) => n + tokens(r), 0) / 1000000,
          };
        }),
        byAccount: Array.from(new Set(rows.map((r) => r.accountId || "unattributed"))).map((id) => {
          const mine = rows.filter((r) => (r.accountId || "unattributed") === id);
          const total = mine.reduce((n, r) => n + tokens(r), 0);
          return { accountId: id, accountLabel: mine[0]?.accountLabel || id, requests: mine.length, totalTokens: total, credits: total / 1000000 };
        }),
        tokensPerCredit: 1000000,
        buckets,
      };
    }
    if (cmd === "accounts" || cmd === "accounts_add" || cmd === "accounts_import" || cmd === "accounts_remove") {
      return location.search.includes("preview-empty") && cmd === "accounts" ? { accounts: [], active: null } : sampleAccounts;
    }
    if (cmd === "accounts_activate") {
      const id = String(args?.id || "");
      sampleAccounts.active = id;
      for (const a of sampleAccounts.accounts) a.active = a.id === id;
      return { ok: true, restarted: runningState.running, running: runningState };
    }
    if (cmd === "accounts_check") {
      const id = args?.id ? String(args.id) : null;
      // `?preview-health=error` makes the Home account fail, so the error state is testable.
      const failHome = location.search.includes("preview-health=error");
      return sampleAccounts.accounts
        .filter((a) => !id || a.id === id)
        .map((a) => {
          const error = failHome && a.id === "acc-home" ? "That token was rejected: 401 (HTTP 401)" : null;
          a.lastCheckAt = new Date().toISOString();
          a.lastCheckOk = !error;
          a.lastError = error;
          return { id: a.id, ok: !error, ms: 110 + a.id.length * 7, error };
        });
    }
    if (cmd === "request_detail") {
      const row = sampleRows.find((r) => r.id === Number(args?.id)) || sampleRows[0];
      const bodies = row.hasBody
        ? {
            request: { model: row.realModel, messages: [{ role: "user", content: "preview request" }], reasoning_effort: row.effort, max_tokens: 256, stream: row.stream },
            response: { stream: true, assembled: true, content: "Preview response text.", reasoning: "Preview reasoning.", toolCalls: [], finishReason: "stop", usage: { prompt_tokens: row.promptTokens, completion_tokens: row.completionTokens, total_tokens: row.totalTokens }, chunks: 12 },
          }
        : { request: null, response: null };
      return { row, hasBody: row.hasBody, ...bodies };
    }
    if (cmd === "start") {
      // `?preview-port-conflict` fails the first start so the conflict dialog is testable.
      if (location.search.includes("preview-port-conflict") && !portConflictShown) {
        portConflictShown = true;
        throw new Error("The port 8787 is already in use. Pick another port in Settings, or use the suggested one.");
      }
      runningState = { running: true, baseUrl: "http://127.0.0.1:8787", v1: "http://127.0.0.1:8787/v1", key: config.apiKey, addr: "127.0.0.1:8787" };
      return runningState;
    }
    if (cmd === "stop") { runningState = { running: false }; return runningState; }
    if (cmd === "test_connection") {
      if (!runningState.running) throw new Error("The proxy is not running. Start it first.");
      return { ok: true, ms: 132, status: 200, model: String(args?.model || config.defaultModel), body: "ok" };
    }
    if (cmd === "desktop_settings") return desktop;
    if (cmd === "set_desktop_settings") {
      const patch = args?.settings;
      if (patch && typeof patch === "object") {
        for (const [key, value] of Object.entries(patch)) {
          if ((key === "autostart" || key === "startHidden" || key === "closeToTray") && typeof value === "boolean") desktop[key] = value;
        }
      }
      return desktop;
    }
    if (cmd === "free_port") {
      const after = Number(args?.after) || 8787;
      return after + 1;
    }
    if (cmd === "save_config") return { ok: true, restartRequired: true };
    if (cmd === "mint_limits") return sampleModels.map((m) => ({ model: m.real, context: m.context, output: m.output }));
    return { ok: true };
  };
}

const rawInvoke: InvokeFn = window.__TAURI__?.core ? window.__TAURI__.core.invoke : previewStub();

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return rawInvoke(cmd, args) as Promise<T>;
}
