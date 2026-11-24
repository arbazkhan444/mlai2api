import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { credits, fmt, percent } from "@/lib/format";
import { ratioOf, recentBuckets, totalOf } from "@/lib/series";
import type { Row, Stats } from "@/tauri";

/** Tiny line chart; no dependency, follows the theme through currentColor. */
function Sparkline({ values }: { values: number[] }) {
  const max = Math.max(1, ...values);
  const step = values.length > 1 ? 100 / (values.length - 1) : 100;
  const points = values.map((v, i) => (i * step).toFixed(2) + "," + (22 - (v / max) * 20).toFixed(2)).join(" ");
  return (
    <svg viewBox="0 0 100 24" preserveAspectRatio="none" className="h-6 w-full text-primary/70" aria-hidden="true">
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Tile({ label, value, series }: { label: string; value: string; series?: number[] }) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle className="text-xl font-semibold tabular-nums">{value}</CardTitle>
        {series && <Sparkline values={series} />}
        {series && <span className="text-[11px] text-muted-foreground">last 60 min</span>}
      </CardHeader>
    </Card>
  );
}

export function StatCards({ stats, rows }: { stats?: Stats; rows?: Row[] }) {
  const series = rows ? (pick: (r: Row) => number) => recentBuckets(rows, pick) : null;
  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
      <Tile label="Requests" value={fmt(stats?.requests)} series={series?.(() => 1)} />
      <Tile label="Tokens" value={fmt(stats?.totalTokens)} series={series?.(totalOf)} />
      <Tile label="Cache hit rate" value={percent(stats?.cachedTokens, stats?.promptTokens)} series={series?.(ratioOf)} />
      <Tile label="Spend (credits)" value={stats ? credits(stats.totalTokens) : "\u2014"} series={series?.(totalOf)} />
    </div>
  );
}
