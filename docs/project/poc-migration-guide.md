# POC 迁移指南

本文说明 ai_glass POC 中哪些经验可以复用，哪些实现不能照搬。正式项目应复用 POC 的问题认知、数据语义和验收场景，而不是复制 Python 分析层、临时 schema 或探索型 WebUI。

POC 路径：

```text
/Users/ivan/.codex/worktrees/memory-analyzer/ai_glass
```

关键文件：

```text
debug_tools/gc_heap_dump.py
tools/memory_analyzer/importer.py
tools/memory_analyzer/store.py
tools/memory_analyzer/reachability.py
tools/memory_analyzer/analysis.py
tools/memory_analyzer/cli.py
tools/memory_analyzer/web.py
tools/memory_analyzer/report.py
test/test_gc_heap_dump.py
test/test_memory_analyzer.py
```

## 迁移原则

- 复用字段语义，不复制临时字段名。
- 复用测试场景，不复制 Python 分析架构。
- 复用查询经验，不复制 POC 的 SQL 拼接方式。
- 复用 WebUI 的用户路径，不复制 HTML 字符串渲染。
- 复用算法直觉，但用 Rust 重新设计性能和缓存边界。

## 可复用经验

| POC 文件 | 可复用经验 | 正式项目落点 |
| --- | --- | --- |
| `debug_tools/gc_heap_dump.py` | start/object/end record 分层；stream gzip；单飞锁；`collect=false`；`include_repr=false`；referent stub | `python/pygco_dump/core.py`、`docs/dump-format.md` |
| `test/test_gc_heap_dump.py` | stub 行为、gzip JSONL、end metadata 的测试方式 | `python/pygco_dump/tests/`、golden fixtures |
| `tools/memory_analyzer/importer.py` | objects/edges 批量导入；type/module/cohort stats；missing/stub 计数 | `pygco-importer`、`pygco-store` |
| `tools/memory_analyzer/store.py` | SQLite 作为本地分析库；索引方向；schema_meta | `docs/sqlite-schema.md`、`pygco-store` |
| `tools/memory_analyzer/reachability.py` | visited set 防循环；depth/limit 截断；type reachability 聚合 | `pygco-analysis` reachability |
| `tools/memory_analyzer/analysis.py` | object detail、edges、paths、missing、stubs、diff、idset、doctor 的查询形态 | `pygco-analysis` query modules |
| `tools/memory_analyzer/cli.py` | Agent 友好的 `json/jsonl`、`--fields`、`--ids-only`、`sql`、`schema` | `pygco-cli` |
| `tools/memory_analyzer/web.py` | Overview/Objects/Object detail/Diff/Findings/SQL/Report 的页面需求 | `web/app` React routes |
| `tools/memory_analyzer/report.py` | report section 组织方式 | `pygco-report` |
| `test/test_memory_analyzer.py` | end-to-end fixture 思路：dump -> SQLite -> analysis -> API/Web smoke | `fixtures/golden`、Rust/API/Web tests |

## 不能照搬的部分

### Python 分析层

POC 的 importer、analysis、reachability 都是 Python 实现。它验证了语义，但不适合作为正式项目实现：

- 大对象图计算会慢。
- 内存模型不可控。
- SQL 和 Python 聚合混在一起。
- 运行时类型约束弱。

正式项目必须用 Rust 重写 importer、store、analysis、API。

### POC SQLite schema

POC schema 是逐步加字段形成的：

- `object_id` 语义正确，但正式项目必须强制所有 query scoped by `snapshot_id`。
- reachability cache 只按 depth 区分，正式项目必须包含 algorithm version、direction、node limit、fanout limit。
- POC 没有 process identity 字段，正式项目必须支持 lifecycle confidence。

以 [docs/sqlite-schema.md](../sqlite-schema.md) 为准。

### POC WebUI

POC WebUI 是 FastAPI + HTML 字符串，适合验证信息架构，不适合正式项目：

- 表格性能和列宽不可控。
- 长 JSON/evidence 容易撑破布局。
- 图渲染没有专业交互模型。
- URL state 和 server-state 没有体系化。

正式项目必须使用 React + shadcn/ui + TanStack Router/Table/Virtual/Query。

### POC CLI 命名

POC 的命令可以参考能力集合，但正式项目命令名、参数名、输出格式以 [docs/cli.md](../cli.md) 为准。

特别注意：

- POC 中 `size`/`shallow_size` 命名混用，正式项目统一展示 `shallow_size`。
- POC 中 reachability 参数不完整，正式项目使用 canonical 参数。
- POC 的 SQL 是本地便利能力，正式项目必须保持只读 guard。

## Golden Fixture 迁移

优先从 POC 测试场景迁移为 golden fixtures：

- tiny object graph
- stub referents
- missing referents
- cycle graph
- diff before/after
- cachetools-like LRU cache graph
- database_cache cohort
- streaming cohort

fixture 不必复制真实业务数据。应构造小而精确的数据，让 expected output 可人工审阅。

## POC 到 Slice 映射

| Slice | POC 参考 | 迁移方式 |
| --- | --- | --- |
| S03 Python producer | `debug_tools/gc_heap_dump.py` | 保留 API 行为，补 process identity 和正式 package |
| S05 dump-format | dump records + tests | 用 serde model 固化 schema |
| S06 store | `store.py` | 只参考表类别和索引方向，按新 schema 重写 |
| S07 importer | `importer.py` | 参考 batch size 和 stats flow，用 Rust streaming 重写 |
| S09 reachability | `reachability.py` | 参考 visited/depth/truncation，补 full cache key |
| S10 object query | `analysis.py` | 参考查询结果形态，重写为 typed Rust queries |
| S11 diff | `analysis.py` diff functions | 参考聚合 diff，新增 lifecycle confidence |
| S12 SQL/idset | `cli.py` + `analysis.py` | 参考 Agent 原子能力，正式化 query contract |
| S13 findings/report | `analysis.py` + `report.py` | 参考 sections 和 evidence，补 kind enum |
| S20-S23 Web | `web.py` | 参考页面和用户路径，不复用实现 |

