# CLI 诊断工作台技术实施 Spec

本文规划如何把 [CLI 诊断工作台整改方案](../cli-diagnostics-workbench.md) 落到 `pygco` 的 Rust crates、SQLite 查询、CLI 输出和测试体系中。

## 目标

实现一个可复用的诊断语义层，让 CLI、report、API、Web UI 都可以基于同一组 facts 和 suspects 回答内存排查问题。

成功状态：

- 常见调查不需要手写 SQL。
- `overview` 能给出高信号入口。
- `suspects` 能找出 orphan retained、high retained root、cache/async/connection/metadata 等线索。
- `explain` 和 `trace` 能把对象 ID 转成可读解释。
- 所有输出都保留 JSON 机器契约和 table/markdown 人类契约。

## 当前实现状态

已落地：

- `pygco findings`：直接读取/刷新持久化 findings，支持 kind/severity/limit/offset 和标准输出格式。
- `pygco suspects` 第一阶段：支持 `orphan-retained`、`high-retained-root`、`truncated-root`、`type-footprint`、`metadata-heavy`、`cache-heavy`、`async-backlog`、`connection-heavy`。
- `suspects --kind orphan-retained` 已在百万对象真实库上验证，可直接发现约 20 MiB orphan generator candidate，且查询预算低于 5 秒。

仍未落地：

- `overview`、`rank`、`explain`、`trace`、`cohorts`。
- `suspects --from/--to` diff suspects。
- CLI/API/Web/report 全部复用同一 typed facts 模型。

## 非目标

- 不在第一阶段改变 dump 格式。
- 不在第一阶段引入长期 SQLite migration；旧库缺少新物化表时必须降级。
- 不在第一阶段证明泄漏；CLI 只能生成 candidates/leads。
- 不要求前端 e2e 参与 CLI 快速开发循环。

## 架构改造

### Crate 边界

| Crate | 改造职责 |
| --- | --- |
| `pygco-analysis` | 新增 diagnostic facts、ranking、suspects、explain、trace enrichment 查询和规则 |
| `pygco-cli` | 新增命令、参数、输出格式，调用 `pygco-analysis` |
| `pygco-report` | 复用 suspects/findings/facts 生成 markdown/json report |
| `pygco-api` | 后续暴露同一 facts/suspects 接口给 Web UI |
| `pygco-store` | 只在必要时增加物化表或索引；第一阶段优先复用现有表 |

### 内部模块建议

```text
crates/pygco-analysis/src/
  diagnostics/
    facts.rs
    rank.rs
    suspects.rs
    explain.rs
    trace.rs
    cohorts.rs
    output.rs
```

如果当前 crate 仍保持单文件实现，可以先用模块级函数和 structs 分区，后续再拆文件。

## 数据结构

### Diagnostic Facts

核心结构建议：

```rust
struct SnapshotFacts {
    snapshot_id: i64,
    object_count: i64,
    edge_count: i64,
    shallow_size_sum: i64,
    stub_count: i64,
    missing_referent_count: i64,
    reachability: ReachabilityStatus,
}

struct TypeFacts {
    type_name: String,
    module: String,
    count: i64,
    stub_count: i64,
    shallow_size_sum: i64,
    in_edges: i64,
    out_edges: i64,
    reachable_size_sum: i64,
    reachable_size_max: i64,
    reachable_truncated_count: i64,
}

struct ModuleFacts {
    module: String,
    object_count: i64,
    shallow_size_sum: i64,
    reachable_size_sum: i64,
    reachable_size_max: i64,
    in_edges: i64,
    out_edges: i64,
}

struct ObjectFacts {
    object_id: i64,
    type_name: String,
    module: String,
    shallow_size: i64,
    reachable_size: i64,
    reachable_count: i64,
    reachable_truncated: bool,
    in_edges: i64,
    out_edges: i64,
    missing_referents: i64,
    stub: bool,
}
```

JSON 输出时 object id 必须是 string，bytes 同时保留 raw number 和 human label。

