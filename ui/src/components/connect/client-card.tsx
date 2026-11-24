import { useState } from "react";
import { CheckIcon, EyeIcon, EyeOffIcon, ExternalLinkIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { CopyButton } from "@/components/copy-button";
import type { ClientField, ClientSpec } from "@/lib/clients";

function FieldRow({ field }: { field: ClientField }) {
  const [shown, setShown] = useState(false);
  const masked = field.secret && !shown;
  return (
    <div className="flex items-center gap-1.5">
      <span className="w-24 shrink-0 text-[11px] text-muted-foreground">{field.label}</span>
      <span className="min-w-0 flex-1 truncate rounded bg-muted/50 px-2 py-1 font-mono text-[11px]">
        {!field.value ? "(not set)" : masked ? "\u2022".repeat(Math.min(28, field.value.length)) : field.value}
      </span>
      {field.secret && (
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={shown ? "Hide " + field.label : "Reveal " + field.label}
          title={shown ? "Hide" : "Reveal"}
          disabled={!field.value}
          onClick={() => setShown((s) => !s)}
        >
          {shown ? <EyeOffIcon /> : <EyeIcon />}
        </Button>
      )}
      <CopyButton value={field.value} label={"Copy " + field.label} disabled={!field.value} />
    </div>
  );
}

export function ClientCard({ client }: { client: ClientSpec }) {
  return (
    <Card className={client.ready ? undefined : "opacity-90"}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {client.name}
          {client.ready ? (
            <Badge variant="outline" className="gap-1 border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-400">
              <CheckIcon className="size-3" />Ready
            </Badge>
          ) : (
            <Badge variant="outline" className="border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-400">
              Not supported
            </Badge>
          )}
        </CardTitle>
        <CardDescription>{client.tagline}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {client.fields?.map((f) => <FieldRow key={f.label} field={f} />)}
        {client.snippet && (
          <div className="relative">
            <div className="absolute right-1 top-1 z-10"><CopyButton value={client.snippet.text} label={"Copy " + client.snippet.title} /></div>
            <pre className="max-h-64 overflow-auto rounded-md border bg-muted/30 p-2.5 pr-10 font-mono text-[11px] leading-relaxed">{client.snippet.text}</pre>
          </div>
        )}
        {client.note && <p className="text-xs text-muted-foreground">{client.note}</p>}
      </CardContent>
      {client.docs && (
        <CardFooter>
          <a
            href={client.docs.href}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
          >
            {client.docs.label}
            <ExternalLinkIcon className="size-3" />
          </a>
        </CardFooter>
      )}
    </Card>
  );
}
