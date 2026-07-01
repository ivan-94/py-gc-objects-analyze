# CLI Reference

本文是当前版本 `pygco` 命令行的用户参考。它只描述已经存在、可以运行的命令；规划中的诊断命令见 [CLI 诊断工作台整改方案](cli-diagnostics-workbench.md) 和 [CLI 诊断工作台技术实施 Spec](project/cli-diagnostics-technical-spec.md)。

如果需要查看从二进制直接生成的 help 文本，见 [Generated CLI Help](generated/cli-help.md)。

## 基本模型

`pygco` 用来分析 Python GC object dump：

1. `pygco_dump` 产出 gzip JSONL dump。
2. `pygco import` 把一个或多个 dump 导入 SQLite。
3. 后续 `summary`、`objects`、`object`、`edges`、`paths`、`diff` 等命令都查询这个 SQLite。
4. `web` / `api` 在本地启动服务，使用同一个 SQLite。

常用流程：

```bash
pygco import dump.jsonl.gz -o analysis.sqlite --rebuild
pygco summary analysis.sqlite --snapshot 1 --format table
pygco objects analysis.sqlite --snapshot 1 --sort reachable-size --limit 20 --format table
pygco web analysis.sqlite
```

一次性导入并打开 Web UI：

```bash
pygco open dump.jsonl.gz --profile
```

