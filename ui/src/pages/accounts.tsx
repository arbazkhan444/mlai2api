import { useState } from "react";
import { DownloadIcon, FlaskConicalIcon, PlusIcon, Trash2Icon, UsersIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { Confirm } from "@/components/confirm";
import { PageHeader } from "@/components/page-header";
import { ago, credits, fmt } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useAccounts, useActivateAccount, useAddAccount, useCheckAccounts, useImportAccount, useRemoveAccount, useRunning, useUsage } from "@/queries";
import type { Account, CheckResult } from "@/tauri";

function AddAccountDialog() {
  const [open, setOpen] = useState(false);
  const [acc, setAcc] = useState({ label: "", token: "", server: "" });
  const add = useAddAccount();
  const field = (key: keyof typeof acc) => ({
    id: "acc-" + key,
    value: acc[key],
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => setAcc((x) => ({ ...x, [key]: e.target.value })),
  });
  const submit = (e: React.SyntheticEvent) => {
    e.preventDefault();
    const token = acc.token.trim();
    if (!token) { toast.add({ type: "warning", title: "Paste the account's device token" }); return; }
    add.mutate({ token, label: acc.label.trim() || null, server: acc.server.trim() || null }, {
      onSuccess: () => {
        setAcc((a) => ({ ...a, token: "", label: "" }));
        setOpen(false);
        toast.add({ type: "success", title: "Account added" });
      },
    });
  };
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button size="sm"><PlusIcon />Add account</Button>} />
      <DialogContent>
        <form onSubmit={submit} className="grid gap-4">
          <DialogHeader>
            <DialogTitle>Add account</DialogTitle>
            <DialogDescription>The token is checked against the server before it's stored.</DialogDescription>
          </DialogHeader>
          <FieldGroup className="gap-3">
            <Field><FieldLabel htmlFor="acc-label">Label (optional)</FieldLabel><Input placeholder="e.g. Work" {...field("label")} /></Field>
            <Field><FieldLabel htmlFor="acc-token">Device token</FieldLabel><Input type="password" placeholder="mlaid_..." autoComplete="off" className="font-mono" {...field("token")} /></Field>
            <Field><FieldLabel htmlFor="acc-server">Server</FieldLabel><Input placeholder="https://code.ml.ai" className="font-mono" {...field("server")} /></Field>
          </FieldGroup>
          <DialogFooter>
            <DialogClose render={<Button variant="outline" type="button">Cancel</Button>} />
            <Button type="submit" disabled={add.isPending}>{add.isPending && <Spinner />}Add account</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ImportButton() {
  const imp = useImportAccount();
  return (
    <Button
      variant="outline"
      size="sm"
      disabled={imp.isPending}
      onClick={() => imp.mutate(undefined, { onSuccess: () => toast.add({ type: "success", title: "Imported from mlai CLI" }) })}
    >
      {imp.isPending ? <Spinner /> : <DownloadIcon />}Import from mlai CLI
    </Button>
  );
}

function Cell({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-md bg-muted/40 px-2 py-1.5">
      <span className="text-[11px] text-muted-foreground">{label}</span>
      <span className="truncate font-medium tabular-nums">{value}</span>
    </div>
  );
}

function ExpiryNote({ at }: { at?: number | null }) {
  if (!at) return null;
  const left = at - Date.now();
  if (left <= 0) {
    return <p className="rounded-md border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-destructive">This token has expired. Import the mlai CLI sign-in again, or add a new device token.</p>;
  }
  if (left < 7 * 864e5) {
    const days = Math.ceil(left / 864e5);
    return <p className="rounded-md border border-amber-500/30 bg-amber-500/10 px-2 py-1.5 text-amber-700 dark:text-amber-400">This token expires in {days} day{days === 1 ? "" : "s"}.</p>;
  }
  return null;
}

function health(a: Account): "ok" | "error" | "unknown" {
  if (a.lastCheckOk === true) return "ok";
  if (a.lastCheckOk === false) return "error";
  return "unknown";
}

