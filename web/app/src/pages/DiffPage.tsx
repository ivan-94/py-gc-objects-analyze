import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeaderCell, TableRow, TableWrap } from "@/components/ui/table";
import { ErrorState, Skeleton } from "@/components/shared/states";
import { MetricCard } from "@/components/shared/metric-card";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient, type Snapshot } from "@/generated/api-client";
import { formatNumber, formatOptionalBytes, signedBytes, signedNumber } from "@/lib/format";
import type { UpdateSearch } from "@/lib/search";

type DiffPageProps = {
  snapshots: Snapshot[];
  fromSnapshot?: number;
  toSnapshot?: number;
  diffState: string;
  updateSearch: UpdateSearch;
};

export function DiffPage({ snapshots, fromSnapshot, toSnapshot, diffState, updateSearch }: DiffPageProps) {
  const from = fromSnapshot ?? snapshots[0]?.snapshot_id;
  const to = toSnapshot ?? snapshots[1]?.snapshot_id ?? snapshots[0]?.snapshot_id;
  const diff = useQuery({
    queryKey: ["diff", from, to],
    queryFn: () => apiClient.diff({ from_snapshot_id: from, to_snapshot_id: to }),
    enabled: Boolean(from && to)
  });
  const diffObjects = useQuery({
    queryKey: ["diff-objects", from, to, diffState],
    queryFn: () => apiClient.diffObjects({ from_snapshot_id: from, to_snapshot_id: to, state: diffState, limit: 100, offset: 0 }),
    enabled: Boolean(from && to)
  });
  const confidenceLevel = diff.data?.confidence.level ?? "unknown";

  return (
    <Page>
      <PageTitle
        title="Diff"
        actions={
          <>
            <SnapshotPicker value={from} setValue={(value) => updateSearch({ from: value })} snapshots={snapshots} />
            <SnapshotPicker value={to} setValue={(value) => updateSearch({ to: value })} snapshots={snapshots} />
            <Select value={diffState} onChange={(event) => updateSearch({ diffState: event.target.value })}>
              <option value="new">New</option>
              <option value="changed">Changed</option>
              <option value="gone">Gone</option>
              <option value="retained">Retained</option>
            </Select>
          </>
        }
      />
      {diff.error ? <ErrorState error={diff.error} /> : null}
      {diff.data ? (
        <>
          <div className={confidenceLevel === "high" ? "rounded-lg border border-border bg-background p-3 text-sm text-muted-foreground" : "rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900"}>
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone={confidenceLevel === "high" ? "success" : "warn"}>{confidenceLevel}</Badge>
              <span>{diff.data.confidence.message}</span>
              {confidenceLevel !== "high" ? <strong>Use aggregate-only interpretation unless process identity is known to match.</strong> : null}
            </div>
          </div>
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <MetricCard label="Object Delta" value={signedNumber(diff.data.summary_delta.object_count)} />
            <MetricCard label="Edge Delta" value={signedNumber(diff.data.summary_delta.edge_count)} />
            <MetricCard label="Shallow Delta" value={signedBytes(diff.data.summary_delta.shallow_size_sum)} />
            <MetricCard label="Lifecycle" value={`${formatNumber(diff.data.object_lifecycle.new_count)} new / ${formatNumber(diff.data.object_lifecycle.gone_count)} gone`} />
          </div>
        </>
      ) : (
        <Skeleton title="Diff" />
      )}
      <TableWrap>
        <Table className="min-w-[1080px]">
          <colgroup>
            <col className="w-[18%]" />
            <col className="w-[9%]" />
            <col className="w-[18%]" />
            <col className="w-[25%]" />
            <col className="w-[10%]" />
            <col className="w-[10%]" />
            <col className="w-[10%]" />
          </colgroup>
          <TableHead>
            <TableRow>
              <TableHeaderCell>object_id</TableHeaderCell>
              <TableHeaderCell>state</TableHeaderCell>
              <TableHeaderCell>type</TableHeaderCell>
              <TableHeaderCell>module</TableHeaderCell>
              <TableHeaderCell className="text-right">from shallow</TableHeaderCell>
              <TableHeaderCell className="text-right">to shallow</TableHeaderCell>
              <TableHeaderCell className="text-right">delta</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {diffObjects.data?.rows.map((row) => (
              <TableRow key={`${row.state}-${row.object_id}`}>
                <TableCell className="truncate font-mono text-xs" title={row.object_id}>{row.object_id}</TableCell>
                <TableCell><Badge tone={row.state === "gone" ? "warn" : "neutral"}>{row.state}</Badge></TableCell>
                <TableCell className="truncate" title={row.type}>{row.type}</TableCell>
                <TableCell className="truncate text-muted-foreground" title={row.module}>{row.module}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatOptionalBytes(row.from_shallow_size)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatOptionalBytes(row.to_shallow_size)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{signedBytes(row.shallow_size_delta)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
      {diffObjects.error ? <ErrorState error={diffObjects.error} /> : null}
    </Page>
  );
}

function SnapshotPicker({ value, setValue, snapshots }: { value?: number; setValue: (value: number) => void; snapshots: Snapshot[] }) {
  return (
    <Select value={value ?? ""} onChange={(event) => setValue(Number(event.target.value))}>
      {snapshots.map((snapshot) => (
        <option key={snapshot.snapshot_id} value={snapshot.snapshot_id}>
          {snapshot.snapshot_id}
        </option>
      ))}
    </Select>
  );
}
