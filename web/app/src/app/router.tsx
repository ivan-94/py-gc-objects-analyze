import { lazy, Suspense } from "react";
import { useQuery } from "@tanstack/react-query";
import { createRootRoute, createRoute, createRouter, Outlet, parseSearchWith, stringifySearchWith } from "@tanstack/react-router";
import { AlertTriangle, BarChart3, Braces, Database, FileText, GitCompare, Layers3, Network, Package, Search, Table2, Tags } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { apiClient } from "@/generated/api-client";
import { CohortsPage, ModulesPage, TypesPage } from "@/pages/aggregates/AggregatePages";
import { DiffPage } from "@/pages/DiffPage";
import { FindingsPage } from "@/pages/FindingsPage";
import { ObjectsPage } from "@/pages/objects/ObjectsPage";
import { OverviewPage } from "@/pages/OverviewPage";
import { ReportPage } from "@/pages/ReportPage";
import { SqlPage } from "@/pages/sql/SqlPage";
import { normalizeSearch, pages, parseSearchNumber, parseSearchString, type AppSearch, type PageId, type UpdateSearch } from "@/lib/search";
import { cn } from "@/lib/utils";

const GraphPage = lazy(() => import("@/pages/graph/GraphPage").then((module) => ({ default: module.GraphPage })));

