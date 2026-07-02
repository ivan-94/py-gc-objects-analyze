# py-gc-objects-analyze 开发任务切片

本文把当前 docs 规范拆成可执行开发 slice 和 todo list。每个 slice 都应能独立验收，后续可以直接转成 issue、PRD slice 或多 Agent 任务。

## Source Manifest

任务来源：

- [docs/README.md](docs/README.md)
- [docs/dump-format.md](docs/dump-format.md)
- [docs/sqlite-schema.md](docs/sqlite-schema.md)
- [docs/api.md](docs/api.md)
- [docs/cli.md](docs/cli.md)
- [docs/analysis-model.md](docs/analysis-model.md)
- [docs/web-ui.md](docs/web-ui.md)
- [docs/performance.md](docs/performance.md)
- [docs/testing.md](docs/testing.md)
- [docs/project/engineering-standards.md](docs/project/engineering-standards.md)
- [docs/project/implementation-blueprint.md](docs/project/implementation-blueprint.md)
- [docs/project/poc-migration-guide.md](docs/project/poc-migration-guide.md)
- [docs/poc-retrospective.md](docs/poc-retrospective.md)

已确认边界：

- Python 端只负责低侵入 dump。
- Rust 端负责 CLI、导入、索引、聚合、分析、API server。
- React + shadcn/ui 负责本地 Web UI。
- SQLite 是临时、可重建分析产物，默认每次 import/open 重建。
- 主入口是 `pygco open <dump...>`。
- 发布版 Web UI 静态资源嵌入 Rust binary。

## 状态标记

- `[ ]` 未开始
- `[~]` 进行中
- `[x]` 完成
- `P0` 第一条可用闭环必须完成
- `P1` 第一版必须完成
- `P2` 第一版后段或 hardening 完成

## 依赖总览

```text
S00 docs contracts
  -> S01 repo scaffold
  -> S02 fixtures
  -> S03 Python producer
  -> S04 dump-format crate
  -> S05 SQLite store
  -> S06 importer
  -> S07 analysis core
  -> S08 CLI import/summary
  -> S09 CLI object/query/diff
  -> S10 API server
  -> S11 Web shell
  -> S12 Web overview/objects
  -> S13 Web detail/graph/diff/findings/sql/report
  -> S14 pygco open
  -> S15 performance/bench
  -> S16 packaging/release
```

## POC 迁移参考

POC 路径：

```text
/Users/ivan/.codex/worktrees/memory-analyzer/ai_glass
```

正式项目要复用 POC 的经验，而不是复制 POC 的架构。具体规则见 [docs/project/poc-migration-guide.md](docs/project/poc-migration-guide.md)。

| Slice | POC reference | Reuse | Avoid |
| --- | --- | --- | --- |
| S02 Golden Fixtures | `test/test_gc_heap_dump.py`, `test/test_memory_analyzer.py` | 小型对象图、stub/missing/cycle/diff 场景 | 直接使用业务真实 dump 作为 fixture |
| S03 Python producer | `debug_tools/gc_heap_dump.py` | gzip JSONL streaming、单飞锁、stub referents、默认 `collect=false`/`repr=false` | 继续放在业务项目 debug_tools 下 |
| S05 Dump Format | POC dump records | start/object/end record 语义 | 让字段随实现漂移 |
| S06 SQLite Store | `tools/memory_analyzer/store.py` | 表类别、索引方向、schema_meta 思路 | 旧 schema、只按 depth 缓存 reachability |
| S07 Importer | `tools/memory_analyzer/importer.py` | batch insert、type/module/cohort stats flow、missing/stub 计数 | Python 聚合实现、未 scoped 的 object id 假设 |
| S09 Reachability | `tools/memory_analyzer/reachability.py` | visited set、防循环、depth/limit/truncated | 只用 depth 做 cache key |
| S10 Object Queries | `tools/memory_analyzer/analysis.py` | object detail、edges、paths、missing、stubs 的结果形态 | CLI/API 直接拼 SQL |
| S11 Diff | `tools/memory_analyzer/analysis.py` | type/module/cohort diff、object lifecycle 思路 | 忽略 producer identity 直接相信 object id |
| S12 SQL/Idset | `tools/memory_analyzer/cli.py`, `analysis.py` | Agent 原子能力、只读 SQL、idset 操作 | 可写 SQL、不可执行示例 |
| S13 Findings/Report | `analysis.py`, `report.py` | evidence/action/report sections | 自由文本 kind、绝对诊断措辞 |
| S20-S23 Web | `tools/memory_analyzer/web.py` | 页面信息架构和用户路径 | FastAPI HTML 字符串、粗糙表格/图 |

