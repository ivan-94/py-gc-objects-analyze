import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ErrorState } from "@/components/shared/states";
import { JsonBlock, JsonSheet } from "@/components/shared/json";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient, type JobData, type SavedIdset, type SchemaSummary } from "@/generated/api-client";
import { formatNumber } from "@/lib/format";
import { objectIdsFromResult, quoteSqlIdentifier, savedIdsetSql } from "@/lib/sql";

export function SqlPage({ snapshotId }: { snapshotId?: number }) {
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("select object_id, type, shallow_size from objects limit 20");
  const [result, setResult] = useState<Record<string, unknown> | null>(null);
  const [jobId, setJobId] = useState<string | null>(null);
  const [explainPlan, setExplainPlan] = useState<Record<string, unknown> | null>(null);
  const [idsetName, setIdsetName] = useState("SQL result");
  const schema = useQuery({ queryKey: ["schema"], queryFn: () => apiClient.schema() });
  const savedIdsets = useQuery({
    queryKey: ["saved-idsets", snapshotId],
    queryFn: () => apiClient.savedIdsets({ snapshot_id: snapshotId }),
    enabled: Boolean(snapshotId)
  });
  const job = useQuery({
    queryKey: ["job", jobId],
    queryFn: () => apiClient.jobStatus(jobId ?? ""),
    enabled: Boolean(jobId),
    refetchInterval: (queryState) => {
      const status = (queryState.state.data as JobData | undefined)?.status;
      return status === "queued" || status === "running" || status === "canceling" ? 100 : false;
    }
  });
  const mutation = useMutation({
    mutationFn: async () => {
      const data = await apiClient.sqlQuery({ query, limit: 200 }) as Record<string, unknown>;
      setResult(data);
      setExplainPlan(null);
      return data;
    }
  });
  const explain = useMutation({
    mutationFn: async () => {
      const data = await apiClient.sqlExplain({ query, limit: 200 }) as Record<string, unknown>;
      setResult(data);
      setExplainPlan(data);
      return data;
    }
  });
  const runJob = useMutation({
    mutationFn: async () => {
      const data = await apiClient.sqlQuery({ query, limit: 200, async: true }) as JobData;
      setJobId(data.job_id);
      return data;
    }
  });
  const cancelJob = useMutation({
    mutationFn: async () => {
      if (!jobId) return null;
      return apiClient.cancelJob(jobId);
    }
  });
  const displayResult = job.data?.result ?? result ?? {};
  const objectIds = objectIdsFromResult(displayResult);
  const saveIdset = useMutation({
    mutationFn: async () => {
      if (!snapshotId) throw new Error("snapshot_id is required to save an idset");
      const data = await apiClient.saveIdset({
        snapshot_id: snapshotId,
        name: idsetName,
        object_ids: objectIds,
        source: { kind: "sql", query, saved_from_rows: objectIds.length }
      });
      await queryClient.invalidateQueries({ queryKey: ["saved-idsets", snapshotId] });
      setIdsetName(`${idsetName} copy`);
      return data;
    }
  });

  return (
    <Page>
      <PageTitle
        title="SQL"
        meta="Read-only SQL workbench for inspecting the local SQLite analysis database."
        actions={
          <>
            <Button onClick={() => mutation.mutate()}>Run</Button>
            <Button variant="secondary" onClick={() => explain.mutate()}>Explain</Button>
            <Button variant="secondary" onClick={() => runJob.mutate()}>Run Job</Button>
            <Button variant="secondary" disabled={!jobId || job.data?.status === "succeeded" || job.data?.status === "failed" || job.data?.status === "canceled"} onClick={() => cancelJob.mutate()}>
              Cancel Job
            </Button>
          </>
        }
      />
      <div className="grid gap-4 xl:grid-cols-[340px_minmax(0,1fr)]">
        <div className="min-w-0 space-y-3">
          <SchemaBrowser schema={schema.data} onSelectTable={(table) => setQuery(`select * from ${quoteSqlIdentifier(table)} limit 50`)} />
          <SavedIdsetsPanel
            idsets={savedIdsets.data?.rows ?? []}
            idsetName={idsetName}
            setIdsetName={setIdsetName}
            objectCount={objectIds.length}
            canSave={Boolean(snapshotId)}
            savePending={saveIdset.isPending}
            onSave={() => saveIdset.mutate()}
            onUse={(idset) => setQuery(savedIdsetSql(idset))}
          />
        </div>
        <div className="min-w-0 space-y-3">
          <Card>
            <CardHeader>
              <CardTitle>Query</CardTitle>
            </CardHeader>
            <CardContent>
              <Textarea
                className="min-h-[280px] resize-y font-mono text-sm leading-6"
                spellCheck={false}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
              />
            </CardContent>
          </Card>
          {mutation.error || explain.error || runJob.error || cancelJob.error || saveIdset.error ? <ErrorState error={(mutation.error ?? explain.error ?? runJob.error ?? cancelJob.error ?? saveIdset.error) as Error} /> : null}
          {saveIdset.data ? <div className="rounded-lg border border-border bg-background p-3 text-sm">Saved idset <strong>{saveIdset.data.idset.name}</strong> with {formatNumber(saveIdset.data.idset.object_count)} objects.</div> : null}
          {jobId ? <JobStatusPanel job={job.data ?? runJob.data ?? undefined} /> : null}
          <Card>
            <CardHeader>
              <CardTitle>Result</CardTitle>
            </CardHeader>
            <CardContent>
              <JsonBlock className="min-h-[260px] bg-muted/20" value={displayResult} />
            </CardContent>
          </Card>
        </div>
      </div>
      {explainPlan ? <JsonSheet title="SQL Explain Plan" value={explainPlan} onClose={() => setExplainPlan(null)} /> : null}
    </Page>
  );
}