### Suspect

```rust
enum SuspectKind {
    OrphanRetained,
    HighRetainedRoot,
    TruncatedRoot,
    TypeFootprint,
    MetadataHeavy,
    CacheHeavy,
    AsyncBacklog,
    ConnectionHeavy,
    StubHeavy,
    DiffGrowth,
}

struct Suspect {
    kind: SuspectKind,
    severity: FindingSeverity,
    confidence: Confidence,
    subject: SuspectSubject,
    metrics: serde_json::Value,
    reason: String,
    limitations: Vec<String>,
    next_command: String,
}
```

`confidence` 建议枚举：

```text
low
medium
high
```

## 查询和规则

### Overview

`overview --compact` 聚合：

- `SnapshotFacts`
- top 5 `Suspect`
- top 10 non-builtin `TypeFacts` by shallow
- top 10 non-builtin `ModuleFacts` by reachable
- cohort summary for cache/async/connection/threading/network

查询预算：百万对象库 < 1s。依赖 `snapshots`、`type_stats`、`type_reachability_stats`、`object_list_metrics`、`cohort_stats`。

### Rank

统一 rank API：

```text
rank_by = type | module | object
metric = count | shallow | reachable | max-reachable | in-edges | out-edges
filters = snapshot, q, type, module, cohort, non_builtin, include_stub
```

实现策略：

- type：优先 `type_stats` + `type_reachability_stats`
- module：优先 `object_list_metrics` 聚合；如果性能不足，再物化 `module_reachability_stats`
- object：优先 `object_list_metrics`

### Orphan Retained

定义：对象没有外部入边，但 estimated reachable size 高。

外部入边排除 self edge：

```sql
WITH external_in AS (
  SELECT to_id AS object_id, COUNT(*) AS external_in_edges
  FROM edges
  WHERE snapshot_id = ?1 AND from_id <> to_id
  GROUP BY to_id
)
SELECT ...
FROM object_list_metrics m
LEFT JOIN external_in e ON e.object_id = m.object_id
WHERE m.snapshot_id = ?1
  AND m.stub = 0
  AND m.reachable_size >= ?2
  AND COALESCE(e.external_in_edges, 0) = 0
ORDER BY m.reachable_size DESC
LIMIT ?3;
```

默认阈值：

```text
min_reachable = 1 MiB
limit = 20
```

若查询在大库上超过 5s，应考虑新增 `object_external_in_stats(snapshot_id, object_id, external_in_edges, self_edges)` 物化表。

### High Retained Root

定义：单对象 estimated reachable size 排名靠前。

优先从 `object_list_metrics` 查：

```text
snapshot_id = ?
stub = false by default
reachable_size desc
```

规则输出要标注 `reachable_truncated`，截断对象不能直接比较为精确大小。

### Metadata Heavy

模块/类型 prefix：

```text
pydantic.
pydantic_core.
fastapi.
starlette.
sqlalchemy.
typing
typing_extensions
```

输出聚合：

- object count
- shallow size
- estimated reachable sum
- max reachable
- truncated count

解释必须说明：这通常是框架常驻 footprint，单 dump 不能证明泄漏，diff 增长才关键。

### Cache / Async / Connection

第一阶段使用 cohort + type/module pattern：

cache patterns:

```text
cache
cached
lru
ttl
pool
inmemory
```

async patterns:

```text
_asyncio.Task
_asyncio.Future
async_generator
asyncio.
anyio.
```

connection patterns:

```text
Connection
ConnectionPool
Redis
HTTPConnection
Socket
PoolManager
```

第二阶段把 patterns 移入规则配置，避免硬编码扩散。

## CLI 命令规格

### `overview`

目标：

```text
pygco overview DB --snapshot 1 --compact --format table
pygco overview DB --snapshot 1 --format json
```

输出 section：

```text
Snapshot
Reachability
Top Suspects
Top Non-Builtin Types
Top Non-Builtin Modules
Cohorts
Next Commands
```

### `rank`

目标：

