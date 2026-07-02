# 快速开始

本文面向第一次使用 `pygco` 排查 Python GC object 内存快照的用户。

## 1. 在 Python 服务中开启 dump

业务进程集成 Python producer 包：

```python
from pygco_dump.fastapi import gc_heap_dump_route

app.add_api_route(
    "/debug/gc-heap-dump",
    gc_heap_dump_route(),
    methods=["GET"],
)
```

这个 endpoint 只负责流式导出 gzip JSONL dump，不做聚合、不做分析。

FastAPI 只是最小示例。Celery worker、Gunicorn/uWSGI `prefork`、管理命令或 daemon 进程可以直接调用底层 `write_gc_dump()`；多进程 worker 推荐按 PID 触发每个进程各自写 dump。详见 [Python Producer 接入指南](producer-integration.md)。

## 2. 拉取 dump 文件

```bash
curl -o before.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
curl -o after.jsonl.gz "http://service/debug/gc-heap-dump?collect=false"
```

需要强制 GC 时可以显式加 `collect=true`。默认不建议在高压环境中自动 collect。

## 3. 一条命令打开 Web UI

```bash
pygco open before.jsonl.gz after.jsonl.gz
```

`pygco open` 会：

1. 创建一个新的临时分析 session。
2. 把 dump 导入 fresh SQLite。
3. 计算基础聚合和必要索引。
4. 启动本地 API server。
5. 打开本地 Web UI。

默认 session 存放在用户 cache root 下。解析顺序是 `PYGCO_HOME`、`XDG_CACHE_HOME/pygco`、`~/.cache/pygco`：

```text
<cache-root>/sessions/<timestamp-random>/
  analysis.sqlite
  import.log
  manifest.json
```

## 4. 显式导入和 CLI 分析

需要可复现命令或自动化分析时，使用显式流程：

```bash
pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
pygco summary analysis.sqlite
pygco diff analysis.sqlite --from 1 --to 2
pygco web analysis.sqlite
```

如果 `analysis.sqlite` 已存在，默认报错。使用 `--rebuild` 显式删除并重建。

## 5. 推荐排查顺序

1. 看 Overview：确认 object count、edge count、shallow size、top types、top modules。
2. 看 Diff：确认快照之间哪些 type/module/cohort 增长。
3. 看 Objects：按 reachable size、shallow size、in edges、out edges 排序。
4. 看 Object Detail：检查 referents、referrers、局部引用图。
5. 看 Owner Paths：找可能的持有者链路。
6. 看 Findings/Leads：把启发式结果当候选线索，不直接当结论。
7. 用 SQL / idset 做临时验证。

## 6. 删除 session

SQLite 是临时分析产物，用完可以删除：

```bash
pygco sessions list --format table
rm -rf ~/.cache/pygco/sessions/<session-id>
```

保留原始 dump 即可复现导入结果。
