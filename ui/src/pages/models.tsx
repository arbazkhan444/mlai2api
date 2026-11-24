import { useEffect, useRef, useState } from "react";
import { FlaskConicalIcon, GaugeIcon, PlusIcon, SaveIcon, Trash2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { toast } from "@/components/ui/toast";
import { Confirm } from "@/components/confirm";
import { PageHeader } from "@/components/page-header";
import { fmt } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useBootstrap, useMintLimits, useRunning, useSaveConfig, useTestConnection } from "@/queries";

const num = "text-right tabular-nums";

interface Row {
  id: number;
  alias: string;
  real: string;
}

export function ModelsPage() {
  const boot = useBootstrap().data;
  const running = useRunning();
  const models = boot?.models || [];
  const mint = useMintLimits();
  const save = useSaveConfig();
  const test = useTestConnection();
  const [rows, setRows] = useState<Row[]>([]);
  const [defaultModel, setDefaultModel] = useState("");
  const [dirty, setDirty] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const seeded = useRef(false);
  const nextId = useRef(1);

  // Seed the editor once from the backend; later bootstrap refreshes must not wipe edits.
  useEffect(() => {
    if (seeded.current || !boot) return;
    seeded.current = true;
    const aliases = (boot.config.aliases || {}) as Record<string, string>;
    setRows(Object.entries(aliases).map(([alias, real]) => ({ id: nextId.current++, alias, real })));
    setDefaultModel(boot.config.defaultModel || "");
  }, [boot]);

  const upstreams = Array.from(new Set([...models.map((m) => m.real || m.id), ...rows.map((r) => r.real)].filter(Boolean))).sort();
  const counts = new Map<string, number>();
  for (const r of rows) counts.set(r.alias.trim(), (counts.get(r.alias.trim()) || 0) + 1);
  const problem = (r: Row): string | null => {
    const alias = r.alias.trim();
    if (!alias) return "Alias is required";
    if (/\s/.test(alias)) return "Alias cannot contain spaces";
    if ((counts.get(alias) || 0) > 1) return "Duplicate alias";
    if (!r.real.trim()) return "Choose an upstream model";
    return null;
  };
  const invalid = rows.some((r) => problem(r) !== null) || rows.length === 0;

  const edit = (id: number, patch: Partial<Row>) => {
    setRows((list) => list.map((r) => (r.id === id ? { ...r, ...patch } : r)));
    setDirty(true);
  };
  const add = () => {
    setRows((list) => [...list, { id: nextId.current++, alias: "", real: upstreams[0] || "" }]);
    setDirty(true);
  };
  const remove = (id: number) => {
    setRows((list) => list.filter((r) => r.id !== id));
    setDirty(true);
  };

  const runTest = (alias: string) => {
    if (!alias) return;
    setTesting(alias);
    test.mutate(alias, {
      onSuccess: (r) =>
        toast.add(
          r.ok
            ? { type: "success", title: alias + " answered in " + r.ms + " ms", description: "Model " + r.model }
            : { type: "error", title: alias + " failed", description: r.error || "HTTP " + r.status + (r.body ? ": " + r.body.slice(0, 160) : "") },
        ),
      onSettled: () => setTesting(null),
    });
  };

  const submit = () => {
    if (!boot || invalid) return;
    const aliases = Object.fromEntries(rows.map((r) => [r.alias.trim(), r.real.trim()]));
    save.mutate(
      { ...boot.config, aliases, defaultModel: defaultModel.trim() || Object.keys(aliases)[0] },
      {
        onSuccess: (res) => {
          setDirty(false);
          toast.add(res.restartRequired
            ? { type: "warning", title: "Saved. Restart the proxy to use the new aliases." }
            : { type: "success", title: "Aliases saved" });
        },
      },
    );
  };

  const readLimits = () => {
    const id = toast.add({ type: "loading", title: "Reading limits (one run per model)\u2026", timeout: 0 });
    mint.mutate(undefined, {
      onSuccess: (learned) => {
        const ok = learned.filter((l) => !l.error).length;
        const failed = learned.length - ok;
        toast.update(id, { type: failed ? "warning" : "success", title: "Learned " + ok + " model(s)" + (failed ? ", " + failed + " failed" : ""), timeout: 6000 });
      },
      onError: () => toast.close(id),
    });
  };

  const defaultOptions = Array.from(new Set([defaultModel, ...rows.map((r) => r.alias.trim())].filter(Boolean)));

  return (
    <>
      <PageHeader title="Models & Aliases" description="Aliases map the model name a client sends to an upstream ML AI Code model.">
        <Button size="sm" onClick={submit} disabled={!dirty || invalid || save.isPending || !boot}>
          {save.isPending ? <Spinner /> : <SaveIcon />}Save changes
        </Button>
      </PageHeader>

      <Card>
        <CardHeader>
          <CardTitle>Aliases {dirty && <span className="ml-1 text-xs font-normal text-amber-600 dark:text-amber-400">unsaved</span>}</CardTitle>
          <CardDescription>These are the model names clients can send. The same alias must map to one upstream model.</CardDescription>
          <CardAction>
            <Button variant="outline" size="sm" onClick={add}><PlusIcon />Add alias</Button>
          </CardAction>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Table className="text-xs">
            <TableHeader>
              <TableRow>
                <TableHead className="w-56">Alias</TableHead>
                <TableHead>Upstream model</TableHead>
                <TableHead className="w-24">Test</TableHead>
                <TableHead className="w-10" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.length ? rows.map((r) => {
                const issue = problem(r);
                return (
                  <TableRow key={r.id}>
                    <TableCell>
                      <Input
                        aria-label="Alias"
                        value={r.alias}
                        placeholder="opus"
                        className={cn("h-8 font-mono", issue && "border-destructive/60")}
                        onChange={(e) => edit(r.id, { alias: e.target.value })}
                      />
                    </TableCell>
                    <TableCell>
                      <Input
                        aria-label={"Upstream model for " + (r.alias || "new alias")}
                        list="upstream-models"
                        value={r.real}
                        placeholder="pixis-opus"
                        className="h-8 font-mono"
                        onChange={(e) => edit(r.id, { real: e.target.value })}
                      />
                    </TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={!running || !!issue || testing !== null}
                        title={running ? "Send one small request with this alias" : "Start the proxy first"}
                        onClick={() => runTest(r.alias.trim())}
                      >
                        {testing === r.alias.trim() ? <Spinner /> : <FlaskConicalIcon />}Test
                      </Button>
                    </TableCell>
                    <TableCell>
                      <Confirm
                        trigger={<Button size="icon-sm" variant="ghost" aria-label={"Remove alias " + r.alias} title="Remove alias"><Trash2Icon /></Button>}
                        title={"Remove the alias " + (r.alias || "new") + "?"}
                        description="Clients that use this alias stop working after you save and restart the proxy."
                        action="Remove"
                        onConfirm={() => remove(r.id)}
                      />
                    </TableCell>
                  </TableRow>
                );
              }) : (
                <TableRow><TableCell colSpan={4} className="py-6 text-center text-muted-foreground">No aliases yet. Add one to start.</TableCell></TableRow>
              )}
            </TableBody>
          </Table>
          <datalist id="upstream-models">
            {upstreams.map((m) => <option key={m} value={m} />)}
          </datalist>
          {!rows.length ? (
            <p className="text-xs text-muted-foreground">Add at least one alias before saving.</p>
          ) : rows.some((r) => problem(r)) ? (
            <p className="text-xs text-destructive">{rows.map(problem).find(Boolean)}</p>
          ) : null}
          <div className="flex items-center gap-2 text-xs">
            <span className="text-muted-foreground">Default model</span>
            <Select items={defaultOptions.map((m) => ({ value: m, label: m }))} value={defaultModel} onValueChange={(v) => { setDefaultModel((v as string) ?? ""); setDirty(true); }}>
              <SelectTrigger size="sm" className="w-48" aria-label="Default model"><SelectValue placeholder="Pick an alias" /></SelectTrigger>
              <SelectContent>
                {defaultOptions.map((m) => <SelectItem key={m} value={m}>{m}</SelectItem>)}
              </SelectContent>
            </Select>
            <span className="text-muted-foreground">Used when a client sends no model name.</span>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Models <span className="ml-1 text-muted-foreground tabular-nums">{models.length || ""}</span></CardTitle>
          <CardDescription>What <span className="font-mono">/v1/models</span> serves, with learned context and output limits.</CardDescription>
          <CardAction>
            <Button variant="outline" size="sm" onClick={readLimits} disabled={mint.isPending}>
              {mint.isPending ? <Spinner /> : <GaugeIcon />}Read limits
            </Button>
          </CardAction>
        </CardHeader>
        <CardContent>
          <Table className="text-xs">
            <TableHeader>
              <TableRow><TableHead>Alias</TableHead><TableHead>Model</TableHead><TableHead className={num}>Context</TableHead><TableHead className={num}>Output</TableHead></TableRow>
            </TableHeader>
            <TableBody>
              {models.length ? models.map((m) => (
                <TableRow key={m.id}>
                  <TableCell className="font-mono">{m.id}</TableCell><TableCell className="font-mono text-muted-foreground">{m.real || m.id}</TableCell>
                  <TableCell className={num}>{fmt(m.context)}</TableCell><TableCell className={num}>{fmt(m.output)}</TableCell>
                </TableRow>
              )) : <TableRow><TableCell colSpan={4} className="py-8 text-center text-muted-foreground">Start the proxy to load the model menu.</TableCell></TableRow>}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </>
  );
}
