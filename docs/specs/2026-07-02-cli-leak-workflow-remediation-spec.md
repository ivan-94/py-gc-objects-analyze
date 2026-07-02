# CLI 泄漏排查工作流整改 Spec

## Source Manifest

### Sources

- 用户反馈：CLI 底层能力够用，但从 dump 到 leak 结论仍需手写 SQL，提出 URL fetch/open、table 输出、annotated paths、leak-candidates、external referrers、report、dump 质量提示、container analyzer、import progress、profile 语义等改进点。
- 用户要求：统一放到 `docs/specs/` 下，使用中文；涵盖变更范围；把文档驱动开发和 TDD 开发写入 `AGENTS.md`；系统性 review 这些改动点，避免只单点修命令。
- Cross-review 结果：外部 Claude Code 和同环境 self-review 均认为总体方向正确，但 P0 包过大；需收窄 `snapshot_metadata`、`trace`、`container`、`overview`、table renderer、import progress/profile 等范围。
- 本轮已读跨 Agent 规则：`~/.agents/docs/agents/workflows.md`、`~/.agents/docs/agents/handoff-policy.md`。
- 项目文档：[README](../../README.md)、[文档入口](../README.md)、[CLI 规范](../cli.md)、[CLI 诊断工作台整改方案](../cli-diagnostics-workbench.md)、[CLI 诊断工作台技术实施 Spec](../project/cli-diagnostics-technical-spec.md)、[SQLite Schema 规范](../sqlite-schema.md)、[Dump Format](../dump-format.md)、[运行安全边界](../runtime-safety.md)、[测试策略](../testing.md)、[Source Manifest](../project/source-manifest.md)。
- 当前实现：[crates/pygco-cli/src/main.rs](../../crates/pygco-cli/src/main.rs)、[crates/pygco-analysis/src/lib.rs](../../crates/pygco-analysis/src/lib.rs)、[crates/pygco-importer/src/lib.rs](../../crates/pygco-importer/src/lib.rs)、[crates/pygco-store/src/lib.rs](../../crates/pygco-store/src/lib.rs)、[crates/pygco-dump-format/src/lib.rs](../../crates/pygco-dump-format/src/lib.rs)。

### Produced Artifacts

- `docs/specs/2026-07-02-cli-leak-workflow-remediation-spec.md`
- `AGENTS.md`
- `docs/README.md` 文档入口更新

### Key Decisions

- 保留 `suspects` 作为候选线索的统一模型，不把 `leak-candidates` 另建成一套事实体系；`pygco leak` 是否需要作为 alias 保持开放问题。
- 把用户反馈抽象为 CLI 全局诊断能力整改，而不是逐条给某个命令打补丁。
- P0 收窄为“质量事实、报告合并 suspects、对象外部入边、路径注解、诊断命令表格”这条最小系统主线；URL fetch/open、长任务体验、`trace` 独立命令和广义 container analyzer 放到后续批次。
- Dump quality 优先复用现有 `import_warnings` 机制；不在第一期新增一对一 `snapshot_metadata` 表。
- 所有持久化规格和后续 issue/PR/HAT 产物必须保留 Source Manifest。
- 项目开发原则固化为文档驱动开发和 TDD：先更新文档/规格，再以测试驱动实现。

### Verification Evidence

- 本 spec 为文档设计产物。推荐校验命令：
  - `python3 scripts/check_docs_commands.py`
  - `cargo test -p pygco-cli`
  - `cargo test -p pygco-analysis`
- 本轮自查重点：现有 `suspects`、`object_detail`、`paths`、`report`、import profile、snapshot schema、dump metadata、table renderer 是否支持这些整改方向。

### Open Questions / Risks

- 是否需要 `pygco leak` alias 取决于后续可用性测试；如需增加，命名只能表达 candidates / investigation，不能暗示单 dump 已确认泄漏。
- `external_in_edges` 大库性能可能需要物化表。
- container 分析受限于当前 dump 无 edge label、dict key、frame locals。
- URL fetch 涉及认证头、URL query、manifest 日志脱敏，必须单独设计安全边界。
- 默认阈值需要更多真实 dump 校准。

## 背景

本次反馈暴露的问题不是“缺一个命令”，而是 `pygco` CLI 仍偏数据访问层：用户能查对象、边、路径、SQL、报告，但从 dump 到 leak 线索需要自己组织调查步骤、写 SQL、解释限制条件。

面向“找 leak”的真实工作流，CLI 应直接回答：

