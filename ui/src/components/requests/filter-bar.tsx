import type { Filters, StatusClass } from "@/lib/requests";
import { ChevronDownIcon, RotateCcwIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

export interface Option {
  value: string;
  label: string;
}

function MultiSelect({ label, options, selected, onChange }: { label: string; options: Option[]; selected: string[]; onChange: (values: string[]) => void }) {
  const toggle = (value: string, on: boolean) => onChange(on ? [...selected, value] : selected.filter((v) => v !== value));
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button variant="outline" size="sm" className={cn("gap-1.5", selected.length > 0 && "border-primary/40")}>
            {label}
            {selected.length > 0 && <Badge variant="secondary" className="px-1 text-[10px] tabular-nums">{selected.length}</Badge>}
            <ChevronDownIcon className="size-3.5 opacity-60" />
          </Button>
        }
      />
      <PopoverContent align="start" className="w-56 p-1.5">
        <div className="flex max-h-72 flex-col gap-0.5 overflow-y-auto">
          {options.map((o) => (
            <label key={o.value} className="flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1.5 text-xs hover:bg-accent">
              <Checkbox checked={selected.includes(o.value)} onCheckedChange={(on) => toggle(o.value, !!on)} />
              <span className="truncate">{o.label}</span>
            </label>
          ))}
        </div>
        <div className="flex items-center justify-between border-t pt-1.5">
          <Button variant="ghost" size="sm" disabled={!selected.length} onClick={() => onChange([])}>Clear</Button>
          <span className="pr-1 text-[11px] text-muted-foreground">{options.length} option{options.length === 1 ? "" : "s"}</span>
        </div>
      </PopoverContent>
    </Popover>
  );
}

export function FilterBar({
  accounts, models, efforts, filters, onChange, onReset, onJump, count,
}: {
  accounts: Option[];
  models: Option[];
  efforts: Option[];
  filters: Filters;
  onChange: (next: Filters) => void;
  onReset: () => void;
  onJump: (id: number) => void;
  count: string;
}) {
  const classes: Option[] = [
    { value: "2xx", label: "2xx success" },
    { value: "4xx", label: "4xx client error" },
    { value: "5xx", label: "5xx / failed" },
  ];
  const active = filters.accounts.length + filters.models.length + filters.efforts.length + filters.classes.length + (filters.q ? 1 : 0);
  return (
    <div className="flex flex-wrap items-center gap-2">
      <MultiSelect label="Account" options={accounts} selected={filters.accounts} onChange={(accounts) => onChange({ ...filters, accounts })} />
      <MultiSelect label="Model" options={models} selected={filters.models} onChange={(models) => onChange({ ...filters, models })} />
      <MultiSelect label="Effort" options={efforts} selected={filters.efforts} onChange={(efforts) => onChange({ ...filters, efforts })} />
      <MultiSelect label="Status" options={classes} selected={filters.classes} onChange={(values) => onChange({ ...filters, classes: values as StatusClass[] })} />
      <Input
        aria-label="Search requests"
        placeholder="Search model, account or #id"
        value={filters.q}
        onChange={(e) => onChange({ ...filters, q: e.target.value })}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          const m = /^#?(\d+)$/.exec(filters.q.trim());
          if (m) onJump(Number(m[1]));
        }}
        className="h-8 w-56"
      />
      <Button variant="ghost" size="sm" disabled={!active} onClick={onReset}><RotateCcwIcon />Reset</Button>
      <span className="ml-auto text-xs tabular-nums text-muted-foreground">{count}</span>
    </div>
  );
}
