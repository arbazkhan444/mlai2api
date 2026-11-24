import { Area, AreaChart, Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { ModelUsage, UsageBucket } from "@/tauri";

const AXIS = { fontSize: 11, fill: "var(--muted-foreground)" } as const;
const TOOLTIP = {
  contentStyle: {
    background: "var(--popover)",
    border: "1px solid var(--border)",
    borderRadius: 8,
    fontSize: 12,
    color: "var(--popover-foreground)",
  },
} as const;

const compact = new Intl.NumberFormat([], { notation: "compact", maximumFractionDigits: 1 });

function bucketLabels(buckets: UsageBucket[]): string[] {
  const span = buckets.length > 1 ? buckets[buckets.length - 1].start - buckets[0].start : 0;
  const short = span <= 2 * 864e5;
  return buckets.map((b) =>
    short
      ? new Date(b.start).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      : new Date(b.start).toLocaleDateString([], { month: "short", day: "numeric" }),
  );
}

/** Read one string field from a Recharts click payload, which is typed loosely by the library. */
function payloadField(value: unknown, key: "model" | "id"): string | null {
  if (!value || typeof value !== "object" || !("payload" in value)) return null;
  const payload: unknown = value.payload;
  if (!payload || typeof payload !== "object") return null;
  const match = Object.entries(payload).find(([name]) => name === key);
  return match && typeof match[1] === "string" ? match[1] : null;
}

/** Charts only. Lazy-loaded so Recharts stays out of the main bundle. */
export default function UsageCharts({ buckets, byModel, byAccount, onModel, onAccount }: {
  buckets: UsageBucket[];
  byModel: ModelUsage[];
  byAccount: { accountId: string; accountLabel: string; requests: number; totalTokens: number }[];
  onModel: (model: string) => void;
  onAccount: (accountId: string) => void;
}) {
  const labels = bucketLabels(buckets);
  const trend = buckets.map((b, i) => ({ label: labels[i], tokens: b.tokens, requests: b.requests }));
  const models = byModel.slice(0, 10).map((m) => ({ model: m.model, tokens: m.totalTokens, requests: m.requests }));
  const accounts = byAccount.map((a) => ({ account: a.accountLabel, id: a.accountId, tokens: a.totalTokens, requests: a.requests }));

  return (
    <div className="grid gap-3 lg:grid-cols-2">
      <div className="rounded-md border p-3">
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">Tokens over time</h3>
        <ResponsiveContainer width="100%" height={200}>
          <AreaChart data={trend} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey="label" tick={AXIS} tickLine={false} axisLine={false} interval="preserveStartEnd" minTickGap={24} />
            <YAxis tick={AXIS} tickLine={false} axisLine={false} width={56} tickFormatter={(v) => compact.format(Number(v))} />
            <Tooltip {...TOOLTIP} />
            <Area type="monotone" dataKey="tokens" name="Tokens" stroke="var(--primary)" strokeWidth={1.5} fill="var(--primary)" fillOpacity={0.15} />
          </AreaChart>
        </ResponsiveContainer>
      </div>

      <div className="rounded-md border p-3">
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">Tokens by model {"\u00b7"} click to open the filtered log</h3>
        <ResponsiveContainer width="100%" height={200}>
          <BarChart data={models} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" vertical={false} />
            <XAxis dataKey="model" tick={AXIS} tickLine={false} axisLine={false} />
            <YAxis tick={AXIS} tickLine={false} axisLine={false} width={56} tickFormatter={(v) => compact.format(Number(v))} />
            <Tooltip {...TOOLTIP} cursor={{ fill: "var(--accent)" }} />
            <Bar
              dataKey="tokens"
              name="Tokens"
              fill="var(--chart-2)"
              radius={[4, 4, 0, 0]}
              cursor="pointer"
              onClick={(entry) => {
                const model = payloadField(entry, "model");
                if (model) onModel(model);
              }}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>

      <div className="rounded-md border p-3 lg:col-span-2">
        <h3 className="mb-2 text-xs font-medium text-muted-foreground">Tokens by account {"\u00b7"} click to open the filtered log</h3>
        <ResponsiveContainer width="100%" height={160}>
          <BarChart data={accounts} layout="vertical" margin={{ top: 4, right: 8, bottom: 0, left: 8 }}>
            <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" horizontal={false} />
            <XAxis type="number" tick={AXIS} tickLine={false} axisLine={false} tickFormatter={(v) => compact.format(Number(v))} />
            <YAxis type="category" dataKey="account" tick={AXIS} tickLine={false} axisLine={false} width={120} />
            <Tooltip {...TOOLTIP} cursor={{ fill: "var(--accent)" }} />
            <Bar
              dataKey="tokens"
              name="Tokens"
              fill="var(--chart-3)"
              radius={[0, 4, 4, 0]}
              cursor="pointer"
              onClick={(entry) => {
                const id = payloadField(entry, "id");
                if (id) onAccount(id);
              }}
            />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
