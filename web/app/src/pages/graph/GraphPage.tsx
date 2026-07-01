import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import cytoscape, { type Core, type ElementDefinition, type LayoutOptions, type StylesheetJson } from "cytoscape";
import fcose from "cytoscape-fcose";
import { Download, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { ErrorState } from "@/components/shared/states";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient, type ObjectRow } from "@/generated/api-client";
import { exportJson } from "@/lib/export";
import { formatBytes, formatNumber } from "@/lib/format";
import { cn } from "@/lib/utils";
import { valueOrUndefined, type UpdateSearch } from "@/lib/search";

type GraphPageProps = {
  snapshotId?: number;
  root: string;
  depth: number;
  nodeLimit: number;
  direction: string;
  updateSearch: UpdateSearch;
};

type GraphLabelMode = "focus" | "important" | "all";
type GraphLayoutMode = "force" | "radial";

type GraphSettings = {
  layoutMode: GraphLayoutMode;
  labelMode: GraphLabelMode;
  nodeScale: number;
  linkDistance: number;
  repel: number;
  gravity: number;
  linkWidth: number;
  showArrows: boolean;
  animate: boolean;
};

type GraphEdge = { from_id: string; to_id: string };

const DEFAULT_GRAPH_SETTINGS: GraphSettings = {
  layoutMode: "force",
  labelMode: "focus",
  nodeScale: 0.76,
  linkDistance: 220,
  repel: 16500,
  gravity: 0.08,
  linkWidth: 0.62,
  showArrows: false,
  animate: true
};

const GRAPH_SETTINGS_STORAGE_KEY = "pygco.graph.settings.v1";

const GRAPH_COLORS = ["#8b5cf6", "#14b8a6", "#f97316", "#06b6d4", "#84cc16", "#e879f9", "#f43f5e", "#eab308"];

const cytoscapeGlobal = globalThis as typeof globalThis & { __pygcoFcoseRegistered?: boolean };
if (!cytoscapeGlobal.__pygcoFcoseRegistered) {
  cytoscape.use(fcose);
  cytoscapeGlobal.__pygcoFcoseRegistered = true;
}

export function GraphPage({ snapshotId, root, depth, nodeLimit, direction, updateSearch }: GraphPageProps) {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [settings, setSettings] = useState<GraphSettings>(loadGraphSettings);
  const [layoutNonce, setLayoutNonce] = useState(0);
  const graph = useQuery({
    queryKey: ["graph", snapshotId, root, depth, nodeLimit, direction],
    queryFn: () => apiClient.graph({ snapshot_id: snapshotId, root_object_id: root, direction, depth, node_limit: nodeLimit, edge_limit: 2000 }),
    enabled: Boolean(snapshotId && root)
  });
  const graphNodes = useMemo(() => withMissingNodes(graph.data?.nodes ?? [], graph.data?.missing_edges ?? []), [graph.data?.nodes, graph.data?.missing_edges]);
  const selectedNode = graphNodes.find((node) => node.object_id === selectedNodeId) ?? graphNodes.find((node) => node.object_id === root);
  const totalEdges = (graph.data?.edges.length ?? 0) + (graph.data?.missing_edges.length ?? 0);

  useEffect(() => {
    saveGraphSettings(settings);
  }, [settings]);

  return (
    <Page className="gap-3">
      <PageTitle
        title="Object Graph"
        meta="Local reference graph"
        actions={graph.data?.truncated ? <Badge tone="warn">truncated</Badge> : null}
      />
      <div className="relative min-h-[720px] overflow-hidden rounded-lg border border-slate-800 bg-[#111318] shadow-sm">
        <GraphCanvas
          nodes={graphNodes}
          edges={graph.data?.edges ?? []}
          missingEdges={graph.data?.missing_edges ?? []}
          root={root}
          selectedNodeId={selectedNode?.object_id}
          settings={settings}
          layoutNonce={layoutNonce}
          onSelect={setSelectedNodeId}
        />
        <div className="pointer-events-none absolute inset-x-3 top-3 z-10 flex flex-col items-start gap-3 xl:inset-x-4 xl:top-4 xl:flex-row xl:justify-between xl:gap-4">
          <GraphStatus nodeCount={graphNodes.length} edgeCount={totalEdges} selectedNode={selectedNode} />
          <GraphControls
            root={root}
            depth={depth}
            nodeLimit={nodeLimit}
            direction={direction}
            settings={settings}
            onSettingsChange={(patch) => setSettings((current) => ({ ...current, ...patch }))}
            onRelayout={() => setLayoutNonce((value) => value + 1)}
            onExport={() => exportJson(graph.data, "pygco-subgraph.json")}
            updateSearch={updateSearch}
          />
        </div>
        <GraphLegend />
        {selectedNode ? <GraphNodeCard root={root} selectedNode={selectedNode} updateSearch={updateSearch} /> : null}
      </div>
      {graph.error ? <ErrorState error={graph.error} /> : null}
    </Page>
  );
}

