import type { ModuleRow, StatRow } from "@/generated/api-client";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeaderCell, TableRow } from "@/components/ui/table";
import { formatBytes, formatNumber } from "@/lib/format";
import { cn } from "@/lib/utils";

export function TypeStatTable({
  title,
  rows,
  reachable = false,
  onTypeClick
}: {
  title: string;
  rows: StatRow[];
  reachable?: boolean;
  onTypeClick?: (row: StatRow) => void;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="overflow-x-auto">
        <Table className="min-w-[640px]">
          <TableHead>
            <TableRow>
              <TableHeaderCell className="w-[58%]">type</TableHeaderCell>
              <TableHeaderCell className="w-[18%] text-right">count</TableHeaderCell>
              <TableHeaderCell className="w-[24%] text-right">{reachable ? "estimated reachable" : "shallow"}</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((row, index) => (
              <TableRow key={`${row.type}:${index}`}>
                <TableCell>
                  <DimensionLink label={row.type} sublabel={row.module !== "builtins" ? row.module : undefined} onClick={onTypeClick ? () => onTypeClick(row) : undefined} />
                </TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.count)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">
                  {formatBytes(reachable ? (row.estimated_reachable_size_sum ?? 0) : row.shallow_size_sum)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

export function ModuleStatTable({ title, rows, onModuleClick }: { title: string; rows: ModuleRow[]; onModuleClick?: (row: ModuleRow) => void }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="overflow-x-auto">
        <Table className="min-w-[640px]">
          <TableHead>
            <TableRow>
              <TableHeaderCell className="w-[58%]">module</TableHeaderCell>
              <TableHeaderCell className="w-[18%] text-right">count</TableHeaderCell>
              <TableHeaderCell className="w-[24%] text-right">shallow</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.module}>
                <TableCell>
                  <DimensionLink label={row.module} onClick={onModuleClick ? () => onModuleClick(row) : undefined} />
                </TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatNumber(row.count)}</TableCell>
                <TableCell className="text-right tabular-nums whitespace-nowrap">{formatBytes(row.shallow_size_sum)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

export function DimensionLink({ label, sublabel, onClick }: { label: string; sublabel?: string; onClick?: () => void }) {
  const content = (
    <>
      <span className="block truncate font-medium" title={label}>
        {label}
      </span>
      {sublabel ? (
        <span className="mt-0.5 block truncate text-xs text-muted-foreground" title={sublabel}>
          {sublabel}
        </span>
      ) : null}
    </>
  );
  if (!onClick) return <div className="min-w-0">{content}</div>;
  return (
    <button
      type="button"
      className={cn("block min-w-0 max-w-full text-left text-primary hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring")}
      onClick={onClick}
    >
      {content}
    </button>
  );
}
