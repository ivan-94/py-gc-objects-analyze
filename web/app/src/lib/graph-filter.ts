export type GraphEdge = { from_id: string; to_id: string };

type GraphNode = { object_id: string };

export function pruneGraphToRoot<T extends GraphNode>(nodes: T[], edges: GraphEdge[], missingEdges: GraphEdge[], root: string) {
  const nodeIds = new Set(nodes.map((node) => node.object_id));
  if (!nodeIds.has(root)) return { nodes: [], edges: [], missingEdges: [] };

  const adjacency = new Map<string, Set<string>>([...nodeIds].map((id) => [id, new Set<string>()]));
  for (const edge of [...edges, ...missingEdges]) {
    if (!nodeIds.has(edge.from_id) || !nodeIds.has(edge.to_id)) continue;
    adjacency.get(edge.from_id)?.add(edge.to_id);
    adjacency.get(edge.to_id)?.add(edge.from_id);
  }

  const reachable = new Set([root]);
  const pending = [root];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current) continue;
    for (const adjacent of adjacency.get(current) ?? []) {
      if (reachable.has(adjacent)) continue;
      reachable.add(adjacent);
      pending.push(adjacent);
    }
  }

  const includesReachableEndpoints = (edge: GraphEdge) => reachable.has(edge.from_id) && reachable.has(edge.to_id);
  return {
    nodes: nodes.filter((node) => reachable.has(node.object_id)),
    edges: edges.filter(includesReachableEndpoints),
    missingEdges: missingEdges.filter(includesReachableEndpoints)
  };
}