function GraphCanvas({
  nodes,
  edges,
  missingEdges,
  root,
  selectedNodeId,
  settings,
  layoutNonce,
  onSelect
}: {
  nodes: ObjectRow[];
  edges: GraphEdge[];
  missingEdges: GraphEdge[];
  root: string;
  selectedNodeId?: string;
  settings: GraphSettings;
  layoutNonce: number;
  onSelect: (id: string) => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const elements = useMemo(() => graphElements(nodes, edges, missingEdges, root, settings), [nodes, edges, missingEdges, root, settings]);

  useEffect(() => {
    if (!containerRef.current) return;
    const cy = cytoscape({
      container: containerRef.current,
      elements,
      minZoom: 0.08,
      maxZoom: 4,
      wheelSensitivity: 0.16,
      autoungrabify: false,
      style: graphStyles(settings)
    });
    cyRef.current = cy;

    cy.on("tap", "node", (event) => onSelect(String(event.target.id())));
    cy.on("mouseover", "node", (event) => event.target.addClass("hover"));
    cy.on("mouseout", "node", (event) => event.target.removeClass("hover"));
    runLayout(cy, root, settings);

    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, [elements, layoutNonce, onSelect, root, settings]);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !selectedNodeId) return;
    cy.nodes().unselect();
    const selected = cy.$id(selectedNodeId);
    if (selected.length > 0) selected.select();
  }, [selectedNodeId]);

  return <div ref={containerRef} className="h-full min-h-[720px] w-full" />;
}

function GraphStatus({ nodeCount, edgeCount, selectedNode }: { nodeCount: number; edgeCount: number; selectedNode?: ObjectRow }) {
  return (
    <div className="pointer-events-auto max-w-full rounded-md border border-white/10 bg-slate-950/80 px-3 py-2 text-slate-100 shadow-xl backdrop-blur">
      <div className="flex flex-wrap items-center gap-2 text-xs text-slate-400">
        <span>{formatNumber(nodeCount)} nodes</span>
        <span className="h-1 w-1 rounded-full bg-slate-600" />
        <span>{formatNumber(edgeCount)} edges</span>
      </div>
      {selectedNode ? (
        <div className="mt-1 max-w-[420px] truncate text-sm font-medium text-slate-100">
          {nodeLabel(selectedNode)}
        </div>
      ) : null}
    </div>
  );
}

