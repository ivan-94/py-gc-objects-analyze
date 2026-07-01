# Source Manifest

本文记录第一版文档的来源，方便后续 Agent 或开发者重读原始上下文，而不是只依赖摘要。

## Conversation Decisions

本轮对齐的产品决策：

- POC 已完成，正式项目放在 `/Users/ivan/workspace/ai/py-gc-objects-analyze`。
- 项目采用文档驱动开发。
- 第一阶段先建立 `docs/` 下的完整用户文档、规范和 references。
- 第一版是本地离线 Python GC object forensic 工具。
- Python runtime 只负责 dump。
- Rust 负责 CLI、导入、索引、聚合、图算法、API server。
- React + shadcn/ui 负责本地 Web UI。
- Web UI 规范必须进入文档。
- SQLite 是临时、可重建分析产物，不做长期存档。
- 默认每次导入重建 SQLite。
- `pygco open dump-a.jsonl.gz dump-b.jsonl.gz` 支持自动生成临时 SQLite 并打开 Web UI。
- `pygco import dump-a.jsonl.gz dump-b.jsonl.gz -o analysis.sqlite --rebuild` + `pygco web analysis.sqlite` 是显式流程。
- 不做过度安全合规设计，只保留必要运行安全边界。
- Python producer 包发行名 `pygco-dump`，import 名 `pygco_dump`。
- Rust CLI 命令名 `pygco`。
- 发布期 Web UI 静态资源嵌入 Rust binary。
- 开发期 Rust API server 和 React dev server 分开运行。
- Cross-review 后补充了 SQLite schema、Local API、process identity、reachability cache key、findings kind enum、idset query contract、TanStack Query server-state 约束。
- 用户指出 docs 偏泛，要求在任务中说明 POC 可参考和不可照搬的实现经验；因此补充 `implementation-blueprint.md`、`poc-migration-guide.md` 和 `task.md` POC 迁移映射。

## POC Source

POC 位于 ai_glass worktree 中：

```text
/Users/ivan/.codex/worktrees/memory-analyzer/ai_glass
```

相关 POC 文件：

```text
debug_tools/gc_heap_dump.py
tools/memory_analyzer/
test/test_gc_heap_dump.py
test/test_memory_analyzer.py
```

POC 验证过的能力：

- FastAPI debug dump endpoint。
- gzip JSONL dump。
- referent stub。
- SQLite import。
- top types/modules/cohorts。
- shallow size 和 reachable size 同时展示。
- object/referrers/referents。
- local object graph。
- findings。
- SQL/schema/idset。
- diff/diff-objects。
- Web UI exploration。

## External References

外部技术依据见 [../references/README.md](../references/README.md)。
