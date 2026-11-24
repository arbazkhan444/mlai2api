export const NO_BODY = "(body not available \u2014 pruned or never written)";

export function fmt(n: unknown): string {
  if (n === null || n === undefined || n === "") return "\u2014";
  return Number(n).toLocaleString();
}

/** ML AI Code's unit: 1 credit = 1,000,000 tokens. */
export function credits(tokens: unknown): string {
  const c = (Number(tokens) || 0) / 1000000;
  if (!c) return "0";
  if (c >= 1000) return Math.round(c).toLocaleString();
  if (c >= 100) return String(Math.round(c));
  if (c >= 1) return String(Math.round(c * 10) / 10);
  if (c >= 0.01) return String(Math.round(c * 100) / 100);
  if (c >= 0.0001) return String(Math.round(c * 10000) / 10000);
  return "<0.0001";
}

export function percent(part: unknown, whole: unknown): string {
  if (!whole) return "\u2014";
  return Math.round(((Number(part) || 0) / Number(whole)) * 1000) / 10 + "%";
}

export function timeOf(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return String(iso);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

/** "just now", "12 min ago", "3 h ago", "5 d ago"; used for health timestamps. */
export function ago(iso: string | null | undefined): string {
  if (!iso) return "never";
  const ms = Date.now() - new Date(iso).getTime();
  if (!Number.isFinite(ms)) return String(iso);
  if (ms < 60000) return "just now";
  const minutes = Math.floor(ms / 60000);
  if (minutes < 60) return minutes + " min ago";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours + " h ago";
  return Math.floor(hours / 24) + " d ago";
}

export function pretty(value: unknown): string {
  const text = JSON.stringify(value, null, 2);
  if (!text) return "(empty)";
  if (text.length > 160000) return text.slice(0, 160000) + "\n\u2026 truncated for display (" + text.length + " chars)";
  return text;
}
