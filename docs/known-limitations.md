# Known Limitations

## First-Version Boundaries

- `pygco` is a local single-user tool. It does not provide login, RBAC, remote sharing, multi-user workspaces, or SaaS hosting.
- `pygco-dump` only writes dumps. It does not aggregate, analyze, redact, authorize, or schedule collection.
- SQLite files are temporary analysis artifacts. They are rebuildable from the source dump and are not treated as long-term archival storage.
- Long-term SQLite migrations are out of scope for the first version.
- Remote attach to a Python process is out of scope; the target process must expose or write a dump through its own integration point.

## Analysis Boundaries

- Reachability size is a bounded estimate. Results may be `estimated`, `truncated`, or `unavailable` when referents are missing or limits are hit.
- Owner paths and local graphs are investigative leads, not proof of the only retaining path.
- Findings are heuristic leads. The tool intentionally avoids absolute claims such as confirmed leaks.
- Object-level diff confidence depends on producer identity and dump sequence. Low confidence falls back to aggregate-first interpretation.

## Performance Boundaries

- `pygco-dump` 是进程内观测器，无法做到零开销。主集合快照至少需要 O(N) 强引用；完整 referent
  stub 模式还需要 O(N) 的 object-id 成员索引。producer 自身的这些对象会从 dump 结果中隔离，
  但它们在采样期间仍占用目标进程内存。
- `include_referents=false` 和 `include_referent_stubs=false` 不构造 object-id 成员索引，适合趋势
  采样；完整引用图应避开延迟敏感时段，并在真实工作负载上验证峰值内存。
- The first version is designed around local workstation analysis. Very large dumps may require tuning reachability limits or using `--no-reachability` during import.
- Web UI tables are paginated or virtualized; exporting or viewing an entire object graph in the browser is intentionally unsupported.
- Benchmark targets are measured against synthetic fixtures and should be rechecked on real workloads before relying on them for capacity planning.
