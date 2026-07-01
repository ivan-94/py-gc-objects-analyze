# 实现蓝图

本文把架构文档进一步细化到实现者可以开工的模块边界、核心类型、函数形态和第一条 tracer bullet。

## 第一条 Tracer Bullet

第一条端到端闭环：

```text
fixtures/golden/tiny-v1.jsonl.gz
  -> pygco import fixtures/golden/tiny-v1.jsonl.gz -o analysis.sqlite --rebuild
  -> SQLite snapshots/objects/edges/type_stats
  -> pygco summary analysis.sqlite --format json
  -> pygco web analysis.sqlite --no-browser
  -> Web Overview page
```

这个闭环不需要先完成全部 graph/diff/sql，但必须证明：

- dump format 能解析。
- SQLite schema 能创建。
- import 能写入 snapshot/object/edge。
- summary 能从 stats 表读出结果。
- API 能返回 summary。
- Web 能渲染 Overview。

## Rust Crate Contracts

### `pygco-dump-format`

核心类型：

```rust
enum DumpRecord {
    MetadataStart(MetadataStart),
    Object(ObjectRecord),
    MetadataEnd(MetadataEnd),
}

struct MetadataStart {
    format: String,
    format_version: u32,
    producer: String,
    producer_version: String,
    producer_run_id: String,
    dump_sequence: u64,
    created_at: String,
    process_started_at: Option<String>,
    host_id: Option<String>,
    container_id: Option<String>,
    pid: Option<u32>,
    include_referents: bool,
    include_referent_stubs: bool,
    include_repr: bool,
    object_count: u64,
}

struct ObjectRecord {
    id: i64,
    type_name: String,
    module: Option<String>,
    qualname: Option<String>,
    size: Option<i64>,
    gc_tracked: Option<bool>,
    stub: bool,
    referents: Vec<i64>,
    repr: Option<String>,
}
```

职责：

- streaming line parse。
- version validation。
- line-numbered error。
- object id JSON string helper。

### `pygco-store`

核心 API：

```rust
struct Store {
    conn: rusqlite::Connection,
}

impl Store {
    fn create_tmp(path: &Path) -> Result<Self>;
    fn create_schema(&self) -> Result<()>;
    fn begin_import(&self) -> Result<()>;
    fn insert_snapshot_stub(&self, input: SnapshotInput) -> Result<SnapshotId>;
    fn insert_objects_batch(&self, rows: &[ObjectRow]) -> Result<()>;
    fn insert_edges_batch(&self, rows: &[EdgeRow]) -> Result<()>;
    fn finalize_snapshot(&self, summary: SnapshotSummary) -> Result<()>;
    fn build_indexes(&self) -> Result<()>;
    fn optimize(&self) -> Result<()>;
}
```

职责：

- schema。
- prepared statements。
- atomic temp DB。
- readonly query guard。
- common DTO mapping。

### `pygco-importer`

核心 API：

```rust
struct ImportOptions {
    rebuild: bool,
    reachability: ReachabilityMode,
    reachability_params: ReachabilityParams,
    cohort_rules_path: Option<PathBuf>,
    profile: bool,
}

fn import_dumps(inputs: Vec<PathBuf>, output: PathBuf, options: ImportOptions) -> Result<ImportSummary>;
```

导入阶段：

```text
open tmp db
create schema
for each dump:
  compute sha256 while streaming
  validate start metadata
  insert snapshot stub
  batch insert objects
  batch insert edges
  finalize snapshot
build stats
build indexes
optional reachability
rename tmp db
```

### `pygco-analysis`

模块：

```text
summary
objects
edges
paths
subgraph
reachability
diff
idset
findings
doctor
```

每个 query module 返回 typed DTO，CLI/API/Web 不直接拼 SQL。

### `pygco-api`

核心要求：

- 所有 endpoints 按 [docs/api.md](../api.md)。
- 所有 object id 输出为 string。
- errors 使用统一 envelope。
- OpenAPI export。
- 长任务使用 job registry。

### `pygco-cli`

CLI 只负责：

- 参数解析。
- 调用 importer/analysis/api。
- 输出格式。
- exit code。

不要把分析 SQL 写进 CLI crate。

## Python Producer Contracts

Python producer 的稳定公共 API：

```python
def iter_gc_dump_records(
    *,
    collect: bool = False,
    include_referents: bool = True,
    include_referent_stubs: bool = True,
    include_repr: bool = False,
    repr_limit: int = 0,
) -> Iterator[dict[str, Any]]:
    ...

def write_gc_dump(
    file: BinaryIO,
    *,
    collect: bool = False,
    include_referents: bool = True,
    include_referent_stubs: bool = True,
    include_repr: bool = False,
    repr_limit: int = 0,
) -> DumpSummary:
    ...
```

FastAPI helper 只是对 `write_gc_dump` 的包装。

## Data Ownership

- Python producer owns raw runtime observation.
- Rust importer owns dump validation and SQLite write.
- Store owns schema and low-level query helpers.
- Analysis owns graph/stat algorithms.
- API owns JSON contract and job lifecycle.
- Web owns presentation, URL state, server-state cache.

## Error Model

所有 Rust errors 最终映射到：

```text
code
message
details
source
```

CLI 显示 human message，JSON/API 保留 structured error。

## Implementation Do Not Copy From POC

- Do not copy Python HTML rendering.
- Do not copy Python analysis modules into production.
- Do not keep reachability cache keyed only by depth.
- Do not use object_id without snapshot_id.
- Do not let CLI own query semantics.
- Do not expose raw integer object ids to JS.