```text
pygco rank DB --by type --metric shallow --non-builtin
pygco rank DB --by module --metric reachable --non-builtin
pygco rank DB --by object --metric max-reachable --include-builtin
```

第一阶段也可以拆成 `types`、`modules`，但内部必须复用同一 rank API。

### `suspects`

目标：

```text
pygco suspects DB --kind orphan-retained --min-reachable 1mb
pygco suspects DB --kind cache --kind async --kind connection
pygco suspects DB --format json
```

参数：

```text
--kind <kind> repeatable
--min-reachable <bytes-or-human>
--non-builtin
--include-stub
--limit <n>
--snapshot <id>
--from <snapshot>
--to <snapshot>
```

### `explain`

目标：

```text
pygco explain DB --id OBJECT_ID --snapshot 1
```

输出：

- object facts
- interpretation
- why it is or is not suspicious
- top referents by shallow/reachable where available
- top referrers
- next commands
- limitations

### `trace`

目标：

```text
pygco trace DB --id OBJECT_ID --direction referrers --depth 5 --fanout 30 --verbose
```

第一阶段可复用 `paths` 算法，但输出必须 enrich 每个节点的 type/module/size/edge count。

### `findings`

目标：

```text
pygco findings DB --snapshot 1 --format table
pygco findings DB --snapshot 1 --format json
```

直接读取持久化 findings，并展示 evidence 摘要与 next command。

## 输出格式

### Table

table 默认面向终端：

- 列数少。
- human bytes。
- 截断列名可接受，但必须保留语义。
- `--verbose` 展开 reason/limitations/next command。

### JSON

JSON 必须稳定：

```json
{
  "data": [],
  "meta": {
    "snapshot_id": 1,
    "generated_at": "...",
    "estimated_reachable": true,
    "limitations": []
  }
}
```

现有命令如果已经返回非包裹对象，不强制一次性迁移；新诊断命令应从一开始使用稳定 meta。

## 性能预算

| Command | Budget |
| --- | ---: |
| `overview --compact` | < 1s |
| `rank --by type` | < 500ms |
| `rank --by module` | < 1s |
| `rank --by object` | < 300ms |
| `suspects --kind orphan-retained` | < 5s |
| `explain --id` | < 500ms |
| `trace --verbose` | < 1.5s |
| `findings` | < 100ms |

若 `orphan-retained` 在真实大库上超过预算，优先新增外部入边物化表，而不是继续依赖临时 CTE。

## 测试策略

### Rust tests

- `pygco-analysis` unit tests：
  - orphan retained candidate
  - self-cycle is not external referrer
  - high retained root with truncated flag
  - metadata-heavy classification
  - cache/async/connection classification
- `pygco-cli` contract tests：
  - command exists
  - JSON schema stable
  - table output contains key columns
  - invalid kind/metric exits with parameter error
- golden fixtures：
  - tiny no suspect
  - orphan generator
  - metadata-heavy
  - async backlog
  - cache-heavy

### Performance tests

- Add query benchmarks for `overview`, `rank`, `suspects`, `explain`, `trace`.
- Use existing synthetic fixtures first.
- Add one optional local benchmark profile for real dump databases under `.pygco/live-dumps/`, excluded from CI.

### Frontend tests

CLI-only changes do not require Web UI e2e. When API/Web consumes diagnostic facts later, add targeted API contract tests first; browser e2e remains optional and should not block CLI iteration.

## Implementation phases

### Phase 1: Facts and compact overview

- Add `SnapshotFacts`, `TypeFacts`, `ModuleFacts`, `ObjectFacts`.
- Add `overview` command.
- Add compact table and JSON output.
- Add non-builtin ranking helpers.

Exit criteria:

- `overview --compact` answers snapshot size, reachability status, top non-builtin types/modules.
- No hand-written SQL needed for basic footprint orientation.

### Phase 2: Suspects engine

