# 分析模型

本文定义 `pygco` 的分析语义。所有 UI 和 CLI 都必须遵循这些定义，避免把启发式线索展示成确定结论。

## 分析层级

`pygco` 的分析分为四层：

1. 原始对象和边：从 dump 导入。
2. 基础统计：type/module/cohort/object degree。
3. 图估算：reachable size、owner paths、subgraph。
4. 启发式线索：findings/leads。

越往上越接近调查建议，越不应该被表述成事实。

## Shallow Size

`shallow_size` 是对象本身的浅层大小。它不包含 referents。

用途：

- 找大容器对象。
- 排查大 list/dict/set/bytes-like 对象。
- 与 reachable size 对照。

限制：

- 不能解释完整 RSS。
- 不能表达对象被谁“拥有”。

## Reachable Size

`reachable_size` 是从某个对象沿 referents 做有界遍历得到的浅层 size 总和。

必须记录：

```text
depth
node_limit
fanout_limit
direction
algorithm_version
truncated
```

第一版 canonical 参数：

```text
direction = referents
depth = 3
node_limit = 10000
fanout_limit = 1000
algorithm_version = 1
```

cache key 必须包含上述所有参数。没有 referents 的 dump，reachability 状态为 `unavailable`。

循环处理：

- 遍历中使用 visited set。
- 同一次 reachable size 计算中，同一 object id 只计一次。
- 遇到循环不会无限递归。

共享对象处理：

- 每个 root 独立计算 reachable size。
- 不试图把共享对象的 size 精确归属给某个 owner。
- 因此多个 root 的 reachable size 相加可能大于全局 shallow size。

展示要求：

- 所有 reachable size 字段都必须标注为 estimated。
- 当 `truncated=true` 时，UI 必须显式显示截断标记。

## Referrer / Referent Graph

导入时只存储 `from_id -> to_id`。

查询时提供：

- referents：当前对象直接持有的对象。
- referrers：直接持有当前对象的对象。
- subgraph：从 root 出发的有限局部图。

Web UI 不允许渲染全量对象图。所有图视图都必须有 depth、node limit、edge limit。

## Missing Referents

missing referent 表示 edge 指向的 object id 没有 object record。

可能原因：

- referent 不在 `gc.get_objects()` 主集合中，producer 未输出 stub。
- 对象在 dump 过程中发生变化。
- producer 读取 referents 时出现异常或跳过。

missing 不是错误。它是图不完整性的显式标记。

## Stub Objects

stub object 是 producer 为非主集合 referent 输出的轻量 object record。

分析语义：

- 可以参与边展示。
- 可以参与 type/module 统计，但必须可过滤。
- 默认不参与完整 reachable size 深层展开。
- UI 必须以不同样式展示 stub。

## Owner Paths

owner path 是从目标对象沿 referrers 向上采样的持有路径。

它用于回答：

```text
这个对象可能被谁持有？
```

它不保证：

- 找到所有 owner。
- 找到唯一 root owner。
- 找到 Python GC root。
- 证明泄漏。

## Cohort

cohort 是基于 type/module/qualname 的规则聚合。

规则示例：

```toml
[[cohort]]
name = "streaming"
type_contains = ["ModelResponseStream", "StreamingChoices"]

[[cohort]]
name = "database_cache"
module_prefix = ["sqlalchemy."]
```

cohort 用途：

- 快速定位业务相关对象群。
- 让 findings 更可读。
- diff 时比较高层资源组。

## Findings / Leads

findings 是启发式线索，不是诊断结论。

每条 finding 必须包含：

- kind
- severity
- title
- message
- suggested action
- evidence
- algorithm_version

`evidence` 是结构化 JSON，schema 版本由 `schema_version` 标识。第一版 evidence 必须包含：

- `schema_version`
- `kind`
- `subject`
- `metrics`
- `links`

`links` 是 `{ label, href }` 数组，用于跳转到相关 Web UI/API 视图。结构化 report 必须暴露 `finding_evidence_schema`，供下游工具和 UI 校验 evidence。

第一版 `kind` 为封闭枚举：

```text
cohort_signal
large_type
large_object
high_out_degree
high_in_degree
missing_referents
stub_heavy_type
diff_growth
```

禁止使用绝对措辞：

- "leak confirmed"
- "root cause"
- "must be"

推荐措辞：

- "candidate"
- "lead"
- "worth inspecting"
- "compare with another snapshot"

## Diagnostic Facts / Suspects

CLI、report、API 和 Web UI 应共享同一组诊断事实层，而不是各自直接拼 SQL。

诊断事实层至少包含：

- `SnapshotFacts`：snapshot 规模、stub/missing/reachability 状态。
- `TypeFacts`：type count、shallow、degree、estimated reachable、truncated。
- `ModuleFacts`：module 聚合 footprint。
- `ObjectFacts`：单对象大小、degree、reachable、stub、missing。
- `CohortFacts`：cache、async、connection、threading、network、observability 等领域聚合。

suspect 是比 finding 更面向调查工作流的临时线索。它可以由 facts 即时生成，也可以在 report 中持久化。

第一批 suspect kind：

```text
orphan-retained
high-retained-root
truncated-root
type-footprint
metadata-heavy
cache-heavy
async-backlog
connection-heavy
stub-heavy
diff-growth
```

每条 suspect 必须包含：

- kind
- severity
- confidence
- subject
- metrics
- reason
- limitations
- next_command

suspect 不能证明泄漏。它只表示“下一步值得查”。`diff-growth` 在同进程连续 dump 中置信度最高；单 dump suspect 默认只能作为 medium 或 low confidence lead。

## Diff

聚合 diff 使用 type/module/cohort 作为主要维度。

对象级 lifecycle diff 使用 object id。它必须显示可信度提示：

- 同进程连续 dump：较可信。
- 跨进程 dump：弱信号。

## SQL

SQL 是高级探索能力。它必须是只读的，并且能通过 `schema` 和 `--explain` 被用户理解。

SQL 结果可以作为 idset、export、report 的输入。