实现任务如果发现 POC 与 docs 冲突，以 docs 为准；如果 POC 暴露 docs 未覆盖的真实问题，先更新 docs 和 task，再实现。

## Milestone 0: 文档与契约冻结

### S00. Contract Freeze

Priority: P0  
Owner: docs/architecture  
Depends on: none

Goal: 把当前文档作为第一版实现契约，避免实现期继续漂移。

Deliverables:

- [x] `docs/README.md` 链接全部有效。
- [x] `docs/dump-format.md`、`docs/sqlite-schema.md`、`docs/api.md` 被标记为 implementation contract。
- [x] `docs/project/source-manifest.md` 更新 cross-review 后修复来源。
- [x] 明确第一版不支持的内容：远程 SaaS、多用户权限、长期 SQLite migration、远程 attach。

Acceptance:

- [x] `rg -n "TODO|TBD|to_id=\\.\\.\\.|keep-session|profile-import|reachability-limit|edge_fanout" docs README.md` 无命中。
- [x] 文档入口覆盖 dump/schema/API/CLI/WebUI/testing/performance。

## Milestone 1: 工程骨架

### S01. Repository Scaffold

Priority: P0  
Owner: platform  
Depends on: S00

Goal: 建立 Rust workspace、Python package、Web app 和基础工具链。

Deliverables:

- [x] 根目录 `Cargo.toml` workspace。
- [x] `crates/pygco-cli`
- [x] `crates/pygco-dump-format`
- [x] `crates/pygco-store`
- [x] `crates/pygco-importer`
- [x] `crates/pygco-analysis`
- [x] `crates/pygco-api`
- [x] `crates/pygco-report`
- [x] `python/pygco_dump`
- [x] `web/app`
- [x] `fixtures/`
- [x] `benches/`
- [x] `.github/workflows/ci.yml` 或等价 CI。
- [x] `justfile` 或 `Makefile`，统一常用命令。
- [x] `Cargo.lock` 不在 `.gitignore` 中，binary workspace 应提交锁文件。

