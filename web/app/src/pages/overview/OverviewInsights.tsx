import { AlertTriangle, ArrowRight, Database, Gauge, Search, ShieldQuestion } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { CohortRow, StatRow, Summary } from "@/generated/api-client";
import { formatBytes, formatNumber, formatPercent } from "@/lib/format";
import { cn } from "@/lib/utils";

type OverviewInsightsProps = {
  summary: Summary;
  types: StatRow[];
  cohorts: CohortRow[];
  typesLoading: boolean;
  cohortsLoading: boolean;
  onViewType: (row: StatRow) => void;
  onViewCohort: (cohort: string) => void;
  onOpenCohorts: () => void;
  onOpenFindings: () => void;
};

type Probe = {
  title: string;
  evidence: string;
  action: string;
  onClick: () => void;
};

const RUNTIME_TYPES = new Set(["function", "dict", "tuple", "module", "cell", "property", "method", "staticmethod", "classmethod", "code", "type"]);

export function OverviewInsights({
  summary,
  types,
  cohorts,
  typesLoading,
  cohortsLoading,
  onViewType,
  onViewCohort,
  onOpenCohorts,
  onOpenFindings
}: OverviewInsightsProps) {
  const stubRatio = safeRatio(summary.missing_stub_summary.stub_count, summary.snapshot.object_count);
  const topAmplifiedCohort = mostAmplifiedCohort(cohorts);
  const topReachableCohort = cohorts[0];
  const topRuntimeType = summary.top_reachable_types.find(isRuntimeType);
  const runtimeReachable = summary.top_reachable_types.filter(isRuntimeType).reduce((sum, row) => sum + (row.estimated_reachable_size_sum ?? 0), 0);
  const topApplicationType = types.find(isApplicationType);
  const probes = recommendedProbes({
    stubRatio,
    topAmplifiedCohort,
    topReachableCohort,
    topRuntimeType,
    topApplicationType,
    onViewType,
    onViewCohort,
    onOpenFindings
  });

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Python Expert Brief</CardTitle>
          <Badge>heuristic leads</Badge>
        </CardHeader>
        <CardContent className="grid gap-4 xl:grid-cols-[minmax(0,1.25fr)_minmax(360px,0.75fr)]">
          <div className="space-y-3">
            <p className="max-w-4xl text-sm leading-6 text-muted-foreground">
              This snapshot is dominated by Python runtime metadata plus a few resource-shaped cohorts. Treat estimated reachable values as overlapping leads, then drill into cohorts, non-builtin types, and
              referrer graphs to find the actual owner.
            </p>
            <div className="grid gap-2 md:grid-cols-2">
              <BriefFact
                label="Data confidence"
                value={`${formatPercent(stubRatio)} stubs`}
                tone={stubRatio > 0.35 ? "warn" : "neutral"}
                detail="High stub coverage can hide referents; use graph/detail views before drawing conclusions."
              />
              <BriefFact
                label="Largest cohort signal"
                value={topReachableCohort ? topReachableCohort.cohort : "No cohort"}
                detail={topReachableCohort ? `${formatBytes(topReachableCohort.estimated_reachable_size_sum ?? 0)} estimated reachable across ${formatNumber(topReachableCohort.count)} objects.` : "No cohort facts are available."}
              />
              <BriefFact
                label="Runtime metadata"
                value={formatBytes(runtimeReachable)}
                detail="Functions, dicts, cells, modules, and descriptors often point at registries or loaded frameworks."
              />
              <BriefFact
                label="Application hotspot"
                value={topApplicationType ? topApplicationType.type : "No app type"}
                detail={topApplicationType ? `${topApplicationType.module} · ${formatBytes(topApplicationType.estimated_reachable_size_sum ?? 0)} estimated reachable.` : "No non-builtin type appears in the current top slice."}
              />
            </div>
          </div>
          <div className="rounded-md border border-border bg-muted/25 p-3">
            <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Recommended probes</div>
            <div className="space-y-2">
              {probes.map((probe) => (
                <div key={probe.title} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border pb-2 last:border-b-0 last:pb-0">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">{probe.title}</div>
                    <div className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">{probe.evidence}</div>
                  </div>
                  <Button variant="secondary" size="sm" onClick={probe.onClick}>
                    {probe.action}
                  </Button>
                </div>
              ))}
            </div>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <InsightCard
          icon={ShieldQuestion}
          title="Data Confidence"
          value={`${formatPercent(stubRatio)} stubs`}
          detail={`${formatNumber(summary.missing_stub_summary.stub_count)} stub objects; ${formatNumber(summary.missing_stub_summary.missing_referent_count)} missing referents.`}
          tone={stubRatio > 0.35 ? "warn" : "neutral"}
          action="Open findings"
          onAction={onOpenFindings}
        />
        <InsightCard
          icon={Gauge}
          title="Reachability Amplification"
          value={topAmplifiedCohort ? `${topAmplifiedCohort.cohort} · ${formatRatio(topAmplifiedCohort)}` : cohortsLoading ? "Loading..." : "No signal"}
          detail={topAmplifiedCohort ? `${formatBytes(topAmplifiedCohort.shallow_size_sum)} shallow expands to ${formatBytes(topAmplifiedCohort.estimated_reachable_size_sum ?? 0)}.` : "Cohort amplification requires reachable and shallow totals."}
          action="View cohort"
          onAction={topAmplifiedCohort ? () => onViewCohort(topAmplifiedCohort.cohort) : onOpenCohorts}
        />
        <InsightCard
          icon={Database}
          title="Resource Cohorts"
          value={topReachableCohort ? `${topReachableCohort.cohort}` : cohortsLoading ? "Loading..." : "No cohorts"}
          detail={topReachableCohort ? `${formatBytes(topReachableCohort.estimated_reachable_size_sum ?? 0)} estimated reachable; ${formatNumber(topReachableCohort.type_count)} contributing types.` : "No cohort facts are available for this snapshot."}
          action="Open cohorts"
          onAction={onOpenCohorts}
        />
        <InsightCard
          icon={Search}
          title="Application Hotspot"
          value={topApplicationType ? topApplicationType.type : typesLoading ? "Loading..." : "No app type"}
          detail={topApplicationType ? `${topApplicationType.module} · ${formatNumber(topApplicationType.count)} objects · ${formatBytes(topApplicationType.estimated_reachable_size_sum ?? 0)} reachable.` : "No non-builtin hotspot in the current top slice."}
          action="Inspect"
          onAction={topApplicationType ? () => onViewType(topApplicationType) : undefined}
        />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Resource Cohort Signals</CardTitle>
          <Button variant="secondary" size="sm" onClick={onOpenCohorts}>
            Open cohorts
            <ArrowRight size={14} />
          </Button>
        </CardHeader>
        <CardContent>
          {cohorts.length ? (
            <div className="grid gap-2 xl:grid-cols-2">
              {cohorts.slice(0, 6).map((cohort) => (
                <button
                  key={cohort.cohort}
                  type="button"
                  className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 rounded-md border border-border bg-background px-3 py-2 text-left transition-colors hover:border-primary/50 hover:bg-muted/35 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onClick={() => onViewCohort(cohort.cohort)}
                >
                  <span className="min-w-0">
                    <span className="block truncate text-sm font-medium">{cohort.cohort}</span>
                    <span className="mt-0.5 block truncate text-xs text-muted-foreground">
                      {formatNumber(cohort.count)} objects · {formatNumber(cohort.type_count)} types · {formatRatio(cohort)} amplification
                    </span>
                  </span>
                  <span className="text-right text-sm tabular-nums">
                    <span className="block font-medium">{formatBytes(cohort.estimated_reachable_size_sum ?? 0)}</span>
                    <span className="text-xs text-muted-foreground">{formatBytes(cohort.shallow_size_sum)} shallow</span>
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div className="rounded-md border border-dashed border-border p-4 text-sm text-muted-foreground">{cohortsLoading ? "Loading cohort signals..." : "No cohort signals available."}</div>
          )}
        </CardContent>
      </Card>
    </>
  );
}

function BriefFact({ label, value, detail, tone = "neutral" }: { label: string; value: string; detail: string; tone?: "neutral" | "warn" }) {
  return (
    <div className="rounded-md border border-border bg-background px-3 py-2">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</span>
        {tone === "warn" ? <AlertTriangle className="text-amber-600" size={13} /> : null}
      </div>
      <div className="mt-1 truncate text-sm font-semibold">{value}</div>
      <div className="mt-1 line-clamp-2 text-xs leading-5 text-muted-foreground">{detail}</div>
    </div>
  );
}

function InsightCard({
  icon: Icon,
  title,
  value,
  detail,
  tone = "neutral",
  action,
  onAction
}: {
  icon: typeof ShieldQuestion;
  title: string;
  value: string;
  detail: string;
  tone?: "neutral" | "warn";
  action?: string;
  onAction?: () => void;
}) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-sm text-muted-foreground">{title}</div>
            <div className="mt-2 truncate text-xl font-semibold tracking-normal text-foreground" title={value}>
              {value}
            </div>
          </div>
          <span className={cn("rounded-md p-2", tone === "warn" ? "bg-amber-100 text-amber-700" : "bg-muted text-muted-foreground")}>
            <Icon size={18} />
          </span>
        </div>
        <div className="mt-2 min-h-10 text-xs leading-5 text-muted-foreground">{detail}</div>
        {action && onAction ? (
          <Button className="mt-3 w-full" variant="secondary" size="sm" onClick={onAction}>
            {action}
          </Button>
        ) : null}
      </CardContent>
    </Card>
  );
}