- Status: partially implemented.
- Done: add first JSON suspect model and `suspects` command.
- Done: implement `orphan-retained`, `high-retained-root`, `truncated-root`, `type-footprint`, `metadata-heavy`, `cache-heavy`, `async-backlog`, `connection-heavy`.
- Done: add thresholds, confidence, limitations, and `next_command`.
- Remaining: extract typed Rust structs from JSON assembly, tune thresholds on more dumps, add diff suspects.

Exit criteria:

- Real dump analysis can find the orphan generator candidate without SQL.
- Output includes reason, metrics, limitations, next command.

### Phase 3: Resource cohorts

- Implement cache/async/connection/threading/network summary functions.
- Add `cohorts` command.
- Wire resource suspects to cohort facts.

Exit criteria:

- CLI can say cache/connection/async are not obviously high for a snapshot without SQL.

### Phase 4: Explain and trace

- Add `explain` command.
- Add type explainers for dict, generator, function, module, type, list/set, Task/Future.
- Add `trace --verbose` enrichment.

Exit criteria:

- Large generator/dict objects become understandable without opening SQL.

### Phase 5: Diff suspects

- Extend `suspects` with `--from` / `--to`.
- Add diff facts for type/module/cohort/object lifecycle.
- Add growth-oriented confidence.

Exit criteria:

- Same-process consecutive dumps can produce high-confidence growth leads.

### Phase 6: API/Web/report reuse

- Expose diagnostic facts through local API.
- Update report generation to use suspects.
- Update Web UI to consume same facts rather than reimplementing ranking logic.

Exit criteria:

- CLI, report, and Web show consistent suspects and explanations.

## Backward compatibility

- Existing `summary`, `objects`, `object`, `edges`, `paths`, `diff`, `sql` keep working.
- New commands can share code but should not silently change old JSON schemas.
- Older SQLite files without `object_list_metrics` must fall back to slower joins or emit a clear degraded-performance warning.
- `sql` remains read-only and available.

## Risks

- Some explainers need richer producer data. Without dict keys or frame locals, explanations must remain probabilistic.
- `orphan-retained` can identify GC-pending garbage, not necessarily leaks.
- Reachable estimates overlap; summing reachable across roots must remain labeled as overlapping.
- More commands increase CLI surface. Keep command names few and composable.

## Source Manifest

### Sources

- User request in side conversation: perform CLI-mode analysis of `.pygco/live-dumps/local-25292-reachable.sqlite`.
- User follow-up: identify CLI issues exposed because analysis relied on `pygco sql`.
- User follow-up: provide a systemic remediation plan.
- User follow-up: implement recommendations into docs and add a detailed technical spec.
- Existing docs: [CLI 规范](../cli.md), [分析模型](../analysis-model.md), [系统架构](../architecture.md), [性能规范](../performance.md), [SQLite Schema 规范](../sqlite-schema.md), [Source Manifest](source-manifest.md).
- Real-data evidence: local CLI exploration against `.pygco/live-dumps/local-25292-reachable.sqlite`, including `pygco objects`, `pygco object`, `pygco paths`, `pygco report`, and read-only `pygco sql`.

### Produced artifacts

- [CLI 诊断工作台整改方案](../cli-diagnostics-workbench.md)
- [CLI 诊断工作台技术实施 Spec](cli-diagnostics-technical-spec.md)

### Key decisions

- Build a diagnostic facts layer in `pygco-analysis` before adding more UI.
- Add `suspects` as the primary way to express heuristic memory leads.
- Keep current commands backward-compatible and introduce new commands incrementally.
- Treat Web/API reuse as a later phase after CLI semantics stabilize.

### Verification evidence

- This spec is documentation-only. Suggested verification after edits: `python3 scripts/check_docs_commands.py`.
- Command examples for unimplemented target commands are in `text` fences so docs validation does not mistake them for current CLI contract.

### Open questions / risks

- Exact default thresholds for suspects require more real dump samples.
- `module_reachability_stats` or `object_external_in_stats` may be needed if first-phase SQL misses performance budgets.
- Producer-level enhancements may be needed for high-quality dict/generator/function explainers.
