# 系统架构

`pygco` 分为 Python producer、Rust analyzer、local Web UI 三部分。

## 总览

```text
Python service/test process
  pygco_dump
    -> gzip JSONL dump

Local machine
  pygco Rust CLI
    -> import/index/aggregate
    -> temporary SQLite
    -> CLI queries
    -> local API server
    -> embedded React Web UI
```

## Python Producer

包名：

- distribution：`pygco-dump`
- import：`pygco_dump`

职责：

- 采集 `gc.get_objects()`。
- 为每个对象输出 object record。
- 可选输出 `gc.get_referents()` 的 object id。
- 可选输出 referent stub。
- 流式 gzip JSONL。
- 提供 FastAPI route helper。

非职责：

- 不计算聚合。
- 不计算 reachable size。
- 不分析 owner path。
- 不写 SQLite。
- 不做远程 attach。

## Rust Workspace

推荐 workspace：

```text
crates/
  pygco-cli/          # binary entrypoint
  pygco-dump-format/  # dump record structs and validation
  pygco-importer/     # streaming import pipeline
  pygco-store/        # SQLite schema and query helpers
  pygco-analysis/     # graph algorithms and aggregations
  pygco-api/          # local HTTP API
  pygco-report/       # markdown/json report builder
web/
  app/                # React + shadcn UI
docs/
```

Rust 模块边界：

| Crate | 职责 |
| --- | --- |
| `pygco-dump-format` | dump schema、serde models、version validation |
| `pygco-importer` | gzip JSONL streaming parser、batch insert、import log |
| `pygco-store` | SQLite schema、migrations-for-rebuild、prepared queries |
| `pygco-analysis` | stats、reachability、diff、paths、idset |
| `pygco-api` | local API server、static assets |
| `pygco-cli` | command parsing、output formatting |
| `pygco-report` | reports and findings |

实现必须以 [SQLite Schema 规范](sqlite-schema.md) 和 [Local API 规范](api.md) 为契约。Rust store/API 与 Web typed client 不应各自发明字段名或错误格式。

## SQLite

SQLite 是临时分析库。schema 变更时默认重建，不做长期 migration 负担。

导入阶段建议：

1. 创建空数据库。
2. 设置 import pragmas。
3. 流式解析 dump。
4. 批量写入 objects。
5. 批量写入 edges。
6. 构建基础 stats。
7. 创建索引。
8. 计算可选 reachability。
9. 写入 import summary。

## Local API Server

API server 只绑定本地地址，默认：

```text
127.0.0.1:<dynamic-port>
```

职责：

- 为 Web UI 提供分页查询。
- 执行有界图查询。
- 提供 report markdown/json。
- 提供 query explain。

API 不负责：

- 多用户认证。
- 远程持久化。
- 权限管理。

## Web UI

开发期：

```text
Rust API server + React dev server
```

默认开发端口：

```text
React dev server: http://127.0.0.1:5173/
Rust API server: http://127.0.0.1:5174/
```

`pygco open --dev` / `pygco web --dev` 启动 Rust API server，并把浏览器目标切到 React dev server；Vite 将 `/api` 代理回 Rust API。

发布期：

```text
React build static assets embedded in Rust binary
```

`pygco-api` 的静态资源解析顺序：

1. `PYGCO_WEB_DIST`
2. source tree `web/app/dist`
3. 编译进二进制的 embedded assets

发布构建应先在 `web/app` 目录运行 `corepack pnpm build`，再构建 Rust binary，这样 embedded assets 会包含真实 React UI。若没有前端构建产物，Rust 编译会嵌入最小占位页，保证 CLI/API 仍可编译和启动。

用户通过 `pygco open` 启动本地 Web UI。

## 数据流

```text
dump.jsonl.gz
  -> importer
  -> snapshots
  -> objects
  -> edges
  -> stats
  -> indexes
  -> API/CLI/WebUI
```

所有昂贵计算必须有明确入口、进度、缓存和取消策略。