function GraphControls({
  root,
  depth,
  nodeLimit,
  direction,
  settings,
  onSettingsChange,
  onRelayout,
  onExport,
  updateSearch
}: {
  root: string;
  depth: number;
  nodeLimit: number;
  direction: string;
  settings: GraphSettings;
  onSettingsChange: (patch: Partial<GraphSettings>) => void;
  onRelayout: () => void;
  onExport: () => void;
  updateSearch: UpdateSearch;
}) {
  return (
    <aside className="pointer-events-auto max-h-[calc(100vh-220px)] w-full overflow-auto rounded-lg border border-white/10 bg-slate-950/88 p-3 text-slate-100 shadow-2xl backdrop-blur sm:w-[320px]">
      <div className="mb-3 flex items-center justify-between">
        <div className="text-xs font-semibold uppercase tracking-wide text-slate-400">Graph controls</div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-8 w-8 border-white/10 text-slate-200 hover:bg-white/10" title="Run layout again" onClick={onRelayout}>
            <RefreshCw size={14} />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8 border-white/10 text-slate-200 hover:bg-white/10" title="Export graph JSON" onClick={onExport}>
            <Download size={14} />
          </Button>
        </div>
      </div>

      <div className="space-y-3">
        <div className="space-y-2">
          <PanelLabel label="Root object" />
          <Input
            className="h-8 border-slate-700 bg-slate-900 font-mono text-xs text-slate-100"
            value={root}
            onChange={(event) => updateSearch({ root: valueOrUndefined(event.target.value) })}
          />
          <div className="grid grid-cols-3 gap-2">
            <Field label="Direction">
              <Select
                className="h-8 border-slate-700 bg-slate-900 text-xs text-slate-100"
                value={direction}
                onChange={(event) => updateSearch({ graphDirection: event.target.value })}
              >
                <option value="both">Both</option>
                <option value="referents">Referents</option>
                <option value="referrers">Referrers</option>
              </Select>
            </Field>
            <Field label="Depth">
              <Input
                className="h-8 border-slate-700 bg-slate-900 text-xs text-slate-100"
                type="number"
                min="0"
                max="10"
                value={depth}
                onChange={(event) => updateSearch({ graphDepth: Number(event.target.value) })}
              />
            </Field>
            <Field label="Limit">
              <Input
                className="h-8 border-slate-700 bg-slate-900 text-xs text-slate-100"
                type="number"
                min="1"
                max="5000"
                value={nodeLimit}
                onChange={(event) => updateSearch({ graphLimit: Number(event.target.value) })}
              />
            </Field>
          </div>
        </div>

        <PanelSection title="Display">
          <Field label="Layout">
            <Select
              className="h-8 border-slate-700 bg-slate-900 text-xs text-slate-100"
              value={settings.layoutMode}
              onChange={(event) => onSettingsChange({ layoutMode: event.target.value as GraphLayoutMode })}
            >
              <option value="force">Force</option>
              <option value="radial">Radial</option>
            </Select>
          </Field>
          <Field label="Labels">
            <Select
              className="h-8 border-slate-700 bg-slate-900 text-xs text-slate-100"
              value={settings.labelMode}
              onChange={(event) => onSettingsChange({ labelMode: event.target.value as GraphLabelMode })}
            >
              <option value="focus">Focus</option>
              <option value="important">Important</option>
              <option value="all">All</option>
            </Select>
          </Field>
          <RangeField label="Node size" min={0.5} max={1.7} step={0.05} value={settings.nodeScale} onChange={(nodeScale) => onSettingsChange({ nodeScale })} />
          <RangeField label="Link width" min={0.35} max={2.2} step={0.05} value={settings.linkWidth} onChange={(linkWidth) => onSettingsChange({ linkWidth })} />
          <ToggleField label="Arrows" checked={settings.showArrows} onChange={(showArrows) => onSettingsChange({ showArrows })} />
        </PanelSection>

        <PanelSection title="Forces">
          <RangeField label="Repel" min={3000} max={24000} step={500} value={settings.repel} onChange={(repel) => onSettingsChange({ repel })} />
          <RangeField label="Link distance" min={60} max={300} step={5} value={settings.linkDistance} onChange={(linkDistance) => onSettingsChange({ linkDistance })} />
          <RangeField label="Center gravity" min={0.03} max={0.8} step={0.01} value={settings.gravity} onChange={(gravity) => onSettingsChange({ gravity })} />
          <ToggleField label="Animate" checked={settings.animate} onChange={(animate) => onSettingsChange({ animate })} />
        </PanelSection>
      </div>
    </aside>
  );
}

function PanelSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="border-t border-white/10 pt-3">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400">{title}</div>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

function PanelLabel({ label }: { label: string }) {
  return <div className="text-[11px] font-medium uppercase tracking-wide text-slate-500">{label}</div>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <PanelLabel label={label} />
      {children}
    </label>
  );
}

function RangeField({ label, min, max, step, value, onChange }: { label: string; min: number; max: number; step: number; value: number; onChange: (value: number) => void }) {
  const updateValue = (event: { currentTarget: HTMLInputElement }) => onChange(Number(event.currentTarget.value));
  return (
    <label className="grid grid-cols-[92px_minmax(0,1fr)_52px] items-center gap-2 text-xs text-slate-300">
      <span>{label}</span>
      <input
        className="h-1.5 accent-violet-400"
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onInput={updateValue}
        onChange={updateValue}
      />
      <span className="text-right font-mono text-[11px] text-slate-500">{formatSliderValue(value)}</span>
    </label>
  );
}

