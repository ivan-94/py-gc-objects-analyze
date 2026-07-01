import type { Snapshot } from "@/generated/api-client";
import type { UpdateSearch } from "@/lib/search";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { PageTitle } from "@/components/shared/page";

type AggregateTitleProps = {
  title: string;
  snapshots: Snapshot[];
  from?: number;
  to?: number;
  deltaAvailable: boolean;
  updateSearch: UpdateSearch;
};

export function AggregateTitle({ title, snapshots, from, to, deltaAvailable, updateSearch }: AggregateTitleProps) {
  const disabled = snapshots.length < 2;
  return (
    <PageTitle
      title={title}
      meta={
        deltaAvailable
          ? `Delta compares snapshot ${from} -> ${to}`
          : disabled
            ? "Delta requires at least two snapshots"
            : "Delta hidden because both selectors point to the same snapshot"
      }
      actions={
        <div className="flex items-center gap-2">
          <Badge tone={deltaAvailable ? "success" : "neutral"}>Delta</Badge>
          <Select disabled={disabled} value={from ?? ""} onChange={(event) => updateSearch({ from: Number(event.target.value) })}>
            {snapshots.map((snapshot) => (
              <option key={snapshot.snapshot_id} value={snapshot.snapshot_id}>
                {snapshot.snapshot_id}
              </option>
            ))}
          </Select>
          <span className="text-sm text-muted-foreground">to</span>
          <Select disabled={disabled} value={to ?? ""} onChange={(event) => updateSearch({ to: Number(event.target.value) })}>
            {snapshots.map((snapshot) => (
              <option key={snapshot.snapshot_id} value={snapshot.snapshot_id}>
                {snapshot.snapshot_id}
              </option>
            ))}
          </Select>
        </div>
      }
    />
  );
}
