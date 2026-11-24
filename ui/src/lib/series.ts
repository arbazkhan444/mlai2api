import type { Row } from "@/tauri";

/**
 * Count/sum requests in the last `minutes`, split into `buckets` slices, oldest first.
 * Rows older than the window are ignored.
 */
export function recentBuckets(rows: Row[], pick: (r: Row) => number, now = Date.now(), minutes = 60, buckets = 12): number[] {
  const size = (minutes * 60000) / buckets;
  const out = new Array<number>(buckets).fill(0);
  for (const row of rows) {
    const ts = new Date(row.ts).getTime();
    if (!ts || ts > now) continue;
    const index = buckets - 1 - Math.floor((now - ts) / size);
    if (index >= 0 && index < buckets) out[index] += pick(row);
  }
  return out;
}

export function totalOf(row: Row): number {
  if (row.totalTokens != null) return row.totalTokens;
  return (row.promptTokens ?? 0) + (row.completionTokens ?? 0);
}

export function ratioOf(row: Row): number {
  return row.promptTokens ? (row.cachedTokens ?? 0) / row.promptTokens : 0;
}