多份 dump 会成为同一个 SQLite 里的多个 snapshot：

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
pygco diff analysis.sqlite --from 1 --to 2 --format markdown
```

## 当前命令一览

| 命令 | 用途 |
| --- | --- |
| `open` | 导入 dump，启动本地服务，并可自动打开浏览器 |
| `import` | 把一个或多个 dump 导入 SQLite |
| `sessions` | 查看 `pygco open` 创建的缓存分析 session |
| `summary` | 查看 snapshot 概览、top type/module/cohort、warning/finding |
| `objects` | 查询对象列表，支持过滤、排序、分页 |
| `object` | 查看单个对象的详情、直接 referents/referrers |
| `edges` | 查询单跳引用关系 |
| `paths` | 对 referent/referrer 路径做有界采样 |
| `diff` | 对比两个 snapshot 的聚合变化 |
| `diff-objects` | 对比两个 snapshot 的对象生命周期 |
| `findings` | 直接列出持久化诊断 leads |
| `suspects` | 按启发式规则生成内存排查线索 |
| `idset` | 对两组 object id SQL 查询做集合运算 |
| `sql` | 对分析库执行只读 SQL |
| `schema` | 查看分析库 schema 元数据 |
| `export-subgraph` | 导出某个对象附近的有界子图 |
| `report` | 生成 markdown/json 报告 |
| `doctor` | 检查分析库健康状态 |
| `web` | 为已有 SQLite 启动本地 Web UI |
| `api` | 为已有 SQLite 启动本地 API server |
| `version` | 打印版本 |

## 全局选项和输出格式

全局选项放在子命令前：

```text
--no-color
--verbose
-h, --help
-V, --version
```

多数查询类命令支持：

```text
--format json|jsonl|table|markdown
--fields <comma-separated-fields>
--limit <n>
--snapshot <id>
```

输出格式：

| 格式 | 适合场景 |
| --- | --- |
| `json` | 默认机器可读输出，适合脚本、Agent、API 对照 |
| `jsonl` | 行式输出，适合管道处理 |
| `table` | 终端快速查看 |
| `markdown` | 复制到报告、issue、PR 说明 |

约定：

- JSON/API 风格输出里的 object id 使用字符串，避免 JavaScript 大整数精度问题。
- `--fields` 用于输出字段投影；字段名以该命令实际 JSON 输出为准。
- `--verbose` 适合排查失败，会输出更多错误链路信息。
- `--no-color` 适合日志、CI、Agent 解析。

退出码：

| Code | 含义 |
| ---: | --- |
| `2` | CLI 参数或用法错误 |
| `10` | dump 格式错误 |
| `11` | import 失败 |
| `20` | query 失败 |
| `70` | 内部错误 |

## `pygco open`

导入一个或多个 dump，启动本地服务，并按配置打开 Web UI。

```bash
pygco open dump.jsonl.gz
```

导入多份 dump 并输出 import profiling：

```bash
pygco open before.jsonl.gz after.jsonl.gz --profile
```

常用参数：

```text
--session-dir <path>
--host <host>                   default: 127.0.0.1
--port <port>                   default: 0
--no-browser
--dev
--dev-server-url <url>          default: http://127.0.0.1:5173/
--cleanup-on-exit
--profile
```

语义：

- 不指定 `--session-dir` 时，`open` 会在用户 cache root 下创建 session 目录保存 SQLite。
- cache root 解析顺序是 `PYGCO_HOME`、`XDG_CACHE_HOME/pygco`、`~/.cache/pygco`。
- 默认 session 目录形如 `<cache-root>/sessions/<timestamp-random>/`，包含 `analysis.sqlite`、`import.log` 和 `manifest.json`。
- `--session-dir <path>` 会使用用户给定目录，适合需要项目本地 session 的场景。
- `--port 0` 表示自动选择空闲端口。
- `--profile` 会把 import 阶段耗时放入 JSON 输出。
- `--dev` 面向前端开发：Rust server 提供 API，React dev server 提供 UI。
- `--cleanup-on-exit` 会在进程退出时清理 session；不传时保留，方便回看 SQLite 和日志。

## `pygco sessions`

查看由默认 `pygco open` 流程创建的缓存分析 session。显式 `pygco import -o <sqlite>` 产物不自动注册为 session。

```bash
pygco sessions list --format table
```

JSON 输出：

```bash
pygco sessions list --format json
```

语义：

- 扫描 `<cache-root>/sessions/*`。
- 输出 `id`、`created_at`、`size_bytes`、`database_path`、`snapshot_count`、`source_dumps` 和 `status`。
- cache root 解析顺序与 `pygco open` 相同：`PYGCO_HOME`、`XDG_CACHE_HOME/pygco`、`~/.cache/pygco`。
- 如果 cache root 或 `sessions/` 不存在，返回空列表。
- 损坏或不完整的 session 不会让整个命令失败；对应行会显示 `missing-db`、`missing-manifest` 或 `invalid-manifest`。

## `pygco import`

把 dump 导入 SQLite。

```bash
pygco import dump.jsonl.gz -o analysis.sqlite --rebuild
```

导入多份 dump：

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild --profile
```

参数：

```text
-o, --output <sqlite>           required
--rebuild
--no-reachability
--reachability-mode full|off    default: full
--reachability-depth <n>        default: 3
--reachability-node-limit <n>   default: 10000
--reachability-fanout-limit <n> default: 1000
--rules <cohort-rules.toml>
--profile
--format json|jsonl|table|markdown
--fields <fields>
```

语义：

- 默认要求输出 SQLite 不存在。
- `--rebuild` 会替换已有 SQLite。
- 一次导入多个 dump 时，每个 dump 是一个 snapshot，snapshot id 通常从 `1` 开始递增。
- 默认计算 estimated reachable size；它是有界估算，不是精确 retained size。
- `--no-reachability` 或 `--reachability-mode off` 会跳过 reachable 估算。
- `--rules` 用来加载 cohort 规则，影响 cache/async/connection 等聚合分类。

性能提示：

- 大 dump 首次导入的主要成本在 SQLite 写入、索引和 reachable 估算。
- 如果只想快速落库做 SQL/浅层统计，可以先用 `--no-reachability`。
- 如果需要排序 `reachable-size`，不要关闭 reachability。

## `pygco summary`

查看 snapshot 概览。

```bash
pygco summary analysis.sqlite --snapshot 1 --format table
```

输出 JSON：

```bash
pygco summary analysis.sqlite --snapshot 1 --limit 20 --format json
```

参数：

```text
--snapshot <id>
--limit <n>                     default: 20
--format json|jsonl|table|markdown
--fields <fields>
```

适合回答：

- 这个 dump 有多少 objects / edges？
- 总 shallow size 多大？
- stub / missing referents 是否异常？
- 哪些 type/module/cohort 排名前列？
- 当前 analysis 是否已经生成 warnings/findings？

限制：

- `summary` 的 JSON 会包含多个 section，真实大 dump 下可能比较长。
- estimated reachable size 可能互相重叠，适合排名，不适合作为“总占用内存”相加。

## `pygco objects`

查询对象列表，支持过滤、排序和分页。

```bash
pygco objects analysis.sqlite --snapshot 1 --sort reachable-size --limit 20 --format table
```

按类型过滤：

```bash
pygco objects analysis.sqlite --snapshot 1 --type dict --sort reachable-size --limit 20 --format table
```

按文本搜索 type/module/object id：

```bash
pygco objects analysis.sqlite --snapshot 1 --q pydantic --sort reachable-size --limit 20 --format table
```

找 incoming references 很多的对象：

```bash
pygco objects analysis.sqlite --snapshot 1 --sort in-edges --order desc --limit 20 --format table
```

过滤参数：

```text
--q <text>
--type <type>
--module <module>
--cohort <cohort>
--min-shallow-size <bytes>
--min-reachable-size <bytes>
--min-in-edges <n>
--min-out-edges <n>
--has-referrers
--missing-referents
--stub true|false
```

排序参数：

```text
--sort object-id|type|module|shallow-size|reachable-size|reachable-count|in-edges|out-edges
--order asc|desc               default: desc
```

分页和输出：

```text
--limit <n>                    default: 100
--offset <n>                   default: 0
--format json|jsonl|table|markdown
--fields <fields>
```

读数说明：

- `shallow-size` 是对象自身大小。
- `reachable-size` 是从该对象向 referents 方向做有界遍历得到的 estimated reachable size。
- `in-edges` 是有多少对象直接引用它；`out-edges` 是它直接引用多少对象。
- `stub true` 表示该对象只是 referent stub，没有完整对象记录。

## `pygco object`

查看单个对象。

```bash
pygco object analysis.sqlite --snapshot 1 --id 281470886362416 --format json
```

参数：

```text
--id <object-id>                required
--snapshot <id>
--format json|jsonl|table|markdown
--fields <fields>
```

输出包含：

- object metadata：type、module、qualname、object id、stub 状态。
- shallow size。
- estimated reachable size/count。
- in/out edge counts。
- missing referent count。
- top referents。
- top referrers。
- 可继续调查的 actions。

限制：

- 当前 dump 不包含 dict key、frame locals、container element label 等语义标签。
- 对 `dict`、`function`、`generator` 这类对象，通常还需要结合 `edges`、`paths`、`export-subgraph` 继续查。

## `pygco edges`

查询单跳引用关系。

查看某个对象直接引用了哪些对象：

```bash
pygco edges analysis.sqlite --snapshot 1 --from 281470886362416 --limit 50 --format table
```

查看哪些对象直接引用了某个对象：

```bash
pygco edges analysis.sqlite --snapshot 1 --to 281470886362416 --limit 50 --format table
```

参数：

```text
--from <object-id>
--to <object-id>
--snapshot <id>
--limit <n>                    default: 100
--offset <n>                   default: 0
--format json|jsonl|table|markdown
--fields <fields>
```

语义：

- `--from` 和 `--to` 二选一。
- `--from` 查 referents。
- `--to` 查 referrers。
- 当前边没有字段名、dict key、list index 等标签，所以它能说明“有引用”，但不能直接说明“是哪一个属性/槽位持有”。

## `pygco paths`

对 referent/referrer 路径做有界采样。

```bash
pygco paths analysis.sqlite --snapshot 1 --id 281470886362416 --direction referrers --depth 5 --fanout 30 --format json
```

参数：

```text
--id <object-id>                required
--snapshot <id>
--direction referents|referrers default: referrers
--depth <n>                     default: 5
--fanout <n>                    default: 30
--limit <n>                     default: 50
--format json|jsonl|table|markdown
--fields <fields>
```

重要限制：

- `paths` 是采样，不是全图最短路径证明。
- `depth`、`fanout`、`limit` 都会影响结果；没有返回路径不等于不存在路径。
- 当前输出偏 object id，需要用 `object` / `edges` 补充关键节点信息。

## `pygco diff`

对比同一个 SQLite 里的两个 snapshot。

```bash
pygco diff analysis.sqlite --from 1 --to 2 --format markdown
```

参数：

```text
--from <snapshot-id>            required
--to <snapshot-id>              required
--limit <n>                     default: 100
--format json|jsonl|table|markdown
--fields <fields>
```

适合回答：

- object/edge/shallow 总量是否增长？
- 哪些 type/module/cohort 增长明显？
- 新增、消失、保留、变化的 object id 数量是多少？

注意：

- object id 生命周期对比只在同进程连续 dump 中更可信。
- 如果两个 snapshot 来自不同进程或重启后进程，object id 重用会让对象级 diff 证据变弱。

## `pygco diff-objects`

列出两个 snapshot 之间的对象生命周期变化。

```bash
pygco diff-objects analysis.sqlite --from 1 --to 2 --state new --limit 50 --format table
```

查看保留的 dict：

```bash
pygco diff-objects analysis.sqlite --from 1 --to 2 --state retained --type dict --limit 50 --format table
```

参数：

```text
--from <snapshot-id>            required
--to <snapshot-id>              required
--state new|gone|retained|changed
--type <type>
--module <module>
--limit <n>                     default: 100
--offset <n>                    default: 0
--ids-only
--format json|jsonl|table|markdown
--fields <fields>
```

使用建议：

- 先用 `diff` 看聚合增长，再用 `diff-objects` 下钻。
- `--ids-only` 适合和 shell、`idset` 或自定义脚本组合。

## `pygco findings`

列出当前分析库里的诊断 leads。它读取并按需刷新 `findings` 表，适合替代手写 SQL 查看报告线索。

```bash
pygco findings analysis.sqlite --snapshot 1 --format table
```

按类型过滤：

```bash
pygco findings analysis.sqlite --snapshot 1 --kind large-type --format json
```

参数：

```text
--snapshot <id>
--kind <kind>
--severity info|warn
--limit <n>                     default: 100
--offset <n>                    default: 0
--format json|jsonl|table|markdown
--fields <fields>
```

当前 finding kind：

```text
cohort-signal
large-type
large-object
high-out-degree
high-in-degree
missing-referents
stub-heavy-type
diff-growth
```

说明：

- `findings` 是“值得继续查”的线索，不是 confirmed leak。
- JSON 输出包含 `evidence`、`links` 和 action，适合 Agent 或脚本继续下钻。

## `pygco suspects`

生成启发式内存排查线索。`suspects` 不依赖用户写 SQL，第一阶段覆盖大对象根、无外部 referrer 的大对象、截断 root、metadata/cache/async/connection 等模式线索。

```bash
pygco suspects analysis.sqlite --snapshot 1 --format table
```

找无外部 referrer 但 estimated reachable 较大的对象：

```bash
pygco suspects analysis.sqlite --snapshot 1 --kind orphan-retained --min-reachable 1mb --format table
```

查看 cache/async/connection 相关线索：

```bash
pygco suspects analysis.sqlite --snapshot 1 --kind cache --kind async --kind connection --format json
```

参数：

```text
--snapshot <id>
--kind <kind>                   repeatable
--min-reachable <bytes>         default: 1mb
--non-builtin
--include-stub
--limit <n>                     default: 20
--offset <n>                    default: 0
--format json|jsonl|table|markdown
--fields <fields>
```

`--min-reachable` 支持纯数字 bytes，也支持 `b`、`kb`、`mb`、`gb` 后缀，例如 `100b`、`512kb`、`1mb`。

当前 suspect kind：

```text
orphan-retained
high-retained-root
truncated-root
type-footprint
metadata-heavy
cache-heavy
async-backlog
connection-heavy
```

常用别名：

```text
cache -> cache-heavy
async -> async-backlog
connection -> connection-heavy
metadata -> metadata-heavy
type -> type-footprint
```

输出语义：

- `kind`：线索类型。
- `severity`：当前风险级别。
- `confidence`：启发式置信度。
- `subject`：对象或类型。
- `metrics`：原始指标。
- `reason`：为什么值得查。
- `limitations`：这条线索不能证明什么。
- `next_command`：建议复制执行的下一步命令。

限制：

- `suspects` 输出的是 candidate/lead，不是 confirmed leak。
- estimated reachable size 可能互相重叠，不能跨 root 相加。
- resource 类线索当前主要基于 type/module/cohort pattern，可能包含合法常驻对象。

## `pygco idset`

对两个返回 object id 的只读 SQL 查询做集合运算。

```bash
pygco idset analysis.sqlite \
  --snapshot 1 \
  --left-query "select object_id from objects where snapshot_id = 1 and type = 'dict'" \
  --right-query "select from_id as object_id from edges where snapshot_id = 1 group by from_id" \
  --op intersect \
  --details \
  --limit 20 \
  --format table
```

参数：

```text
--left-query <sql>              required
--right-query <sql>             required
--op intersect|union|left-diff|right-diff|symdiff
--snapshot <id>
--details
--ids-only
--limit <n>                     default: 1000
--format json|jsonl|table|markdown
--fields <fields>
```

规则：

- 左右查询都必须把 object id 放在第一列。
- 推荐列名为 `object_id`，但当前读取第一列即可。
- `--details` 会把结果 object id join 回对象元数据。
- 查询只允许读，不允许写库。

适合场景：

- 找“同时满足两个条件”的对象。
- 对 SQL 结果做差集，排除某类对象。
- 为后续 Web UI 或脚本分析准备 id 列表。

## `pygco sql`

执行只读 SQL。它是当前 CLI 的 escape hatch，用来完成尚未产品化成一等命令的调查。

```bash
pygco sql analysis.sqlite \
  --query "select type, count(*) as n from objects where snapshot_id = 1 group by type order by n desc limit 20" \
  --format table
```

查看 SQLite query plan：

```bash
pygco sql analysis.sqlite \
  --query "select object_id from objects where snapshot_id = 1 and type = 'dict' limit 10" \
  --explain \
  --format table
```

参数：

```text
-q, --query <sql>               required
--limit <n>                     default: 1000
--explain
--format json|jsonl|table|markdown
--fields <fields>
```

约束：

- 只允许 `SELECT` / `WITH ... SELECT` 风格的只读查询。
- SQLite query-only 模式会打开。
- 输出包含 `elapsed_ms`。

使用建议：

- 如果 `summary`、`objects`、`object`、`edges`、`paths`、`diff` 能回答问题，优先用这些命令。
- 如果你需要写 SQL，先用 `pygco schema` 或 [SQLite Schema 规范](sqlite-schema.md) 确认表和字段。

## `pygco schema`

查看分析库 schema 元数据。

```bash
pygco schema analysis.sqlite --format table
```

参数：

```text
--snapshot <id>
--limit <n>                     default: 100
--format json|jsonl|table|markdown
--fields <fields>
```

用途：

- 辅助写 `sql` / `idset`。
- 检查当前 SQLite 是否包含预期表、索引和版本信息。

## `pygco export-subgraph`

导出某个对象附近的有界对象图。

```bash
pygco export-subgraph analysis.sqlite \
  --snapshot 1 \
  --id 281470886362416 \
  --depth 2 \
  --direction both \
  --node-limit 500 \
  --edge-limit 2000 \
  --graph-format dot \
  --format json
```

参数：

```text
--id <object-id>                required
--snapshot <id>
--depth <n>                     default: 2
--direction referents|referrers|both
--node-limit <n>                default: 500
--edge-limit <n>                default: 2000
--graph-format json|jsonl|dot   default: json
--format json|jsonl|table|markdown
--fields <fields>
```

限制：

- 这是局部子图导出，不是全图导出。
- 大 `depth`、大 `node-limit` 会快速变得很难读，也会更慢。
- 当前节点标签仍偏 id/type/module；如果要做可视化解释，通常需要后处理。

## `pygco report`

生成报告。

```bash
pygco report analysis.sqlite --snapshot 1 --format markdown
```

输出 JSON 报告：

```bash
pygco report analysis.sqlite --snapshot 1 --limit 20 --format json
```

参数：

```text
--snapshot <id>
--limit <n>                     default: 20
--format json|jsonl|table|markdown
--fields <fields>
```

使用建议：

- 人类阅读优先用 `report --format markdown`。
- 自动化处理可以用 `report --format json`。
- 如果只想查看诊断 leads，用 `pygco findings` 更直接。

## `pygco doctor`

检查分析库健康状态。

```bash
pygco doctor analysis.sqlite --format table
```

参数：

```text
--snapshot <id>
--limit <n>                     default: 20
--format json|jsonl|table|markdown
--fields <fields>
```

检查内容包括：

- schema 元数据。
- snapshot 数量。
- object / edge 数量。
- import warnings。
- reachability/cache 可用性。

适合场景：

- Web UI 或 CLI 查询结果看起来不对时，先跑 `doctor`。
- 导入完成后确认 SQLite 是否完整。

## `pygco web`

为已有 SQLite 启动本地 Web UI。

```bash
pygco web analysis.sqlite --host 127.0.0.1 --port 3791
```

参数：

```text
--host <host>                   default: 127.0.0.1
--port <port>                   default: 0
--no-browser
--dev
--dev-server-url <url>          default: http://127.0.0.1:5173/
```

说明：

- `--port 0` 表示自动选择空闲端口。
- `--no-browser` 适合自动化、远程环境或你想手动打开 URL 的场景。
- `--dev` 面向前端开发，通常配合 React dev server。

## `pygco api`

为已有 SQLite 启动本地 API server。

```bash
pygco api analysis.sqlite --host 127.0.0.1 --port 5174 --no-browser
```

参数：

```text
--host <host>                   default: 127.0.0.1
--port <port>                   default: 0
--no-browser
--dev
--dev-server-url <url>          default: http://127.0.0.1:5173/
```

说明：

- `api` 与 `web` 使用同一套 server 参数。
- 它适合前端开发、API probing、脚本化本地集成。

## `pygco version`

打印版本。

```bash
pygco version
```

## 当前诊断能力缺口

当前 CLI 能分析真实 dump，但还没有把所有诊断工作流产品化成一等命令。不要把下面这些缺口误认为已经实现：

- 还没有 `pygco overview --compact`；当前用 `summary`，但大 dump 下输出偏长。
- 还没有语义化 `explain`；当前 `object` 是事实展示，不会直接解释“为什么可疑”。
- `paths` 当前是有界采样，输出仍偏 id，不是完整所有者证明。

这些缺口的系统性整改见 [CLI 诊断工作台整改方案](cli-diagnostics-workbench.md) 和 [CLI 诊断工作台技术实施 Spec](project/cli-diagnostics-technical-spec.md)。

## 当前版本调查 recipes

找 estimated reachable 最大的对象：

```bash
pygco objects analysis.sqlite --snapshot 1 --sort reachable-size --order desc --limit 20 --format table
```

查看一个可疑对象：

```bash
pygco object analysis.sqlite --snapshot 1 --id 281470886362416 --format json
```

查看直接 referrers：

```bash
pygco edges analysis.sqlite --snapshot 1 --to 281470886362416 --limit 50 --format table
```

采样 referrer paths：

```bash
pygco paths analysis.sqlite --snapshot 1 --id 281470886362416 --direction referrers --depth 5 --fanout 30 --format json
```

查看非 builtins type 的 shallow 排名：

```bash
pygco sql analysis.sqlite \
  --query "select type, module, count, shallow_size_sum from type_stats where snapshot_id = 1 and module <> 'builtins' order by shallow_size_sum desc limit 20" \
  --format table
```

查看 persisted findings：

```bash
pygco findings analysis.sqlite --snapshot 1 --limit 20 --format table
```

查找 orphan-retained 线索：

```bash
pygco suspects analysis.sqlite --snapshot 1 --kind orphan-retained --min-reachable 1mb --format table
```

查看 module footprint：

```bash
pygco sql analysis.sqlite \
  --query "select module, count, shallow_size_sum from module_stats where snapshot_id = 1 order by shallow_size_sum desc limit 20" \
  --format table
```

导出可疑对象附近子图：

```bash
pygco export-subgraph analysis.sqlite --snapshot 1 --id 281470886362416 --depth 2 --direction both --graph-format dot --format json
```

## Source Manifest

本页依据以下来源维护：

- `target/debug/pygco --help`
- `target/debug/pygco <command> --help`
- [Generated CLI Help](generated/cli-help.md)
- [SQLite Schema 规范](sqlite-schema.md)
- `scripts/check_docs_commands.py`

更新 CLI 行为时，应同步更新本页，并运行：

```bash
python3 scripts/check_docs_commands.py
```
