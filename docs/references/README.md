# References

本文列出第一版设计参考的官方文档和技术资料。实现阶段需要优先查阅这些来源。

## Python Runtime

- Python `gc` module: <https://docs.python.org/3/library/gc.html>
- Python `sys.getsizeof`: <https://docs.python.org/3/library/sys.html#sys.getsizeof>

相关点：

- `gc.get_objects()` 用于获取 GC tracked objects。
- `gc.get_referents()` 用于获取对象直接引用的 referents。
- `gc.is_tracked()` 用于判断对象是否由 GC 跟踪。
- `sys.getsizeof()` 只能提供浅层大小。

## SQLite

- SQLite transactions: <https://sqlite.org/lang_transaction.html>
- SQLite EXPLAIN QUERY PLAN: <https://www.sqlite.org/eqp.html>
- SQLite PRAGMA reference: <https://www.sqlite.org/pragma.html>
- SQLite CLI reference: <https://sqlite.org/cli.html>

相关点：

- import pipeline 需要批量事务。
- `EXPLAIN QUERY PLAN` 用于解释查询是否使用索引。
- `PRAGMA query_only` 可用于只读 SQL 探针。

## Rust

- Cargo workspaces: <https://doc.rust-lang.org/cargo/reference/workspaces.html>
- Rust book on workspaces: <https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html>

相关点：

- 正式项目使用 Rust workspace 拆分 CLI、store、analysis、API。

## Web

- React docs: <https://react.dev/>
- Vite guide: <https://vite.dev/guide/>
- shadcn/ui Vite installation: <https://ui.shadcn.com/docs/installation/vite>
- TanStack Table virtualization guide: <https://tanstack.com/table/v8/docs/guide/virtualization>
- TanStack Virtual: <https://tanstack.com/virtual/latest>
- TanStack Router: <https://tanstack.com/router/latest>
- TanStack Query: <https://tanstack.com/query/latest>

相关点：

- Web UI 使用 React + TypeScript + Vite。
- 表格使用 TanStack Table，虚拟滚动使用 TanStack Virtual。
- URL state 使用 TanStack Router search params。
- Server state 使用 TanStack Query 或等价方案。

## Charts and Graphs

- Apache ECharts: <https://echarts.apache.org/>
- ECharts dataset concept: <https://apache.github.io/echarts-handbook/en/concepts/dataset/>
- Cytoscape.js: <https://js.cytoscape.org/>
- Cytoscape.js layouts discussion: <https://blog.js.cytoscape.org/2020/05/11/layouts/>

相关点：

- 聚合图表使用 ECharts 或等价专业图表库。
- 对象图只渲染局部图，避免大图视觉噪声和性能问题。
