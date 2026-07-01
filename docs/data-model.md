# Dump 与 SQLite 数据模型

## 核心决策

`pygco` 把 dump 和 SQLite 明确分层：

```text
raw dumps
  -> fresh temporary analysis.sqlite
  -> CLI / Web UI analysis
  -> delete SQLite after use
```

- dump 是输入。
- SQLite 是临时、可重建、高性能分析产物。
- 默认每次导入都重建 SQLite。
- 不把长期存档、迁移兼容、权限模型作为第一版目标。

## 为什么默认重建 SQLite

这个工具服务于临时排查。默认重建比长期维护一个分析库更简单、更可靠：

- schema 可以随工具演进，不需要复杂 migration。
- 聚合缓存不会被旧算法污染。
- 导入流程可以按一次性批处理优化。
- diff 语义更明确：一次 session 内导入多个 snapshot，然后比较。
- 用户心智模型简单：保留 dump，需要时重新导入。

## Snapshot 模型

一个 SQLite 可以包含多个 snapshot：

```text
before.jsonl.gz -> snapshot 1
after.jsonl.gz  -> snapshot 2
                  analysis.sqlite
```

多 snapshot 支持：

- type/module/cohort diff
- object lifecycle diff
- trend-like comparison
- before/after report

## 文件关系

每个 snapshot 必须记录：

- `snapshot_id`
- dump source path 或 URI
- dump sha256
- dump format version
- producer name/version
- import analyzer version
- import options
- import started/finished time
- object count
- edge count
- shallow size sum

`snapshot_id` 是当前 analysis session 内的自增整数 handle。它可以出现在 CLI、URL 和 API 中，但不是跨 session 的稳定身份。跨 session 复现应使用 dump sha256、source URI 和 import options。

SQLite 不默认内嵌 dump 原文。

## 命令语义

推荐探索路径：

```bash
pygco open before.jsonl.gz after.jsonl.gz
```

默认 `pygco open` 会把生成的 `analysis.sqlite` 放在用户 cache root 下：

```text
PYGCO_HOME
XDG_CACHE_HOME/pygco
~/.cache/pygco
```

每个 cache session 包含 `analysis.sqlite`、`import.log` 和 `manifest.json`。这些 SQLite 文件仍是可重建缓存；长期保留原始 dump。

显式路径：

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
pygco web analysis.sqlite
```

显式 `pygco import -o <sqlite>` 不会自动注册为 cache session，路径和生命周期由调用方管理。

如果 `analysis.sqlite` 已存在：

- 默认报错。
- `--rebuild` 显式删除并重建。
- `--append` 可以作为后续高级能力，但不是主流程。

## 生命周期 diff 语义

Object id 来自 Python `id(obj)`。

| 场景 | object lifecycle diff 可信度 | 推荐分析方式 |
| --- | --- | --- |
| 同进程连续 dump | 高 | object id、type/module/cohort、reachable size |
| 同服务不同进程 | 中低 | type/module/cohort、owner chain、reachable size |
| 不同版本/不同机器 | 低 | 只看聚合和趋势线索 |

文档和 UI 必须在对象级 diff 页面提示这个限制。

## Process Identity

对象生命周期 diff 需要判断两个 snapshot 是否来自同一个 Python 进程生命周期。仅靠 pid 和时间不够，因为容器重启和 PID 复用会误导分析。

dump metadata 应记录：

- `producer_run_id`：Python producer 在进程启动时生成的 UUID。
- `dump_sequence`：同一 producer run 内递增的 dump 序号。
- `process_started_at`：进程启动时间，允许 best-effort。
- `host_id`：主机身份，允许配置或 best-effort。
- `container_id`：容器身份，允许为空。

CLI/WebUI 的 object lifecycle confidence：

| 条件 | confidence |
| --- | --- |
| `producer_run_id` 相同且 `dump_sequence` 递增 | high |
| producer run 缺失但 host/container/pid/process_started_at 一致 | medium |
| 只有 pid 或 created_at 可比 | low |
| 不同 host/container/run | aggregate-only |

`aggregate-only` 时，UI 不应默认展示 retained/new/gone object id 结论，只展示 type/module/cohort 聚合 diff。

## 可重建缓存

以下数据属于可重建缓存：

- type stats
- module stats
- cohort stats
- object reachability stats
- type reachability stats
- owner path samples
- query result cache

缓存必须记录算法版本和参数。算法或参数变化时应重算。

Reachability cache key 必须包含：

```text
snapshot_id
object_id
algorithm_version
direction
depth
node_limit
fanout_limit
```
