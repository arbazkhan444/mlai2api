import { InfoIcon } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/page-header";
import { CopyButton } from "@/components/copy-button";
import { ClientCard } from "@/components/connect/client-card";
import { clientSpecs } from "@/lib/clients";
import { localKey, localV1 } from "@/lib/requests";
import { useBootstrap } from "@/queries";

const TROUBLE: [string, string, string][] = [
  ["Connection refused", "The proxy is stopped, or the client uses a different port.", "Press Start, then copy the address from the status bar."],
  ["401 or 403", "Wrong or missing key.", "Copy the API key from a card above and send it as Authorization: Bearer."],
  ["404 model not found", "The model name is not an alias.", "Use an id from Models & Aliases, or one that the bridge exposes."],
  ["404 on /v1/messages", "This is an Anthropic-style client.", "Use an OpenAI-compatible client. Claude Code is listed above as not supported."],
  ["Slow or 429", "The upstream account is rate-limited.", "Check Accounts; the bridge switches accounts when one is limited."],
];

export function ConnectPage() {
  const boot = useBootstrap().data;
  const v1 = localV1(boot?.running, boot?.config);
  const root = v1.replace(/\/v1\/?$/, "");
  const key = localKey(boot?.running, boot?.config);
  const aliases = (boot?.config?.aliases || {}) as Record<string, string>;
  const model = (boot?.config?.defaultModel as string) || Object.keys(aliases)[0] || "default";
  const specs = clientSpecs({ v1, root, key, model });

  return (
    <>
      <PageHeader title="Connect" description="Copy the values into your client. Every card uses the address of the running bridge." />

      <Card size="sm">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-sm"><InfoIcon className="size-4 text-muted-foreground" />The /v1 trap</CardTitle>
          <CardDescription>
            Some clients add <span className="font-mono">/v1</span> to the base URL themselves. If yours does, use the root address instead of the one ending in /v1.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap gap-4 text-xs">
          <span className="flex items-center gap-1.5"><span className="text-muted-foreground">OpenAI-compatible base URL</span><span className="font-mono">{v1}</span><CopyButton value={v1} label="Copy /v1 base URL" /></span>
          <span className="flex items-center gap-1.5"><span className="text-muted-foreground">Root (client adds /v1)</span><span className="font-mono">{root}</span><CopyButton value={root} label="Copy root URL" /></span>
        </CardContent>
      </Card>

      <div className="grid gap-3 md:grid-cols-2">
        {specs.map((c) => <ClientCard key={c.id} client={c} />)}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Troubleshooting</CardTitle>
          <CardDescription>Start from the message the client shows.</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid gap-2 text-xs">
            {TROUBLE.map(([symptom, cause, fix]) => (
              <div key={symptom} className="grid gap-1 rounded-md border p-2.5 sm:grid-cols-[10rem_1fr_1fr] sm:gap-3">
                <span className="font-medium">{symptom}</span>
                <span className="text-muted-foreground">{cause}</span>
                <span>{fix}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    </>
  );
}
