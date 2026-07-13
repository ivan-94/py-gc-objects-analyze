import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import cytoscape, { type Core, type ElementDefinition, type LayoutOptions, type StylesheetJson } from "cytoscape";
import fcose from "cytoscape-fcose";
import { Download, PanelRightClose, PanelRightOpen, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { ErrorState } from "@/components/shared/states";
import { Page, PageTitle } from "@/components/shared/page";
import { apiClient, type ObjectRow } from "@/generated/api-client";
import { exportJson } from "@/lib/export";
import { formatBytes, formatNumber } from "@/lib/format";
import { pruneGraphToRoot } from "@/lib/graph-filter";
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
  hiddenLegendKeys: string[];
  controlsCollapsed: boolean;
};

type GraphRenderSettings = Omit<GraphSettings, "hiddenLegendKeys" | "controlsCollapsed">;
type GraphEdge = { from_id: string; to_id: string };
type LegendItem = { key: string; label: string; color?: string; ringColor?: string; count?: number; kind: "node" | "edge"; toggleable?: boolean };
type NodeSemanticKey = "root" | "object" | "stub" | "missing" | "container" | "callable" | "class" | "data" | "module";
type NodeSemantic = { key: NodeSemanticKey; label: string; color: string; ringColor: string; rank: number };

const DEFAULT_GRAPH_SETTINGS: GraphSettings = {
  layoutMode: "force",
  labelMode: "focus",
  nodeScale: 0.76,
  linkDistance: 220,
  repel: 16500,
  gravity: 0.08,
  linkWidth: 0.62,
  showArrows: true,
  animate: true,
  hiddenLegendKeys: [],
  controlsCollapsed: false
};

const GRAPH_SETTINGS_STORAGE_KEY = "pygco.graph.settings.v1";
const GRAPH_SETTINGS_STORAGE_VERSION = 2;

const NODE_SEMANTICS: Record<NodeSemanticKey, NodeSemantic> = {
  root: { key: "root", label: "root", color: "#a78bfa", ringColor: "rgb(196 181 253 / 0.7)", rank: 0 },
  object: { key: "object", label: "object", color: "#94a3b8", ringColor: "rgb(148 163 184 / 0.5)", rank: 1 },
  container: { key: "container", label: "container", color: "#38bdf8", ringColor: "rgb(125 211 252 / 0.55)", rank: 2 },
  callable: { key: "callable", label: "callable/code", color: "#2dd4bf", ringColor: "rgb(94 234 212 / 0.55)", rank: 3 },
  class: { key: "class", label: "class/type", color: "#e879f9", ringColor: "rgb(240 171 252 / 0.55)", rank: 4 },
  data: { key: "data", label: "data/scalar", color: "#84cc16", ringColor: "rgb(163 230 53 / 0.55)", rank: 5 },
  module: { key: "module", label: "module/package", color: "#fb923c", ringColor: "rgb(253 186 116 / 0.55)", rank: 6 },
  stub: { key: "stub", label: "stub", color: "#fbbf24", ringColor: "rgb(252 211 77 / 0.6)", rank: 7 },
  missing: { key: "missing", label: "missing", color: "#fb7185", ringColor: "rgb(248 113 113 / 0.6)", rank: 8 }
};

