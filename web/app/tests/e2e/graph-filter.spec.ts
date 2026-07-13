import { expect, test } from "@playwright/test";

import { pruneGraphToRoot } from "../../src/lib/graph-filter";

test("removes nodes disconnected from the root after graph filtering", () => {
  const nodes = [{ object_id: "root" }, { object_id: "left" }, { object_id: "right" }, { object_id: "orphan-a" }, { object_id: "orphan-b" }];
  const graph = pruneGraphToRoot(
    nodes,
    [
      { from_id: "root", to_id: "left" },
      { from_id: "right", to_id: "root" },
      { from_id: "orphan-a", to_id: "orphan-b" }
    ],
    [],
    "root"
  );

  expect(graph.nodes.map((node) => node.object_id)).toEqual(["root", "left", "right"]);
  expect(graph.edges).toEqual([
    { from_id: "root", to_id: "left" },
    { from_id: "right", to_id: "root" }
  ]);
  expect(graph.missingEdges).toEqual([]);
});
