import { useEffect, useMemo, useState } from "react";
import { ArrowUpIcon, InboxIcon, Trash2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { Confirm } from "@/components/confirm";
import { PageHeader } from "@/components/page-header";
import { FilterBar, type Option } from "@/components/requests/filter-bar";
import { DetailSheet } from "@/components/requests/detail-sheet";
import { LogTable } from "@/components/requests/log-table";
import { fmt } from "@/lib/format";
import {
  accountKey, accountName, DEFAULT_SORT, effortKey, FILTER_KEY, loadFilters, localKey, localV1, matchesFilter, nextSort, sortRows, type Filters, type Sort,
} from "@/lib/requests";
import { useBootstrap, useClearHistory, useHistory } from "@/queries";
import type { Row } from "@/tauri";

const unique = (values: string[]): Option[] => Array.from(new Set(values)).sort().map((v) => ({ value: v, label: v }));

export function RequestsPage({ detailId, onOpen, onClose }: { detailId: number | null; onOpen: (id: number) => void; onClose: () => void }) {
  const [live, setLive] = useState(true);
  const [away, setAway] = useState(false);
  const [scrollKey, setScrollKey] = useState(0);
  const [filters, setFilters] = useState<Filters>(loadFilters);
  const [sort, setSort] = useState<Sort>(DEFAULT_SORT);
  const [snapshot, setSnapshot] = useState<Row[]>([]);

  const { data, isLoading } = useHistory(true);
  const boot = useBootstrap().data;
  const clear = useClearHistory();
  const rows = useMemo(() => data || [], [data]);

  // While paused the table keeps showing a frozen copy; polling continues so new rows can be counted.
  const paused = !live || away || detailId !== null;
  useEffect(() => { if (!paused) setSnapshot(rows); }, [paused, rows]);
  const shown = paused ? snapshot : rows;
  const frozenMax = snapshot.reduce((max, r) => Math.max(max, r.id), 0);
  const pending = paused ? rows.filter((r) => r.id > frozenMax).length : 0;

  useEffect(() => { localStorage.setItem(FILTER_KEY, JSON.stringify(filters)); }, [filters]);

  const accounts = useMemo(() => {
    const labels = new Map<string, string>();
    for (const r of rows) labels.set(accountKey(r), accountName(r));
    for (const a of boot?.accounts?.accounts || []) labels.set(a.id, a.label);
    return Array.from(labels, ([value, label]) => ({ value, label })).sort((a, b) => a.label.localeCompare(b.label));
  }, [rows, boot]);

  const models = useMemo(() => unique(rows.map((r) => r.model)), [rows]);
  const efforts = useMemo(() => unique(rows.map(effortKey)), [rows]);

  const filtered = shown.filter((r) => matchesFilter(r, filters));
  const sorted = useMemo(() => sortRows(filtered, sort), [filtered, sort]);
  const stats = boot?.stats;
  const curlContext = { url: localV1(boot?.running, boot?.config), key: localKey(boot?.running, boot?.config) };

  const reset = () => setFilters({ accounts: [], models: [], efforts: [], classes: [], q: "" });
  const applyPatch = (patch: Partial<Filters>) => setFilters((f) => ({ ...f, ...patch }));

  return (
    <>
      <PageHeader
        title="Requests"
        description={stats ? fmt(stats.requests) + " requests \u00b7 " + fmt(stats.errors) + " errors \u00b7 " + fmt(stats.totalTokens) + " tokens" : undefined}
      >
        <Label className="gap-2 text-xs font-normal text-muted-foreground">
          <Switch size="sm" checked={live} onCheckedChange={setLive} />Live
        </Label>
        <Confirm
          trigger={<Button variant="outline" size="sm" disabled={!rows.length || clear.isPending}><Trash2Icon />Clear history</Button>}
          title="Clear request history?"
          description="This deletes every logged request and its stored bodies. Usage totals reset."
          action="Clear history"
          onConfirm={() => clear.mutate(undefined, { onSuccess: () => toast.add({ type: "success", title: "History cleared" }) })}
        />
      </PageHeader>

      <FilterBar
        accounts={accounts}
        models={models}
        efforts={efforts}
        filters={filters}
        onChange={setFilters}
        onReset={reset}
        onJump={onOpen}
        count={rows.length ? sorted.length + " of " + shown.length + " shown" + (paused ? " \u00b7 paused" : "") : ""}
      />

      {pending > 0 && (
        <div className="flex justify-center">
          <Button size="sm" onClick={() => { setLive(true); setAway(false); setScrollKey((k) => k + 1); }} className="rounded-full shadow">
            <ArrowUpIcon />{pending} new {"\u2014"} show
          </Button>
        </div>
      )}

      {isLoading ? (
        <div className="flex h-64 items-center justify-center"><Spinner /></div>
      ) : sorted.length ? (
        <LogTable
          rows={sorted}
          sort={sort}
          onSort={(key) => setSort((s) => nextSort(s, key))}
          onOpen={onOpen}
          onFilter={applyPatch}
          onAwayFromTop={setAway}
          scrollToTopKey={scrollKey}
          selectedId={detailId}
          curlContext={curlContext}
        />
      ) : (
        <Empty className="py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon"><InboxIcon /></EmptyMedia>
            <EmptyTitle>{rows.length ? "No requests match" : "No requests yet"}</EmptyTitle>
            <EmptyDescription>{rows.length ? "Reset the filters to see every request." : "Send a request through the proxy and it shows up here."}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}

      <p className="text-xs text-muted-foreground">
        Click a row to inspect it {"\u00b7"} Arrow keys move, Enter opens {"\u00b7"} [ and ] walk the list {"\u00b7"} paste <span className="font-mono">#id</span> in search to jump
      </p>

      <DetailSheet id={detailId} list={sorted} onOpen={onOpen} onClose={onClose} />
    </>
  );
}
