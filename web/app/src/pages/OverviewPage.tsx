import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState, ErrorState, Skeleton } from "@/components/shared/states";
import { JsonBlock } from "@/components/shared/json";
import { MetricCard } from "@/components/shared/metric-card";
import { ModuleStatTable, TypeStatTable } from "@/components/shared/stat-table";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient, type ModuleRow, type StatRow } from "@/generated/api-client";
import { formatBytes, formatNumber } from "@/lib/format";
import type { UpdateSearch } from "@/lib/search";

export function OverviewPage({ snapshotId, updateSearch }: { snapshotId?: number; updateSearch: UpdateSearch }) {
  const summary = useQuery({
    queryKey: ["summary", snapshotId],
    queryFn: () => apiClient.summary({ snapshot_id: snapshotId }),
    enabled: Boolean(snapshotId)
  });
  const types = useQuery({
    queryKey: ["overview-types", snapshotId],
    queryFn: () => apiClient.types({ snapshot_id: snapshotId, limit: 100 }),
    enabled: Boolean(snapshotId)
  });

  if (summary.isLoading) return <Skeleton title="Overview" />;
  if (summary.error) return <ErrorState error={summary.error} />;
  const data = summary.data;
  if (!data) return <EmptyState label="No dump imported" />;

  const nonBuiltinTypes = (types.data ?? [])
    .filter(isNonBuiltinType)
    .sort((a, b) => (b.estimated_reachable_size_sum ?? b.shallow_size_sum) - (a.estimated_reachable_size_sum ?? a.shallow_size_sum))
    .slice(0, 12);
  const viewType = (row: StatRow) =>
    updateSearch(
      { page: "objects", q: undefined, type: row.type, module: row.module || undefined, cohort: undefined, selected: undefined, offset: undefined },
      { history: "push" }
    );
  const viewModule = (row: ModuleRow) =>
    updateSearch(
      { page: "objects", q: undefined, type: undefined, module: row.module, cohort: undefined, selected: undefined, offset: undefined },
      { history: "push" }
    );

  return (
    <Page>
      <PageTitle title="Overview" actions={<Badge>estimated values marked</Badge>} />
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <MetricCard label="Objects" value={formatNumber(data.snapshot.object_count)} />
        <MetricCard label="Edges" value={formatNumber(data.snapshot.edge_count)} />
        <MetricCard label="Shallow Size" value={formatBytes(data.snapshot.shallow_size_sum)} />
        <MetricCard
          label="Missing / Stub"
          value={`${formatNumber(data.missing_stub_summary.missing_referent_count)} / ${formatNumber(data.missing_stub_summary.stub_count)}`}
        />
      </div>
      <div className="grid gap-3 xl:grid-cols-2">
        <TypeStatTable title="Top Types By Shallow Size" rows={data.top_types_by_shallow_size} onTypeClick={viewType} />
        <TypeStatTable title="Top Estimated Reachable Types" rows={data.top_reachable_types} reachable onTypeClick={viewType} />
        <ApplicationTypeTable rows={nonBuiltinTypes} loading={types.isLoading} onTypeClick={viewType} />
        <ModuleStatTable title="Top Modules" rows={data.top_modules_by_shallow_size} onModuleClick={viewModule} />
        <Warnings rows={data.import_warnings} />
      </div>
    </Page>
  );
}

function ApplicationTypeTable({ rows, loading, onTypeClick }: { rows: StatRow[]; loading: boolean; onTypeClick: (row: StatRow) => void }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Top Non-Builtin Types</CardTitle>
      </CardHeader>
      <CardContent>
        {loading ? (
          <Skeleton title="Non-builtin types" className="border-0 p-0" />
        ) : rows.length ? (
          <div className="space-y-2">
            {rows.map((row) => (
              <div key={`${row.module}:${row.type}`} className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-border pb-2 last:border-b-0">
                <div className="min-w-0">
                  <button type="button" className="block max-w-full truncate text-left text-sm font-medium text-primary hover:underline" title={row.type} onClick={() => onTypeClick(row)}>
                    {row.type}
                  </button>
                  <button type="button" className="block max-w-full truncate text-left text-xs text-muted-foreground hover:text-foreground" title={row.module} onClick={() => onTypeClick(row)}>
                    {row.module}
                  </button>
                </div>
                <div className="text-right text-sm tabular-nums">
                  <div>{formatBytes(row.estimated_reachable_size_sum ?? 0)}</div>
                  <div className="text-xs text-muted-foreground">{formatNumber(row.count)} objects</div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <EmptyState label="No non-builtin types in the current top 100." className="border-0 p-0" />
        )}
      </CardContent>
    </Card>
  );
}

function Warnings({ rows }: { rows: unknown[] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Import Warnings</CardTitle>
      </CardHeader>
      <CardContent>{rows.length ? <JsonBlock value={rows} /> : <EmptyState label="No warnings" className="border-0 p-0" />}</CardContent>
    </Card>
  );
}

function isNonBuiltinType(row: StatRow) {
  return row.module !== "builtins" && !row.module.startsWith("_frozen_importlib");
}
