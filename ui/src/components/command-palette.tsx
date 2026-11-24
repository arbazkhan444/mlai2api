import { useState } from "react";
import { FlaskConicalIcon, KeyRoundIcon, PlayIcon, SquareIcon } from "lucide-react";
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { toast } from "@/components/ui/toast";
import { copyText } from "@/components/copy-button";
import { localKey, localV1 } from "@/lib/requests";
import { useBootstrap, useStart, useStop, useTestConnection } from "@/queries";

export interface PalettePage {
  id: string;
  label: string;
}

export function CommandPalette({ open, onOpenChange, pages, onGo, onOpenRequest }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  pages: PalettePage[];
  onGo: (id: string) => void;
  onOpenRequest: (id: number) => void;
}) {
  const [query, setQuery] = useState("");
  const boot = useBootstrap().data;
  const start = useStart();
  const stop = useStop();
  const test = useTestConnection();
  const running = !!boot?.running?.running;
  const v1 = localV1(boot?.running, boot?.config);
  const key = localKey(boot?.running, boot?.config);
  const idMatch = /^#?(\d+)$/.exec(query.trim());

  const run = (action: () => void) => {
    onOpenChange(false);
    setQuery("");
    action();
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title="Command palette" description="Go to a page, run a command, or open a request by id">
      <Command>
        <CommandInput placeholder="Go to a page, run a command, or type #123" value={query} onValueChange={setQuery} />
        <CommandList>
          <CommandEmpty>No matching command.</CommandEmpty>
          <CommandGroup heading="Go to">
            {pages.map((p) => (
              <CommandItem key={p.id} value={"go " + p.label} onSelect={() => run(() => onGo(p.id))}>{p.label}</CommandItem>
            ))}
          </CommandGroup>
          <CommandGroup heading="Proxy">
            <CommandItem value="start stop proxy" onSelect={() => run(() => (running ? stop.mutate() : start.mutate()))}>
              {running ? <SquareIcon /> : <PlayIcon />}
              {running ? "Stop proxy" : "Start proxy"}
            </CommandItem>
            <CommandItem value="test connection" disabled={!running} onSelect={() => run(() => test.mutate(undefined))}>
              <FlaskConicalIcon />Test connection
            </CommandItem>
            <CommandItem
              value="copy base url"
              onSelect={() => run(async () => { if (await copyText(v1)) toast.add({ type: "success", title: "Base URL copied" }); })}
            >
              Copy base URL
            </CommandItem>
            <CommandItem
              value="copy api key"
              disabled={!key}
              onSelect={() => run(async () => { if (await copyText(key)) toast.add({ type: "success", title: "API key copied" }); })}
            >
              <KeyRoundIcon />Copy API key
            </CommandItem>
          </CommandGroup>
          {idMatch && (
            <CommandGroup heading="Request">
              <CommandItem value={"#" + idMatch[1] + " open request #" + idMatch[1]} onSelect={() => run(() => onOpenRequest(Number(idMatch[1])))}>
                Open request #{idMatch[1]}
              </CommandItem>
            </CommandGroup>
          )}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