- 这个 dump 的证据质量如何？
- 哪些对象、类型、模块、cohort 最值得查？
- 为什么它可疑？置信度和限制是什么？
- 谁引用了它，路径是否能直接读懂？
- 它是不是常见容器堆积？
- 下一条命令该跑什么？
- 报告能否复用同一套线索，而不是只列 generic finding？

## 目标

把 `pygco` 从“SQLite 查询入口”推进到“本地内存泄漏排查工作台”：

- 用户不写 SQL 也能完成 80% 常见 leak triage。
- 人类输出默认可读，机器输出稳定可解析。
- 单 dump 输出只表达 candidate/lead/suspect，不表达 confirmed leak。
- same-process 多 snapshot diff 才能提高到更强的增长证据。
- CLI、report、API、Web 后续复用同一套 diagnostic facts。

## 非目标

- 不用单个 dump 证明泄漏。
- 不删除 `pygco sql`，它仍是高级逃生口。
- 不在第一批改动里要求 Web UI 同步实现。
- 不假装知道 dump 没采集的信息，例如 dict key、字段名、局部变量名。
- 不引入长期 SQLite migration 体系；分析 DB 仍是可重建产物。

## 当前状态

已具备的基础：

- `pygco open` 支持本地 dump 导入并启动本地 Web/API。
- `pygco import` 写 SQLite，并记录 `source_uri`、`source_basename`、`dump_sha256`。
- `pygco findings` 已能列持久化 leads。
- `pygco suspects` 已有第一阶段：`orphan-retained`、`high-retained-root`、`truncated-root`、`type-footprint`、`metadata-heavy`、`cache-heavy`、`async-backlog`、`connection-heavy`。
- `orphan-retained` 内部已经计算 `self_edges` 和 `external_in_edges`。
- `paths` 能采样 referent/referrer 路径，但主要输出 object id。
- `report` 当前合并 `summary` 和 `findings`，但还没有合并 `suspects`。

主要缺口：

- dump 质量限制没有成为所有诊断输出的顶部上下文。
- table 输出仍像 `key=value` 日志，不像表格。
- 对象详情没有直接暴露 external incoming edge 语义。
- 引用路径缺少节点摘要，导致用户必须手动查每个 id。
- report 没有承接 leak triage 的高价值线索。
- container/resource 视图还没有成为一等能力。
- import/profile 体验偏工程日志，不够解释阶段语义；这是后续 UX 改进，不进入 P0。

## 系统性 Review：不要单点修命令

用户反馈里的 10 点可以抽象为 7 类横向能力。整改时应按“能力层”修，再映射到所有相关命令。

| 横向能力 | 用户反馈表现 | 不应只修的命令 | 应覆盖的命令/界面 |
| --- | --- | --- | --- |
| 输入获取与来源追踪 | 需要先 `curl` 再 import | 只给 `open` 支持 URL | `fetch`、`open`、session manifest、Source Manifest、report |
| 输出表格契约 | table 不像表格 | 只美化 `suspects` | `summary`、`objects`、`object`、`edges`、`paths`、`diff`、`findings`、`suspects`、`sessions`、未来 `overview/container` |
| 诊断事实层 | leak-candidates 手写 SQL | 新增一个孤立命令 | `suspects`、`overview`、`report`、API、Web 复用同一 facts |
| 对象注解 | paths 只有 object id | 只给 `paths --annotate` | `object`、`edges`、`paths`、`export-subgraph`、Web graph、report links |
| 证据质量提示 | `collect_before_dump=false`、`repr=false` 影响结论 | 只在 report 写一段 | `import` warning、`summary`、`overview`、`suspects`、`object`、`paths`、`report` |
| 资源/容器解释 | deque/queue/cache 需要 SQL | 只做 `container` | `suspects` resource kinds、`container`、`overview` cohort summary、report |
| 长任务可观测性 | import 像卡住、profile 难读 | 只加 spinner | import progress、profile schema、import log、session manifest、doctor/report |

因此，后续 issue 拆分不能写成“给 paths 加 annotate”这种窄切片，而应写成“建立对象注解 facts，并让 paths/object/report 复用”。这样能避免每个命令各自拼 SQL、各自解释字段、各自生成不一致的输出。

## 统一设计原则

### 1. Diagnostic Facts 优先

先在 `pygco-analysis` 中建立可复用 facts，再让 CLI 命令选择性展示。

核心 facts：

