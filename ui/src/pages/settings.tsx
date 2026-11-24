import { useState } from "react";
import { KeyRoundIcon, MonitorIcon, MoonIcon, SunIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { CopyButton } from "@/components/copy-button";
import { PageHeader } from "@/components/page-header";
import { useTheme, type Theme } from "@/components/theme-provider";
import { cn } from "@/lib/utils";
import { useBootstrap, useDesktopSettings, useSaveConfig, useSetDesktopSettings } from "@/queries";
import type { Config, DesktopSettings } from "@/tauri";

interface Form { host: string; port: string; server: string; project: string; folder: string; runTask: string; defaultModel: string; effort: string }

function formFrom(c: Config): Form {
  return {
    host: c.host || "127.0.0.1",
    port: String(c.port || 8787),
    server: c.server || "https://code.ml.ai",
    project: c.project || "",
    folder: c.folder || "",
    runTask: c.runTask || "",
    defaultModel: c.defaultModel || "mlai-code",
    effort: c.reasoningEffort || "",
  };
}

const EFFORTS = [{ value: "", label: "inherit" }, ...["low", "medium", "high", "max"].map((v) => ({ value: v, label: v }))];
const THEMES: [Theme, string, typeof SunIcon][] = [["system", "System", MonitorIcon], ["light", "Light", SunIcon], ["dark", "Dark", MoonIcon]];

const saved = (r: { restartRequired?: boolean }, what: string) =>
  toast.add(r.restartRequired
    ? { type: "warning", title: what + " \u2014 restart required", description: "Stop and start the proxy to apply the change." }
    : { type: "success", title: what });

function L({ k, children }: { k: keyof Form; children: React.ReactNode }) {
  return <FieldLabel htmlFor={"cfg-" + k}>{children}</FieldLabel>;
}

function ConfigForm({ config }: { config: Config }) {
  const [form, setForm] = useState<Form>(() => formFrom(config));
  const save = useSaveConfig();
  const field = (key: keyof Form) => ({
    id: "cfg-" + key,
    value: form[key],
    onChange: (e: React.ChangeEvent<HTMLInputElement>) => setForm((x) => ({ ...x, [key]: e.target.value })),
  });
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    save.mutate({
      ...config,
      host: form.host.trim() || "127.0.0.1",
      port: Number(form.port) || 8787,
      server: form.server.trim(),
      project: form.project.trim(),
      folder: form.folder.trim(),
      runTask: form.runTask.trim(),
      defaultModel: form.defaultModel.trim(),
      reasoningEffort: form.effort || null,
    }, { onSuccess: (r) => saved(r, "Config saved") });
  };
  return (
    <form onSubmit={submit}>
    <Card>
      <CardHeader>
        <CardTitle>Proxy</CardTitle>
        <CardDescription>Where the bridge listens and which ML AI Code project it runs in.</CardDescription>
      </CardHeader>
      <CardContent>
        <FieldGroup className="grid gap-4 sm:grid-cols-2">
          <Field><L k="host">Host</L><Input className="font-mono" {...field("host")} /></Field>
          <Field><L k="port">Port</L><Input type="number" min={1} max={65535} className="font-mono tabular-nums" {...field("port")} /></Field>
          <Field className="sm:col-span-2"><L k="server">Server</L><Input className="font-mono" {...field("server")} /></Field>
          <Field className="sm:col-span-2"><L k="project">Project id</L><Input className="font-mono" {...field("project")} /></Field>
          <Field><L k="folder">Folder</L><Input {...field("folder")} /></Field>
          <Field><L k="runTask">Run task</L><Input {...field("runTask")} /></Field>
          <Field><L k="defaultModel">Default model</L><Input className="font-mono" {...field("defaultModel")} /></Field>
          <Field>
            <FieldLabel>Reasoning effort</FieldLabel>
            <Select items={EFFORTS} value={form.effort} onValueChange={(v) => setForm((x) => ({ ...x, effort: (v as string) ?? "" }))}>
              <SelectTrigger className="w-full" aria-label="Reasoning effort"><SelectValue /></SelectTrigger>
              <SelectContent>{EFFORTS.map((o) => <SelectItem key={o.value} value={o.value}>{o.label}</SelectItem>)}</SelectContent>
            </Select>
          </Field>
        </FieldGroup>
      </CardContent>
      <CardFooter className="border-t py-3">
        <Button type="submit" size="sm" disabled={save.isPending}>{save.isPending && <Spinner />}Save config</Button>
      </CardFooter>
    </Card>
    </form>
  );
}

