# py-gc-objects-analyze 文档

`py-gc-objects-analyze` 是一个本地离线的 Python GC object 内存分析工具。它面向需要排查 Python 服务内存增长、对象持有链、缓存膨胀、流式响应残留、线程/连接资源残留等问题的开发者。

第一版工具遵循三个原则：

- 被分析的 Python 进程只负责低侵入 dump。
- 所有重计算、索引、聚合、diff 和图遍历都在本地 Rust 工具里完成。
- Web UI 是本地专业分析界面，不是远程 SaaS。

## 文档入口

- [快速开始](quickstart.md)
- [安装与构建](install.md)
- [核心概念](concepts.md)
- [Dump 与 SQLite 数据模型](data-model.md)
- [Dump 文件格式规范](dump-format.md)
- [SQLite Schema 规范](sqlite-schema.md)
- [Local API 规范](api.md)
- [CLI 规范](cli.md)
- [CLI 诊断工作台整改方案](cli-diagnostics-workbench.md)
- [Generated CLI Help](generated/cli-help.md)
- [Generated OpenAPI JSON](generated/openapi.json)
- [分析模型](analysis-model.md)
- [Web UI 规范](web-ui.md)
- [Web UI Walkthrough](web-ui-walkthrough.md)
- [系统架构](architecture.md)
- [性能规范](performance.md)
- [运行安全边界](runtime-safety.md)
- [Known Limitations](known-limitations.md)
- [测试策略](testing.md)
- [POC 反思](poc-retrospective.md)
- [工程规范](project/engineering-standards.md)
- [CLI 诊断工作台技术实施 Spec](project/cli-diagnostics-technical-spec.md)
- [实现蓝图](project/implementation-blueprint.md)
- [POC 迁移指南](project/poc-migration-guide.md)
- [Source Manifest](project/source-manifest.md)
- [References](references/README.md)

## 一句话模型

```text
Python process
  -> pygco_dump writes raw JSONL gzip dump
  -> pygco imports dumps into a fresh temporary SQLite
  -> pygco CLI / local Web UI analyzes objects, references, sizes, diffs
  -> SQLite can be deleted after the investigation
```

## 已确认的一版边界

- 命令名：`pygco`
- Python dump 包发行名：`pygco-dump`
- Python import 名：`pygco_dump`
- 主流程：`pygco open dump-a.jsonl.gz dump-b.jsonl.gz`
- 显式流程：`pygco import dump-a.jsonl.gz dump-b.jsonl.gz -o analysis.sqlite --rebuild` 后 `pygco web analysis.sqlite`
- 发布期 Web UI 静态资源嵌入 Rust binary。
- 开发期 Rust API server 与 React dev server 分开运行。