function AccountCard({ account, usage, busy, onCheck, onUse, onRemove, useBusy, removeBusy }: {
  account: Account;
  usage?: { requests: number; totalTokens: number };
  busy: boolean;
  onCheck: () => void;
  onUse: () => void;
  onRemove: () => void;
  useBusy: boolean;
  removeBusy: boolean;
}) {
  const state = health(account);
  return (
    <Card className={cn(account.active && "border-primary/40")}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <span
            aria-hidden="true"
            className={cn(
              "size-2 rounded-full",
              state === "ok" && "bg-emerald-500",
              state === "error" && "bg-destructive",
              state === "unknown" && "bg-muted-foreground/50",
            )}
          />
          {account.label}
          {account.active && <Badge variant="secondary">Active</Badge>}
        </CardTitle>
        <CardDescription className="truncate font-mono">{account.tokenHint} {"\u00b7"} {account.server}</CardDescription>
        <div className="flex flex-wrap items-center gap-1 pt-1">
          <Button size="sm" variant="outline" disabled={busy} onClick={onCheck}>
            {busy ? <Spinner /> : <FlaskConicalIcon />}Check
          </Button>
          {!account.active && (
            <>
              <Button size="sm" variant="outline" disabled={useBusy} onClick={onUse}>{useBusy ? <Spinner /> : null}Use</Button>
              <Confirm
                trigger={<Button size="icon-sm" variant="ghost" aria-label={"Remove " + account.label} disabled={removeBusy}><Trash2Icon /></Button>}
                title={"Remove " + account.label + "?"}
                description="Its stored token is deleted. You can add it again later with the device token."
                action="Remove account"
                onConfirm={onRemove}
              />
            </>
          )}
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-3">
          <Cell label="Last used" value={ago(account.lastUsed)} />
          <Cell label="Last check" value={account.lastCheckAt ? ago(account.lastCheckAt) : "never"} />
          <Cell label="Health" value={state === "ok" ? "ok" : state === "error" ? "failed" : "not checked"} />
          <Cell label="Requests" value={fmt(usage?.requests ?? 0)} />
          <Cell label="Tokens" value={fmt(usage?.totalTokens ?? 0)} />
          <Cell label="Spend" value={credits(usage?.totalTokens ?? 0) + " cr"} />
        </div>
        {state === "error" && account.lastError && (
          <p className="rounded-md border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-xs text-destructive">{account.lastError}</p>
        )}
        <ExpiryNote at={account.expires} />
      </CardContent>
    </Card>
  );
}

export function AccountsPage() {
  const data = useAccounts().data;
  const usage = useUsage().data;
  const on = useRunning();
  const activate = useActivateAccount();
  const remove = useRemoveAccount();
  const check = useCheckAccounts();
  const list = data?.accounts || [];
  const byAccount = new Map((usage?.byAccount || []).map((a) => [a.accountId, a]));

  const report = (results: CheckResult[]) => {
    const ok = results.filter((r) => r.ok).length;
    const failed = results.filter((r) => !r.ok);
    const label = (id: string) => list.find((a) => a.id === id)?.label || id;
    toast.add(
      failed.length
        ? { type: "warning", title: ok + " of " + results.length + " accounts ok", description: failed.map((r) => label(r.id) + ": " + (r.error || "failed")).join("\n") }
        : { type: "success", title: ok === 1 ? "Account is reachable" : "All " + ok + " accounts are reachable" },
    );
  };

  return (
    <>
      <PageHeader title="Accounts" description="Requests are signed with the active account's device token. Switching accounts restarts the proxy.">
        <Button
          variant="outline"
          size="sm"
          disabled={check.isPending || !list.length}
          onClick={() => check.mutate(undefined, { onSuccess: report })}
        >
          {check.isPending && check.variables === undefined ? <Spinner /> : <FlaskConicalIcon />}Check all
        </Button>
        <ImportButton />
        <AddAccountDialog />
      </PageHeader>

      {list.length ? (
        <div className="grid gap-3 md:grid-cols-2">
          {list.map((a) => (
            <AccountCard
              key={a.id}
              account={a}
              usage={byAccount.get(a.id)}
              busy={check.isPending && check.variables === a.id}
              useBusy={activate.isPending}
              removeBusy={remove.isPending}
              onCheck={() => check.mutate(a.id, { onSuccess: report })}
              onUse={() =>
                activate.mutate({ id: a.id, restart: on }, {
                  onSuccess: (r) => toast.add({ type: "success", title: r?.restarted ? "Account switched \u2014 proxy restarted" : "Account switched" }),
                })
              }
              onRemove={() => remove.mutate(a.id, { onSuccess: () => toast.add({ type: "success", title: "Account removed" }) })}
            />
          ))}
        </div>
      ) : (
        <Empty className="py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon"><UsersIcon /></EmptyMedia>
            <EmptyTitle>No accounts yet</EmptyTitle>
            <EmptyDescription>Add one with a device token, or import the mlai CLI sign-in from this machine.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent className="flex-row justify-center"><ImportButton /><AddAccountDialog /></EmptyContent>
        </Empty>
      )}

      <p className="text-xs text-muted-foreground">
        Check signs in with each account's token (a read-only whoami call). It does not spend a model run. Only the mlai CLI import carries a token expiry date.
      </p>
    </>
  );
}