function ToggleField({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="flex items-center justify-between gap-3 text-xs text-slate-300">
      <span>{label}</span>
      <input className="h-4 w-4 accent-violet-400" type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

function GraphLegend() {
  return (
    <div className="pointer-events-none absolute bottom-4 left-4 z-10 flex max-w-[calc(100%-2rem)] flex-wrap items-center gap-4 rounded-md border border-white/10 bg-slate-950/78 px-3 py-2 text-xs text-slate-300 shadow-xl backdrop-blur">
      <LegendDot className="bg-violet-400 ring-violet-300/70" label="root" />
      <LegendDot className="bg-slate-300 ring-slate-400/50" label="object" />
      <LegendDot className="bg-amber-300 ring-amber-300/60" label="stub" />
      <LegendDot className="bg-red-300 ring-red-300/60" label="missing" />
      <span className="h-px w-8 bg-slate-500" />
      <span>reference</span>
    </div>
  );
}

function LegendDot({ className, label }: { className: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-2">
      <span className={cn("h-2.5 w-2.5 rounded-full ring-2", className)} />
      {label}
    </span>
  );
}

function GraphNodeCard({ root, selectedNode, updateSearch }: { root: string; selectedNode: ObjectRow; updateSearch: UpdateSearch }) {
  const isRoot = selectedNode.object_id === root;
  return (
    <div className="pointer-events-auto absolute bottom-16 left-4 right-4 z-10 rounded-lg border border-white/10 bg-slate-950/86 p-3 text-slate-100 shadow-2xl backdrop-blur sm:bottom-4 sm:left-auto sm:w-[360px]">
      <div className="min-w-0">
        <div className="truncate text-sm font-semibold">{nodeLabel(selectedNode)}</div>
        <button
          className={cn(
            "mt-1 block max-w-full break-all rounded-sm text-left font-mono text-[11px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-400",
            isRoot
              ? "cursor-default text-slate-500"
              : "text-violet-300 underline decoration-violet-400/50 underline-offset-2 hover:text-violet-100"
          )}
          disabled={isRoot}
          title={isRoot ? "Current root object" : "Set as root object"}
          onClick={() => updateSearch({ root: selectedNode.object_id, selected: undefined }, { history: "push" })}
        >
          {selectedNode.object_id}
        </button>
        <div className="mt-1 truncate text-xs text-slate-400">{selectedNode.module}</div>
      </div>
      <div className="mt-3 grid grid-cols-4 gap-2">
        <Metric label="shallow" value={formatBytes(selectedNode.shallow_size)} />
        <Metric label="reachable" value={formatBytes(selectedNode.estimated_reachable_size)} />
        <Metric label="in" value={formatNumber(selectedNode.in_edges)} />
        <Metric label="out" value={formatNumber(selectedNode.out_edges)} />
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md bg-white/[0.04] px-2 py-1.5">
      <div className="truncate text-[10px] uppercase tracking-wide text-slate-500">{label}</div>
      <div className="truncate text-xs font-semibold tabular-nums text-slate-100">{value}</div>
    </div>
  );
}

function graphElements(nodes: ObjectRow[], edges: GraphEdge[], missingEdges: GraphEdge[], root: string, settings: GraphSettings): ElementDefinition[] {
  const depthById = graphDepths(root, edges, missingEdges);
  const maxLogSize = Math.max(1, ...nodes.map((node) => Math.log10(node.estimated_reachable_size + 1)));
  const importantIds = importantNodeIds(nodes, root);
  return [
    ...nodes.map((node) => {
      const isRoot = node.object_id === root;
      const isMissing = node.type === "<missing>";
      const visibleLabel = labelForMode(node, root, importantIds, settings.labelMode);
      const sizeScore = Math.log10(node.estimated_reachable_size + 1) / maxLogSize;
      const nodeSize = Math.round((7 + Math.sqrt(sizeScore) * 18 + (isRoot ? 8 : 0)) * settings.nodeScale);
      return {
        data: {
          id: node.object_id,
          label: nodeLabel(node),
          visibleLabel,
          nodeSize,
          depth: depthById.get(node.object_id) ?? 999,
          color: isRoot ? "#a78bfa" : nodeColor(node),
          borderColor: isRoot ? "#ddd6fe" : isMissing ? "#fca5a5" : node.stub ? "#fbbf24" : "#cbd5e1"
        },
        classes: [isRoot ? "root" : "", node.stub ? "stub" : "", isMissing ? "missing" : ""].filter(Boolean).join(" ")
      };
    }),
    ...edges.map((edge, index) => ({ data: { id: `edge-${index}`, source: edge.from_id, target: edge.to_id } })),
    ...missingEdges.map((edge, index) => ({ data: { id: `missing-${index}`, source: edge.from_id, target: edge.to_id }, classes: "missing" }))
  ];
}

function graphStyles(settings: GraphSettings): StylesheetJson {
  return [
    {
      selector: "node",
      style: {
        "background-color": "data(color)",
        "border-color": "data(borderColor)",
        "border-opacity": 0.65,
        "border-width": 1.4,
        color: "#e5e7eb",
        label: "data(visibleLabel)",
        "font-size": "8px",
        "min-zoomed-font-size": 8,
        "text-background-color": "#111318",
        "text-background-opacity": 0.72,
        "text-background-padding": "2px",
        "text-margin-y": -6,
        height: "data(nodeSize)",
        opacity: 0.9,
        width: "data(nodeSize)"
      }
    },
    { selector: "node.root", style: { "border-width": 3, "border-opacity": 1 } },
    { selector: "node.stub", style: { "background-color": "#fbbf24", "border-color": "#fde68a" } },
    { selector: "node.missing", style: { "background-color": "#fecaca", "border-color": "#f87171", "border-style": "dashed" } },
    { selector: "node:selected", style: { label: "data(label)", "border-color": "#f8fafc", "border-width": 4, "z-index": 30 } },
    { selector: "node.hover", style: { label: "data(label)", "border-color": "#f8fafc", "border-width": 3, "z-index": 25 } },
    {
      selector: "edge",
      style: {
        width: settings.linkWidth,
        "line-color": "#64748b",
        "target-arrow-color": "#94a3b8",
        "target-arrow-shape": settings.showArrows ? "triangle" : "none",
        "curve-style": "straight",
        opacity: 0.18
      }
    },
    { selector: "edge:selected", style: { opacity: 0.9, width: Math.max(settings.linkWidth * 1.8, 1.4), "line-color": "#c4b5fd" } },
    { selector: "edge.missing", style: { "line-color": "#f97316", "line-style": "dashed", "target-arrow-color": "#fb923c", opacity: 0.55 } }
  ];
}

function runLayout(cy: Core, root: string, settings: GraphSettings) {
  if (settings.layoutMode === "radial") {
    cy.layout({
      name: "concentric",
      fit: true,
      padding: 72,
      animate: settings.animate,
      animationDuration: 320,
      minNodeSpacing: 42,
      concentric: (node) => Math.max(1, 20 - Number(node.data("depth") ?? 20)),
      levelWidth: () => 1
    }).run();
    return;
  }

  cy.layout({
    name: "fcose",
    quality: "default",
    randomize: false,
    animate: settings.animate,
    animationDuration: 450,
    fit: true,
    padding: 58,
    packComponents: true,
    nodeDimensionsIncludeLabels: false,
    nodeRepulsion: () => settings.repel,
    idealEdgeLength: () => settings.linkDistance,
    edgeElasticity: () => 0.42,
    gravity: settings.gravity,
    gravityRange: 3.6,
    numIter: 2600,
    tile: true,
    tilingPaddingHorizontal: 42,
    tilingPaddingVertical: 42
  } as unknown as LayoutOptions).run();

  const rootNode = cy.$id(root);
  if (rootNode.length > 0) rootNode.select();
}

function graphDepths(root: string, edges: GraphEdge[], missingEdges: GraphEdge[]) {
  const adjacency = new Map<string, Set<string>>();
  for (const edge of [...edges, ...missingEdges]) {
    addNeighbor(adjacency, edge.from_id, edge.to_id);
    addNeighbor(adjacency, edge.to_id, edge.from_id);
  }
  const depth = new Map<string, number>([[root, 0]]);
  const queue = [root];
  for (let index = 0; index < queue.length; index += 1) {
    const current = queue[index];
    const nextDepth = (depth.get(current) ?? 0) + 1;
    for (const next of adjacency.get(current) ?? []) {
      if (depth.has(next)) continue;
      depth.set(next, nextDepth);
      queue.push(next);
    }
  }
  return depth;
}

function addNeighbor(adjacency: Map<string, Set<string>>, from: string, to: string) {
  const neighbors = adjacency.get(from) ?? new Set<string>();
  neighbors.add(to);
  adjacency.set(from, neighbors);
}

function importantNodeIds(nodes: ObjectRow[], root: string) {
  return new Set(
    [...nodes]
      .sort((left, right) => nodeImportance(right, root) - nodeImportance(left, root))
      .slice(0, 14)
      .map((node) => node.object_id)
  );
}

function nodeImportance(node: ObjectRow, root: string) {
  if (node.object_id === root) return Number.MAX_SAFE_INTEGER;
  return node.in_edges + node.out_edges + Math.log10(node.estimated_reachable_size + 1);
}

function labelForMode(node: ObjectRow, root: string, importantIds: Set<string>, mode: GraphLabelMode) {
  if (mode === "all") return nodeLabel(node);
  if (node.object_id === root) return nodeLabel(node);
  if (mode === "important" && (importantIds.has(node.object_id) || node.stub || node.type === "<missing>")) return nodeLabel(node);
  return "";
}

function nodeColor(node: ObjectRow) {
  if (node.type === "<missing>") return "#fecaca";
  if (node.stub) return "#fbbf24";
  if (node.module === "builtins") return "#94a3b8";
  const key = node.module || node.type;
  let hash = 0;
  for (let index = 0; index < key.length; index += 1) hash = (hash * 31 + key.charCodeAt(index)) >>> 0;
  return GRAPH_COLORS[hash % GRAPH_COLORS.length];
}

function withMissingNodes(nodes: ObjectRow[], missingEdges: GraphEdge[]): ObjectRow[] {
  const known = new Set(nodes.map((node) => node.object_id));
  const missing = missingEdges
    .map((edge) => edge.to_id)
    .filter((id, index, ids) => !known.has(id) && ids.indexOf(id) === index)
    .map((id) => ({
      object_id: id,
      type: "<missing>",
      module: "<missing>",
      shallow_size: 0,
      estimated_reachable_size: 0,
      reachable_truncated: 0,
      in_edges: 0,
      out_edges: 0,
      stub: 1,
      missing_referents: 0
    }));
  return [...nodes, ...missing];
}

function nodeLabel(node: ObjectRow) {
  if (node.type === "<missing>") return "missing";
  if (node.module === "builtins") return node.type;
  const modulePrefix = node.module.split(".").slice(0, 2).join(".");
  return `${modulePrefix}.${node.type}`;
}

function formatSliderValue(value: number) {
  return Number.isInteger(value) ? formatNumber(value) : value.toFixed(2);
}

function loadGraphSettings(): GraphSettings {
  if (typeof window === "undefined") return DEFAULT_GRAPH_SETTINGS;
  try {
    const raw = window.localStorage.getItem(GRAPH_SETTINGS_STORAGE_KEY);
    if (!raw) return DEFAULT_GRAPH_SETTINGS;
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed)) return DEFAULT_GRAPH_SETTINGS;
    const settings = isRecord(parsed.settings) ? parsed.settings : parsed;
    return normalizeGraphSettings(settings);
  } catch {
    return DEFAULT_GRAPH_SETTINGS;
  }
}

