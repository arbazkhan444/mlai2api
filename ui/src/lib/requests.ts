import type { Config, Detail, Row, RunningState } from "@/tauri";

export type StatusClass = "2xx" | "4xx" | "5xx";

export interface Filters {
  accounts: string[];
  models: string[];
  efforts: string[];
  classes: StatusClass[];
  q: string;
}

export const EMPTY_FILTERS: Filters = { accounts: [], models: [], efforts: [], classes: [], q: "" };

export const FILTER_KEY = "mlaiReqFilters";

export function loadFilters(): Filters {
  try {
    const saved = JSON.parse(localStorage.getItem(FILTER_KEY) || "null") as Partial<Filters> | null;
    if (!saved) return { ...EMPTY_FILTERS };
    return {
      accounts: saved.accounts || [],
      models: saved.models || [],
      efforts: saved.efforts || [],
      classes: saved.classes || [],
      q: saved.q || "",
    };
  } catch {
    return { ...EMPTY_FILTERS };
  }
}

/** Replace the Requests filters before navigating there, e.g. after clicking a Usage chart. */
export function focusRequests(patch: Partial<Filters>) {
  localStorage.setItem(FILTER_KEY, JSON.stringify({ ...EMPTY_FILTERS, ...patch }));
}

export const accountKey = (r: Row) => r.accountId || "unattributed";
export const accountName = (r: Row) => r.accountLabel || (r.accountId ? r.accountId : "Unattributed");
export const effortKey = (r: Row) => r.effort || "inherit";

export function statusClass(status: number): StatusClass {
  if (status === 0 || status >= 500) return "5xx";
  if (status >= 400) return "4xx";
  return "2xx";
}

/** A null total means "not reported", which is different from zero. */
export function totalTokens(r: Row): number | null {
  if (r.totalTokens !== null && r.totalTokens !== undefined) return r.totalTokens;
  if (r.promptTokens == null && r.completionTokens == null) return null;
  return (r.promptTokens ?? 0) + (r.completionTokens ?? 0);
}

/** `#12` or `12`; anything else is free text. */
export function pastedId(q: string): number | null {
  const m = /^#?(\d+)$/.exec(q.trim());
  return m ? Number(m[1]) : null;
}

export function matchesFilter(r: Row, f: Filters): boolean {
  if (f.accounts.length && !f.accounts.includes(accountKey(r))) return false;
  if (f.models.length && !f.models.includes(r.model)) return false;
  if (f.efforts.length && !f.efforts.includes(effortKey(r))) return false;
  if (f.classes.length && !f.classes.includes(statusClass(r.status))) return false;
  const q = f.q.trim().toLowerCase();
  if (!q || pastedId(q) !== null) return true;
  return [r.id, r.model, r.realModel, accountName(r), effortKey(r)].join(" ").toLowerCase().includes(q);
}

export type SortKey = "time" | "duration" | "tokens";
export interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}
export const DEFAULT_SORT: Sort = { key: "time", dir: "desc" };

export function sortRows(rows: Row[], sort: Sort): Row[] {
  const value = (r: Row): number => {
    if (sort.key === "duration") return r.ms;
    if (sort.key === "tokens") return totalTokens(r) ?? -1;
    return new Date(r.ts).getTime() || r.id;
  };
  const out = [...rows].sort((a, b) => value(a) - value(b) || a.id - b.id);
  return sort.dir === "desc" ? out.reverse() : out;
}

/** Clicking the active column flips the direction; a new column starts descending. */
export function nextSort(current: Sort, key: SortKey): Sort {
  return current.key === key ? { key, dir: current.dir === "desc" ? "asc" : "desc" } : { key, dir: "desc" };
}

export function compactNumber(n: number | null | undefined): string {
  if (n === null || n === undefined) return "\u2014";
  if (n < 1000) return String(n);
  if (n < 1_000_000) return (Math.round(n / 100) / 10).toLocaleString() + "k";
  return (Math.round(n / 100_000) / 10).toLocaleString() + "M";
}

export function duration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "\u2014";
  if (ms < 1000) return ms + " ms";
  return (Math.round(ms / 100) / 10).toLocaleString() + " s";
}

export function localV1(running?: RunningState | null, config?: Config | null): string {
  if (running?.v1) return running.v1.replace(/\/+$/, "");
  const host = (config?.host as string) || "127.0.0.1";
  const port = (config?.port as number) || 8787;
  return "http://" + host + ":" + port + "/v1";
}

/** The proxy's own key, which clients must send as a bearer token. */
export function localKey(running?: RunningState | null, config?: Config | null): string {
  return running?.key || (config?.apiKey as string) || "";
}

/** The stored request body as the proxy sent it upstream. */
export function requestBodyText(detail: Detail): string | null {
  const body = detail.request;
  if (body === null || body === undefined) return null;
  if (typeof body === "string") return body;
  return JSON.stringify(body, null, 2);
}

const singleQuote = (s: string) => s.replace(/'/g, "'\\''");
const psQuote = (s: string) => s.replace(/'/g, "''");

export function buildCurl({ shell, url, key, body }: { shell: "bash" | "powershell"; url: string; key: string; body: string }): string {
  const target = url.replace(/\/+$/, "") + "/chat/completions";
  if (shell === "powershell") {
    return [
      "$headers = @{ 'Content-Type' = 'application/json'; Authorization = 'Bearer " + psQuote(key) + "' }",
      "$body = @'",
      body,
      "'@",
      "Invoke-RestMethod -Uri '" + psQuote(target) + "' -Method Post -Headers $headers -Body $body",
    ].join("\n");
  }
  return [
    "curl '" + singleQuote(target) + "' \\",
    "  -H 'Content-Type: application/json' \\",
    "  -H 'Authorization: Bearer " + singleQuote(key) + "' \\",
    "  --data-raw '" + singleQuote(body) + "'",
  ].join("\n");
}
