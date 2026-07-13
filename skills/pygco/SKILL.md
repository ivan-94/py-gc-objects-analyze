---
name: pygco
description: 使用 pygco 分析 Python GC object dump。用于安装接入、对象内存排查、snapshot diff 和 CLI 查询。
---

# pygco

把 `pygco` 视为本地 Python GC object memory forensics 工具：

```text
目标 Python 进程
  -> pygco-dump 生成 gzip JSONL
  -> pygco 导入临时 SQLite
  -> CLI / Web UI / local API 查询
```

| 组件 | 作用 |
| --- | --- |
| `pygco-dump` / `pygco_dump` | 安装到目标 Python 环境，在进程内生成 dump |
| `pygco` | 安装到分析机器，获取、导入、查询、对比和展示 dump |

`pygco` 不会远程 attach 到任意 Python 进程。目标程序必须主动写出 dump，或提供受控的 dump endpoint、管理命令、task、signal 等触发入口。

## 安装与 dump 前置

安装 `pygco` CLI：

```bash
curl -fsSL https://github.com/ivan-94/py-gc-objects-analyze/releases/latest/download/install.sh | sh
pygco version
```

默认安装到 `$HOME/.local/bin/pygco`。目标 Python 环境需要 Python 3.10+；按接入方式安装 producer：

```bash
python -m pip install pygco-dump                 # 框架无关
python -m pip install "pygco-dump[fastapi]"      # FastAPI helper
```

框架无关程序可直接写文件：

```python
from pathlib import Path
from pygco_dump import write_gc_dump

with Path("heap.jsonl.gz").open("wb") as file:
    write_gc_dump(file, collect=False, include_repr=False)
```

FastAPI 程序可注册内部诊断路由：

```python
from pygco_dump.fastapi import gc_heap_dump_route

app.add_api_route(
    "/debug/gc-heap-dump",
    gc_heap_dump_route(),
    methods=["GET"],
)
```

producer 只负责输出 dump，不提供鉴权、脱敏、聚合或调度。保持 `collect=false` 和 `include_repr=false`，限制并发、响应大小与访问者；不要把 endpoint 暴露到公网，也不要把私有 dump 上传到公共系统。

## 输入与分析产物

| 名称 | 含义 |
| --- | --- |
| dump | gzip JSONL 原始证据，包含 object、size 和可选 referent 记录 |
| snapshot | dump 导入 SQLite 后的本地编号；通常从 `1` 递增 |
| analysis SQLite | 从 dump 派生的临时数据库，可从原始 dump 重建 |
| cache session | `pygco open` 管理的目录，包含 `analysis.sqlite`、`import.log` 和 `manifest.json` |
| finding | 导入期持久化的结构化诊断线索 |
| suspect | 查询时生成的启发式调查候选 |

## CLI 能力

### 获取、导入和 session

| 命令 | 能力 |
| --- | --- |
| `open` | 导入一个或多个本地/URL dump，创建 cache session，并启动本地 Web UI/API |
| `fetch` | 从 HTTP(S) 下载 dump，流式计算 SHA-256，并输出脱敏来源元数据 |
| `import` | 把一个或多个本地 dump 导入显式 SQLite；支持 reachability 参数、cohort rules、profiling 和进度输出 |
| `sessions list` | 枚举 `open` 创建的缓存 session，并显示路径、大小、snapshot、来源和损坏状态 |

`open` 的默认 cache root 依次为 `PYGCO_HOME`、`XDG_CACHE_HOME/pygco`、`~/.cache/pygco`。`fetch` 和 URL `open` 支持 request header、timeout 和最大响应大小，并对 secret header 与 URL query 做日志脱敏。

### 概览与对象引用