- `SnapshotQualityFacts`
- `ObjectAnnotation`
- `ExternalRefFacts`
- `Suspect`
- `ContainerFacts`
- `ResourceCohortFacts`
- `OutputLimitations`
- `NextCommand`

命令不应各自重新解释对象、路径、质量、suspect。CLI、report、API、Web 都应消费同一 facts。

### 2. 输出分层

- JSON：稳定结构、原始数字、object id 为 string、包含 limitations。
- table：默认精简字段、aligned columns、human bytes。
- markdown：用于报告和 issue/PR/HAT，保留证据链和下一步命令。

### 3. 质量上下文前置

凡是会影响结论可信度的 dump 质量信息，都应出现在相关命令输出顶部或 metadata 中。

### 4. 候选不是结论

所有 leak 相关输出必须使用：

- candidate
- suspect
- lead
- confidence
- limitation

禁止单 dump 输出 confirmed leak。

### 5. 文档驱动 + TDD

后续任何实现切片都必须：

1. 先更新 spec/CLI 文档/数据契约。
2. 写失败测试，覆盖 JSON contract、table output、错误路径和关键算法。
3. 再实现功能。
4. 跑相关测试和文档命令校验。
5. 在 PR/HAT 产物中记录 Source Manifest 和验证证据。

此原则同步写入根目录 `AGENTS.md`。

## P0：先让现有排查链路可用

### P0.1 Dump Quality Facts 与质量横幅

新增可复用质量事实层，覆盖：

| Fact | 来源 | 严重度 | 影响 |
| --- | --- | --- | --- |
| `collect_before_dump=false` | dump metadata | warn | orphan-retained 可能包含 GC 前未回收循环垃圾 |
| `include_repr=false` | dump metadata | info | 无法看字符串内容、repr、部分 dict key 线索 |
| `include_referents=false` | dump metadata | warn | reachable、paths、retainment 估计不可用或较弱 |
| `include_referent_stubs=false` | dump metadata | info/warn | missing referent 影响图完整性 |
| no edge labels | 当前格式能力 | info | 无法显示字段名、dict key、list index、局部变量名 |
| high stub ratio | imported stats | warn | 类型/大小结论可能不完整 |
| missing referents | imported stats | warn | 部分边指向未导入对象 |
| reachability disabled/unavailable | import options / warning | warn | reachable 排名和 retainment 类线索不可用 |

覆盖命令：

- `import`：把核心质量风险写入 `import_warnings`。
- `summary`：顶部展示 quality。
- `overview`：顶部展示 quality。
- `suspects`：metadata 中携带影响解释的 quality warnings。
- `object`：当 orphan/external ref 解释受质量影响时展示 limitation。
- `paths --annotate`：展示路径搜索和 edge label 限制。
- `report`：第一屏展示 quality。

承载方式：

- 第一选择：导入阶段根据 `MetadataStart` 和已导入统计写入 `import_warnings`，使用稳定 `code` 表达质量事实。
- `include_referents=false` 已有 `reachability_unavailable` 模式；其余质量事实沿用同一机制，例如 `collect_before_dump_false`、`repr_unavailable`、`edge_labels_unavailable`、`stub_ratio_high`。
- 如未来需要保留完整 producer metadata，仅考虑在 `snapshots` 增加 `raw_start_metadata_json` 这类调试字段；第一期不新增 `snapshot_metadata` 表。

旧 DB 降级：

- 没有新 quality warning 时返回空 warnings，而不是失败。
- 若未来增加 raw metadata 字段，旧 DB 缺字段时输出 `quality_status=partial` 或 `unknown`，并保持 `summary/report` 可用。

### P0.2 统一表格输出

建立统一 table renderer，而不是每个命令手写。

要求：

- 支持 command-specific 默认字段。
- `--fields` 优先于默认字段。
- 文本左对齐，数字和 bytes 右对齐。
- long text 在人类输出中截断，JSON 不截断。
- nested fields 支持稳定 flatten，例如 `subject.object_id`、`metrics.estimated_reachable_size`。

第一批只迁移 leak triage 主线命令，避免一次性改变所有 CLI 的人类输出。后续再逐步迁移其他命令。

第一批默认列：

| 命令 | 默认 table 字段 |
| --- | --- |
| `suspects` | `rank,kind,severity,confidence,subject,estimated_reachable_size,reason,next_command` |
| `findings` | `severity,kind,title,action` |
| `object` | object key/value summary + referents/referrers table |
| `paths --annotate` | `path,depth,object_id,type,module,shallow_size,estimated_reachable_size,external_in_edges` |

