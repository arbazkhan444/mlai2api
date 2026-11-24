import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { ChevronLeftIcon, ChevronRightIcon, CopyIcon, Maximize2Icon, Minimize2Icon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { CopyButton, copyText } from "@/components/copy-button";
import { duration, localKey, localV1, requestBodyText, statusClass, buildCurl } from "@/lib/requests";
import { credits, fmt } from "@/lib/format";
import { useBootstrap, useRequestDetail } from "@/queries";
import type { Row } from "@/tauri";

const JsonViewer = lazy(() => import("./json-viewer"));
const WIDTH_KEY = "mlaiDrawerW";
const NO_BODY = "No body was stored for this request. Bodies are kept only for the most recent requests (retention), or body storage is turned off in Settings.";

function Cell({ label, value, mono }: { label: string; value: React.ReactNode; mono?: boolean }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5 rounded-md bg-muted/50 px-2.5 py-1.5">
      <span className="text-[11px] text-muted-foreground">{label}</span>
      <span className={"truncate font-medium tabular-nums " + (mono ? "font-mono text-xs" : "")}>{value}</span>
    </div>
  );
}

function StatusBadge({ status }: { status: number }) {
  const cls = statusClass(status);
  return (
    <Badge
      variant="outline"
      className={
        cls === "2xx"
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400"
          : cls === "4xx"
            ? "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-400"
            : "border-destructive/30 bg-destructive/10 text-destructive"
      }
    >
      {status}
    </Badge>
  );
}

function TimingBar({ row }: { row: Row }) {
  const total = Math.max(row.ms || 0, 1);
  const first = row.firstTokenMs;
  const tone = statusClass(row.status) === "2xx" ? "bg-emerald-500" : statusClass(row.status) === "4xx" ? "bg-amber-500" : "bg-red-500";
  const head = first == null ? 100 : Math.min(100, Math.max(0, (first / total) * 100));
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex h-2.5 w-full overflow-hidden rounded-full bg-muted" role="img" aria-label="Request timing">
        <div className="bg-muted-foreground/40" style={{ width: head + "%" }} />
        {first != null && <div className={tone} style={{ width: 100 - head + "%" }} />}
      </div>
      <div className="flex justify-between text-[11px] tabular-nums text-muted-foreground">
        <span>0</span>
        <span>{first == null ? "no first token recorded" : "first token " + duration(first)}</span>
        <span>end {duration(row.ms)}</span>
      </div>
    </div>
  );
}

function BodySection({ title, text, onExpand }: { title: string; text: string | null; onExpand: () => void }) {
  return (
    <section className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1">
        <h3 className="text-xs font-medium text-muted-foreground">{title}</h3>
        {text && (
          <div className="ml-auto flex items-center gap-1">
            <CopyButton value={text} label={"Copy " + title.toLowerCase()} />
            <Button size="icon-sm" variant="ghost" aria-label={"Expand " + title.toLowerCase()} title={"Expand " + title.toLowerCase()} onClick={onExpand}>
              <Maximize2Icon />
            </Button>
          </div>
        )}
      </div>
      {text ? (
        <Suspense fallback={<div className="flex h-24 items-center justify-center"><Spinner /></div>}>
          <JsonViewer value={text} height="18rem" />
        </Suspense>
      ) : (
        <p className="rounded-md border border-dashed px-3 py-6 text-center text-xs text-muted-foreground">{NO_BODY}</p>
      )}
    </section>
  );
}