function recommendedProbes({
  stubRatio,
  topAmplifiedCohort,
  topReachableCohort,
  topRuntimeType,
  topApplicationType,
  onViewType,
  onViewCohort,
  onOpenFindings
}: {
  stubRatio: number;
  topAmplifiedCohort?: CohortRow;
  topReachableCohort?: CohortRow;
  topRuntimeType?: StatRow;
  topApplicationType?: StatRow;
  onViewType: (row: StatRow) => void;
  onViewCohort: (cohort: string) => void;
  onOpenFindings: () => void;
}) {
  const probes: Probe[] = [];
  if (topAmplifiedCohort) {
    probes.push({
      title: `${topAmplifiedCohort.cohort} amplification`,
      evidence: `${formatBytes(topAmplifiedCohort.shallow_size_sum)} shallow expands to ${formatBytes(topAmplifiedCohort.estimated_reachable_size_sum ?? 0)} estimated reachable.`,
      action: "Inspect",
      onClick: () => onViewCohort(topAmplifiedCohort.cohort)
    });
  } else if (topReachableCohort) {
    probes.push({
      title: `${topReachableCohort.cohort} cohort`,
      evidence: `${formatBytes(topReachableCohort.estimated_reachable_size_sum ?? 0)} estimated reachable across ${formatNumber(topReachableCohort.count)} objects.`,
      action: "Inspect",
      onClick: () => onViewCohort(topReachableCohort.cohort)
    });
  }
  if (topApplicationType) {
    probes.push({
      title: `${topApplicationType.type} hotspot`,
      evidence: `${topApplicationType.module} contributes ${formatBytes(topApplicationType.estimated_reachable_size_sum ?? 0)} estimated reachable.`,
      action: "Objects",
      onClick: () => onViewType(topApplicationType)
    });
  }
  if (topRuntimeType) {
    probes.push({
      title: `${topRuntimeType.type} ownership roots`,
      evidence: `${formatBytes(topRuntimeType.estimated_reachable_size_sum ?? 0)} estimated reachable in Python runtime metadata. Check referrers before blaming the builtin type itself.`,
      action: "Objects",
      onClick: () => onViewType(topRuntimeType)
    });
  }
  if (stubRatio > 0.25) {
    probes.push({
      title: "High stub coverage",
      evidence: `${formatPercent(stubRatio)} of objects are stubs. Confirm dump settings and treat reachable totals as directional leads.`,
      action: "Findings",
      onClick: onOpenFindings
    });
  }
  return probes.slice(0, 4);
}

function mostAmplifiedCohort(cohorts: CohortRow[]) {
  return [...cohorts]
    .filter((cohort) => cohort.shallow_size_sum > 0 && (cohort.estimated_reachable_size_sum ?? 0) > 0)
    .sort((left, right) => amplification(right) - amplification(left))[0];
}

function amplification(cohort: CohortRow) {
  return safeRatio(cohort.estimated_reachable_size_sum ?? 0, cohort.shallow_size_sum);
}

function formatRatio(cohort: CohortRow) {
  const ratio = amplification(cohort);
  if (!Number.isFinite(ratio) || ratio <= 0) return "-";
  if (ratio < 10) return `${ratio.toFixed(1)}x`;
  return `${Math.round(ratio).toLocaleString()}x`;
}

function safeRatio(numerator: number, denominator: number) {
  if (!denominator) return 0;
  return numerator / denominator;
}

function isRuntimeType(row: StatRow) {
  return row.module === "builtins" && RUNTIME_TYPES.has(row.type);
}

function isApplicationType(row: StatRow) {
  return row.module !== "builtins" && !row.module.startsWith("_frozen_importlib");
}
