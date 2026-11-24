import { lazy, Suspense, useEffect, useState } from "react";
import { RefreshCwIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { toast } from "@/components/ui/toast";
import { PageHeader } from "@/components/page-header";
import { StatCards } from "@/components/stat-cards";
import { credits, fmt, percent } from "@/lib/format";
import type { Filters } from "@/lib/requests";
import { qc, useUsageWindow } from "@/queries";

const UsageCharts = lazy(() => import("@/components/usage/usage-charts"));
const RANGE_KEY = "mlaiUsageRange";

const RANGES = [
  { id: "1h", label: "Last hour", ms: 3600_000, buckets: 12 },
  { id: "24h", label: "Last 24 hours", ms: 24 * 3600_000, buckets: 24 },
  { id: "7d", label: "Last 7 days", ms: 7 * 24 * 3600_000, buckets: 28 },
  { id: "30d", label: "Last 30 days", ms: 30 * 24 * 3600_000, buckets: 30 },
  { id: "all", label: "All time", ms: 0, buckets: 1 },
] as const;

const num = "text-right tabular-nums";

function EmptyRow({ cols, text }: { cols: number; text: string }) {
  return <TableRow><TableCell colSpan={cols} className="py-8 text-center text-muted-foreground">{text}</TableCell></TableRow>;
}

export function UsagePage({ onFilterRequests }: { onFilterRequests: (patch: Partial<Filters>) => void }) {
  const [range, setRange] = useState<string>(() => localStorage.getItem(RANGE_KEY) || "24h");
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => { localStorage.setItem(RANGE_KEY, range); }, [range]);
  // The window end moves forward on a slow tick so the charts stay live without a new query key every render.
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(id);
  }, []);

  const spec = RANGES.find((r) => r.id === range) ?? RANGES[1];
  const from = spec.ms ? now - spec.ms : 0;
  const usage = useUsageWindow(spec.id, from, now, spec.buckets);
  const u = usage.data;
  const s = u?.stats;
  const hitRate = s?.requests ? percent(s.cachedTokens, s.promptTokens) + " of input was cached" : "no requests in this range";

  const refresh = async () => {
    setNow(Date.now());
    await qc.refetchQueries({ queryKey: ["usage"] });
    toast.add({ title: "Usage refreshed", timeout: 2500 });
  };

  return (
    <>
      <PageHeader title="Usage" description="Tokens, cache hits and spend for one time range. Click a bar to see the matching requests.">
        <Select items={RANGES.map((r) => ({ value: r.id, label: r.label }))} value={range} onValueChange={(v) => setRange((v as string) ?? "24h")}>
          <SelectTrigger size="sm" className="w-40" aria-label="Time range"><SelectValue /></SelectTrigger>
          <SelectContent>
            {RANGES.map((r) => <SelectItem key={r.id} value={r.id}>{r.label}</SelectItem>)}
          </SelectContent>
        </Select>
        <Button variant="outline" size="sm" onClick={refresh} disabled={usage.isFetching}>
          {usage.isFetching ? <Spinner /> : <RefreshCwIcon />}Refresh
        </Button>
      </PageHeader>

      <StatCards stats={s} />
      <p className="text-xs text-muted-foreground">
        {spec.label}: {hitRate}. Spend uses 1 credit = 1,000,000 tokens. Cached input is a subset of input; output includes reasoning.
      </p>

      {range !== "all" && u && (
        <Suspense fallback={<div className="flex h-56 items-center justify-center rounded-md border"><Spinner /></div>}>
          <UsageCharts
            buckets={u.buckets}
            byModel={u.byModel}
            byAccount={u.byAccount}
            onModel={(model) => onFilterRequests({ models: [model] })}
            onAccount={(accountId) => onFilterRequests({ accounts: [accountId] })}
          />
        </Suspense>
      )}

      <Card>
        <CardHeader><CardTitle>Per model</CardTitle></CardHeader>
        <CardContent>
          <Table className="text-xs">
            <TableHeader>
              <TableRow>
                <TableHead>Model</TableHead><TableHead>Maps to</TableHead><TableHead className={num}>Requests</TableHead>
                <TableHead className={num}>Input</TableHead><TableHead className={num}>Output</TableHead><TableHead className={num}>Cached</TableHead>
                <TableHead className={num}>Hit rate</TableHead><TableHead className={num}>Spend</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!u ? null : u.byModel.length ? u.byModel.map((m) => (
                <TableRow key={m.model}>
                  <TableCell className="font-mono">{m.model}</TableCell><TableCell className="font-mono text-muted-foreground">{m.realModel || ""}</TableCell>
                  <TableCell className={num}>{fmt(m.requests)}</TableCell><TableCell className={num}>{fmt(m.promptTokens)}</TableCell>
                  <TableCell className={num}>{fmt(m.completionTokens)}</TableCell><TableCell className={num}>{fmt(m.cachedTokens)}</TableCell>
                  <TableCell className={num}>{percent(m.cachedTokens, m.promptTokens)}</TableCell><TableCell className={num}>{credits(m.totalTokens)}</TableCell>
                </TableRow>
              )) : <EmptyRow cols={8} text="No usage in this range. Send a request through the proxy." />}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Per account</CardTitle></CardHeader>
        <CardContent>
          <Table className="text-xs">
            <TableHeader>
              <TableRow><TableHead>Account</TableHead><TableHead className={num}>Requests</TableHead><TableHead className={num}>Tokens</TableHead><TableHead className={num}>Spend</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {!u ? null : u.byAccount.length ? u.byAccount.map((a) => (
                <TableRow key={a.accountId}>
                  <TableCell>{a.accountLabel}</TableCell><TableCell className={num}>{fmt(a.requests)}</TableCell>
                  <TableCell className={num}>{fmt(a.totalTokens)}</TableCell><TableCell className={num}>{credits(a.totalTokens)}</TableCell>
                </TableRow>
              )) : <EmptyRow cols={4} text="No usage in this range." />}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}
