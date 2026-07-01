# Dump 文件格式规范

Implementation contract: this document is the first-version contract between `pygco_dump` producers and Rust import/analyzer consumers.

本文定义 `pygco-dump` 与 `pygco` Rust analyzer 之间的文件契约。

## 文件格式

第一版 dump 文件为 gzip 压缩的 JSON Lines：

```text
*.jsonl.gz
```

约束：

- UTF-8 编码。
- 每行一个 JSON object。
- 第一行必须是 `metadata` `phase=start`。
- 最后一行必须是 `metadata` `phase=end`。
- 中间主要是 `object` records。
- 解析器必须支持流式读取，不能要求一次性载入完整 dump。

## 版本

起始版本：

```json
{
  "format": "pygco-dump-jsonl",
  "format_version": 1
}
```

兼容规则：

- Rust importer 必须拒绝未知 major version。
- Rust importer 可以兼容同 major 下的新增可选字段。
- producer 不得改变既有字段语义。

## Start Metadata Record

示例：

```json
{
  "record_type": "metadata",
  "phase": "start",
  "format": "pygco-dump-jsonl",
  "format_version": 1,
  "producer": "pygco_dump",
  "producer_version": "0.1.0",
  "producer_run_id": "01J1EXAMPLE...",
  "dump_sequence": 3,
  "created_at": "2026-07-01T03:00:00Z",
  "process_started_at": "2026-07-01T02:30:00Z",
  "host_id": "host-a",
  "container_id": "container-a",
  "pid": 12345,
  "python_version": "3.11.15",
  "platform": "Linux-...",
  "collect_before_dump": false,
  "include_referents": true,
  "include_referent_stubs": true,
  "include_repr": false,
  "repr_limit": 0,
  "gc_count": [100, 2, 3],
  "gc_stats": null,
  "object_count": 944384
}
```

必填字段：

- `record_type`
- `phase`
- `format`
- `format_version`
- `producer`
- `producer_version`
- `producer_run_id`
- `dump_sequence`
- `created_at`
- `pid`
- `python_version`
- `platform`
- `collect_before_dump`
- `include_referents`
- `include_referent_stubs`
- `include_repr`
- `repr_limit`
- `object_count`

推荐字段：

- `process_started_at`
- `host_id`
- `container_id`
- `gc_count`
- `gc_stats`

`gc_count` 的语义为 Python `gc.get_count()` 返回值，按 list of integers 存储。它是诊断信息，不参与核心分析。

`gc_stats` 的语义为 Python `gc.get_stats()` 返回值。由于 Python 版本间可能变化，第一版允许为 `null`，importer 不应依赖它。

## Object Record

示例：

```json
{
  "record_type": "object",
  "id": 140067815097616,
  "type": "cachetools.LRUCache",
  "module": "cachetools",
  "qualname": "LRUCache",
  "size": 56,
  "gc_tracked": true,
  "stub": false,
  "referents": [140067815108736, 140067815258048]
}
```

必填字段：

- `record_type`
- `id`
- `type`
- `size`

推荐字段：

- `module`
- `qualname`
- `gc_tracked`
- `stub`
- `referents`

可选字段：

- `repr`

字段语义：

| 字段 | 类型 | 语义 |
| --- | --- | --- |
| `id` | integer | Python `id(obj)` |
| `type` | string | 类型展示名；没有 module 时也必须可读 |
| `module` | string | `type(obj).__module__` |
| `qualname` | string | `type(obj).__qualname__` |
| `size` | integer/null | Python runtime 侧浅层 size |
| `gc_tracked` | boolean/null | `gc.is_tracked(obj)` |
| `stub` | boolean | 是否为轻量 stub |
| `referents` | integer array | 当前对象直接引用的对象 id |
| `repr` | string | 截断后的 repr，默认不输出 |

## Stub Object Record

stub object 示例：

```json
{
  "record_type": "object",
  "id": 94922745202656,
  "type": "abc.ABCMeta",
  "module": "abc",
  "qualname": "ABCMeta",
  "size": 1688,
  "gc_tracked": false,
  "stub": true,
  "referents": []
}
```

stub 表示：

- 该对象不在 `gc.get_objects()` 主集合中。
- 它被某个主集合对象通过 `gc.get_referents()` 引用。
- producer 可以可靠获取它的 id/type/module/qualname/size/gc_tracked。
- producer 不递归展开 stub 的 referents。

分析器必须把 stub 和 missing referent 区分开：

- stub：对象有记录，但记录不完整。
- missing：边指向的 object id 没有任何 object record。

## End Metadata Record

示例：

```json
{
  "record_type": "metadata",
  "phase": "end",
  "dumped_count": 944384,
  "stub_count": 1024,
  "total_object_records": 945408,
  "elapsed_ms": 3500
}
```

必填字段：

- `record_type`
- `phase`
- `dumped_count`
- `stub_count`
- `total_object_records`
- `elapsed_ms`

## Producer 行为要求

Python producer 必须：

- 流式 gzip 输出。
- 支持 `collect=false` 默认值。
- 同一进程同时只允许一个 dump 在运行。
- 不在 dump 过程中做聚合分析。
- 不递归计算深层 size。
- 默认不输出 `repr`。
- 进程启动时生成稳定的 `producer_run_id`。
- 同一进程内为每次 dump 递增 `dump_sequence`。

`repr` 默认关闭的原因不是安全合规，而是性能和副作用：

- 可能很慢。
- 可能很大。
- 可能触发用户自定义逻辑。
- 可能让 dump 文件难以处理。

## Importer 行为要求

Rust importer 必须：

- 流式读取 gzip JSONL。
- 分阶段导入：objects、edges、stats、indexes。
- 先批量写入，再创建重索引。
- 对重复 object id 做确定性处理并报告错误。
- 对缺失 referent 保留边，并在查询层标记 missing。
- 记录 dump sha256 和 import options。
