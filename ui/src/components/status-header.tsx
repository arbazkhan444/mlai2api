import { useState } from "react";
import { FlaskConicalIcon, PlayIcon, SearchIcon, SquareIcon } from "lucide-react";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { CopyButton } from "@/components/copy-button";
import { cn } from "@/lib/utils";
import { invoke } from "@/tauri";
import { useBootstrap, useSaveConfig, useStart, useStop, useTestConnection } from "@/queries";

export function StatusHeader({ onOpenPalette }: { onOpenPalette?: () => void }) {
  const boot = useBootstrap();
  const start = useStart();
  const stop = useStop();
  const test = useTestConnection();
  const saveConfig = useSaveConfig();
  const [conflict, setConflict] = useState<{ port: number; next: number | null } | null>(null);
  const data = boot.data;
  const on = !!data?.running?.running;
  const busy = start.isPending || stop.isPending;
  const failed = boot.isError || (!on && start.isError);
  const state = failed ? "Error" : on ? "Running" : "Stopped";
  const addr = data?.running?.addr || (data ? (data.config.host || "127.0.0.1") + ":" + (data.config.port || 8787) : "");
  const acc = data?.accounts;
  const active = acc?.accounts.find((a) => a.id === acc.active);

  // A bind failure names the busy port; offer the next free one instead of leaving the user stuck.
  const startProxy = () =>
    start.mutate(undefined, {
      onError: async (error) => {
        const match = /port (\d+) is already in use/i.exec(String(error));
        if (!match) return;
        const port = Number(match[1]);
        const next = await invoke<number>("free_port", { after: port }).catch(() => null);
        setConflict({ port, next });
      },
    });

  const useNextPort = () => {
    if (!conflict?.next || !data) return;
    saveConfig.mutate({ ...data.config, port: conflict.next }, {
      onSuccess: () => {
        setConflict(null);
        start.mutate();
      },
    });
  };

  return (
    <header className="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b bg-background/80 px-3 backdrop-blur">
      <SidebarTrigger />
      <Separator orientation="vertical" className="mx-1 h-4 data-vertical:self-center" />
      <Badge
        variant="outline"
        className={cn(
          "gap-1.5",
          state === "Running" && "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
          state === "Error" && "border-destructive/30 bg-destructive/10 text-destructive",
        )}
        aria-live="polite"
      >
        <span className={cn("size-1.5 rounded-full bg-muted-foreground", state === "Running" && "bg-emerald-500", state === "Error" && "bg-destructive")} />
        {state}
      </Badge>
      {addr && (
        <span className="flex items-center gap-0.5 font-mono text-xs text-muted-foreground">
          {addr}
          <CopyButton value={on ? data?.running?.v1 || "http://" + addr + "/v1" : "http://" + addr + "/v1"} label="Copy base URL" />
        </span>
      )}
      <div className="ml-auto flex items-center gap-3">
        <span className="hidden text-xs text-muted-foreground sm:inline">
          Account <span className="font-medium text-foreground">{data ? active?.label || "Unattributed" : "\u2014"}</span>
        </span>
        <Button
          size="sm"
          variant="ghost"
          onClick={onOpenPalette}
          aria-label="Open command palette"
          title="Command palette (Ctrl+K)"
          className="gap-1.5"
        >
          <SearchIcon />
          <kbd className="hidden font-mono text-[10px] text-muted-foreground sm:inline">Ctrl K</kbd>
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={!on || test.isPending}
          title="Send one small request through the proxy"
          onClick={() =>
            test.mutate(undefined, {
              onSuccess: (r) =>
                toast.add(
                  r.ok
                    ? { type: "success", title: "Test passed in " + r.ms + " ms", description: "Model " + r.model }
                    : { type: "error", title: "Test failed", description: r.error || "HTTP " + r.status + (r.body ? ": " + r.body.slice(0, 160) : "") },
                ),
            })
          }
        >
          {test.isPending ? <Spinner /> : <FlaskConicalIcon />}Test
        </Button>
        <Button
          size="sm"
          variant={on ? "outline" : "default"}
          disabled={busy || !data}
          onClick={() => (on ? stop.mutate() : startProxy())}
          className="min-w-20"
        >
          {busy ? <Spinner /> : on ? <SquareIcon /> : <PlayIcon />}
          {on ? "Stop" : "Start"}
        </Button>
      </div>
      <AlertDialog open={conflict !== null} onOpenChange={(open) => { if (!open) setConflict(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Port {conflict?.port} is already in use</AlertDialogTitle>
            <AlertDialogDescription>
              Another program holds this port. Use port {conflict?.next ?? "\u2026"} instead, or choose a different port in Settings.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={useNextPort} disabled={!conflict?.next || saveConfig.isPending}>
              Use port {conflict?.next ?? "\u2026"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </header>
  );
}
