import { useEffect, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDownIcon, ArrowUpIcon, CopyIcon, MoreHorizontalIcon, TerminalIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSub, DropdownMenuSubContent, DropdownMenuSubTrigger, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { toast } from "@/components/ui/toast";
import { copyText } from "@/components/copy-button";
import { fmt, timeOf } from "@/lib/format";
import { accountKey, accountName, buildCurl, compactNumber, duration, effortKey, requestBodyText, statusClass, type Filters, type Sort, type SortKey } from "@/lib/requests";
import { invoke } from "@/tauri";
import type { Detail, Row } from "@/tauri";
import { cn } from "@/lib/utils";

const GRID = "grid grid-cols-[5.5rem_3.75rem_minmax(9rem,1.8fr)_7.5rem_4.5rem_7rem_4.75rem_4.5rem_2rem] items-center gap-2 px-3";
const COMPACT_GRID = "grid grid-cols-[5.5rem_3.75rem_minmax(9rem,1fr)_6.5rem_4.75rem] items-center gap-2 px-3";

function StatusBadge({ status }: { status: number }) {
  const cls = statusClass(status);
  return (
    <Badge
      variant="outline"
      className={cn(
        "justify-center px-1.5 tabular-nums",
        cls === "2xx" && "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
        cls === "4xx" && "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-400",
        cls === "5xx" && "border-destructive/30 bg-destructive/10 text-destructive",
      )}
    >
      {status || "err"}
    </Badge>
  );
}

function Tokens({ row }: { row: Row }) {
  const parts = [
    "input " + fmt(row.promptTokens) + (row.cachedTokens != null ? " \u00b7 cached " + fmt(row.cachedTokens) : ""),
    "output " + fmt(row.completionTokens) + (row.reasoningTokens != null ? " \u00b7 reasoning " + fmt(row.reasoningTokens) : ""),
  ];
  if (row.promptTokens == null && row.completionTokens == null) parts.push("tokens not reported for this request");
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <span className="tabular-nums whitespace-nowrap text-muted-foreground">
            {compactNumber(row.promptTokens)} <span className="opacity-50">{"\u2192"}</span> {compactNumber(row.completionTokens)}
          </span>
        }
      />
      <TooltipContent side="top" className="whitespace-pre">
        {parts.join("\n")}
      </TooltipContent>
    </Tooltip>
  );
}

