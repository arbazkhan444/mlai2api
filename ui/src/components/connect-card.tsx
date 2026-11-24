import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { CopyButton } from "@/components/copy-button";
import { useBootstrap } from "@/queries";

function CopyRow({ label, value, copyable }: { label: string; value: string; copyable: boolean }) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-20 shrink-0 text-xs text-muted-foreground">{label}</span>
      <code className="min-w-0 flex-1 truncate rounded-md bg-muted px-2 py-1 font-mono text-xs select-all">{value}</code>
      <CopyButton value={value} label={"Copy " + label.toLowerCase()} disabled={!copyable} />
    </div>
  );
}

export function ConnectCard({ className }: { className?: string }) {
  const { data } = useBootstrap();
  const on = !!data?.running?.running;
  const key = data?.config?.apiKey;
  return (
    <Card className={className}>
      <CardHeader>
        <CardTitle>Connect a client</CardTitle>
        <CardDescription>Use any OpenAI-compatible client with these values.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <CopyRow label="Base URL" value={!data ? "\u2014" : on ? data.running.v1 || "" : "start the proxy to get a URL"} copyable={on} />
        <CopyRow label="API key" value={!data ? "\u2014" : key || "(no key set)"} copyable={!!key} />
      </CardContent>
    </Card>
  );
}
