# CLI 诊断工作台整改方案

本文定义 `pygco` CLI 从“SQLite 查询入口”升级为“内存排查工作台”的目标状态。它不是当前命令帮助的替代品；当前已实现命令仍以 [CLI 规范](cli.md) 和 [Generated CLI Help](generated/cli-help.md) 为准。

## 背景

真实 dump 分析暴露出一个结构性问题：高价值结论主要来自手写 SQL，而不是来自 `pygco` 的一等命令。

这说明 CLI 当前只覆盖了数据访问层，尚未提供足够的诊断语义层。用户能拿到 objects、edges、paths、summary，但很难直接回答：

- 哪些对象最像泄漏或可回收垃圾？
- 哪些非 builtin 类型是主要 footprint？
- cache、connection、async task 有没有堆积？
- 一个大对象为什么可疑，下一步看什么？
- 单个 dump 只能说明“可疑”，需要怎样用 diff 判断增长？

整改目标是让 80% 的常见内存排查不需要用户知道 SQLite 表名。

## 设计原则

- 诊断语义优先：CLI 输出应回答调查问题，而不是只暴露表结构。
- SQL 保留为 escape hatch：`pygco sql` 仍然重要，但不应是主工作流。
- 默认 compact：终端默认输出高信号摘要，详细内容通过 `--verbose` 展开。
- 非 builtin 优先：默认视图应降低 `builtins`、stub 和解释器元数据噪音。
- 可疑点不是结论：所有 suspect/finding 必须使用 candidate/lead/confidence 语义，不能写成 confirmed leak。
- 每条线索给下一步命令：CLI 应输出可复制的 `next_command`。
- Web、report、CLI 复用同一诊断事实层，避免同一问题多套解释。

## 当前缺口

| 缺口 | 影响 | 目标状态 |
| --- | --- | --- |
| `findings` 已有但仍需更好呈现 | 已能直接读取 leads，但 table 输出还偏原始字段 | 优化 `pygco findings` 的人类输出 |
| `summary` 过大 | 真实 dump JSON 容易淹没重点 | `overview --compact` 分层输出 |
| 缺少非 builtin 排名 | 业务/框架 footprint 被 builtin 噪音盖住 | `rank/types/modules --non-builtin` |
| 可疑孤岛分析已初步落地 | 已能用 `suspects --kind orphan-retained` 找大对象孤岛 | 继续扩展 diff suspects 和 explain |
| `paths` 只输出 ID | 引用路径不可读 | `trace --verbose` 输出 type/module/size/reason |
| `object` 详情语义不足 | 大 dict/generator 仍需猜 | `explain --id` 按类型解释 |
| 没有 cache/async/connection 视图 | 资源泄漏排查要写 SQL | `cohorts cache|async|connection` |
| reachable sum 易误解 | 多 root 共享对象会重复计数 | 输出必须标注 estimated / overlapping |

## 目标命令组

目标 CLI 信息架构：

```text
overview   快速入口：snapshot 规模、stub/reachability 状态、top suspects、top non-builtin footprint
rank       排名入口：按 type/module/object 排 count/shallow/reachable/max-reachable
suspects   可疑点入口：孤岛、大 root、截断 root、cache、async、connection、metadata 等
explain    单对象解释：按对象类型生成语义摘要和下一步
trace      引用路径解释：比 paths 更人类可读
cohorts    领域视图：cache、async、connection、threading、network、observability
findings   已持久化 leads
diff       增长判断：type/module/cohort/object lifecycle
sql        高级只读逃生口
```

示例目标命令面：

```text
pygco overview DB --compact
pygco rank DB --by type --metric shallow --non-builtin
pygco rank DB --by module --metric reachable --non-builtin
pygco suspects DB --kind orphan-retained --min-reachable 1mb
pygco suspects DB --kind cache --kind async --kind connection
pygco explain DB --id OBJECT_ID
pygco trace DB --id OBJECT_ID --direction referrers --verbose
pygco cohorts DB cache
pygco findings DB
```

实现状态：

- 已实现：`findings`、`suspects` 的第一阶段版本。
- 未实现：`overview`、`rank`、`explain`、`trace`、`cohorts`、diff suspects。
- 未实现命令不得同步到 generated help。

## Suspect 分类

`suspects` 是 CLI 诊断工作台的核心。它基于启发式规则生成候选线索。

| kind | 触发条件 | 典型解释 | 置信度 |
| --- | --- | --- | --- |
| `orphan-retained` | 无外部 referrer 且 reachable 大 | 可能是 GC 前未回收循环垃圾，或 dump 期间临时孤岛 | medium |
| `high-retained-root` | 单对象 estimated reachable 大 | 可能是模块全局表、缓存、大容器或 root object | medium |
| `truncated-root` | reachable 遍历被 node/fanout limit 截断 | 需要提高参数或局部追踪 | low/medium |
| `type-footprint` | 非 builtin type count/shallow 高 | 可能是业务对象、schema、ORM metadata | medium |
| `metadata-heavy` | Pydantic/FastAPI/SQLAlchemy/typing 元数据高 | 常驻框架 footprint，需 diff 判断是否增长 | low/medium |
| `cache-heavy` | cache/pool/lru/in-memory cache 聚合高 | 可能是缓存容量或 eviction 问题 | medium |
| `async-backlog` | Task/Future/async_generator 数量或 reachable 高 | 可能是任务堆积或未完成协程 | medium |
| `connection-heavy` | Redis/MySQL/HTTP/DB connection/pool 数量高 | 可能是连接泄漏或池配置异常 | medium |
| `stub-heavy` | stub 比例高 | 结论可信度下降，需要提示 producer/dump 限制 | info |
| `diff-growth` | 两个 snapshot 间增长明显 | 单 dump 可疑，diff 才能接近泄漏判断 | high when same process |

