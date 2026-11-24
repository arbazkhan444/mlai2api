import { MutationCache, QueryCache, QueryClient, useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "@/components/ui/toast";
import { invoke } from "./tauri";
import type { Accounts, Bootstrap, CheckResult, Config, DesktopSettings, Detail, MintLimit, RunningState, Row, TestResult, Usage, UsageWindow } from "./tauri";

const fail = (e: unknown) => toast.add({ type: "error", title: String(e), priority: "high" });

export const qc = new QueryClient({
  queryCache: new QueryCache({ onError: fail }),
  // `meta.silent` lets a caller show its own, more useful message (e.g. the port-conflict dialog).
  mutationCache: new MutationCache({
    onError: (error, _variables, _context, mutation) => {
      if (!mutation.meta?.silent) fail(error);
    },
  }),
  defaultOptions: {
    queries: { networkMode: "always", retry: false, refetchOnWindowFocus: false },
    mutations: { networkMode: "always" },
  },
});

const refresh = (...keys: string[]) => Promise.all(keys.map((k) => qc.invalidateQueries({ queryKey: [k] })));

// --- queries ---

/** Config, running state, stats. The proxy auto-starts in the background, so re-check once if it isn't up yet. */
export const useBootstrap = () =>
  useQuery({
    queryKey: ["bootstrap"],
    queryFn: () => invoke<Bootstrap>("bootstrap"),
    refetchInterval: (q) => (q.state.dataUpdateCount === 1 && !q.state.data?.running?.running ? 1500 : false),
  });

export const useRunning = () => !!useBootstrap().data?.running?.running;

export const useStartupTab = () =>
  useQuery({ queryKey: ["startup_tab"], queryFn: () => invoke<string>("startup_tab").catch(() => ""), staleTime: Infinity });

/** Request log; polls while the proxy runs. Mount one poller per page. */
export const useHistory = (live: boolean) => {
  const running = useRunning();
  return useQuery({
    queryKey: ["history"],
    queryFn: () => invoke<Row[]>("history", { limit: 2000 }),
    refetchInterval: running && live ? 3000 : false,
  });
};

export const useUsage = () => {
  const running = useRunning();
  return useQuery({ queryKey: ["usage"], queryFn: () => invoke<Usage>("usage"), refetchInterval: running ? 3000 : false });
};

/** Usage inside one window, with chart buckets. `range` is a query key so each range caches separately. */
export const useUsageWindow = (range: string, fromMs: number, toMs: number, buckets: number) => {
  const running = useRunning();
  return useQuery({
    queryKey: ["usage", range, fromMs, toMs, buckets],
    queryFn: () => invoke<UsageWindow>("usage_window", { fromMs, toMs, buckets }),
    refetchInterval: running ? 3000 : false,
  });
};

/** Accounts poll slowly so "last used / last check" labels stay fresh. */
export const useAccounts = () =>
  useQuery({ queryKey: ["accounts"], queryFn: () => invoke<Accounts>("accounts"), refetchInterval: 30000 });

export const useRequestDetail = (id: number | null) =>
  useQuery({
    queryKey: ["detail", id],
    queryFn: () => invoke<Detail>("request_detail", { id }),
    enabled: id !== null,
  });

// --- mutations ---

export const useStart = () =>
  useMutation({ mutationFn: () => invoke<RunningState>("start"), meta: { silent: true }, onSettled: () => refresh("bootstrap", "history") });

export const useStop = () =>
  useMutation({ mutationFn: () => invoke<RunningState>("stop"), onSettled: () => refresh("bootstrap") });

export const useSaveConfig = () =>
  useMutation({
    mutationFn: (config: Config) => invoke<{ restartRequired?: boolean }>("save_config", { config }),
    onSuccess: () => refresh("bootstrap"),
  });

export const useMintLimits = () =>
  useMutation({ mutationFn: () => invoke<MintLimit[]>("mint_limits"), onSuccess: () => refresh("bootstrap") });

export const useClearHistory = () =>
  useMutation({ mutationFn: () => invoke("clear_history"), onSuccess: () => refresh("history", "usage", "bootstrap") });

export const useAddAccount = () =>
  useMutation({
    mutationFn: (a: { token: string; label: string | null; server: string | null }) => invoke<Accounts>("accounts_add", a),
    onSuccess: () => refresh("accounts", "bootstrap"),
  });

export const useImportAccount = () =>
  useMutation({ mutationFn: () => invoke<Accounts>("accounts_import"), onSuccess: () => refresh("accounts", "bootstrap") });

export const useActivateAccount = () =>
  useMutation({
    mutationFn: (a: { id: string; restart: boolean }) => invoke<{ restarted?: boolean }>("accounts_activate", a),
    onSuccess: () => refresh("accounts", "bootstrap"),
  });

export const useRemoveAccount = () =>
  useMutation({ mutationFn: (id: string) => invoke<Accounts>("accounts_remove", { id }), onSuccess: () => refresh("accounts", "bootstrap") });

export const useTestConnection = () => useMutation({ mutationFn: (model?: string) => invoke<TestResult>("test_connection", { model: model ?? null }) });

export const useCheckAccounts = () =>
  useMutation({
    mutationFn: (id?: string) => invoke<CheckResult[]>("accounts_check", { id: id ?? null }),
    onSuccess: () => refresh("accounts"),
  });

export const useDesktopSettings = () =>
  useQuery({ queryKey: ["desktop"], queryFn: () => invoke<DesktopSettings>("desktop_settings"), staleTime: Infinity });

export const useSetDesktopSettings = () =>
  useMutation({
    mutationFn: (settings: Partial<DesktopSettings>) => invoke<DesktopSettings>("set_desktop_settings", { settings }),
    onSuccess: () => refresh("desktop"),
  });