const CONTAINER_TYPES = new Set(["dict", "list", "tuple", "set", "frozenset", "deque", "defaultdict", "ordereddict", "weakset", "weakkeydictionary", "weakvaluedictionary", "mappingproxy", "chainmap"]);
const DATA_TYPES = new Set(["str", "int", "float", "bool", "bytes", "bytearray", "nonetype", "ellipsis", "slice", "range", "complex"]);

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
  const hiddenLegendKeys = useMemo(() => new Set(settings.hiddenLegendKeys), [settings.hiddenLegendKeys]);
  const renderSettings = useMemo<GraphRenderSettings>(
    () => ({
      layoutMode: settings.layoutMode,
      labelMode: settings.labelMode,
      nodeScale: settings.nodeScale,
      linkDistance: settings.linkDistance,
      repel: settings.repel,
      gravity: settings.gravity,
      linkWidth: settings.linkWidth,
      showArrows: settings.showArrows,
      animate: settings.animate
    }),
    [settings.animate, settings.gravity, settings.labelMode, settings.layoutMode, settings.linkDistance, settings.linkWidth, settings.nodeScale, settings.repel, settings.showArrows]
  );
  const visibleGraph = useMemo(
    () => filterGraphData(graphNodes, graph.data?.edges ?? [], graph.data?.missing_edges ?? [], root, hiddenLegendKeys),
    [graph.data?.edges, graph.data?.missing_edges, graphNodes, hiddenLegendKeys, root]
  );
  const selectedNode = graphNodes.find((node) => node.object_id === selectedNodeId) ?? graphNodes.find((node) => node.object_id === root);
  const totalEdges = visibleGraph.edges.length + visibleGraph.missingEdges.length;

  useEffect(() => {
    saveGraphSettings(settings);
  }, [settings]);

  const toggleLegendKey = (key: string) => {
    if (key === "root") return;
    setSettings((current) => {
      const hidden = new Set(current.hiddenLegendKeys);
      if (hidden.has(key)) hidden.delete(key);
      else hidden.add(key);
      return { ...current, hiddenLegendKeys: [...hidden].sort() };
    });
  };

  return (
    <Page className="gap-3">
      <PageTitle
        title="Object Graph"
        meta="Local reference graph"
        actions={
          graph.data?.truncated || selectedNode ? (
            <div className="flex w-full flex-wrap items-center justify-end gap-3 sm:w-auto sm:flex-nowrap">
              {selectedNode ? <GraphNodeCard root={root} selectedNode={selectedNode} updateSearch={updateSearch} /> : null}
              {graph.data?.truncated ? <Badge className="order-first sm:order-last" tone="warn">truncated</Badge> : null}
            </div>
          ) : null
        }
      />
      <div data-testid="graph-surface" className="relative min-h-[720px] overflow-hidden rounded-lg border border-slate-800 bg-[#111318] shadow-sm">
        <GraphCanvas
          nodes={visibleGraph.nodes}
          edges={visibleGraph.edges}
          missingEdges={visibleGraph.missingEdges}
          root={root}
          selectedNodeId={selectedNode?.object_id}
          settings={renderSettings}
          layoutNonce={layoutNonce}
          onSelect={setSelectedNodeId}
        />
        <div className="pointer-events-none absolute inset-x-3 top-3 z-10 flex flex-col items-start gap-3 xl:inset-x-4 xl:top-4 xl:flex-row xl:justify-between xl:gap-4">
          <GraphStatus nodeCount={visibleGraph.nodes.length} edgeCount={totalEdges} selectedNode={selectedNode} />
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
        <GraphLegend
          items={graphLegendItems(graphNodes, graph.data?.edges.length ?? 0, graph.data?.missing_edges.length ?? 0, root)}
          hiddenKeys={hiddenLegendKeys}
          onToggle={toggleLegendKey}
        />
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
  settings: GraphRenderSettings;
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
    cy.elements().unselect();
    const selected = cy.$id(selectedNodeId);
    if (selected.length > 0) {
      selected.select();
      selected.connectedEdges().select();
    }
  }, [elements, layoutNonce, selectedNodeId]);

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
  const buttonClass = "h-8 w-8 border-white/10 text-slate-200 hover:bg-white/10";

  if (settings.controlsCollapsed) {
    return (
      <aside className="pointer-events-auto flex items-center gap-1 rounded-lg border border-white/10 bg-slate-950/88 p-2 text-slate-100 shadow-2xl backdrop-blur">
        <Button variant="ghost" size="icon" className={buttonClass} title="Expand graph controls" onClick={() => onSettingsChange({ controlsCollapsed: false })}>
          <PanelRightOpen size={14} />
        </Button>
        <Button variant="ghost" size="icon" className={buttonClass} title="Run layout again" onClick={onRelayout}>
          <RefreshCw size={14} />
        </Button>
        <Button variant="ghost" size="icon" className={buttonClass} title="Export graph JSON" onClick={onExport}>
          <Download size={14} />
        </Button>
      </aside>
    );
  }

  return (
    <aside className="pointer-events-auto max-h-[calc(100vh-220px)] w-full overflow-auto rounded-lg border border-white/10 bg-slate-950/88 p-3 text-slate-100 shadow-2xl backdrop-blur sm:w-[320px]">
      <div className="mb-3 flex items-center justify-between">
        <div className="text-xs font-semibold uppercase tracking-wide text-slate-400">Graph controls</div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className={buttonClass} title="Collapse graph controls" onClick={() => onSettingsChange({ controlsCollapsed: true })}>
            <PanelRightClose size={14} />
          </Button>
          <Button variant="ghost" size="icon" className={buttonClass} title="Run layout again" onClick={onRelayout}>
            <RefreshCw size={14} />
          </Button>
          <Button variant="ghost" size="icon" className={buttonClass} title="Export graph JSON" onClick={onExport}>
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

function GraphLegend({ items, hiddenKeys, onToggle }: { items: LegendItem[]; hiddenKeys: Set<string>; onToggle: (key: string) => void }) {
  return (
    <div className="pointer-events-auto absolute bottom-4 left-4 right-4 z-10 flex max-h-40 flex-wrap items-center gap-2 overflow-auto rounded-md border border-white/10 bg-slate-950/78 px-3 py-2 text-xs text-slate-300 shadow-xl backdrop-blur sm:right-[392px]">
      {items.map((item) => {
        const hidden = hiddenKeys.has(item.key);
        return (
          <LegendButton key={item.key} item={item} hidden={hidden} onToggle={onToggle} />
        );
      })}
    </div>
  );
}

function LegendButton({ item, hidden, onToggle }: { item: LegendItem; hidden: boolean; onToggle: (key: string) => void }) {
  const toggleable = item.toggleable !== false;
  return (
    <button
      className={cn(
        "inline-flex max-w-[260px] items-center gap-2 rounded-md border border-white/0 px-1.5 py-1 text-left transition-colors",
        toggleable && "hover:border-white/10 hover:bg-white/[0.04]",
        hidden && "opacity-35 saturate-0"
      )}
      disabled={!toggleable}
      title={toggleable ? `${hidden ? "Show" : "Hide"} ${item.label}` : item.label}
      onDoubleClick={() => {
        if (toggleable) onToggle(item.key);
      }}
    >
      {item.kind === "edge" ? (
        <span className="h-px w-8 shrink-0 bg-slate-500" />
      ) : (
        <span
          className="h-2.5 w-2.5 shrink-0 rounded-full ring-2"
          style={{ backgroundColor: item.color, boxShadow: `0 0 0 2px ${item.ringColor ?? "rgb(148 163 184 / 0.5)"}` }}
        />
      )}
      <span className={cn("truncate", hidden && "line-through")}>{item.label}</span>
      {typeof item.count === "number" ? <span className="shrink-0 font-mono text-[10px] text-slate-500">{formatNumber(item.count)}</span> : null}
    </button>
  );
}

function GraphNodeCard({ root, selectedNode, updateSearch }: { root: string; selectedNode: ObjectRow; updateSearch: UpdateSearch }) {
  const isRoot = selectedNode.object_id === root;
  return (
    <section data-testid="graph-node-details" className="w-full max-w-[44rem] self-end border-t border-slate-200 pt-3 text-slate-900 sm:flex sm:h-[76px] sm:w-[44rem] sm:items-center sm:gap-5 sm:border-l sm:border-t-0 sm:pl-5 sm:pt-0">
      <div className="min-w-0 sm:w-52 sm:shrink-0">
        <div className="truncate text-sm font-semibold text-slate-900">{nodeLabel(selectedNode)}</div>
        <button
          className={cn(
            "mt-1 block max-w-full break-all rounded-sm text-left font-mono text-[11px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-400",
            isRoot
              ? "cursor-default text-slate-500"
              : "text-violet-700 underline decoration-violet-400/50 underline-offset-2 hover:text-violet-950"
          )}
          disabled={isRoot}
          title={isRoot ? "Current root object" : "Set as root object"}
          onClick={() => updateSearch({ root: selectedNode.object_id, selected: undefined }, { history: "push" })}
        >
          {selectedNode.object_id}
        </button>
        <div className="mt-1 truncate text-xs text-slate-400">{selectedNode.module}</div>
      </div>
      <div className="mt-3 grid grid-cols-4 gap-2 sm:mt-0 sm:min-w-0 sm:flex-1">
        <Metric label="shallow" value={formatBytes(selectedNode.shallow_size)} />
        <Metric label="reachable" value={formatBytes(selectedNode.estimated_reachable_size)} />
        <Metric label="in" value={formatNumber(selectedNode.in_edges)} />
        <Metric label="out" value={formatNumber(selectedNode.out_edges)} />
      </div>
    </section>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 border-l border-slate-200 pl-2 first:border-l-0 first:pl-0">
      <div className="truncate text-[10px] uppercase tracking-wide text-slate-500">{label}</div>
      <div className="truncate text-xs font-semibold tabular-nums text-slate-800">{value}</div>
    </div>
  );
}

function filterGraphData(nodes: ObjectRow[], edges: GraphEdge[], missingEdges: GraphEdge[], root: string, hiddenKeys: Set<string>) {
  const visibleNodes = nodes.filter((node) => isNodeVisible(node, root, hiddenKeys));
  const visibleIds = new Set(visibleNodes.map((node) => node.object_id));
  const showReferences = !hiddenKeys.has("reference");
  if (!showReferences) return { nodes: visibleNodes, edges: [], missingEdges: [] };
  return pruneGraphToRoot(
    visibleNodes,
    edges.filter((edge) => visibleIds.has(edge.from_id) && visibleIds.has(edge.to_id)),
    missingEdges.filter((edge) => visibleIds.has(edge.from_id) && visibleIds.has(edge.to_id)),
    root
  );
}

function isNodeVisible(node: ObjectRow, root: string, hiddenKeys: Set<string>) {
  if (node.object_id === root) return true;
  return !hiddenKeys.has(nodeLegendKey(node, root));
}

function graphLegendItems(nodes: ObjectRow[], edgeCount: number, missingEdgeCount: number, root: string): LegendItem[] {
  const groups = new Map<string, LegendItem>();
  groups.set("root", { ...legendItemForSemantic("root"), count: nodes.some((node) => node.object_id === root) ? 1 : 0, toggleable: false });

  for (const node of nodes) {
    const key = nodeLegendKey(node, root);
    if (key === "root") continue;
    const existing = groups.get(key);
    if (existing) {
      existing.count = (existing.count ?? 0) + 1;
      continue;
    }
    groups.set(key, legendItemForNode(key, node));
  }

  const orderedNodeItems = [...groups.values()].sort((left, right) => {
    const leftRank = legendRank(left.key);
    const rightRank = legendRank(right.key);
    if (leftRank !== rightRank) return leftRank - rightRank;
    return (right.count ?? 0) - (left.count ?? 0) || left.label.localeCompare(right.label);
  });

  return [
    ...orderedNodeItems,
    { key: "reference", label: "reference", count: edgeCount + missingEdgeCount, kind: "edge" }
  ];
}

function legendItemForNode(key: string, _node: ObjectRow): LegendItem {
  return { ...legendItemForSemantic(key as NodeSemanticKey), count: 1 };
}

function legendItemForSemantic(key: NodeSemanticKey): LegendItem {
  const semantic = NODE_SEMANTICS[key];
  return { key, label: semantic.label, color: semantic.color, ringColor: semantic.ringColor, kind: "node" };
}

function nodeLegendKey(node: ObjectRow, root: string) {
  return nodeSemantic(node, root).key;
}

function legendRank(key: string) {
  return NODE_SEMANTICS[key as NodeSemanticKey]?.rank ?? 99;
}

function graphElements(nodes: ObjectRow[], edges: GraphEdge[], missingEdges: GraphEdge[], root: string, settings: GraphRenderSettings): ElementDefinition[] {
  const depthById = graphDepths(root, edges, missingEdges);
  const maxLogSize = Math.max(1, ...nodes.map((node) => Math.log10(node.estimated_reachable_size + 1)));
  const importantIds = importantNodeIds(nodes, root);
  return [
    ...nodes.map((node) => {
      const isRoot = node.object_id === root;
      const isMissing = node.type === "<missing>";
      const semantic = nodeSemantic(node, root);
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
          color: semantic.color,
          borderColor: isRoot ? "#ddd6fe" : isMissing ? "#fca5a5" : node.stub ? "#fbbf24" : semantic.color
        },
        classes: [isRoot ? "root" : "", node.stub ? "stub" : "", isMissing ? "missing" : ""].filter(Boolean).join(" ")
      };
    }),
    ...edges.map((edge, index) => ({ data: { id: `edge-${index}`, source: edge.from_id, target: edge.to_id } })),
    ...missingEdges.map((edge, index) => ({ data: { id: `missing-${index}`, source: edge.from_id, target: edge.to_id }, classes: "missing" }))
  ];
}