每条 suspect 必须包含：

- `kind`
- `severity`
- `confidence`
- `subject`
- `metrics`
- `reason`
- `next_command`
- `limitations`

## Object Explain

`explain` 是 `object` 的诊断化版本。它不只列字段，还要解释对象像什么。

目标类型解释器：

| 类型 | 解释重点 |
| --- | --- |
| `dict` | 猜测 module globals、class namespace、instance `__dict__`、cache dict、大 mapping |
| `generator` | 是否自引用、是否无外部 referrer、是否持有大 list/set/frame-like referents |
| `function` | closure、globals、defaults、module、是否被大量对象引用 |
| `module` | module dict、top outgoing references、是否是大 root |
| `type` / class | class dict、MRO、Pydantic/ORM/schema 相关线索 |
| `list` / `set` | 容器大小、reachable、referrer 情况、是否为孤岛的一部分 |
| `Task` / `Future` | async 状态相关 referents、callback 链、堆积判断 |

当 dump 格式无法提供 dict key、frame locals 或容器元素语义时，CLI 必须明确说明限制，避免装作知道。

## Trace 输出要求

`trace` 应替代裸 `paths` 成为人类排查入口。

目标输出不是 ID 数组，而是带解释的路径：

```text
generator 281470886362416
  external referrers: 0
  self-cycle: yes
  retains set 16.0 MiB
  retains list 4.6 MiB
  interpretation: orphan-retained candidate
```

如果找不到外部 path，输出应解释：

```text
No external referrer path found within depth/fanout limits.
This object may be self-referenced, unreachable from sampled roots, or outside the sampled path budget.
```

## Overview 输出要求

`overview --compact` 应作为默认调查入口，输出：

- snapshot 规模：objects、edges、shallow、stub ratio、missing referents
- reachability 状态：available/unavailable、参数、truncated 概况
- top suspects：最多 5 条
- top non-builtin shallow types
- top non-builtin reachable modules
- cohort summary：cache、async、connection、threading
- suggested next commands

`summary` 可以保留为完整结构化输出，但不应承担人类入口职责。

## Diff-first 工作流

单个 dump 只能说明“当前占用”和“可疑线索”。泄漏判断需要连续 dump。

目标工作流：

```text
1. pygco overview warmup.sqlite --compact
2. pygco overview after-load.sqlite --compact
3. pygco diff warmup-after.sqlite --from 1 --to 2
4. pygco suspects warmup-after.sqlite --from 1 --to 2
```

diff suspects 应优先显示：

- type/module count growth
- shallow growth
- new retained roots
- orphan-retained growth
- cache/connection/async growth
- metadata-heavy growth

## 输出契约

人类输出：

- 默认 table/compact。
- byte 同时显示 human readable 单位。
- estimated reachable 必须标记为 estimated。
- truncate 必须显式显示。
- 每条线索输出 next command。

机器输出：

- JSON 保留 raw bytes。
- JSON 字段稳定，适合 agent 和脚本。
- 所有 object id 序列化为 string。
- `confidence`、`severity`、`kind` 使用封闭枚举。

## 验收标准

以 `local-25292-reachable.sqlite` 这类百万对象真实库为验收样本，达到以下结果：

- 不写 SQL 也能发现约 20 MiB 的 orphan generator candidate。
- 不写 SQL 也能确认 `cozepy` 本身不是主要 footprint。
- 不写 SQL 也能列出 Pydantic/FastAPI/SQLAlchemy/typing 是主要非 builtin metadata footprint。
- 不写 SQL 也能确认 cache、connection、async task 没有明显堆积。
- `overview --compact` 在百万对象库上小于 1 秒。
- 常用 `rank` / `cohorts` 小于 500 ms。
- 重型 `suspects` 小于 5 秒，超过时必须可 profile。

## 非目标

- 不把单个 dump 的 suspect 表述为 confirmed leak。
- 不实现远程 attach 或生产服务内在线分析。
- 不把 SQL 移除；SQL 仍是高级探索能力。
- 不要求 Web UI e2e 参与 CLI 快速迭代；CLI 变更以 Rust unit/contract/golden 测试为主。

## Open questions / risks

- Some object explainers need richer producer metadata, such as dict keys, frame locals, generator origin, or container element samples.
- Threshold defaults for suspects must be tuned against more real dumps.
- `estimated reachable sum` remains overlapping by design and must stay labeled as estimated.