| 命令 | 能力 |
| --- | --- |
| `overview` | 提供轻量 triage 概览：质量、snapshot、top non-builtin types、cohorts、limitations 和 next commands |
| `summary` | 提供更完整的 snapshot、type、module、cohort、warning 和 finding 汇总 |
| `objects` | 按文本、type、module、cohort、size、edge count、stub 等条件过滤、排序和分页对象 |
| `object` | 展示单个对象的 metadata、shallow/reachable metrics、直接边摘要和后续动作 |
| `edges` | 查询一个对象的直接 referents（`--from`）或 referrers（`--to`） |
| `paths` | 对 referent/referrer 路径做 depth、fanout、limit 有界采样，可附带节点摘要 |
| `container` | 聚合 deque、queue、cache、dict、list、set 等容器的直接 referent 类型和 top items |
| `export-subgraph` | 把对象附近的有界局部图导出为 JSON、JSONL 或 DOT |

### Snapshot 对比与诊断线索

| 命令 | 能力 |
| --- | --- |
| `diff` | 比较两个 snapshot 的 object、edge、size 以及 type/module/cohort 聚合变化 |
| `diff-objects` | 按 `new`、`gone`、`retained`、`changed` 展示对象生命周期行 |
| `findings` | 按 kind 和 severity 查询导入期持久化 leads，包括 evidence、links 和 action |
| `suspects` | 生成对象 root、type footprint、metadata、cache、async、connection 等启发式候选 |
| `report` | 输出包含 quality、snapshot、suspects、findings 和算法参数的 Markdown/JSON 报告 |
| `doctor` | 检查 schema、snapshot、索引、object/edge、warning 和 reachability 可用性 |

`suspects` 支持 `orphan-retained`、`high-retained-root`、`truncated-root`、`type-footprint`、`metadata-heavy`、`cache-heavy`、`async-backlog`、`connection-heavy`。

### 高级查询与本地服务

| 命令 | 能力 |
| --- | --- |
| `schema` | 输出 SQLite 表、字段、索引和版本摘要 |
| `sql` | 执行只读 `SELECT` / `WITH ... SELECT` 或 `EXPLAIN QUERY PLAN` |
| `idset` | 对两组 object-id SQL 结果执行 `intersect`、`union`、`left-diff`、`right-diff`、`symdiff` |
| `web` | 为已有 SQLite 启动本地 Web UI/API，默认绑定 `127.0.0.1` |
| `api` | 为已有 SQLite 启动本地 API server，默认绑定 `127.0.0.1` |
| `version` | 输出 CLI 版本 |

当前 `idset` 运行时接受 `left-diff`、`right-diff`、`symdiff`；generated help 中的 `left-only/right-only` 与运行行为不一致。

## 输出合同

- 多数查询命令支持 `--format json|jsonl|table|markdown` 和 `--fields`。
- JSON 适合 Agent 和自动化，JSONL 适合流式处理，table 适合终端，Markdown 适合报告。
- JSON/API 风格输出把 object id 编码为字符串，避免 JavaScript 大整数精度丢失。
- import progress 写入 stderr，不污染 stdout JSON。
- 全局 `--no-color` 与 `--verbose` 放在子命令前。
- 典型退出码为：用法 `2`、dump 格式 `10`、import `11`、query `20`、内部错误 `70`。

## 分析语义与边界

- `shallow_size` 只表示对象自身大小。
- `reachable_size` 是有界遍历估算值，可能 `truncated` 或 `unavailable`，不同 root 的值可能重叠。
- `paths` 和 `export-subgraph` 是局部、有界图，不代表唯一或完整 owner path。
- `findings`、`suspects`、cohort 和 orphan-retained 都是 investigation leads，不是 confirmed leak。
- 引用边当前缺少字段名、dict key、list index 和局部变量名。
- object id 生命周期只在同一 Python 进程运行内的连续 dump 中较可信。
- GC dump 不覆盖 native allocations、allocator arenas、mmap、线程栈和部分非 GC-tracked 对象。
- SQLite 是可重建分析产物；把原始 dump 作为持久证据保留。

## 精确参考

- CLI 参数：`docs/cli.md`、`docs/generated/cli-help.md` 或 `pygco <command> --help`。
- HTTP、Celery、prefork worker、signal 接入：`docs/producer-integration.md`。
- 结果语义：`docs/concepts.md`、`docs/analysis-model.md`、`docs/known-limitations.md`。
