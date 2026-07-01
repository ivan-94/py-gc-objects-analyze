# SQLite Schema 规范

Implementation contract: this document is the first-version contract between importer, analysis, API, CLI, SQL probe, and Web UI code.

SQLite 是 `pygco` 的临时、可重建分析数据库。schema 是 Rust importer、analysis、API server、SQL 探针和 Web UI 之间的核心契约。

## 基本约定

- SQLite 文件默认由 `pygco import` / `pygco open` 重建。
- 不承诺长期 schema migration；schema 版本不兼容时应从 dump 重新导入。
- 所有对象 id 在 SQLite 中使用 `INTEGER` 存储。
- 所有 API/JSON/URL 中的 object id 必须序列化为 string，避免 JavaScript safe integer 问题。
- `object_id` 必须始终和 `snapshot_id` 共同使用。

## Schema Version

```sql
CREATE TABLE schema_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

必填 key：

```text
schema_version
tool_version
created_at
```

## snapshots

```sql
CREATE TABLE snapshots (
  snapshot_id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_uri TEXT NOT NULL,
  source_basename TEXT NOT NULL,
  dump_sha256 TEXT NOT NULL,
  dump_format TEXT NOT NULL,
  dump_format_version INTEGER NOT NULL,
  producer TEXT NOT NULL,
  producer_version TEXT NOT NULL,
  producer_run_id TEXT,
  dump_sequence INTEGER,
  process_started_at TEXT,
  host_id TEXT,
  container_id TEXT,
  pid INTEGER,
  python_version TEXT,
  platform TEXT,
  created_at TEXT,
  imported_at TEXT NOT NULL,
  import_options_json TEXT NOT NULL,
  object_count INTEGER NOT NULL DEFAULT 0,
  edge_count INTEGER NOT NULL DEFAULT 0,
  stub_count INTEGER NOT NULL DEFAULT 0,
  missing_referent_count INTEGER NOT NULL DEFAULT 0,
  shallow_size_sum INTEGER NOT NULL DEFAULT 0
);
```

索引：

```sql
CREATE UNIQUE INDEX idx_snapshots_dump_sha256 ON snapshots(dump_sha256);
CREATE INDEX idx_snapshots_producer_run ON snapshots(producer_run_id, dump_sequence);
```

`snapshot_id` 是 session 内自增 handle，可用于 CLI 和 URL。事实来源仍是 `dump_sha256` 与 source metadata。

## objects

```sql
CREATE TABLE objects (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  qualname TEXT NOT NULL,
  shallow_size INTEGER,
  gc_tracked INTEGER,
  stub INTEGER NOT NULL DEFAULT 0,
  repr TEXT,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

索引：

```sql
CREATE INDEX idx_objects_snapshot_type ON objects(snapshot_id, type);
CREATE INDEX idx_objects_snapshot_type_object ON objects(snapshot_id, type, object_id);
CREATE INDEX idx_objects_snapshot_module ON objects(snapshot_id, module);
CREATE INDEX idx_objects_snapshot_size ON objects(snapshot_id, shallow_size DESC);
CREATE INDEX idx_objects_snapshot_stub ON objects(snapshot_id, stub);
```

要求：

- `object_id` 不允许跨 snapshot 单独作为 key。
- stub object 必须 `stub=1`。
- missing referent 不写入 objects。

## edges

```sql
CREATE TABLE edges (
  snapshot_id INTEGER NOT NULL,
  from_id INTEGER NOT NULL,
  edge_index INTEGER NOT NULL,
  to_id INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, from_id, edge_index),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

索引：

```sql
CREATE INDEX idx_edges_snapshot_from ON edges(snapshot_id, from_id);
CREATE INDEX idx_edges_snapshot_to ON edges(snapshot_id, to_id);
CREATE INDEX idx_edges_snapshot_from_to ON edges(snapshot_id, from_id, to_id);
```

`edge_index` 保留 producer 输出 referents 的顺序，并允许同一对象多次引用同一 referent。

## object_edge_stats

```sql
CREATE TABLE object_edge_stats (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  missing_referents INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);
```

索引：

```sql
CREATE INDEX idx_object_edge_stats_in ON object_edge_stats(snapshot_id, in_edges DESC);
CREATE INDEX idx_object_edge_stats_out ON object_edge_stats(snapshot_id, out_edges DESC);
CREATE INDEX idx_object_edge_stats_missing ON object_edge_stats(snapshot_id, missing_referents DESC);
```

`object_edge_stats` 是导入后构建的 per-object 物化统计，用于 Objects 页快速展示、排序和过滤 in/out/missing edge counts。它不替代 `edges`，也不改变 missing referent 的语义：missing referent 仍然表示 `edges.to_id` 在同 snapshot 的 `objects` 中不存在。

## object_list_metrics

```sql
CREATE TABLE object_list_metrics (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  shallow_size INTEGER NOT NULL,
  stub INTEGER NOT NULL,
  reachable_count INTEGER NOT NULL,
  reachable_size INTEGER NOT NULL,
  reachable_truncated INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  missing_referents INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);
```

索引：

```sql
CREATE INDEX idx_object_list_metrics_reachable
  ON object_list_metrics(snapshot_id, reachable_size DESC, object_id);
CREATE INDEX idx_object_list_metrics_type_reachable
  ON object_list_metrics(snapshot_id, type, reachable_size DESC, object_id);
CREATE INDEX idx_object_list_metrics_module_reachable
  ON object_list_metrics(snapshot_id, module, reachable_size DESC, object_id);
CREATE INDEX idx_object_list_metrics_in_edges
  ON object_list_metrics(snapshot_id, in_edges DESC, object_id);
CREATE INDEX idx_object_list_metrics_out_edges
  ON object_list_metrics(snapshot_id, out_edges DESC, object_id);
```

`object_list_metrics` 是 Objects 页的排序/过滤投影表，每个 object 一行，合并 `objects`、`object_edge_stats` 与当前默认 reachability 参数下的 `object_reachability`。没有 reachability 行的对象必须保留，并以 `reachable_count=0`、`reachable_size=0`、`reachable_truncated=0` 表示。

## type_stats

```sql
CREATE TABLE type_stats (
  snapshot_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  stub_count INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, type),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

## module_stats

```sql
CREATE TABLE module_stats (
  snapshot_id INTEGER NOT NULL,
  module TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, module),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

## cohort_stats

```sql
CREATE TABLE cohort_stats (
  snapshot_id INTEGER NOT NULL,
  cohort TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  type_count INTEGER NOT NULL,
  details_json TEXT NOT NULL,
  rules_version TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, cohort),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

## object_reachability

```sql
CREATE TABLE object_reachability (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  algorithm_version INTEGER NOT NULL,
  direction TEXT NOT NULL,
  depth INTEGER NOT NULL,
  node_limit INTEGER NOT NULL,
  fanout_limit INTEGER NOT NULL,
  reachable_count INTEGER NOT NULL,
  reachable_size INTEGER NOT NULL,
  truncated INTEGER NOT NULL,
  computed_at TEXT NOT NULL,
  PRIMARY KEY (
    snapshot_id,
    object_id,
    algorithm_version,
    direction,
    depth,
    node_limit,
    fanout_limit
  ),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);
```

索引：

```sql
CREATE INDEX idx_object_reachability_size
  ON object_reachability(snapshot_id, algorithm_version, direction, depth, node_limit, fanout_limit, reachable_size DESC);
```

cache key 必须包含算法版本和所有参数。

## type_reachability_stats

```sql
CREATE TABLE type_reachability_stats (
  snapshot_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  algorithm_version INTEGER NOT NULL,
  direction TEXT NOT NULL,
  depth INTEGER NOT NULL,
  node_limit INTEGER NOT NULL,
  fanout_limit INTEGER NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  reachable_size_sum INTEGER NOT NULL,
  reachable_size_avg REAL NOT NULL,
  reachable_size_max INTEGER NOT NULL,
  truncated_count INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, type, algorithm_version, direction, depth, node_limit, fanout_limit)
);
```

## findings

```sql
CREATE TABLE findings (
  finding_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  severity TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  action TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  algorithm_version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);
```

## saved_idsets

`idset` 命令默认不持久化结果。Web UI 中用户显式保存时使用：

```sql
CREATE TABLE saved_idsets (
  idset_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  source_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE saved_idset_objects (
  idset_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  PRIMARY KEY (idset_id, object_id),
  FOREIGN KEY (idset_id) REFERENCES saved_idsets(idset_id) ON DELETE CASCADE
);
```

## import_warnings

```sql
CREATE TABLE import_warnings (
  warning_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER,
  level TEXT NOT NULL,
  code TEXT NOT NULL,
  message TEXT NOT NULL,
  context_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
```

## PRAGMA

导入期建议：

```sql
PRAGMA foreign_keys = OFF;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = MEMORY;
```

导入完成后：

```sql
PRAGMA foreign_keys = ON;
PRAGMA optimize;
```

如果导入失败，删除 `.tmp.sqlite`，不保留半成品。