Acceptance:

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --workspace`
- [x] `python -m pytest python/`
- [x] `pnpm --dir web/app typecheck`
- [x] `pnpm --dir web/app build`

### S02. Golden Fixtures

Priority: P0  
Owner: test/data  
Depends on: S01

Goal: 建立最小可复现 dump 和 expected outputs，作为所有实现的固定验收输入。

POC reference:

- Reuse: `test/test_gc_heap_dump.py` 和 `test/test_memory_analyzer.py` 里的 tiny graph、stub、missing、diff 场景。
- Avoid: 不要把 POC 的真实业务 dump 或大文件直接放进 repo；fixture 应小、可读、可人工验证。

Deliverables:

- [x] `fixtures/golden/tiny-v1.jsonl.gz`
- [x] `fixtures/golden/stubs-v1.jsonl.gz`
- [x] `fixtures/golden/missing-referents-v1.jsonl.gz`
- [x] `fixtures/golden/cycles-v1.jsonl.gz`
- [x] `fixtures/golden/diff-before-v1.jsonl.gz`
- [x] `fixtures/golden/diff-after-v1.jsonl.gz`
- [x] `fixtures/golden/expected/*.json`
- [x] fixture generator 脚本，可重建 fixtures。

Acceptance:

- [x] 每个 dump 都符合 [docs/dump-format.md](docs/dump-format.md)。
- [x] expected 覆盖 summary、objects、edges、diff、reachability。
- [x] cycles fixture 能证明 reachable traversal 不会无限循环。
- [x] stubs/missing fixture 能同时覆盖 stub 和 missing 语义。

## Milestone 2: Python Dump Producer

### S03. `pygco-dump` Core Producer

Priority: P0  
Owner: Python producer  
Depends on: S02

Goal: 实现低侵入 Python dump 生成能力。

POC reference:

- Reuse: `debug_tools/gc_heap_dump.py` 的 `iter_gc_heap_dump_records()`、gzip streaming、`include_referent_stubs`、单飞锁、默认关闭 repr。
- Avoid: 不要保留 ai_glass FastAPI/debug_tools 耦合；正式包必须是独立 `pygco_dump`。

Deliverables:

- [x] `python/pygco_dump/pyproject.toml`
- [x] `pygco_dump/__init__.py`
- [x] `pygco_dump/core.py`
- [x] `iter_gc_dump_records()`
- [x] `write_gc_dump()`
- [x] gzip JSONL streaming writer。
- [x] `producer_run_id` 进程级 UUID。
- [x] `dump_sequence` 递增计数。
- [x] `process_started_at` best-effort。
- [x] `host_id` / `container_id` best-effort。
- [x] `collect=false` 默认值。
- [x] `include_repr=false` 默认值。
- [x] 单飞锁，防止同进程并发 dump。

Acceptance:

- [x] 输出 start/object/end records。
- [x] start metadata 包含 required 字段。
- [x] object record 包含 id/type/module/qualname/size/gc_tracked/stub/referents。
- [x] end metadata 包含 dumped_count/stub_count/total_object_records/elapsed_ms。
- [x] `repr` 只有显式开启才输出。
- [x] 单元测试覆盖 collect、referents、stubs、repr、并发锁。

### S04. Python Framework Helpers

Priority: P1  
Owner: Python producer  
Depends on: S03

Goal: 提供接入服务的 helper，但不强绑定框架。

Deliverables:

- [x] `pygco_dump.fastapi.gc_heap_dump_route()`
- [x] FastAPI optional extra。
- [x] plain WSGI/ASGI agnostic write example。
- [x] examples 目录。

Acceptance:

- [x] FastAPI TestClient 可下载 gzip JSONL。
- [x] helper 不引入 Rust analyzer 依赖。
- [x] endpoint 支持 collect/include_referents/include_referent_stubs/include_repr/repr_limit。

## Milestone 3: Rust Format, Store, Importer

### S05. Dump Format Crate

Priority: P0  
Owner: Rust dump-format  
Depends on: S02

Goal: Rust 端严格解析和验证 dump records。

POC reference:

- Reuse: POC 的 start/object/end record shape 和 stub/missing 语义。
- Avoid: 不要继续让 record 字段靠 Python dict 自由漂移；正式实现必须 serde model + version validation。

Deliverables:

- [x] `pygco-dump-format` serde models。
- [x] `MetadataStart`
- [x] `ObjectRecord`
- [x] `MetadataEnd`
- [x] version validation。
- [x] line-numbered parse errors。
- [x] object id string conversion helpers for JSON API。

Acceptance:

- [x] 拒绝未知 major version。
- [x] 接受同 major 下新增 optional 字段。
- [x] malformed JSONL 报行号。
- [x] golden dump parse tests 全通过。

### S06. SQLite Store Crate

Priority: P0  
Owner: Rust store  
Depends on: S05

Goal: 实现 [docs/sqlite-schema.md](docs/sqlite-schema.md) 的 SQLite schema 和 query 基础。

POC reference:

- Reuse: `store.py` 里的表类别、索引方向、schema_meta 思路。
- Avoid: 不要复制旧 `object_reachability_stats(snapshot_id, object_id, depth)` cache key；正式 schema 必须包含算法版本和所有参数。

Deliverables:

- [x] `pygco-store` crate。
- [x] schema create SQL。
- [x] `.tmp.sqlite` 构建和 atomic rename。
- [x] import pragmas。
- [x] prepared insert statements。
- [x] common row DTO。
- [x] schema version check。
- [x] read-only SQL guard。

Acceptance:

- [x] 建库后所有表和索引存在。
- [x] `object_id` 主键始终 scoped by `snapshot_id`。
- [x] import 失败删除半成品。
- [x] `PRAGMA query_only` 用于 SQL 探针。

### S07. Streaming Importer

Priority: P0  
Owner: Rust importer  
Depends on: S05, S06

Goal: 流式导入一个或多个 dump，生成 fresh analysis SQLite。

POC reference:

- Reuse: `importer.py` 的 objects/edges batch insert、type/module/cohort stats flow、missing/stub 计数。
- Avoid: 不要在内存里保留可无限增长的全量中间结构；不要把 stats 逻辑散落在 importer 和 analysis 多处。

Deliverables:

- [x] gzip streaming reader。
- [x] JSONL streaming parser。
- [x] sha256 计算。
- [x] multi snapshot import。
- [x] batch insert objects。
- [x] batch insert edges。
- [x] duplicate object id detection within snapshot。
- [x] missing referent counting。
- [x] import warnings。
- [x] import profile timings。

Acceptance:

- [x] `tiny-v1` import 成功。
- [x] `diff-before/after` 进入同一个 SQLite，snapshot id 为 1/2。
- [x] duplicate object id 在同 snapshot 内报错。
- [x] 同 object id 可存在于不同 snapshot。
- [x] import profile 包含 decode/parse/insert_objects/insert_edges/build_stats/build_indexes。

### S08. Base Stats Builder

Priority: P0  
Owner: Rust store/analysis  
Depends on: S07

Goal: 导入后构建基础聚合和索引。

Deliverables:

- [x] snapshot object/edge/stub/missing counts。
- [x] type_stats。
- [x] module_stats。
- [x] cohort_stats。
- [x] in/out degree support。
- [x] default cohort rules。
- [x] user cohort rules file loader。

Acceptance:

- [x] expected summary 与 golden expected 匹配。
- [x] type/module/cohort stats 支持 CLI/Web/API 排序。
- [x] stub 默认可过滤。

## Milestone 4: Analysis Core

### S09. Reachability Engine

Priority: P0  
Owner: Rust analysis  
Depends on: S07, S08

Goal: 实现 estimated reachable size，处理循环和截断。

POC reference:

- Reuse: `reachability.py` 的 visited set、防循环、depth/limit/truncated 行为。
- Avoid: 不要一次性加载不可控大图导致内存暴涨；不要把 shared object 归属解释成精确 ownership。

Deliverables:

- [x] `pygco-analysis` reachability module。
- [x] canonical params: direction/depth/node_limit/fanout_limit/algorithm_version。
- [x] visited set cycle handling。
- [x] truncated flag。
- [x] object_reachability cache write。
- [x] type_reachability_stats。
- [x] unavailable state for no-referents dump。

Acceptance:

- [x] cycles fixture 不无限循环。
- [x] shared object 不在同 root 里重复计数。
- [x] cache key 含全部参数。
- [x] CLI/Web 输出标注 estimated。

### S10. Object Query Engine

Priority: P0  
Owner: Rust analysis/store  
Depends on: S08, S09

Goal: 提供 objects/object/edges/paths/subgraph 的统一查询能力。

POC reference:

- Reuse: `analysis.py` 中 object detail、object_edges、object_paths、missing_referents、stub_objects 的结果字段思路。
- Avoid: 不要让 API/CLI/Web 各自拼查询；查询语义必须集中在 Rust analysis/store。

Deliverables:

- [x] paged object list query。
- [x] filters: q/type/module/cohort/size/degree/stub/missing。
- [x] sorts: object_id/type/module/shallow/reachable/in_edges/out_edges。
- [x] object detail query。
- [x] one-hop referents/referrers。
- [x] bounded paths。
- [x] local subgraph export model。

Acceptance:

- [x] 空字符串 filter 按 unset 处理。
- [x] object id JSON/API 序列化为 string。
- [x] missing edge 和 stub node 明确标记。
- [x] subgraph 必须有 depth/node/edge limit。

### S11. Diff and Lifecycle Engine

Priority: P1  
Owner: Rust analysis  
Depends on: S08, S09

Goal: 实现 snapshot diff 和 object lifecycle diff。

POC reference:

- Reuse: `analysis.py` 的 type/module/cohort delta 和 object lifecycle grouping。
- Avoid: 不要像 POC 一样只凭 object id 做 lifecycle 判断；必须先计算 confidence。

Deliverables:

- [x] summary delta。
- [x] type/module/cohort delta。
- [x] reachable size delta。
- [x] object lifecycle new/gone/retained/changed。
- [x] lifecycle confidence calculation。
- [x] aggregate-only mode when confidence insufficient。

Acceptance:

- [x] same producer_run_id + ordered dump_sequence gives high confidence。
- [x] missing process identity gives low/aggregate-only warning。
- [x] diff fixture expected 匹配。

### S12. SQL, Schema, Idset

Priority: P1  
Owner: Rust analysis/store  
Depends on: S06, S10

Goal: 支持高级只读探针和 object-id set operations。

POC reference:

- Reuse: POC CLI 的 `sql`、`schema`、`idset` Agent 原子能力。
- Avoid: 不要允许写 SQL；不要让 idset query 契约不明确。

Deliverables:

- [x] `schema` query。
- [x] read-only SQL query。
- [x] SQL explain。
- [x] elapsed_ms。
- [x] idset intersect/union/left-diff/right-diff/symdiff。
- [x] optional details hydrate。
- [x] saved idset Web support。

Acceptance:

- [x] 非 SELECT/WITH query 被拒绝。
- [x] 两侧 idset query 支持第一列或 `object_id` 列。
- [x] idset 示例可执行。

### S13. Findings and Reports

Priority: P1  
Owner: Rust report/analysis  
Depends on: S08, S09, S11

Goal: 生成启发式 leads 和 report。

POC reference:

- Reuse: `analysis.py` findings 的 evidence/action 结构和 `report.py` 的 section 组织。
- Avoid: 不要使用自由文本 kind；不要把 lead 写成已确认泄漏。

Deliverables:

- [x] finding kind enum。
- [x] severity enum。
- [x] evidence JSON schema。
- [x] algorithm_version。
- [x] findings table writer。
- [x] Markdown report。
- [x] JSON report。

Acceptance:

- [x] 不使用 "leak confirmed" 等绝对措辞。
- [x] findings 可按 kind/severity 过滤。
- [x] report 包含算法参数和 links。

## Milestone 5: CLI

### S14. CLI Foundation

Priority: P0  
Owner: Rust CLI  
Depends on: S01, S07

Goal: `pygco` binary 可运行，输出格式统一。

Deliverables:

- [x] clap command tree。
- [x] global output formats: json/jsonl/table/markdown。
- [x] `--fields`
- [x] `--limit`
- [x] `--offset`
- [x] no-color/verbose。
- [x] structured errors and exit codes。

Acceptance:

- [x] `pygco --help` 可读。
- [x] 所有 error 带 code/message。
- [x] json/jsonl 输出稳定。

### S15. CLI Import, Summary, Web

Priority: P0  
Owner: Rust CLI  
Depends on: S07, S08

Goal: 完成显式导入和基础分析流程。

Deliverables:

- [x] `pygco import`
- [x] `pygco summary`
- [x] `pygco doctor`
- [x] `pygco web`
- [x] `--rebuild`
- [x] `--profile`
- [x] `--no-reachability`
- [x] `--reachability-mode`
- [x] `--reachability-depth`
- [x] `--reachability-node-limit`
- [x] `--reachability-fanout-limit`

Acceptance:

- [x] `pygco import tiny-v1.jsonl.gz -o analysis.sqlite --rebuild`
- [x] `pygco summary analysis.sqlite --format json`
- [x] `pygco doctor analysis.sqlite`
- [x] import 默认 full reachability，除非显式关闭。

### S16. CLI Object and Analysis Commands

Priority: P1  
Owner: Rust CLI  
Depends on: S10, S11, S12, S13

Goal: 完成文档中的所有分析命令。

POC reference:

- Reuse: POC CLI 对 Agent 友好的 `jsonl`、`--fields`、`--ids-only` 设计。
- Avoid: 不要让 CLI command 名和 docs 漂移；CLI help 应能反向校验 docs。

Deliverables:

- [x] `pygco objects`
- [x] `pygco object`
- [x] `pygco edges`
- [x] `pygco paths`
- [x] `pygco diff`
- [x] `pygco diff-objects`
- [x] `pygco idset`
- [x] `pygco sql`
- [x] `pygco schema`
- [x] `pygco export-subgraph`
- [x] `pygco report`

Acceptance:

- [x] 每个命令有 JSON 和 JSONL tests。
- [x] idset 示例与 docs 保持同步。
- [x] diff-objects 展示 confidence warning。

## Milestone 6: Local API Server

### S17. API Server Foundation

Priority: P0  
Owner: Rust API  
Depends on: S06, S10

Goal: 本地 API server 可以服务 typed API 和静态 Web assets。

Deliverables:

- [x] `pygco-api` crate。
- [x] HTTP framework selection。
- [x] bind `127.0.0.1` default。
- [x] common response envelope。
- [x] common error envelope。
- [x] object id string serializer。
- [x] OpenAPI export。
- [x] static file serving hook。

Acceptance:

- [x] `GET /api/session`
- [x] `GET /api/snapshots`
- [x] OpenAPI JSON 可生成。
- [x] API errors match docs/api.md。

### S18. API Endpoints

Priority: P1  
Owner: Rust API  
Depends on: S17, S10, S11, S12, S13

Goal: 覆盖 Web UI 所需 endpoints。

Deliverables:

- [x] `/api/summary`
- [x] `/api/objects`
- [x] `/api/objects/{object_id}`
- [x] `/api/objects/{object_id}/edges`
- [x] `/api/objects/{object_id}/paths`
- [x] `/api/graph`
- [x] `/api/types`
- [x] `/api/modules`
- [x] `/api/cohorts`
- [x] `/api/diff`
- [x] `/api/diff/objects`
- [x] `/api/findings`
- [x] `/api/sql/query`
- [x] `/api/sql/explain`
- [x] `/api/idset`
- [x] `/api/schema`
- [x] `/api/report.md`
- [x] `/api/report.json`

Acceptance:

- [x] API integration tests use golden SQLite。
- [x] empty filter query params do not error。
- [x] graph endpoints enforce limits。

### S19. Long Running Jobs

Priority: P2  
Owner: Rust API  
Depends on: S18

Goal: 支持长查询、重计算、导出任务的进度和取消。

Deliverables:

- [x] job registry。
- [x] `GET /api/jobs/{job_id}`
- [x] `POST /api/jobs/{job_id}/cancel`
- [x] cancellation tokens。
- [x] progress state。

Acceptance:

- [x] expensive SQL 可取消。
- [x] full reachability recompute 可取消。
- [x] Web UI 能展示 progress。

## Milestone 7: Web UI

### S20. Web App Scaffold

Priority: P0  
Owner: Web  
Depends on: S01, S17

Goal: React app 可以连接本地 API，形成专业分析界面骨架。

POC reference:

- Reuse: `web.py` 的页面路由和分析路径：overview、objects、object detail、diff、findings、SQL、report。
- Avoid: 不要复用 FastAPI HTML 字符串、内联 JS、临时 CSS。

Deliverables:

- [x] Vite + React + TypeScript。
- [x] shadcn/ui setup。
- [x] Tailwind setup。
- [x] TanStack Router。
- [x] TanStack Query。
- [x] API client generation from OpenAPI。
- [x] App shell: top bar, left nav, content area。
- [x] snapshot selector。

Acceptance:

- [x] `pnpm lint`
- [x] `pnpm typecheck`
- [x] `pnpm build`
- [x] Web app can call `/api/session`。

### S21. Overview and Objects Pages

Priority: P0  
Owner: Web  
Depends on: S18, S20

Goal: 完成第一屏和对象列表，支撑主要探索流程。

Deliverables:

- [x] Overview page。
- [x] Objects page。
- [x] server-side filters。
- [x] server-side sorting。
- [x] pagination/virtualization。
- [x] URL state。
- [x] column resize。
- [x] estimated/truncated badges。
- [x] empty/loading/error states。

Acceptance:

- [x] Objects 空 filter 不触发 API error。
- [x] 长 type/module 不挤成竖排。
- [x] Objects 翻页目标 < 300ms on benchmark DB。

### S21.5. Aggregate Pages

Priority: P1
Owner: Web
Depends on: S18, S21

Goal: 完成 docs/web-ui.md 中的 Types / Modules / Cohorts 聚合维度页面。

Deliverables:

- [x] Types page。
- [x] Modules page。
- [x] Cohorts page。
- [x] count / shallow size / estimated reachable / max reachable 展示。
- [x] in/out edge totals for types/modules。
- [x] top example type summaries for cohorts。
- [x] diff delta columns when from/to snapshots are selected。
- [x] drill down to Objects with exact type/module/cohort filters。

Acceptance:

- [x] Left nav includes Types / Modules / Cohorts。
- [x] aggregate pages load from `/api/types` / `/api/modules` / `/api/cohorts`。
- [x] aggregate drill-down roundtrips through URL state into Objects filters。

### S22. Object Detail and Graph Pages

Priority: P1  
Owner: Web  
Depends on: S18, S21

Goal: 支持单对象钻取和局部引用图。

Deliverables:

- [x] object detail drawer。
- [x] referents table。
- [x] referrers table。
- [x] owner path samples。
- [x] local graph page。
- [x] missing edge style。
- [x] stub node style。
- [x] expand selected node。
- [x] export subgraph action。

Acceptance:

- [x] 图页面不空白。
- [x] graph 默认 depth <= 2，node limit <= 500。
- [x] missing/stub 可见且有 legend。

### S23. Diff, Findings, SQL, Report Pages

Priority: P1  
Owner: Web  
Depends on: S18, S19, S21

Goal: 覆盖剩余专业分析页面。

Deliverables:

- [x] Diff page。
- [x] Diff objects table。
- [x] lifecycle confidence banner。
- [x] Findings table。
- [x] evidence drawer。
- [x] SQL page。
- [x] schema browser。
- [x] explain plan drawer。
- [x] idset save/use action。
- [x] Report page。

Acceptance:

- [x] Findings evidence 不撑破列宽。
- [x] SQL 只读错误展示可行动信息。
- [x] Diff confidence 不足时提示 aggregate-only。

## Milestone 8: `pygco open` End-to-End

### S24. One Command Open Flow

Priority: P0  
Owner: CLI/API/Web integration  
Depends on: S15, S17, S20

Goal: `pygco open dump...` 一条命令完成导入、启动 API、打开 Web UI。

Deliverables:

- [x] session dir creation。
- [x] `.pygco/sessions/<timestamp>/analysis.sqlite`
- [x] import log。
- [x] dynamic port selection。
- [x] local browser open。
- [x] `--no-browser`
- [x] `--cleanup-on-exit`
- [x] embedded static assets in release mode。
- [x] dev mode proxy to React dev server。

Acceptance:

- [x] `pygco open fixtures/golden/tiny-v1.jsonl.gz --no-browser` 启动成功。
- [x] API `/api/session` 返回 session info。
- [x] Web UI can load overview。
- [x] default keeps session after exit。

## Milestone 9: Performance and Reliability

### S25. Import Benchmarks

Priority: P1  
Owner: performance  
Depends on: S07, S08

Goal: 建立导入性能基准。

Deliverables:

- [x] synthetic dump generator。
- [x] medium fixture。
- [x] large fixture。
- [x] pathological fixture。
- [x] benchmark harness。
- [x] import throughput report。

Acceptance:

- [x] benchmark records decode/parse/insert/index/stats/reachability。
- [x] memory usage 不随 dump size 线性增长。

### S26. Query and API Benchmarks

Priority: P1  
Owner: performance  
Depends on: S18, S21

Goal: 验证 query budget 和 Web API latency。

Deliverables:

- [x] summary query benchmark。
- [x] objects page benchmark。
- [x] object detail benchmark。
- [x] graph query benchmark。
- [x] SQL explain benchmark。
- [x] API p95 report。

Acceptance:

- [x] Objects page target < 300ms on benchmark DB。
- [x] Overview target < 1s on benchmark DB。
- [x] graph target < 1.5s with default limits。

### S27. Runtime Safety and Failure Handling

Priority: P1  
Owner: platform  
Depends on: S03, S07, S17, S24

Goal: 保证工具失败时可理解、可清理、不污染主流程。

Deliverables:

- [x] producer single-flight tests。
- [x] import failure cleanup。
- [x] malformed dump line error。
- [x] API invalid filter error。
- [x] SQL read-only violation error。
- [x] graph truncation warnings。
- [x] CLI exit code tests。

Acceptance:

- [x] 每类错误包含 code/message/details。
- [x] import 半成品不会留下目标 SQLite。
- [x] Web UI error state 告诉用户下一步。

## Milestone 10: Packaging and Release

### S28. Rust Binary Release

Priority: P1  
Owner: release  
Depends on: S24

Goal: 发布可离线使用的 `pygco` binary。

Deliverables:

- [x] release build。
- [x] embedded Web UI assets。
- [x] version command。
- [x] changelog。
- [x] install instructions。

Acceptance:

- [x] clean machine can run `pygco open tiny-v1.jsonl.gz`。
- [x] static assets served from binary。

### S29. Python Package Release

Priority: P1  
Owner: release/Python  
Depends on: S03, S04

Goal: 发布 `pygco-dump`。

Deliverables:

- [x] package metadata。
- [x] optional `fastapi` extra。
- [x] README snippet。
- [x] wheel build。
- [x] sdist build。

Acceptance:

- [x] `pip install pygco-dump[fastapi]` works in clean venv。
- [x] FastAPI helper example works。

### S30. Documentation Release Pass

Priority: P1  
Owner: docs  
Depends on: S28, S29

Goal: 文档与实际命令/API/行为保持一致。

Deliverables:

- [x] quickstart commands verified。
- [x] CLI docs generated or synced from clap。
- [x] API docs synced from OpenAPI。
- [x] screenshots or short Web UI walkthrough。
- [x] performance numbers inserted。
- [x] known limitations updated。

Acceptance:

- [x] 新用户按 quickstart 能完成 first analysis。
- [x] docs 不含 dead command 或 stale flag。

## Cross-Cutting Todo List

### Correctness

- [x] All object ids scoped by snapshot id in Rust queries.
- [x] All JSON/API object ids serialized as string.
- [x] All cache tables include algorithm_version and parameters.
- [x] Lifecycle confidence calculated before showing object-level diff conclusions.
- [x] Stub and missing referent states never conflated.

### Performance

- [x] Import is streaming from gzip to batch insert.
- [x] Heavy indexes created after batch insert.
- [x] Large table endpoints are paginated or virtualized.
- [x] Graph endpoints enforce depth/node/edge limits.
- [x] Long jobs support cancellation.

### UX

- [x] Overview first screen loads without a landing page.
- [x] Objects table text does not wrap into vertical words.
- [x] Evidence JSON opens in drawer.
- [x] Estimated values and truncated values are visibly labeled.
- [x] URL state roundtrips filters/sort/pagination/snapshot.

### Testing

- [x] Golden fixtures exist before main implementation.
- [x] CLI command outputs have snapshot tests.
- [x] API endpoints have integration tests.
- [x] Web UI has Playwright smoke tests.
- [x] Benchmarks run locally and small subset runs in CI.

### Release

- [x] `pygco` binary ships embedded Web UI.
- [x] `pygco-dump` package ships independently.
- [x] Release notes mention dump/schema/algorithm versions.
- [x] Quickstart verified on clean checkout.

## Suggested First Implementation Order

1. S01 Repository Scaffold
2. S02 Golden Fixtures
3. S03 Python Producer Core
4. S05 Dump Format Crate
5. S06 SQLite Store Crate
6. S07 Streaming Importer
7. S08 Base Stats Builder
8. S09 Reachability Engine
9. S14 CLI Foundation
10. S15 CLI Import/Summary/Doctor
11. S17 API Server Foundation
12. S20 Web App Scaffold
13. S24 `pygco open` End-to-End

This order gets to a real vertical tracer bullet quickly: `dump -> import -> SQLite -> summary -> API -> Web overview`.
