import { useCallback, useEffect, useState } from "react";
import { CableIcon, ChartColumnIcon, LayoutDashboardIcon, ScrollTextIcon, SettingsIcon, BoxesIcon, UsersIcon } from "lucide-react";
import {
  Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarHeader, SidebarInset, SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarRail,
} from "@/components/ui/sidebar";
import { StatusHeader } from "@/components/status-header";
import { CommandPalette } from "@/components/command-palette";
import { AccountsPage } from "@/pages/accounts";
import { ConnectPage } from "@/pages/connect";
import { ModelsPage } from "@/pages/models";
import { OverviewPage } from "@/pages/overview";
import { RequestsPage } from "@/pages/requests";
import { SettingsPage } from "@/pages/settings";
import { UsagePage } from "@/pages/usage";
import { focusRequests } from "@/lib/requests";
import { useStartupTab } from "@/queries";

const PAGES = [
  { id: "overview", label: "Overview", icon: LayoutDashboardIcon },
  { id: "requests", label: "Requests", icon: ScrollTextIcon },
  { id: "usage", label: "Usage", icon: ChartColumnIcon },
  { id: "models", label: "Models & Aliases", icon: BoxesIcon },
  { id: "accounts", label: "Accounts", icon: UsersIcon },
  { id: "connect", label: "Connect", icon: CableIcon },
  { id: "settings", label: "Settings", icon: SettingsIcon },
] as const;
type Page = (typeof PAGES)[number]["id"];

const TAB_KEY = "mlaiTab";
// Old tab names (and MLAI_BRIDGE_TAB values) map onto the new pages.
const LEGACY: Record<string, Page> = { proxy: "overview" };
function toPage(v: string | null | undefined): Page | null {
  if (!v) return null;
  const p = LEGACY[v] ?? v;
  return PAGES.some((x) => x.id === p) ? (p as Page) : null;
}

const DETAIL_HASH = /^#\/requests\/(\d+)$/;
const readDetailHash = () => {
  const m = DETAIL_HASH.exec(location.hash);
  return m ? Number(m[1]) : null;
};
function writeDetailHash(id: number | null) {
  try {
    history.replaceState(null, "", id === null ? location.pathname + location.search : "#/requests/" + id);
  } catch {
    // file:// previews may refuse history writes; the drawer still opens.
  }
}

export default function App() {
  const [page, setPageState] = useState<Page>(() => toPage(localStorage.getItem(TAB_KEY)) ?? "overview");
  const [detailId, setDetailId] = useState<number | null>(readDetailHash);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const setPage = (p: Page) => {
    localStorage.setItem(TAB_KEY, p);
    setPageState(p);
  };
  const openDetail = useCallback((id: number) => {
    setDetailId(id);
    writeDetailHash(id);
  }, []);
  const closeDetail = useCallback(() => {
    setDetailId(null);
    writeDetailHash(null);
  }, []);
  useEffect(() => {
    const onHash = () => setDetailId(readDetailHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // MLAI_BRIDGE_TAB (via `startup_tab`) overrides the saved page once at boot.
  const forced = toPage(useStartupTab().data);
  useEffect(() => { if (forced) setPage(forced); }, [forced]);

  return (
    <SidebarProvider className="h-svh">
      <Sidebar collapsible="icon">
        <SidebarHeader>
          <div className="flex h-8 items-center gap-2 px-1">
            <div className="flex size-6 shrink-0 items-center justify-center rounded-md bg-primary font-mono text-[11px] font-semibold text-primary-foreground">ml</div>
            <span className="truncate text-sm font-semibold group-data-[collapsible=icon]:hidden">MLAI Bridge</span>
          </div>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu className="gap-0.5">
                {PAGES.map(({ id, label, icon: Icon }) => (
                  <SidebarMenuItem key={id}>
                    <SidebarMenuButton isActive={page === id} tooltip={label} onClick={() => setPage(id)} aria-current={page === id ? "page" : undefined}>
                      <Icon />
                      <span>{label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarRail />
      </Sidebar>
      <SidebarInset className="min-w-0 overflow-hidden">
        <StatusHeader onOpenPalette={() => setPaletteOpen(true)} />
        <div className="flex-1 overflow-y-auto">
          <div className="mx-auto flex max-w-6xl flex-col gap-4 p-4 md:p-6">
            {page === "overview" && <OverviewPage detailId={detailId} onOpen={openDetail} onClose={closeDetail} onAllRequests={() => setPage("requests")} onGo={setPage} />}
            {page === "requests" && <RequestsPage detailId={detailId} onOpen={openDetail} onClose={closeDetail} />}
            {page === "usage" && (
              <UsagePage
                onFilterRequests={(patch) => {
                  focusRequests(patch);
                  setPage("requests");
                }}
              />
            )}
            {page === "models" && <ModelsPage />}
            {page === "accounts" && <AccountsPage />}
            {page === "connect" && <ConnectPage />}
            {page === "settings" && <SettingsPage />}
          </div>
        </div>
      </SidebarInset>
      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        pages={PAGES.map(({ id, label }) => ({ id, label }))}
        onGo={(id) => setPage(id as Page)}
        onOpenRequest={(id) => {
          setPage("requests");
          openDetail(id);
        }}
      />
    </SidebarProvider>
  );
}
