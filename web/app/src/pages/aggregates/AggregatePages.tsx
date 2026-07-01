import { useQuery } from "@tanstack/react-query";
import { ArrowDown } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeaderCell, TableRow, TableWrap } from "@/components/ui/table";
import { ErrorState } from "@/components/shared/states";
import { Page } from "@/components/shared/page";
import { DimensionLink } from "@/components/shared/stat-table";
import { apiClient, type CohortRow, type ModuleRow, type Snapshot, type StatRow } from "@/generated/api-client";
import { useAggregateDeltas } from "@/hooks/use-aggregate-deltas";
import { formatBytes, formatNumber, signedBytes, signedNumber } from "@/lib/format";
import type { UpdateSearch } from "@/lib/search";
import { cn } from "@/lib/utils";
import { AggregateTitle } from "@/pages/aggregates/AggregateTitle";

type AggregatePageProps = {
  snapshotId?: number;
  snapshots: Snapshot[];
  fromSnapshot?: number;
  toSnapshot?: number;
  sort?: string;
  updateSearch: UpdateSearch;
};

type AggregateSort = "count" | "shallow-size" | "reachable-size";

export function TypesPage(props: AggregatePageProps) {
  const sort = normalizeAggregateSort(props.sort);
  const rows = useQuery({
    queryKey: ["types", props.snapshotId, sort],
    queryFn: () => apiClient.types({ snapshot_id: props.snapshotId, limit: 100, sort }),
    enabled: Boolean(props.snapshotId)
  });
  const delta = useAggregateDeltas(props.snapshots, props.fromSnapshot, props.toSnapshot, "type_delta", "type");
  const updateSort = (nextSort: AggregateSort) => props.updateSearch({ sort: nextSort, selected: undefined, offset: undefined }, { history: "push" });
  return (
    <Page>
      <AggregateTitle title="Types" snapshots={props.snapshots} from={delta.from} to={delta.to} deltaAvailable={delta.isDeltaAvailable} updateSearch={props.updateSearch} />
      <TableWrap>
        <Table className="table-fixed" style={{ width: 1430 }}>
          <colgroup>
            <col style={{ width: 260 }} />
            <col style={{ width: 220 }} />
            <col style={{ width: 120 }} />
            <col style={{ width: 120 }} />
            <col style={{ width: 170 }} />
            <col style={{ width: 150 }} />
            <col style={{ width: 170 }} />
            <col style={{ width: 130 }} />
            <col style={{ width: 90 }} />
          </colgroup>
          <TableHead>
            <TableRow>
              <TableHeaderCell>type</TableHeaderCell>
              <TableHeaderCell>module</TableHeaderCell>
              <SortableHeader active={sort === "count"} align="right" onClick={() => updateSort("count")}>count</SortableHeader>
              <SortableHeader active={sort === "shallow-size"} align="right" onClick={() => updateSort("shallow-size")}>shallow</SortableHeader>
              <SortableHeader active={sort === "reachable-size"} align="right" onClick={() => updateSort("reachable-size")}>estimated reachable</SortableHeader>
              <TableHeaderCell className="text-right">max reachable</TableHeaderCell>
              <TableHeaderCell className="text-right">in / out</TableHeaderCell>
              <TableHeaderCell className="text-right">delta</TableHeaderCell>
              <TableHeaderCell>objects</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.data?.map((row) => {
              const rowDelta = delta.deltas.get(row.type);
              return (
                <TableRow key={`${row.module}:${row.type}`}>
                  <TableCell>
                    <DimensionLink label={row.type} onClick={() => props.updateSearch({ page: "objects", q: undefined, type: row.type, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" })} />
                  </TableCell>
                  <TableCell>
                    <DimensionLink label={row.module} onClick={() => props.updateSearch({ page: "objects", q: undefined, type: undefined, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" })} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.count)}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(row.shallow_size_sum)}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(row.estimated_reachable_size_sum ?? 0)}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(row.estimated_reachable_size_max ?? 0)}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.in_edges ?? 0)} / {formatNumber(row.out_edges ?? 0)}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{delta.isDeltaAvailable && rowDelta ? `${signedNumber(rowDelta.count_delta)} / ${signedBytes(rowDelta.shallow_size_delta)}` : "-"}</TableCell>
                  <TableCell>
                    <Button variant="secondary" size="sm" onClick={() => props.updateSearch({ page: "objects", q: undefined, type: row.type, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" })}>
                      View
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableWrap>
      {rows.error ? <ErrorState error={rows.error} /> : null}
    </Page>
  );
}

export function ModulesPage(props: AggregatePageProps) {
  const sort = normalizeAggregateSort(props.sort);
  const rows = useQuery({
    queryKey: ["modules", props.snapshotId, sort],
    queryFn: () => apiClient.modules({ snapshot_id: props.snapshotId, limit: 100, sort }),
    enabled: Boolean(props.snapshotId)
  });
  const delta = useAggregateDeltas(props.snapshots, props.fromSnapshot, props.toSnapshot, "module_delta", "module");
  return (
    <AggregateListPage
      title="Modules"
      rows={rows.data ?? []}
      error={rows.error}
      deltaAvailable={delta.isDeltaAvailable}
      deltas={delta.deltas}
      from={delta.from}
      to={delta.to}
      snapshots={props.snapshots}
      sort={sort}
      updateSearch={props.updateSearch}
      rowKey={(row) => row.module}
      name={(row) => row.module}
      secondary={() => ""}
      count={(row) => row.count}
      shallow={(row) => row.shallow_size_sum}
      reachable={(row) => row.estimated_reachable_size_sum ?? 0}
      onView={(row) => props.updateSearch({ page: "objects", q: undefined, type: undefined, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" })}
    />
  );
}

export function CohortsPage(props: AggregatePageProps) {
  const sort = normalizeAggregateSort(props.sort);
  const rows = useQuery({
    queryKey: ["cohorts", props.snapshotId, sort],
    queryFn: () => apiClient.cohorts({ snapshot_id: props.snapshotId, limit: 100, sort }),
    enabled: Boolean(props.snapshotId)
  });
  const delta = useAggregateDeltas(props.snapshots, props.fromSnapshot, props.toSnapshot, "cohort_delta", "cohort");
  return (
    <AggregateListPage
      title="Cohorts"
      rows={rows.data ?? []}
      error={rows.error}
      deltaAvailable={delta.isDeltaAvailable}
      deltas={delta.deltas}
      from={delta.from}
      to={delta.to}
      snapshots={props.snapshots}
      sort={sort}
      updateSearch={props.updateSearch}
      rowKey={(row) => row.cohort}
      name={(row) => row.cohort}
      secondary={(row) => `${row.type_count} types`}
      count={(row) => row.count}
      shallow={(row) => row.shallow_size_sum}
      reachable={(row) => row.estimated_reachable_size_sum ?? 0}
      onView={(row) => props.updateSearch({ page: "objects", q: undefined, type: undefined, module: undefined, cohort: row.cohort, selected: undefined, offset: undefined }, { history: "push" })}
    />
  );
}

type AggregateListPageProps<T extends ModuleRow | CohortRow> = {
  title: string;
  rows: T[];
  error: unknown;
  snapshots: Snapshot[];
  from?: number;
  to?: number;
  sort: AggregateSort;
  deltaAvailable: boolean;
  deltas: Map<string, { count_delta: number; shallow_size_delta: number }>;
  updateSearch: UpdateSearch;
  rowKey: (row: T) => string;
  name: (row: T) => string;
  secondary: (row: T) => string;
  count: (row: T) => number;
  shallow: (row: T) => number;
  reachable: (row: T) => number;
  onView: (row: T) => void;
};

function AggregateListPage<T extends ModuleRow | CohortRow>({
  title,
  rows,
  error,
  snapshots,
  from,
  to,
  sort,
  deltaAvailable,
  deltas,
  updateSearch,
  rowKey,
  name,
  secondary,
  count,
  shallow,
  reachable,
  onView
}: AggregateListPageProps<T>) {
  const updateSort = (nextSort: AggregateSort) => updateSearch({ sort: nextSort, selected: undefined, offset: undefined }, { history: "push" });
  return (
    <Page>
      <AggregateTitle title={title} snapshots={snapshots} from={from} to={to} deltaAvailable={deltaAvailable} updateSearch={updateSearch} />
      <TableWrap>
        <Table className="table-fixed" style={{ width: 980 }}>
          <colgroup>
            <col style={{ width: 320 }} />
            <col style={{ width: 130 }} />
            <col style={{ width: 140 }} />
            <col style={{ width: 190 }} />
            <col style={{ width: 120 }} />
            <col style={{ width: 80 }} />
          </colgroup>
          <TableHead>
            <TableRow>
              <TableHeaderCell>{title === "Modules" ? "module" : "cohort"}</TableHeaderCell>
              <SortableHeader active={sort === "count"} align="right" onClick={() => updateSort("count")}>count</SortableHeader>
              <SortableHeader active={sort === "shallow-size"} align="right" onClick={() => updateSort("shallow-size")}>shallow</SortableHeader>
              <SortableHeader active={sort === "reachable-size"} align="right" onClick={() => updateSort("reachable-size")}>estimated reachable</SortableHeader>
              <TableHeaderCell className="text-right">delta</TableHeaderCell>
              <TableHeaderCell>objects</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((row) => {
              const key = rowKey(row);
              const rowDelta = deltas.get(key);
              return (
                <TableRow key={key}>
                  <TableCell>
                    <DimensionLink label={name(row)} sublabel={secondary(row) || undefined} onClick={() => onView(row)} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(count(row))}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(shallow(row))}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(reachable(row))}</TableCell>
                  <TableCell className="text-right tabular-nums whitespace-nowrap">{deltaAvailable && rowDelta ? `${signedNumber(rowDelta.count_delta)} / ${signedBytes(rowDelta.shallow_size_delta)}` : "-"}</TableCell>
                  <TableCell>
                    <Button variant="secondary" size="sm" onClick={() => onView(row)}>
                      View
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableWrap>
      {error ? <ErrorState error={error} /> : null}
    </Page>
  );
}

function SortableHeader({ active, align, onClick, children }: { active: boolean; align?: "left" | "right"; onClick: () => void; children: string }) {
  return (
    <TableHeaderCell className={cn(align === "right" && "text-right")}>
      <button
        type="button"
        className={cn(
          "inline-flex items-center gap-1 rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          align === "right" && "justify-end",
          active && "text-foreground"
        )}
        onClick={onClick}
      >
        <span>{children}</span>
        {active ? <ArrowDown size={13} /> : null}
      </button>
    </TableHeaderCell>
  );
}

function normalizeAggregateSort(sort?: string): AggregateSort {
  if (sort === "count" || sort === "reachable-size") return sort;
  return "shallow-size";
}