function RowMenu({ row, onFilter, curlContext }: { row: Row; onFilter?: (patch: Partial<Filters>) => void; curlContext?: { url: string; key: string } }) {
  const copyCurl = async (shell: "bash" | "powershell") => {
    if (!curlContext) return;
    const detail = await invoke<Detail>("request_detail", { id: row.id });
    const body = requestBodyText(detail);
    if (!body) {
      toast.add({ type: "warning", title: "No stored body for this request", description: "Enable body storage in Settings, or pick a newer request." });
      return;
    }
    if (await copyText(buildCurl({ shell, url: curlContext.url, key: curlContext.key, body }))) {
      toast.add({ type: "success", title: "Copied cURL (" + shell + ")" });
    }
  };
  const filter = (patch: Partial<Filters>) => onFilter && (onFilter(patch), toast.add({ type: "info", title: "Filter applied", description: "Clear the filter bar to see every request again." }));
  return (
    <div className="opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
      <DropdownMenu>
        <DropdownMenuTrigger render={<Button size="icon-sm" variant="ghost" aria-label={"Actions for request #" + row.id} title="Row actions"><MoreHorizontalIcon /></Button>} />
        <DropdownMenuContent align="end">
          {onFilter && (
            <>
              <DropdownMenuItem onClick={() => filter({ models: [row.model] })}>Filter by model: {row.model}</DropdownMenuItem>
              <DropdownMenuItem onClick={() => filter({ accounts: [accountKey(row)] })}>Filter by account: {accountName(row)}</DropdownMenuItem>
              <DropdownMenuItem onClick={() => filter({ efforts: [effortKey(row)] })}>Filter by effort: {effortKey(row)}</DropdownMenuItem>
              <DropdownMenuItem onClick={() => filter({ classes: [statusClass(row.status) === "5xx" ? "5xx" : statusClass(row.status) === "4xx" ? "4xx" : "2xx"] })}>Filter by status: {statusClass(row.status)}</DropdownMenuItem>
              <DropdownMenuSeparator />
            </>
          )}
          <DropdownMenuItem onClick={async () => { if (await copyText(String(row.id))) toast.add({ type: "success", title: "Copied request id" }); }}>
            <CopyIcon />Copy ID
          </DropdownMenuItem>
          <DropdownMenuSub>
            <DropdownMenuSubTrigger disabled={!curlContext}><TerminalIcon />Copy as cURL</DropdownMenuSubTrigger>
            <DropdownMenuSubContent>
              <DropdownMenuItem onClick={() => copyCurl("bash")}>bash</DropdownMenuItem>
              <DropdownMenuItem onClick={() => copyCurl("powershell")}>PowerShell</DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

function SortHead({ label, k, sort, onSort, className }: { label: string; k?: SortKey; sort?: Sort; onSort?: (k: SortKey) => void; className?: string }) {
  if (!k || !onSort) return <span className={cn("px-1 text-[11px] font-medium text-muted-foreground", className)}>{label}</span>;
  const active = sort?.key === k;
  return (
    <button
      type="button"
      onClick={() => onSort(k)}
      className={cn("flex items-center gap-1 px-1 text-left text-[11px] font-medium text-muted-foreground hover:text-foreground", active && "text-foreground", className)}
      aria-label={"Sort by " + label}
    >
      {label}
      {active && (sort?.dir === "desc" ? <ArrowDownIcon className="size-3" /> : <ArrowUpIcon className="size-3" />)}
    </button>
  );
}

export function LogTable({
  rows, compact, sort, onSort, onOpen, onFilter, onAwayFromTop, scrollToTopKey, selectedId, curlContext,
}: {
  rows: Row[];
  compact?: boolean;
  sort?: Sort;
  onSort?: (k: SortKey) => void;
  onOpen: (id: number) => void;
  onFilter?: (patch: Partial<Filters>) => void;
  onAwayFromTop?: (away: boolean) => void;
  scrollToTopKey?: number;
  selectedId?: number | null;
  curlContext?: { url: string; key: string };
}) {
  const parentRef = useRef<HTMLDivElement>(null);
  const away = useRef(false);
  const [activeId, setActiveId] = useState<number | null>(selectedId ?? null);
  const virt = useVirtualizer({
    count: compact ? 0 : rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 32,
    overscan: 12,
  });

  useEffect(() => { if (selectedId != null) setActiveId(selectedId); }, [selectedId]);
  useEffect(() => {
    if (scrollToTopKey === undefined) return;
    parentRef.current?.scrollTo({ top: 0 });
    virt.scrollToIndex(0);
  }, [scrollToTopKey, virt]);

  const onScroll = () => {
    const el = parentRef.current;
    if (!el || !onAwayFromTop) return;
    const next = el.scrollTop > 8;
    if (next !== away.current) {
      away.current = next;
      onAwayFromTop(next);
    }
  };

  const step = (delta: number) => {
    if (!rows.length) return;
    const current = rows.findIndex((r) => r.id === activeId);
    const index = Math.max(0, Math.min(rows.length - 1, (current === -1 ? 0 : current) + delta));
    setActiveId(rows[index].id);
    virt.scrollToIndex(index, { align: "auto" });
  };

  if (compact) {
    return (
      <div className="overflow-hidden rounded-md border">
        <div className={cn(COMPACT_GRID, "h-8 border-b bg-muted/40")}>
          <SortHead label="Time" />
          <SortHead label="Status" />
          <SortHead label="Model" />
          <SortHead label="Tokens" />
          <SortHead label="Duration" />
        </div>
        {rows.slice(0, 8).map((r) => (
          <div
            key={r.id}
            role="button"
            tabIndex={0}
            className={cn(COMPACT_GRID, "h-8 cursor-pointer border-b text-xs last:border-0 hover:bg-accent/50")}
            onClick={() => onOpen(r.id)}
            onKeyDown={(e) => { if (e.key === "Enter") onOpen(r.id); }}
          >
            <span className="font-mono tabular-nums text-muted-foreground">{timeOf(r.ts)}</span>
            <StatusBadge status={r.status} />
            <span className="truncate font-mono" title={r.model !== r.realModel ? r.model + " \u2192 " + r.realModel : r.model}>
              {r.model !== r.realModel ? <>{r.model} <span className="text-muted-foreground">{"\u2192"} {r.realModel}</span></> : r.model}
            </span>
            <span className="tabular-nums text-muted-foreground">{compactNumber(r.promptTokens)} {"\u2192"} {compactNumber(r.completionTokens)}</span>
            <span className="text-right tabular-nums text-muted-foreground">{duration(r.ms)}</span>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="flex h-[calc(100svh-16rem)] min-h-80 flex-col overflow-hidden rounded-md border">
      <div className={cn(GRID, "h-8 shrink-0 border-b bg-muted/40")}>
        <SortHead label="Time" k="time" sort={sort} onSort={onSort} />
        <SortHead label="Status" />
        <SortHead label="Model" />
        <SortHead label="Account" />
        <SortHead label="Effort" />
        <SortHead label="Tokens" k="tokens" sort={sort} onSort={onSort} />
        <SortHead label="Duration" k="duration" sort={sort} onSort={onSort} />
        <SortHead label="TTFT" />
        <span />
      </div>
      <div
        ref={parentRef}
        role="grid"
        tabIndex={0}
        aria-label="Request log"
        onScroll={onScroll}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") { e.preventDefault(); step(1); }
          if (e.key === "ArrowUp") { e.preventDefault(); step(-1); }
          if (e.key === "Enter" && activeId != null) { e.preventDefault(); onOpen(activeId); }
        }}
        className="min-h-0 flex-1 overflow-auto outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
      >
        <div style={{ height: virt.getTotalSize(), position: "relative" }}>
          {virt.getVirtualItems().map((v) => {
            const r = rows[v.index];
            const selected = r.id === activeId;
            return (
              <div
                key={r.id}
                role="row"
                style={{ position: "absolute", top: 0, left: 0, right: 0, height: v.size, transform: "translateY(" + v.start + "px)" }}
                className={cn(
                  GRID,
                  "group/row cursor-pointer border-b border-border/50 text-xs hover:bg-accent/50",
                  selected && "bg-accent",
                )}
                onClick={() => { setActiveId(r.id); onOpen(r.id); }}
              >
                <span className="font-mono tabular-nums text-muted-foreground">{timeOf(r.ts)}</span>
                <StatusBadge status={r.status} />
                <span className="truncate font-mono" title={r.model !== r.realModel ? r.model + " \u2192 " + r.realModel : r.model}>
                  {r.model !== r.realModel ? <>{r.model} <span className="text-muted-foreground">{"\u2192"} {r.realModel}</span></> : r.model}
                </span>
                <span className="truncate text-muted-foreground">{accountName(r)}</span>
                <span className="truncate text-muted-foreground">{effortKey(r)}</span>
                <Tokens row={r} />
                <span className="text-right tabular-nums text-muted-foreground">{duration(r.ms)}</span>
                <span className="text-right tabular-nums text-muted-foreground">{duration(r.firstTokenMs)}</span>
                <RowMenu row={r} onFilter={onFilter} curlContext={curlContext} />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