function graphStyles(settings: GraphRenderSettings): StylesheetJson {
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
    { selector: "node.stub", style: { "background-color": NODE_SEMANTICS.stub.color, "border-color": "#fde68a" } },
    { selector: "node.missing", style: { "background-color": NODE_SEMANTICS.missing.color, "border-color": "#fecaca", "border-style": "dashed" } },
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
    { selector: "edge:selected", style: { opacity: 0.9, width: Math.max(settings.linkWidth * 1.8, 1.4), "line-color": "#c4b5fd", "target-arrow-color": "#c4b5fd" } },
    { selector: "edge.missing", style: { "line-color": "#f97316", "line-style": "dashed", "target-arrow-color": "#fb923c", opacity: 0.55 } },
    { selector: "edge.missing:selected", style: { opacity: 0.95, width: Math.max(settings.linkWidth * 1.8, 1.4), "line-color": "#fdba74", "target-arrow-color": "#fdba74" } }
  ];
}

function runLayout(cy: Core, root: string, settings: GraphRenderSettings) {
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

function nodeSemantic(node: ObjectRow, root: string): NodeSemantic {
  if (node.object_id === root) return NODE_SEMANTICS.root;
  if (node.type === "<missing>") return NODE_SEMANTICS.missing;
  if (node.stub) return NODE_SEMANTICS.stub;

  const type = node.type.toLowerCase();
  const shortType = type.split(".").at(-1) ?? type;
  if (DATA_TYPES.has(shortType)) return NODE_SEMANTICS.data;
  if (CONTAINER_TYPES.has(shortType)) return NODE_SEMANTICS.container;
  if (isCallableType(type)) return NODE_SEMANTICS.callable;
  if (isClassType(type)) return NODE_SEMANTICS.class;
  if (shortType === "module") return NODE_SEMANTICS.module;
  return NODE_SEMANTICS.object;
}

function isCallableType(type: string) {
  return type === "code" || type === "cell" || type.includes("function") || type.includes("method") || type.includes("descriptor") || type.includes("property") || type.includes("callable") || type.endsWith("partial");
}

function isClassType(type: string) {
  if (type === "type" || type === "abc.abcmeta") return true;
  if (type === "nonetype") return false;
  return type.includes("metaclass") || type.endsWith("type");
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
    const normalized = normalizeGraphSettings(settings);
    return parsed.version === GRAPH_SETTINGS_STORAGE_VERSION ? normalized : { ...normalized, showArrows: DEFAULT_GRAPH_SETTINGS.showArrows };
  } catch {
    return DEFAULT_GRAPH_SETTINGS;
  }
}

function saveGraphSettings(settings: GraphSettings) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(GRAPH_SETTINGS_STORAGE_KEY, JSON.stringify({ version: GRAPH_SETTINGS_STORAGE_VERSION, settings }));
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
    animate: typeof value.animate === "boolean" ? value.animate : DEFAULT_GRAPH_SETTINGS.animate,
    hiddenLegendKeys: stringArray(value.hiddenLegendKeys),
    controlsCollapsed: typeof value.controlsCollapsed === "boolean" ? value.controlsCollapsed : DEFAULT_GRAPH_SETTINGS.controlsCollapsed
  };
}

function numberInRange(value: unknown, min: number, max: number, fallback: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringArray(value: unknown) {
  if (!Array.isArray(value)) return DEFAULT_GRAPH_SETTINGS.hiddenLegendKeys;
  return value.filter((item): item is string => typeof item === "string" && isLegendFilterKey(item));
}

function isLegendFilterKey(value: string) {
  return value === "reference" || value in NODE_SEMANTICS;
}
