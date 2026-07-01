# 工程规范

本文定义正式项目的工程化要求。

## Repository Layout

推荐结构：

```text
.
├── crates/
│   ├── pygco-cli/
│   ├── pygco-dump-format/
│   ├── pygco-importer/
│   ├── pygco-store/
│   ├── pygco-analysis/
│   ├── pygco-api/
│   └── pygco-report/
├── python/
│   └── pygco_dump/
├── web/
│   └── app/
├── docs/
├── fixtures/
└── benches/
```

## Rust

要求：

- 使用 Cargo workspace。
- CLI/API/analysis/store 分 crate。
- 公共数据结构集中在 dump-format/store。
- 错误类型必须可读并携带上下文。
- SQL 使用 prepared statements。
- 大文件路径使用 streaming API。

质量门槛：

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace
cargo bench
```

## Python Producer

要求：

- 使用 `pyproject.toml`。
- 包名 `pygco-dump`。
- import 名 `pygco_dump`。
- 不依赖业务框架核心。
- FastAPI helper 放在可选 extra。

质量门槛：

```bash
ruff check
pytest
```

## Web

要求：

- React + TypeScript + Vite。
- shadcn/ui。
- TanStack Router/Table/Virtual/Query。
- 图表/图可视化库按文档选型。
- API client 类型化。
- Rust API 导出 OpenAPI JSON，Web 通过生成类型或等价机制消费。
- URL state schema 化。

质量门槛：

```bash
cd web/app
corepack pnpm lint
corepack pnpm typecheck
corepack pnpm test
corepack pnpm build
corepack pnpm test:e2e
```

## Documentation-Driven Development

新能力开发顺序：

1. 更新 docs 中的用户语义和规范。
2. 写 golden fixture 或验收用例。
3. 实现 Rust/Python/Web。
4. 更新 CLI help/API schema。
5. 跑测试和 benchmark。
6. 更新 docs 示例。

禁止先写实现再反推文档。

## Versioning

需要独立版本：

- tool version
- dump format version
- SQLite schema version
- reachability algorithm version
- cohort rules version

## Compatibility

第一版不承诺长期 SQLite schema migration。

承诺：

- 同 major dump format 可导入。
- SQLite 可通过重新导入 dump 重建。
- CLI 输出字段变更需要 changelog。

## Release

发布产物：

- `pygco` Rust binary
- embedded Web UI assets
- `pygco-dump` Python package

发布版必须能离线使用本地 dump。