export function DetailSheet({ id, list, onOpen, onClose }: { id: number | null; list: Row[]; onOpen: (id: number) => void; onClose: () => void }) {
  const { data: detail, isFetching } = useRequestDetail(id);
  const boot = useBootstrap().data;
  const [width, setWidth] = useState(() => Number(localStorage.getItem(WIDTH_KEY)) || 760);
  const [expanded, setExpanded] = useState<"request" | "response" | "raw" | null>(null);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const index = id === null ? -1 : list.findIndex((r) => r.id === id);
  const prev = index > 0 ? list[index - 1] : null;
  const next = index >= 0 && index < list.length - 1 ? list[index + 1] : null;

  useEffect(() => setExpanded(null), [id]);
  useEffect(() => { scrollRef.current?.scrollTo({ top: 0 }); }, [id]);

  // `[` / `]` walk the filtered list, except while typing in the body viewer.
  useEffect(() => {
    if (id === null) return;
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.closest(".cm-editor") || t.closest("input") || t.closest("textarea"))) return;
      if (e.key === "[" && prev) { e.preventDefault(); onOpen(prev.id); }
      if (e.key === "]" && next) { e.preventDefault(); onOpen(next.id); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [id, prev, next, onOpen]);

  const row = detail?.row;
  const requestText = detail ? requestBodyText(detail) : null;
  const responseText = detail?.response === null || detail?.response === undefined ? null : JSON.stringify(detail.response, null, 2);
  const rawText = detail ? JSON.stringify({ meta: detail.row, request: detail.request ?? null, response: detail.response ?? null }, null, 2) : null;
  const curl = (shell: "bash" | "powershell") =>
    requestText === null
      ? null
      : buildCurl({ shell, url: localV1(boot?.running, boot?.config), key: localKey(boot?.running, boot?.config), body: requestText });

  const copyJson = async () => {
    if (rawText && (await copyText(rawText))) toast.add({ type: "success", title: "Copied request #" + id + " as JSON" });
  };

  const onHandleDown = (e: React.PointerEvent<HTMLDivElement>) => {
    drag.current = { x: e.clientX, width };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onHandleMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const next = Math.min(Math.max(drag.current.width + (drag.current.x - e.clientX), 480), window.innerWidth - 160);
    setWidth(next);
  };
  const onHandleUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    drag.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
    localStorage.setItem(WIDTH_KEY, String(width));
  };

  return (
    <Sheet open={id !== null} onOpenChange={(open) => { if (!open) onClose(); }}>
      <SheetContent
        className="w-full max-w-[94vw] gap-0 p-0 data-[side=right]:sm:max-w-none"
        style={{ width: "min(" + width + "px, 94vw)" }}
      >
        <div
          role="separator"
          aria-label="Resize detail panel"
          aria-orientation="vertical"
          className="absolute inset-y-0 left-0 z-10 w-1.5 cursor-col-resize hover:bg-primary/30"
          onPointerDown={onHandleDown}
          onPointerMove={onHandleMove}
          onPointerUp={onHandleUp}
        />
        <SheetHeader className="flex flex-col gap-1 border-b px-4 py-3 pr-12">
          <div className="flex items-center gap-1">
            <SheetTitle className="truncate font-mono text-sm">{row ? "Request #" + row.id + " \u00b7 " + row.model : "Request #" + id}</SheetTitle>
            <div className="ml-auto flex items-center gap-1">
              <Button size="icon-sm" variant="ghost" aria-label="Previous request" title="Previous ([)" disabled={!prev} onClick={() => prev && onOpen(prev.id)}>
                <ChevronLeftIcon />
              </Button>
              <Button size="icon-sm" variant="ghost" aria-label="Next request" title="Next (])" disabled={!next} onClick={() => next && onOpen(next.id)}>
                <ChevronRightIcon />
              </Button>
            </div>
          </div>
          <SheetDescription className="tabular-nums">
            {row ? new Date(row.ts).toLocaleString() + " \u00b7 " + duration(row.ms) + (row.stream ? " \u00b7 streamed" : "") : "Loading\u2026"}
            {index >= 0 && list.length > 1 ? " \u00b7 " + (index + 1) + " of " + list.length : ""}
          </SheetDescription>
          <div className="flex items-center gap-1 pt-1">
            <Button size="sm" variant="outline" onClick={copyJson} disabled={!detail}><CopyIcon />Copy as JSON</Button>
            <DropdownMenu>
              <DropdownMenuTrigger render={<Button size="sm" variant="outline" disabled={!requestText}><CopyIcon />Copy as cURL</Button>} />
              <DropdownMenuContent align="start">
                <DropdownMenuItem onClick={async () => { const text = curl("bash"); if (text && (await copyText(text))) toast.add({ type: "success", title: "Copied cURL (bash)" }); }}>
                  bash
                </DropdownMenuItem>
                <DropdownMenuItem onClick={async () => { const text = curl("powershell"); if (text && (await copyText(text))) toast.add({ type: "success", title: "Copied cURL (PowerShell)" }); }}>
                  PowerShell
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </SheetHeader>

        {!detail || !row ? (
          <div className="flex flex-1 items-center justify-center">{isFetching && <Spinner />}</div>
        ) : (
          <div ref={scrollRef} className="flex flex-1 flex-col gap-5 overflow-y-auto p-4">
            {expanded ? (
              <section className="flex flex-col gap-1.5">
                <div className="flex items-center gap-1">
                  <h3 className="text-xs font-medium text-muted-foreground">
                    {expanded === "request" ? "Request body" : expanded === "response" ? "Response body" : "Raw JSON"}
                  </h3>
                  <div className="ml-auto flex items-center gap-1">
                    <CopyButton value={(expanded === "request" ? requestText : expanded === "response" ? responseText : rawText) || ""} label="Copy body" />
                    <Button size="icon-sm" variant="ghost" aria-label="Collapse" title="Collapse" onClick={() => setExpanded(null)}><Minimize2Icon /></Button>
                  </div>
                </div>
                <Suspense fallback={<div className="flex h-40 items-center justify-center"><Spinner /></div>}>
                  <JsonViewer
                    value={(expanded === "request" ? requestText : expanded === "response" ? responseText : rawText) || ""}
                    height="calc(100svh - 11rem)"
                  />
                </Suspense>
              </section>
            ) : (
              <>
                <section className="flex flex-col gap-2">
                  <h3 className="text-xs font-medium text-muted-foreground">Overview</h3>
                  <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-3">
                    <Cell label="Status" value={<StatusBadge status={row.status} />} />
                    <Cell label="Account" value={row.accountLabel || "Unattributed"} />
                    <Cell label="Effort" value={row.effort || "inherit"} />
                    <Cell label="Model" value={row.model !== row.realModel ? row.model + " \u2192 " + row.realModel : row.model} mono />
                    <Cell label="Stream" value={row.stream ? "yes" : "no"} />
                    <Cell label="Request ID" value={row.id} mono />
                  </div>
                </section>

                <section className="flex flex-col gap-2">
                  <h3 className="text-xs font-medium text-muted-foreground">Timing</h3>
                  <TimingBar row={row} />
                </section>

                <section className="flex flex-col gap-2">
                  <h3 className="text-xs font-medium text-muted-foreground">Usage &amp; cost</h3>
                  <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-3">
                    <Cell label="Input" value={fmt(row.promptTokens)} />
                    <Cell label="Cached input" value={fmt(row.cachedTokens)} />
                    <Cell label="Output" value={fmt(row.completionTokens)} />
                    <Cell label="Reasoning" value={fmt(row.reasoningTokens)} />
                    <Cell label="Total" value={fmt(row.totalTokens)} />
                    <Cell label="Spend" value={credits(row.totalTokens) + " cr"} />
                  </div>
                </section>

                <BodySection title="Request body" text={requestText} onExpand={() => setExpanded("request")} />
                <BodySection title="Response body" text={responseText} onExpand={() => setExpanded("response")} />
                <section className="flex flex-col gap-1.5">
                  <div className="flex items-center gap-1">
                    <h3 className="text-xs font-medium text-muted-foreground">Raw JSON</h3>
                    <div className="ml-auto flex items-center gap-1">
                      <CopyButton value={rawText || ""} label="Copy raw JSON" />
                      <Button size="icon-sm" variant="ghost" aria-label="Expand raw JSON" title="Expand raw JSON" onClick={() => setExpanded("raw")}><Maximize2Icon /></Button>
                    </div>
                  </div>
                  <Suspense fallback={<div className="flex h-24 items-center justify-center"><Spinner /></div>}>
                    <JsonViewer value={rawText || ""} height="14rem" />
                  </Suspense>
                </section>
              </>
            )}
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
