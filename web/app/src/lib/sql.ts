import type { SavedIdset } from "@/generated/api-client";

export function quoteSqlIdentifier(identifier: string) {
  return `"${identifier.replaceAll('"', '""')}"`;
}

export function objectIdsFromResult(result: Record<string, unknown>) {
  const rows = Array.isArray(result.rows) ? result.rows : [];
  const ids = new Set<string>();
  for (const row of rows) {
    if (!row || typeof row !== "object" || !("object_id" in row)) continue;
    const rawId = (row as { object_id?: unknown }).object_id;
    if (typeof rawId === "string" && rawId.trim()) ids.add(rawId);
    if (typeof rawId === "number" && Number.isFinite(rawId)) ids.add(String(rawId));
  }
  return [...ids];
}

export function savedIdsetSql(idset: SavedIdset) {
  return [
    "select o.object_id, o.type, o.module, o.shallow_size",
    "from saved_idset_objects sio",
    "join objects o on o.snapshot_id = " + idset.snapshot_id + " and o.object_id = sio.object_id",
    "where sio.idset_id = " + idset.idset_id,
    "order by o.object_id",
    "limit 200"
  ].join("\n");
}