function ApiKeyCard({ config }: { config: Config }) {
  const save = useSaveConfig();
  const key = config.apiKey || "";
  const regen = () => {
    const bytes = new Uint8Array(24);
    crypto.getRandomValues(bytes);
    const next = "mlba_" + Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
    save.mutate({ ...config, apiKey: next }, { onSuccess: (r) => saved(r, "New key saved") });
  };
  return (
    <Card>
      <CardHeader>
        <CardTitle>API key</CardTitle>
        <CardDescription>Clients send this as the bearer token.</CardDescription>
      </CardHeader>
      <CardContent className="flex items-center gap-2">
        <code className="min-w-0 flex-1 truncate rounded-md bg-muted px-2 py-1.5 font-mono text-xs select-all">{key || "(no key set)"}</code>
        <CopyButton value={key} label="Copy API key" disabled={!key} />
        <Button variant="outline" size="sm" onClick={regen} disabled={save.isPending}>{save.isPending ? <Spinner /> : <KeyRoundIcon />}New key</Button>
      </CardContent>
    </Card>
  );
}

function ThemeCard() {
  const { theme, setTheme } = useTheme();
  return (
    <Card>
      <CardHeader><CardTitle>Appearance</CardTitle></CardHeader>
      <CardContent>
        <Field>
          <FieldLabel>Theme</FieldLabel>
          <div role="radiogroup" aria-label="Theme" className="inline-flex w-fit rounded-lg border p-0.5">
            {THEMES.map(([value, label, Icon]) => (
              <Button
                key={value}
                role="radio"
                aria-checked={theme === value}
                variant="ghost"
                size="sm"
                className={cn("text-muted-foreground", theme === value && "bg-muted text-foreground")}
                onClick={() => setTheme(value)}
              ><Icon />{label}</Button>
            ))}
          </div>
          <FieldDescription>System follows the Windows light/dark setting.</FieldDescription>
        </Field>
      </CardContent>
    </Card>
  );
}

const DESKTOP_ROWS: { key: keyof DesktopSettings; label: string; hint: string }[] = [
  { key: "autostart", label: "Start with Windows", hint: "Adds the bridge to your sign-in programs. It is off until you turn it on." },
  { key: "startHidden", label: "Start hidden", hint: "Launch into the tray only. Click the tray icon to show the window." },
  { key: "closeToTray", label: "Close to tray", hint: "Closing the window hides it instead of quitting. Quit from the tray menu." },
];

function DesktopCard() {
  const settings = useDesktopSettings();
  const update = useSetDesktopSettings();
  const value = settings.data;
  const toggle = (key: keyof DesktopSettings, label: string, on: boolean) =>
    update.mutate({ [key]: on }, {
      onSuccess: (next) => toast.add({ type: "success", title: label + (next[key] ? " on" : " off") }),
    });
  return (
    <Card>
      <CardHeader>
        <CardTitle>Desktop</CardTitle>
        <CardDescription>How the app behaves on this Windows machine. Window size and position are remembered automatically.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {DESKTOP_ROWS.map((row) => (
          <label key={row.key} className="flex items-start justify-between gap-4 rounded-md border p-3">
            <span className="flex flex-col gap-0.5">
              <span className="text-sm font-medium">{row.label}</span>
              <span className="text-xs text-muted-foreground">{row.hint}</span>
            </span>
            <Switch
              aria-label={row.label}
              checked={!!value?.[row.key]}
              disabled={!value || update.isPending}
              onCheckedChange={(on) => toggle(row.key, row.label, !!on)}
            />
          </label>
        ))}
        <p className="text-xs text-muted-foreground">
          Only one copy runs at a time: launching the app again focuses the open window.
        </p>
      </CardContent>
    </Card>
  );
}

export function SettingsPage() {
  const boot = useBootstrap().data;
  return (
    <>
      <PageHeader title="Settings" description={boot?.configPath} />
      {boot && <ApiKeyCard config={boot.config} />}
      {boot && <ConfigForm config={boot.config} />}
      <DesktopCard />
      <ThemeCard />
    </>
  );
}
