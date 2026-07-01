import { useQuery } from "@tanstack/react-query";

import { apiClient, type AggregateDelta, type Snapshot } from "@/generated/api-client";

export function useAggregateDeltas(
  snapshots: Snapshot[],
  fromSnapshot: number | undefined,
  toSnapshot: number | undefined,
  field: "type_delta" | "module_delta" | "cohort_delta",
  key: "type" | "module" | "cohort"
) {
  const from = fromSnapshot ?? snapshots[0]?.snapshot_id;
  const to = toSnapshot ?? snapshots[1]?.snapshot_id ?? snapshots[0]?.snapshot_id;
  const query = useQuery({
    queryKey: ["aggregate-delta", field, from, to],
    queryFn: () => apiClient.diff({ from_snapshot_id: from, to_snapshot_id: to }),
    enabled: Boolean(from && to && from !== to)
  });
  const deltas = new Map<string, AggregateDelta>();
  for (const row of query.data?.[field] ?? []) {
    const id = row[key];
    if (id) deltas.set(id, row);
  }
  return { from, to, deltas, isDeltaAvailable: Boolean(from && to && from !== to), query };
}
