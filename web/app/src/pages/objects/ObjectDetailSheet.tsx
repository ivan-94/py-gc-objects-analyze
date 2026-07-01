import { useQuery } from "@tanstack/react-query";
import { Copy, Network } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Sheet } from "@/components/ui/sheet";
import { EmptyState, Skeleton } from "@/components/shared/states";
import { apiClient, type ObjectRow } from "@/generated/api-client";
import { formatBytes, formatNumber } from "@/lib/format";
import type { UpdateSearch } from "@/lib/search";

type ObjectDetailSheetProps = {
  snapshotId?: number;
  objectId: string;
  onClose: () => void;
  updateSearch: UpdateSearch;
};

export function ObjectDetailSheet({ snapshotId, objectId, onClose, updateSearch }: ObjectDetailSheetProps) {
  const detail = useQuery({
    queryKey: ["object", snapshotId, objectId],
    queryFn: () => apiClient.objectDetail(objectId, { snapshot_id: snapshotId }),
    enabled: Boolean(snapshotId && objectId)
  });
  const paths = useQuery({
    queryKey: ["paths", snapshotId, objectId],
    queryFn: () => apiClient.objectPaths(objectId, { snapshot_id: snapshotId, direction: "referrers", depth: 5, fanout_limit: 30, limit: 5 }),
    enabled: Boolean(snapshotId && objectId)
  });

  const object = detail.data?.object;

  return (
    <Sheet
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={objectId}
      description={object ? `${object.module}.${object.type}` : "Object detail"}
      footer={
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" onClick={() => void navigator.clipboard?.writeText(objectId)}>
            <Copy size={15} />
            Copy id
          </Button>
          <Button
            onClick={() => {
              updateSearch({ page: "graph", root: objectId, selected: undefined, graphDepth: 2, graphLimit: 200, graphDirection: "both" }, { history: "push" });
            }}
          >
            <Network size={15} />
            Open graph
          </Button>
        </div>
      }
    >
      {!object ? (
        <Skeleton title="Object detail" />
      ) : (
        <div className="space-y-4">
          <ObjectSummary object={object} />
          <ReferenceRows title="Top Referents" rows={detail.data?.top_referents ?? []} updateSearch={updateSearch} />
          <ReferenceRows title="Top Referrers" rows={detail.data?.top_referrers ?? []} updateSearch={updateSearch} />
          <PathSamples rows={paths.data?.paths ?? []} />
        </div>
      )}
    </Sheet>
  );
}

function ObjectSummary({ object }: { object: ObjectRow }) {
  return (
    <div className="grid gap-3 md:grid-cols-2">
      <Metric label="shallow" value={formatBytes(object.shallow_size)} />
      <Metric
        label="estimated reachable"
        value={formatBytes(object.estimated_reachable_size)}
        badge={object.reachable_truncated ? <Badge tone="warn">truncated</Badge> : undefined}
      />
      <Metric label="incoming refs" value={formatNumber(object.in_edges)} />
      <Metric label="outgoing refs" value={formatNumber(object.out_edges)} />
      <Metric label="state" value={object.stub ? "stub" : "tracked"} badge={object.missing_referents ? <Badge tone="warn">missing refs</Badge> : undefined} />
      <Metric label="module" value={object.module} />
    </div>
  );
}

function Metric({ label, value, badge }: { label: string; value: string; badge?: React.ReactNode }) {
  return (
    <Card>
      <CardContent className="p-3">
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="mt-1 flex min-w-0 items-center gap-2">
          <strong className="min-w-0 break-words text-sm font-semibold">{value}</strong>
          {badge}
        </div>
      </CardContent>
    </Card>
  );
}

function ReferenceRows({ title, rows, updateSearch }: { title: string; rows: ObjectRow[]; updateSearch: UpdateSearch }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {rows.length ? (
          rows.map((row) => (
            <button
              key={`${title}-${row.object_id}`}
              className="grid w-full grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-md border border-border p-2 text-left hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => updateSearch({ selected: row.object_id }, { history: "push" })}
            >
              <span className="min-w-0">
                <span className="block break-all font-mono text-xs text-muted-foreground">{row.object_id}</span>
                <span className="block break-words text-sm font-medium">{row.type}</span>
                <span className="block break-words text-xs text-muted-foreground">{row.module}</span>
              </span>
              <span className="text-right text-xs tabular-nums text-muted-foreground">
                <span className="block">{formatBytes(row.estimated_reachable_size)}</span>
                <span className="block">{formatNumber(row.in_edges)} in</span>
              </span>
            </button>
          ))
        ) : (
          <EmptyState label="No rows" className="border-0 p-0" />
        )}
      </CardContent>
    </Card>
  );
}

function PathSamples({ rows }: { rows: string[][] }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Owner Path Samples</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2">
        {rows.length ? (
          rows.map((path, index) => (
            <div key={index} className="rounded-md bg-muted px-3 py-2 font-mono text-xs break-all text-muted-foreground">
              {path.join(" -> ")}
            </div>
          ))
        ) : (
          <EmptyState label="No sampled paths" className="border-0 p-0" />
        )}
      </CardContent>
    </Card>
  );
}