后续迁移：

- `summary`、`objects`、`edges`、`diff`、`diff-objects`、`sessions list` 仍应最终接入同一 renderer，但不作为 P0 验收门槛。

验收：

- `pygco suspects DB --format table` 在 120 列终端可扫读。
- `pygco objects ... --fields object_id,type,shallow_size --format table` 仍稳定工作。
- JSON/JSONL 输出不受人类表格截断影响。

### P0.3 对象外部入边语义

`pygco object` 增加：

- `self_edges`
- `external_in_edges`
- `external_referrer_count`，可选但建议同时做，避免重复边误导
- `is_orphan_retained_candidate`
- `orphan_retained_reason`
- `limitations`

定义：

```text
self_edges = count(edges where from_id = object_id and to_id = object_id)
external_in_edges = count(edges where to_id = object_id and from_id <> object_id)
external_referrer_count = count(distinct from_id where to_id = object_id and from_id <> object_id)
is_orphan_retained_candidate =
  external_in_edges == 0
  and estimated_reachable_size >= 1 MiB
  and stub == false
```

说明：

- 判定沿用现有 `orphan-retained` 语义：无外部 incoming edge。
- `external_referrer_count` 是辅助展示字段，用于让用户区分“边数量”和“不同 referrer 数量”；它不参与第一期 orphan 判定，避免引入额外 distinct 查询成本。

系统性覆盖：

- `suspects --kind orphan-retained` 使用同一计算。
- `object` 展示同一 facts。
- `paths --annotate` 的每个节点展示同一 facts。
- `export-subgraph` 可后续把该 facts 写入节点属性。
- Web Object Detail 后续复用 API facts。

### P0.4 对象注解与路径可读性

给 `paths` 增加兼容 flag：

```text
pygco paths DB --id OBJECT_ID --direction referrers --annotate --format table
```

`paths --annotate` 同时承担事实注解、基础 interpretation 和 next command。暂不新增独立 `trace` 命令，避免与 `paths --annotate` 形成重复命令面。

节点注解字段：

- `object_id`
- `type`
- `module`
- `qualname`
- `shallow_size`
- `estimated_reachable_size`
- `in_edges`
- `out_edges`
- `self_edges`
- `external_in_edges`
- `external_referrer_count`
- `stub`
- `missing`

必须输出 limitations：

- path search is bounded by depth/fanout/limit。
- no path found does not prove no path exists。
- no edge labels means owner field/key/local name is unavailable。

JSON 契约：

```json
{
  "object_id": "281470886362416",
  "direction": "referrers",
  "annotated": true,
  "paths": [
    {
      "path_index": 0,
      "nodes": [
        {
          "depth": 0,
          "object_id": "281470886362416",
          "type": "generator",
          "module": "builtins",
          "shallow_size": 112,
          "estimated_reachable_size": 20971520,
          "external_in_edges": 0,
          "external_referrer_count": 0,
          "stub": false,
          "missing": false
        }
      ],
      "interpretation": [
        "No external referrer was found in this bounded sample."
      ],
      "next_commands": [
        "pygco object DB --snapshot 1 --id 281470886362416 --format json"
      ]
    }
  ],
  "limitations": []
}
```

Table 契约：

- `--format table` 输出 flattened path-node rows，每行表示一个 path 中的一个 node。
- 推荐列：`path_index,depth,object_id,type,module,shallow_size,estimated_reachable_size,external_in_edges,interpretation`。
- interpretation 可只在 path 起点或终点行展示，避免重复刷屏。

### P0.5 Report 合并 suspects

`pygco report` 默认包含：

1. Quality。
2. Snapshot summary。
3. Top suspects。
4. Top persisted findings。
5. Limitations。
6. Next commands。
7. Algorithm parameters。

排序规则：

1. `warn` before `info`。
2. `confidence high > medium > low`。
3. `orphan-retained`、`cache-heavy`、`async-backlog`、`connection-heavy` 优先于 generic metadata。
4. 相同级别按 `estimated_reachable_size` 或 `shallow_size_sum` 降序。

系统性覆盖：

- `report_json` 使用与 `suspects` 相同的 `Suspect` 结构。
- suspect 排序归 `pygco-analysis::suspects()` 或同一 analysis-layer helper 所有。
- markdown report 只做展示，不重新计算或改写解释。
- `suspects` 与 `report` 的 top 1 suspect 必须一致。
- API/Web 后续从同一 report/facts 入口取数据。

## P1：建立一等诊断入口