function saveGraphSettings(settings: GraphSettings) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(GRAPH_SETTINGS_STORAGE_KEY, JSON.stringify({ settings }));
  } catch {
    // Ignore storage quota/private mode failures; graph controls still work for the current session.
  }
}

function normalizeGraphSettings(value: Record<string, unknown>): GraphSettings {
  return {
    layoutMode: value.layoutMode === "radial" || value.layoutMode === "force" ? value.layoutMode : DEFAULT_GRAPH_SETTINGS.layoutMode,
    labelMode: value.labelMode === "focus" || value.labelMode === "important" || value.labelMode === "all" ? value.labelMode : DEFAULT_GRAPH_SETTINGS.labelMode,
    nodeScale: numberInRange(value.nodeScale, 0.5, 1.7, DEFAULT_GRAPH_SETTINGS.nodeScale),
    linkDistance: numberInRange(value.linkDistance, 60, 300, DEFAULT_GRAPH_SETTINGS.linkDistance),
    repel: numberInRange(value.repel, 3000, 24000, DEFAULT_GRAPH_SETTINGS.repel),
    gravity: numberInRange(value.gravity, 0.03, 0.8, DEFAULT_GRAPH_SETTINGS.gravity),
    linkWidth: numberInRange(value.linkWidth, 0.35, 2.2, DEFAULT_GRAPH_SETTINGS.linkWidth),
    showArrows: typeof value.showArrows === "boolean" ? value.showArrows : DEFAULT_GRAPH_SETTINGS.showArrows,
    animate: typeof value.animate === "boolean" ? value.animate : DEFAULT_GRAPH_SETTINGS.animate
  };
}

function numberInRange(value: unknown, min: number, max: number, fallback: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
