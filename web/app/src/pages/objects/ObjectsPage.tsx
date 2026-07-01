import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Table, TableBody, TableCell, TableHead, TableHeaderCell, TableRow, TableWrap } from "@/components/ui/table";
import { ErrorState } from "@/components/shared/states";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient } from "@/generated/api-client";
import { formatBytes, formatNumber } from "@/lib/format";
import { clampInt, valueOrUndefined, type AppSearch, type UpdateSearch } from "@/lib/search";
import { ObjectDetailSheet } from "@/pages/objects/ObjectDetailSheet";

type ObjectsPageProps = {
  snapshotId?: number;
  q: string;
  typeName?: string;
  moduleName?: string;
  cohort?: string;
  sort: string;
  limit?: number;
  offset?: number;
  selected?: string;
  updateSearch: UpdateSearch;
};

const objectSorts = [
  ["reachable-size", "Reachable"],
  ["shallow-size", "Shallow"],
  ["in-edges", "In edges"],
  ["out-edges", "Out edges"],
  ["type", "Type"],
  ["module", "Module"],
  ["object-id", "Object id"]
] as const;

export function ObjectsPage({ snapshotId, q, typeName, moduleName, cohort, sort, limit, offset, selected, updateSearch }: ObjectsPageProps) {
  const pageLimit = clampInt(limit, 100, 1, 500);
  const pageOffset = clampInt(offset, 0, 0, 1_000_000);
  const objects = useQuery({
    queryKey: ["objects", snapshotId, q, typeName, moduleName, cohort, sort, pageLimit, pageOffset],
    queryFn: () =>
      apiClient.objects({
        snapshot_id: snapshotId,
        q: valueOrUndefined(q),
        type: typeName,
        module: moduleName,
        cohort,
        sort,
        order: "desc",
        limit: pageLimit,
        offset: pageOffset
      }),
    enabled: Boolean(snapshotId)
  });
  const rows = objects.data?.data ?? [];
  const total = objects.data?.meta?.total as number | undefined;
  const hasPrevious = pageOffset > 0;
  const hasNext = total === undefined ? rows.length === pageLimit : pageOffset + rows.length < total;
  const resetOffset = (patch: Partial<AppSearch>) => updateSearch({ ...patch, offset: undefined, selected: undefined });

  return (
    <Page>
      <PageTitle
        title="Objects"
        meta={total !== undefined ? `${formatNumber(total)} rows match current filters` : "Object list"}
        actions={
          <>
            <Input className="w-64" value={q} onChange={(event) => resetOffset({ q: valueOrUndefined(event.target.value), type: undefined, module: undefined, cohort: undefined })} placeholder="Filter type, module, object id" />
            <Select value={sort} onChange={(event) => resetOffset({ sort: event.target.value })}>
              {objectSorts.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </Select>
          </>
        }
      />
      <FilterBar typeName={typeName} moduleName={moduleName} cohort={cohort} updateSearch={updateSearch} />
      <TableWrap>
        <Table className="min-w-[1360px]">
          <colgroup>
            <col className="w-[20%]" />
            <col className="w-[18%]" />
            <col className="w-[24%]" />
            <col className="w-[10%]" />
            <col className="w-[13%]" />
            <col className="w-[5%]" />
            <col className="w-[5%]" />
            <col className="w-[5%]" />
          </colgroup>
          <TableHead>
            <TableRow>
              <TableHeaderCell>object_id</TableHeaderCell>
              <TableHeaderCell>type</TableHeaderCell>
              <TableHeaderCell>module</TableHeaderCell>
              <TableHeaderCell className="text-right">shallow</TableHeaderCell>
              <TableHeaderCell className="text-right">estimated reachable</TableHeaderCell>
              <TableHeaderCell className="text-right">in</TableHeaderCell>
              <TableHeaderCell className="text-right">out</TableHeaderCell>
              <TableHeaderCell>state</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((row) => (
              <TableRow
                key={row.object_id}
                role="button"
                tabIndex={0}
                className="cursor-pointer"
                onClick={() => updateSearch({ selected: row.object_id }, { history: "push" })}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    updateSearch({ selected: row.object_id }, { history: "push" });
                  }
                }}
              >
                <TableCell className="truncate font-mono text-xs" title={row.object_id}>{row.object_id}</TableCell>
                <TableCell>
                  <button
                    type="button"
                    className="block max-w-full truncate text-left font-medium text-primary hover:underline"
                    title={row.type}
                    onClick={(event) => {
                      event.stopPropagation();
                      updateSearch({ type: row.type, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" });
                    }}
                  >
                    {row.type}
                  </button>
                </TableCell>
                <TableCell>
                  <button
                    type="button"
                    className="block max-w-full truncate text-left text-muted-foreground hover:text-foreground hover:underline"
                    title={row.module}
                    onClick={(event) => {
                      event.stopPropagation();
                      updateSearch({ type: undefined, module: row.module, cohort: undefined, selected: undefined, offset: undefined }, { history: "push" });
                    }}
                  >
                    {row.module}
                  </button>
                </TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(row.shallow_size)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">
                  <span>{formatBytes(row.estimated_reachable_size)}</span>
                  {row.reachable_truncated ? <Badge tone="warn" className="ml-2">truncated</Badge> : null}
                </TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.in_edges)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.out_edges)}</TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {row.stub ? <Badge>stub</Badge> : null}
                    {row.missing_referents ? <Badge tone="warn">missing</Badge> : null}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
      <div className="flex flex-wrap items-center justify-end gap-2 text-sm text-muted-foreground">
        <span>
          {total === undefined
            ? `${formatNumber(pageOffset + 1)}-${formatNumber(pageOffset + rows.length)}`
            : `${formatNumber(Math.min(pageOffset + 1, total))}-${formatNumber(Math.min(pageOffset + rows.length, total))} of ${formatNumber(total)}`}
        </span>
        <label className="flex items-center gap-2">
          Rows
          <Select className="w-20" value={pageLimit} onChange={(event) => updateSearch({ limit: Number(event.target.value), offset: undefined, selected: undefined })}>
            {[25, 50, 100, 200].map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </Select>
        </label>
        <Button variant="secondary" disabled={!hasPrevious} onClick={() => updateSearch({ offset: Math.max(0, pageOffset - pageLimit), selected: undefined }, { history: "push" })}>
          Previous
        </Button>
        <Button variant="secondary" disabled={!hasNext} onClick={() => updateSearch({ offset: pageOffset + pageLimit, selected: undefined }, { history: "push" })}>
          Next
        </Button>
      </div>
      {objects.error ? <ErrorState error={objects.error} /> : null}
      {selected ? <ObjectDetailSheet snapshotId={snapshotId} objectId={selected} onClose={() => updateSearch({ selected: undefined }, { history: "push" })} updateSearch={updateSearch} /> : null}
    </Page>
  );
}

function FilterBar({
  typeName,
  moduleName,
  cohort,
  updateSearch
}: {
  typeName?: string;
  moduleName?: string;
  cohort?: string;
  updateSearch: UpdateSearch;
}) {
  const filters = [
    typeName ? ["type", typeName, { type: undefined, selected: undefined }] : undefined,
    moduleName ? ["module", moduleName, { module: undefined, selected: undefined }] : undefined,
    cohort ? ["cohort", cohort, { cohort: undefined, selected: undefined }] : undefined
  ].filter(Boolean) as [string, string, Partial<AppSearch>][];
  if (!filters.length) return null;
  return (
    <div className="flex flex-wrap gap-2">
      {filters.map(([label, value, patch]) => (
        <span key={label} className="inline-flex items-center gap-2 rounded-md border border-border bg-background px-2 py-1 text-sm">
          <strong className="text-xs text-muted-foreground">{label}</strong>
          <span className="max-w-72 truncate">{value}</span>
          <button className="px-1 text-muted-foreground hover:text-foreground" aria-label={`Clear ${label} filter`} onClick={() => updateSearch(patch, { history: "push" })}>
            ×
          </button>
        </span>
      ))}
    </div>
  );
}