const rootRoute = createRootRoute({
  component: () => <Outlet />
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  validateSearch: (search: Record<string, unknown>): AppSearch => ({
    page: typeof search.page === "string" && pages.has(search.page as PageId) ? (search.page as PageId) : undefined,
    snapshot: parseSearchNumber(search.snapshot),
    q: parseSearchString(search.q),
    type: parseSearchString(search.type),
    module: parseSearchString(search.module),
    cohort: parseSearchString(search.cohort),
    sort: parseSearchString(search.sort),
    limit: parseSearchNumber(search.limit),
    offset: parseSearchNumber(search.offset),
    selected: parseSearchString(search.selected),
    root: parseSearchString(search.root),
    graphDepth: parseSearchNumber(search.graphDepth),
    graphLimit: parseSearchNumber(search.graphLimit),
    graphDirection: parseSearchString(search.graphDirection),
    from: parseSearchNumber(search.from),
    to: parseSearchNumber(search.to),
    diffState: parseSearchString(search.diffState)
  }),
  component: AppShell
});

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({
  routeTree,
  parseSearch: parseSearchWith((value) => value),
  stringifySearch: stringifySearchWith(JSON.stringify)
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const navItems = [
  ["overview", "Overview", BarChart3],
  ["objects", "Objects", Table2],
  ["types", "Types", Tags],
  ["modules", "Modules", Package],
  ["cohorts", "Cohorts", Layers3],
  ["graph", "Object Graph", Network],
  ["diff", "Diff", GitCompare],
  ["findings", "Findings", AlertTriangle],
  ["sql", "SQL", Database],
  ["report", "Report", FileText]
] as const;

function AppShell() {
  const search = indexRoute.useSearch();
  const navigate = indexRoute.useNavigate();
  const updateSearch: UpdateSearch = (patch, options) => {
    void navigate({
      search: (previous) => normalizeSearch({ ...previous, ...patch }),
      replace: options?.history !== "push"
    });
  };
  const route = search.page ?? "overview";
  const snapshots = useQuery({ queryKey: ["snapshots"], queryFn: () => apiClient.snapshots() });
  const selectedSnapshot = search.snapshot ?? snapshots.data?.rows?.[0]?.snapshot_id;
  const snapshotRows = snapshots.data?.rows ?? [];

  return (
    <div className="grid min-h-dvh grid-cols-1 grid-rows-[auto_auto_1fr] overflow-x-hidden bg-background text-foreground lg:grid-cols-[220px_minmax(0,1fr)] lg:grid-rows-[56px_1fr]">
      <header className="col-span-full flex flex-wrap items-center gap-3 border-b border-border bg-background px-4 py-2">
        <div className="flex w-44 items-center gap-2 font-semibold">
          <Database size={18} />
          pygco
        </div>
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          Snapshot
          <Select className="w-80" value={selectedSnapshot ?? ""} onChange={(event) => updateSearch({ snapshot: Number(event.target.value), selected: undefined }, { history: "push" })}>
            {snapshotRows.map((snapshot) => (
              <option key={snapshot.snapshot_id} value={snapshot.snapshot_id}>
                {snapshot.snapshot_id} · {snapshot.source_basename}
              </option>
            ))}
          </Select>
        </label>
        <div className="flex min-w-56 flex-1 items-center gap-2 rounded-md border border-input bg-background px-2 shadow-sm">
          <Search size={16} className="text-muted-foreground" />
          <Input
            className="h-9 flex-1 border-0 px-0 shadow-none focus-visible:ring-0"
            value={search.q ?? ""}
            onChange={(event) => updateSearch({ q: event.target.value || undefined, selected: undefined, offset: undefined })}
            placeholder={route === "objects" ? "Filter objects" : "Search objects"}
          />
        </div>
      </header>
      <aside className="flex min-w-0 w-full gap-1 overflow-x-auto border-b border-border bg-background p-2 lg:block lg:overflow-visible lg:border-b-0 lg:border-r">
        {navItems.map(([id, label, Icon]) => (
          <Button
            key={id}
            variant={route === id ? "secondary" : "ghost"}
            className={cn("w-auto !justify-start whitespace-nowrap lg:mb-1 lg:w-full lg:px-3", route === id && "text-primary")}
            onClick={() => updateSearch({ page: id }, { history: "push" })}
          >
            <Icon className="shrink-0" size={16} />
            {label}
          </Button>
        ))}
      </aside>
      <main className="min-w-0 p-4 lg:p-5">
        <Suspense fallback={<div className="rounded-lg border border-border bg-background p-5 text-sm text-muted-foreground">Loading page...</div>}>
          {route === "overview" && <OverviewPage snapshotId={selectedSnapshot} updateSearch={updateSearch} />}
          {route === "objects" && (
            <ObjectsPage
              snapshotId={selectedSnapshot}
              q={search.q ?? ""}
              typeName={search.type}
              moduleName={search.module}
              cohort={search.cohort}
              sort={search.sort ?? "reachable-size"}
              limit={search.limit}
              offset={search.offset}
              selected={search.selected}
              updateSearch={updateSearch}
            />
          )}
          {route === "types" && <TypesPage snapshotId={selectedSnapshot} snapshots={snapshotRows} fromSnapshot={search.from} toSnapshot={search.to} sort={search.sort} updateSearch={updateSearch} />}
          {route === "modules" && <ModulesPage snapshotId={selectedSnapshot} snapshots={snapshotRows} fromSnapshot={search.from} toSnapshot={search.to} sort={search.sort} updateSearch={updateSearch} />}
          {route === "cohorts" && <CohortsPage snapshotId={selectedSnapshot} snapshots={snapshotRows} fromSnapshot={search.from} toSnapshot={search.to} sort={search.sort} updateSearch={updateSearch} />}
          {route === "graph" && (
            <GraphPage
              snapshotId={selectedSnapshot}
              root={search.root ?? search.selected ?? ""}
              depth={search.graphDepth ?? 2}
              nodeLimit={search.graphLimit ?? 200}
              direction={search.graphDirection ?? "both"}
              updateSearch={updateSearch}
            />
          )}
          {route === "diff" && <DiffPage snapshots={snapshotRows} fromSnapshot={search.from} toSnapshot={search.to} diffState={search.diffState ?? "new"} updateSearch={updateSearch} />}
          {route === "findings" && <FindingsPage snapshotId={selectedSnapshot} />}
          {route === "sql" && <SqlPage snapshotId={selectedSnapshot} />}
          {route === "report" && <ReportPage snapshotId={selectedSnapshot} />}
        </Suspense>
      </main>
    </div>
  );
}