function SavedIdsetsPanel({
  idsets,
  idsetName,
  setIdsetName,
  objectCount,
  canSave,
  savePending,
  onSave,
  onUse
}: {
  idsets: SavedIdset[];
  idsetName: string;
  setIdsetName: (name: string) => void;
  objectCount: number;
  canSave: boolean;
  savePending: boolean;
  onSave: () => void;
  onUse: (idset: SavedIdset) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Saved Idsets</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-2 border-b border-border pb-3">
          <Input value={idsetName} onChange={(event) => setIdsetName(event.target.value)} placeholder="idset name" />
          <Button className="w-full" variant="secondary" disabled={!canSave || !objectCount || savePending} onClick={onSave}>
            Save Idset
          </Button>
          <div className="text-xs text-muted-foreground">{formatNumber(objectCount)} object ids in current result</div>
        </div>
        <div className="space-y-2">
          {idsets.length ? idsets.map((idset) => (
            <div key={idset.idset_id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
              <span className="min-w-0">
                <strong className="block break-words text-sm">{idset.name}</strong>
                <em className="text-xs not-italic text-muted-foreground">{formatNumber(idset.object_count)} objects</em>
              </span>
              <Button variant="secondary" size="sm" onClick={() => onUse(idset)}>Use</Button>
            </div>
          )) : <div className="text-sm text-muted-foreground">No saved idsets</div>}
        </div>
      </CardContent>
    </Card>
  );
}

function JobStatusPanel({ job }: { job?: JobData }) {
  if (!job) return <div className="rounded-lg border border-border bg-background p-3 text-sm text-muted-foreground">Starting job...</div>;
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-background p-3 text-sm">
      <span className="font-mono text-xs text-muted-foreground">{job.job_id}</span>
      <Badge tone={job.status === "failed" || job.status === "canceled" ? "warn" : "neutral"}>{job.status}</Badge>
      <progress className="h-2 w-40" value={job.progress} max={1} />
      {job.message ? <span>{job.message}</span> : null}
    </div>
  );
}

function SchemaBrowser({ schema, onSelectTable }: { schema?: SchemaSummary; onSelectTable: (table: string) => void }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Schema</CardTitle>
      </CardHeader>
      <CardContent className="max-h-[calc(100vh-220px)] overflow-auto">
        {schema?.tables.map((table) => (
          <details key={table.name} className="border-t border-border py-2 first:border-t-0" open={table.name === "objects" || table.name === "edges"}>
            <summary>
              <button className="font-mono text-sm text-primary" onClick={() => onSelectTable(table.name)}>{table.name}</button>
            </summary>
            <div className="mt-2 space-y-1 pl-4">
              {(schema.columns[table.name] ?? []).map((column) => (
                <div key={column.name} className="grid grid-cols-[minmax(0,1fr)_auto] gap-2 text-xs">
                  <span className="break-words font-mono">{column.name}</span>
                  <em className="not-italic text-muted-foreground">{column.type || "ANY"}</em>
                </div>
              ))}
            </div>
          </details>
        ))}
      </CardContent>
    </Card>
  );
}
