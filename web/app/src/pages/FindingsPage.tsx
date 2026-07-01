import { useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeaderCell, TableRow, TableWrap } from "@/components/ui/table";
import { ErrorState } from "@/components/shared/states";
import { JsonButton, JsonSheet } from "@/components/shared/json";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient } from "@/generated/api-client";

export function FindingsPage({ snapshotId }: { snapshotId?: number }) {
  const [selectedEvidence, setSelectedEvidence] = useState<{ title: string; value: unknown } | null>(null);
  const findings = useQuery({
    queryKey: ["findings", snapshotId],
    queryFn: () => apiClient.findings({ snapshot_id: snapshotId }),
    enabled: Boolean(snapshotId)
  });
  return (
    <Page>
      <PageTitle title="Findings" />
      <TableWrap>
        <Table className="min-w-[1240px]">
          <colgroup>
            <col className="w-[8%]" />
            <col className="w-[12%]" />
            <col className="w-[22%]" />
            <col className="w-[28%]" />
            <col className="w-[22%]" />
            <col className="w-[8%]" />
          </colgroup>
          <TableHead>
            <TableRow>
              <TableHeaderCell>severity</TableHeaderCell>
              <TableHeaderCell>kind</TableHeaderCell>
              <TableHeaderCell>title</TableHeaderCell>
              <TableHeaderCell>message</TableHeaderCell>
              <TableHeaderCell>action</TableHeaderCell>
              <TableHeaderCell>evidence</TableHeaderCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {findings.data?.rows.map((row, index) => (
              <TableRow key={index}>
                <TableCell><Badge tone={row.severity === "warn" ? "warn" : "neutral"}>{row.severity}</Badge></TableCell>
                <TableCell className="truncate" title={row.kind}>{row.kind}</TableCell>
                <TableCell className="font-medium">{row.title}</TableCell>
                <TableCell className="text-muted-foreground">{row.message}</TableCell>
                <TableCell>{row.action}</TableCell>
                <TableCell><JsonButton title={row.title} value={row.evidence} onOpen={setSelectedEvidence} /></TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
      {findings.error ? <ErrorState error={findings.error} /> : null}
      {selectedEvidence ? <JsonSheet title={selectedEvidence.title} value={selectedEvidence.value} onClose={() => setSelectedEvidence(null)} /> : null}
    </Page>
  );
}
