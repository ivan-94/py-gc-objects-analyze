import { useQuery } from "@tanstack/react-query";

import { Page, PageTitle } from "@/components/shared/page";
import { apiClient } from "@/generated/api-client";

export function ReportPage({ snapshotId }: { snapshotId?: number }) {
  const report = useQuery({
    queryKey: ["report", snapshotId],
    queryFn: () => apiClient.reportMarkdown({ snapshot_id: snapshotId }),
    enabled: Boolean(snapshotId)
  });
  return (
    <Page>
      <PageTitle title="Report" />
      <pre className="max-w-full overflow-auto rounded-lg border border-border bg-background p-4 text-sm whitespace-pre-wrap">{report.data}</pre>
    </Page>
  );
}
