# AGENTS.md

## 项目开发原则

本项目采用文档驱动开发和 TDD 开发。

- 文档驱动开发：新增或修改用户可见行为、CLI contract、SQLite schema、API、Web UI、报告格式、测试策略或工程流程前，先更新对应文档或 spec。未实现的目标态命令必须明确标注为规划内容，不能混入当前 CLI 参考。
- TDD 开发：实现功能或修复 bug 时先写失败测试，再实现，再重构。测试应覆盖 JSON contract、人类输出、错误路径、兼容性和关键算法语义。
- 兼容性：SQLite 是可重建分析产物，但 CLI/API/JSON 输出仍应避免无说明的破坏性变更。旧 DB 无新表或新字段时应 graceful fallback 或给出清晰错误。
- 验证：文档变更优先运行 `python3 scripts/check_docs_commands.py`；Rust CLI/analysis 变更优先运行相关 `cargo test -p ...`；Web UI 变更按 `docs/testing.md` 执行对应测试。