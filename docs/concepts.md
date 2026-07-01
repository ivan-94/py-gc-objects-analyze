# 核心概念

## GC Object Dump

GC object dump 是从 Python 运行时导出的对象快照。它以 `gc.get_objects()` 为主集合，记录每个对象的 id、type、module、浅层 size，以及可选 referents。

它不是完整 RSS 快照。以下内存通常不在 GC object census 中：

- native allocations
- allocator arenas
- mmap
- C extension 内部缓冲区
- thread stacks
- shared libraries
- 部分非 GC tracked Python 对象

## Object ID

`object_id` 使用 Python `id(obj)`。它在同一个 Python 进程生命周期内可以用于关联对象；跨进程、跨重启之后只能作为弱信号。

因此：

- 同进程连续 dump：object lifecycle diff 有较高参考价值。
- 不同进程 dump：优先看 type/module/cohort/reachable size/owner chain 差异。

工具使用 `producer_run_id`、`dump_sequence`、process start metadata 来判断 object lifecycle diff 的可信度。

## Referent 和 Referrer

- `referent`：当前对象直接引用的对象。
- `referrer`：直接引用当前对象的对象。

dump 文件只记录从对象到 referent 的边。referrer 是本地导入 SQLite 后反向索引得到的。

## Stub Object

有些 referent 不在 `gc.get_objects()` 主集合中，但可以通过 `gc.get_referents(obj)` 看到。Python producer 可以为这些对象输出轻量 stub。

stub 只表示“这个 referent 在 dump 时存在，并被某个对象引用过”。它不保证具备完整 referents，也不保证能代表完整对象图。

## Shallow Size

`shallow_size` 来自 Python runtime 侧的 `sys.getsizeof(obj)` 风格计算，只代表对象本身的浅层大小，不递归包含 referents。

## Reachable Size

`reachable_size` 是本地分析器基于引用图估算的可达对象总浅层大小。它必须记录算法参数：

- depth
- node limit
- edge fanout limit
- traversal direction
- algorithm version

这个值用于排序和定位线索，不是精确内存归属。

## Cohort

cohort 是工具内置或用户定义的一组类型/模块规则。例如：

- database cache
- streaming response
- threading
- network I/O
- ORM metadata

cohort 只提供调查入口，不能单独证明泄漏。

## Analysis SQLite

SQLite 是从 dump 导入得到的临时分析数据库。它包含对象表、边表、聚合表、索引和可重算缓存。它不是原始事实来源。

## Session

一次 `pygco open` 或一次显式 `import` 产生一个 analysis session。一个 session 可以包含多个 dump snapshot。