### P1.1 `overview`

目标：

```text
pygco overview DB --snapshot 1 --format table
pygco overview DB --snapshot 1 --format json
```

输出 section：

1. Quality。
2. Snapshot。
3. Top suspects summary，默认只使用已持久化或轻量 facts。
4. Top non-builtin types。
5. Top non-builtin modules。
6. Resource cohorts。
7. Next commands。

预算：

- 百万对象级真实库 `< 1s`。

约束：

- `overview` 默认不能运行可能超过预算的 heavy suspect 查询。
- 如果 top suspects 需要 `suspects --kind orphan-retained` 这类重查询，`overview` 应展示“heavy suspects omitted” limitation 和 next command。
- 后续可以增加 `--with-suspects` 或依赖物化 stats 后再展示完整 top suspects。

### P1.2 `container`

目标：

```text
pygco container DB --id OBJECT_ID --snapshot 1 --top-items --item-types --format table
```

第一批支持：

- `collections.deque`
- `queue.Queue`
- LRU/cache-like objects

输出：

- container object facts。
- direct referent count。
- item type aggregation。
- top direct items by shallow size。
- estimated reachable summary。
- limitations：无 edge labels、无 dict keys、无 frame locals。

系统性覆盖：

- `container` facts 应被 `suspects cache-heavy`、`overview`、`report` 复用。
- 第一期使用 type-name / module pattern 的已知堆积模式 analyzer。
- `dict`、`list`、`set` 仅能做泛元素类型聚合，等待 edge labels、dict key samples 或 index 信息后再进入专用解释器。

## P2：输入获取与长任务体验

### P2.1 URL fetch/open

目标：

```text
pygco fetch https://example.com/gc-heap-dump -o dump.jsonl.gz
pygco open https://example.com/gc-heap-dump
```

要求：

- HTTP GET，支持 redirect。
- TLS 默认验证。
- 从 `Content-Disposition`、URL path 或 SHA-256 推断文件名。
- streaming 计算 SHA-256。
- 支持 `--header KEY=VALUE`、`--timeout`、`--max-bytes`。
- 不在日志、manifest、report、错误消息中泄露 secret header。
- session manifest 记录 original URL、final URL、local path、SHA-256、content length、fetch time。
- 默认隐藏敏感 query；`--verbose` 才展示完整诊断信息。
- 必须脱敏的 header/value 来源至少包括 `Authorization`、`Cookie`、`Set-Cookie`、`X-Api-Key`、`X-Auth-Token`。
- 错误消息不得原样输出包含 secret 的 response body；如需保留 body，必须截断并脱敏。

### P2.2 Help 与 discoverability

更新 help：

- “从 dump 找 leak candidates”的标准流程。
- URL dump 流程。
- JSON/JSONL 给 agent/script 使用。
- `overview`、`suspects`、`object`、`paths --annotate`、`container` 的 next-step 示例。

### P2.3 Import progress 与 profile 语义

`import` 增加：

```text
--progress auto|always|never
```

规则：

- 默认 `auto`，stderr 是 TTY 时展示。
- progress 只写 stderr，不能污染 JSON stdout。
- 阶段包括 read/decode、insert objects、insert edges、build stats、build indexes、reachability、object_list_metrics、findings、finalize。

profile 输出新增语义：

- `wall_time_ms`
- `self_time_ms`
- `nested`
- `snapshot_id`
- `phase_kind`

保留旧 profile 字段兼容，但文档说明不要把嵌套阶段机械相加。

### P2.4 API/Web 复用

CLI 语义稳定后：

- API 暴露 quality facts、suspects、annotated paths、container facts。
- Web 复用同一 facts。
- CLI 是 reference contract。

## 变更范围

### 文档

- `docs/specs/2026-07-02-cli-leak-workflow-remediation-spec.md`：本规格。
- `docs/README.md`：增加规格入口。
- `docs/cli.md`：实现时更新用户可运行命令。
- `docs/generated/cli-help.md`：实现后由脚本生成。
- `docs/sqlite-schema.md`：新增 raw metadata 字段或物化 stats 时更新。
- `docs/api.md` / `docs/web-ui.md`：API/Web 接入 facts 时更新。
- `AGENTS.md`：写入项目级文档驱动和 TDD 原则。

### Rust crates

