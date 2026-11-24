import { ArrowRightIcon, InboxIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ConnectCard } from "@/components/connect-card";
import { Onboarding } from "@/components/onboarding";
import { PageHeader } from "@/components/page-header";
import { DetailSheet } from "@/components/requests/detail-sheet";
import { LogTable } from "@/components/requests/log-table";
import { StatCards } from "@/components/stat-cards";
import { useBootstrap, useHistory, useStart, useUsage } from "@/queries";

export function OverviewPage({ detailId, onOpen, onClose, onAllRequests, onGo }: {
  detailId: number | null;
  onOpen: (id: number) => void;
  onClose: () => void;
  onAllRequests: () => void;
  onGo: (page: "accounts" | "connect") => void;
}) {
  const usage = useUsage();
  const boot = useBootstrap().data;
  const start = useStart();
  const rows = useHistory(true).data;
  const accountCount = boot?.accounts?.accounts.length ?? 0;
  const firstRun = !!boot && (!boot.configExists || accountCount === 0);
  return (
    <>
      <PageHeader title="Overview" />
      {firstRun && (
        <Onboarding
          hasAccount={accountCount > 0}
          running={!!boot?.running?.running}
          startPending={start.isPending}
          onAddAccount={() => onGo("accounts")}
          onStart={() => start.mutate()}
          onConnect={() => onGo("connect")}
        />
      )}
      <ConnectCard />
      <StatCards stats={usage.data?.stats} rows={rows?.length ? rows : undefined} />
      <Card>
        <CardHeader>
          <CardTitle>Latest requests</CardTitle>
          <CardAction>
            <Button variant="ghost" size="sm" onClick={onAllRequests}>All requests<ArrowRightIcon /></Button>
          </CardAction>
        </CardHeader>
        <CardContent>
          {rows?.length ? (
            <LogTable rows={rows.slice(0, 8)} onOpen={onOpen} compact />
          ) : (
            <Empty>
              <EmptyHeader>
                <EmptyMedia variant="icon"><InboxIcon /></EmptyMedia>
                <EmptyTitle>No requests yet</EmptyTitle>
                <EmptyDescription>Point a client at the base URL above and send a request.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          )}
        </CardContent>
      </Card>
      <DetailSheet id={detailId} list={rows || []} onOpen={onOpen} onClose={onClose} />
    </>
  );
}
