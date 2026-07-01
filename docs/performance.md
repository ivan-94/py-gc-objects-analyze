# 性能规范

`pygco` 的核心目标之一是处理百万级对象、千万级边的 Python GC object dump。

## 性能原则

- Rust importer 必须流式处理 dump。
- SQLite 是临时分析库，可以为导入性能牺牲长期存储复杂度。
- 重索引应在批量写入后创建。
- Web UI 永远不加载全量对象表。
- 图算法必须有明确 depth、fanout、node limit。
- 所有昂贵操作必须可 profile。

## 目标规模

第一版目标：

| 指标 | 目标 |
| --- | --- |
| objects | 1M 到 5M |
| edges | 5M 到 50M |
| dump size gzip | 100MB 到 5GB |
| SQLite size | 可大于 dump，允许临时占用 |
| import memory | 不随 dump 大小线性增长 |

## Import Pipeline

导入阶段：

1. 创建 fresh SQLite。
2. 设置 import pragmas。
3. 流式解压 gzip。
4. 流式解析 JSONL。
5. 分批写入 objects。
6. 分批写入 edges。
7. 计算基础 stats。
8. 创建索引。
9. 计算可选 reachability。

批大小必须可配置，并在 benchmark 中覆盖。

## SQLite 策略

SQLite 是临时工作文件。导入期可以使用偏性能配置，但必须保证导入失败时能明确报错并删除半成品。

建议：

- 在临时路径构建 `.tmp.sqlite`。
- 导入成功后 rename 成目标 SQLite。
- 导入失败后删除 `.tmp.sqlite`。
- 使用事务包裹批量写入。
- 创建索引晚于批量导入。

## Query Budget

CLI/API 查询目标：

| 查询 | 目标 |
| --- | --- |
| summary | < 1 s |
| objects page | < 300 ms |
| type/module top list | < 500 ms |
| object detail | < 500 ms |
| one-hop edges | < 500 ms |
| local subgraph | < 1.5 s |
| SQL explain | < 500 ms |

超过目标不一定失败，但必须能被 benchmark 发现。

## Reachability

reachable size 是高成本估算，不能无界计算。

默认参数：

```text
depth = 3
node_limit = 10000
fanout_limit = 1000
direction = referents
algorithm_version = 1
```

要求：

- 使用 visited set 处理循环。
- 每个 root 独立计算。
- 保存 `truncated`。
- 保存 algorithm version。
- cache key 包含 depth、node_limit、fanout_limit、direction、algorithm_version。
- 支持跳过 builtins/core modules 的视图级过滤，但底层结果要可解释。

默认 import/open 使用 `reachability_mode=full`，即为每个非 stub object 计算一次有界 estimated reachable size。用户可以通过 `--no-reachability` 或 `--reachability-mode off` 跳过。

如果 dump 不包含 referents，reachability 状态为 `unavailable`，相关排序和图查询必须降级。

## Graph Queries

局部图查询必须有：

- depth limit
- node limit
- edge limit
- direction
- timeout 或 cancellation

不提供“加载全图到浏览器”的功能。

## Benchmarks

项目必须维护 synthetic dumps：

```text
fixtures/synthetic/
  medium.jsonl.gz
  large.jsonl.gz
  pathological.jsonl.gz
```

benchmark 维度：

- gzip decode throughput
- JSONL parse throughput
- object insert throughput
- edge insert throughput
- index build time
- summary query time
- objects query time
- reachability compute time
- Web API p95 latency

## Current Benchmark Snapshot

Source Manifest:

- Fixture generator: `fixtures/generators/generate_synthetic.py --all`
- Import report: `benches/reports/medium-import.json`
- Memory report: `benches/reports/import-memory.json`
- Query/API report: `benches/reports/medium-query-api.json`
- Benchmark DB: `.scratch/medium-bench.sqlite`, generated from `fixtures/synthetic/medium.jsonl.gz`
- Binary: `target/debug/pygco`

Medium fixture:

| Metric | Value |
| --- | ---: |
| objects | 10,000 |
| edges | 30,000 |
| import elapsed | 719 ms |
| peak RSS | 20.75 MiB |
| RSS per object | 2,175.8 bytes |
| insert_objects | 33 ms |
| insert_edges | 54 ms |
| build_stats | 63 ms |
| build_indexes | 89 ms |
| reachability | 291 ms |

Memory scaling snapshot:

| Fixture | Objects | Edges | Peak RSS | RSS per object |
| --- | ---: | ---: | ---: | ---: |
| medium | 10,000 | 30,000 | 20.75 MiB | 2,175.8 bytes |
| large | 50,000 | 200,000 | 42.81 MiB | 897.84 bytes |

Between medium and large, object count grows 5.0x while peak RSS grows 2.063x in this local run. This supports the first-version requirement that import memory does not scale linearly with dump size.

Medium query/API p95:

| Query | CLI p95 | API p95 | Target |
| --- | ---: | ---: | ---: |
| summary / overview | 38.331 ms | 42.136 ms | 1,000 ms |
| objects page | 150.208 ms | 157.909 ms | 300 ms |
| object detail | 10.905 ms | 25.736 ms | 500 ms |
| local graph | 10.563 ms | 119.525 ms | 1,500 ms |
| SQL explain | 10.131 ms | 14.585 ms | 500 ms |

## Profiling

CLI 需要提供：

```bash
pygco import fixtures/golden/tiny-v1.jsonl.gz -o analysis.sqlite --rebuild --profile
pygco sql analysis.sqlite --query "select object_id from objects limit 10" --explain
pygco doctor analysis.sqlite
```

profile 输出必须能定位到阶段：

```text
decode
parse
insert_objects
insert_edges
build_stats
build_indexes
reachability
```
