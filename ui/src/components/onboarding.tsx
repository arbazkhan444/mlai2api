import { CheckIcon, CircleIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";

function Step({ done, title, detail, action }: { done: boolean; title: string; detail: string; action?: React.ReactNode }) {
  return (
    <div className="flex items-start gap-3 rounded-md border p-3">
      <span className={cn("mt-0.5", done ? "text-emerald-600 dark:text-emerald-400" : "text-muted-foreground")}>
        {done ? <CheckIcon className="size-4" /> : <CircleIcon className="size-4" />}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-sm font-medium">{title}</span>
        <span className="text-xs text-muted-foreground">{detail}</span>
      </div>
      {action}
    </div>
  );
}

export function Onboarding({ hasAccount, running, startPending, onAddAccount, onStart, onConnect }: {
  hasAccount: boolean;
  running: boolean;
  startPending: boolean;
  onAddAccount: () => void;
  onStart: () => void;
  onConnect: () => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Get started</CardTitle>
        <CardDescription>Three steps and your first request can flow through the bridge.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <Step
          done={hasAccount}
          title="Add an account"
          detail="Use a device token from the mlai CLI, or import the sign-in you already have."
          action={!hasAccount && <Button size="sm" variant="outline" onClick={onAddAccount}>Add account</Button>}
        />
        <Step
          done={running}
          title="Start the proxy"
          detail="The bridge listens on 127.0.0.1 and only accepts your local key."
          action={!running && (
            <Button size="sm" variant="outline" onClick={onStart} disabled={!hasAccount || startPending}>
              {startPending ? <Spinner /> : null}Start
            </Button>
          )}
        />
        <Step
          done={false}
          title="Connect a client"
          detail="Copy the base URL, key and model alias into Cline, Roo Code, opencode or curl."
          action={<Button size="sm" variant="outline" onClick={onConnect}>Open Connect</Button>}
        />
      </CardContent>
    </Card>
  );
}