- `pygco-dump-format`：必要时暴露 raw metadata 序列化辅助。
- `pygco-store`：优先复用 `import_warnings`；仅在需要时新增 raw metadata 字段或 `object_external_in_stats`。
- `pygco-importer`：生成质量 warnings；P2 再增加 progress/profile 语义。
- `pygco-analysis`：新增 facts 层、质量分析、对象注解、container/resource facts。
- `pygco-report`：复用 facts 和 suspects。
- `pygco-cli`：命令参数、表格输出、URL fetch/open。
- `pygco-api`：P2/P3 后暴露同一 facts。

### Python producer

第一阶段不要求改 producer 格式。后续如要更高质量 explain，可另开 spec 增加：

- edge labels
- dict key samples
- frame local names
- generator/function origin

## 测试策略

遵循 TDD：先写失败测试，再实现。

### Unit Tests

- quality facts from metadata。
- missing metadata fallback。
- self edge vs external incoming edge。
- orphan candidate flag。
- table field selection and alignment。
- byte human formatting。
- annotated path node enrichment。
- container item type aggregation。
- report suspect ordering。
- URL fetch secret redaction。

### CLI Contract Tests

- `summary --format json` 包含 quality。
- `suspects --format table` 有 aligned header 和默认字段。
- `object --format json` 包含 external ref facts。
- `paths --annotate --format json` 返回 node objects。
- `report --format markdown` 包含 Quality 和 Top Suspects。
- `suspects` 与 `report` 的 top 1 suspect 一致。
- P2 实现 progress 时，`import --progress never --format json` 不污染 stdout。
- P2 实现 fetch 时，`fetch/open` 的 manifest、import log、report、错误输出不包含 secret header 值。

### Golden Fixtures

优先复用现有 fixtures。必要时新增：

- self-cycle orphan candidate。
- external referrer prevents orphan。
- no forced GC metadata。
- no repr metadata。
- queue/deque/cache-like container with many string items。

### Manual Validation

若本地有真实大库，验证：

- 不写 SQL 能看到 top orphan candidate。
- report 与 suspects 的 top candidate 一致。
- annotated paths 可读。
- table 在 120 列终端可扫读。
- P0 性能预算可接受。

## 性能预算

| 操作 | 预算 |
| --- | ---: |
| `summary` quality facts | < 100 ms |
| `suspects --kind orphan-retained` | < 5 s |
| `object --id` with external refs | < 500 ms |
| `paths --annotate` defaults | < 1.5 s |
| `report` markdown | < 5 s |
| `overview` compact | < 1 s |
| `container --top-items` | < 1 s |

## 分阶段验收

### P0 验收

- `report` 包含 quality 和 top suspects。
- `object` 区分 self edge 和 external incoming edge。
- `paths --annotate` 展示路径节点摘要。
- `suspects --format table` 是 aligned table。
- `suspects` 与 `report` 的 top 1 suspect 一致。
- 现有 CLI contract tests 通过。

### P1 验收

- `overview` 能作为第一入口完成 compact triage。
- `container` 能解释常见容器堆积，不写 SQL。
- 一次真实 leak triage 可以通过 overview/suspects/object/paths/container 完成主要判断。

### P2 验收

- `fetch URL -o dump.jsonl.gz` 能下载、hash、记录来源。
- `open URL` 能创建正常 session。
- secret headers、敏感 URL query、错误 response body 不泄露到普通日志/manifest/report/错误输出。
- import progress 写 stderr，不破坏 JSON stdout。

## 后续 issue 拆分建议

| Issue | 建议标题 | 覆盖的横向能力 | 阶段 |
| --- | --- | --- | --- |
| 1 | Quality facts via `import_warnings` + report suspects | 证据质量提示、诊断事实层 | P0 |
| 2 | Object external ref facts + shared object annotation | 对象注解、诊断事实层 | P0 |
| 3 | `paths --annotate` JSON/table contract + interpretation | 对象注解、输出表格契约 | P0 |
| 4 | Diagnostic table renderer core + first adapters | 输出表格契约 | P0 |
| 5 | `overview` compact triage with cheap facts only | 诊断事实层、证据质量提示 | P1 |
| 6 | Deque/queue/cache container analyzer | 资源/容器解释、诊断事实层 | P1 |
| 7 | URL fetch/open + source manifest + redaction | 输入获取与来源追踪 | P2 |
| 8 | Import progress/profile semantics | 长任务可观测性 | P2 |

拆 issue 时每个 issue 都应写明：

- 修改哪些文档。
- 先写哪些失败测试。
- 涉及哪些命令，不只写一个命令。
- JSON contract 是否变更。
- 是否需要旧 DB graceful fallback。
