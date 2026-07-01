# POC 反思

POC 已证明方向可行：Python runtime dump、SQLite 本地索引、CLI/WebUI 分析可以帮助定位当前对象、引用关系、reachable size、missing/stub、findings。

正式项目不能把 POC 的粗糙部分原样带入。

## POC 做得好的地方

- 验证了 `gc.get_objects()` + `gc.get_referents()` 可以形成可分析对象图。
- 验证了 referent stub 可以减少 missing 节点。
- 验证了本地 SQLite 能承载近百万对象、数百万边的查询。
- 验证了 shallow size 与 reachable size 同时展示更有解释力。
- 验证了对象局部图、findings、SQL/idset 对 Agent 排查有帮助。
- 验证了 WebUI 对人类探索有价值。

## 不应带入正式项目的问题

### 1. Python 分析层过厚

POC 中大量分析逻辑写在 Python 里，性能和工程边界都不适合生产级工具。

正式项目要求：

- dump producer 保持极薄。
- import、聚合、图算法、CLI、API 全部用 Rust。

### 2. Dump spec 不正式

POC dump 字段随需求演化，没有完整版本契约。

正式项目要求：

- 写明 format/version/record types。
- 写明 required/recommended/optional 字段。
- 写明 stub/missing 语义。
- 写明兼容策略。

### 3. SQLite schema 是探索型

POC schema 和索引围绕当前问题临时增加。

正式项目要求：

- schema 从查询模型反推。
- 导入期和查询期索引分开设计。
- SQLite 默认重建，不背 migration 包袱。
- 所有缓存记录算法版本和参数。

### 4. CLI 命令是调试长出来的

POC CLI 功能多，但命名和信息架构不够统一。

正式项目要求：

- command group 清晰。
- 输出格式一致。
- 支持 json/jsonl/table/markdown。
- Agent 友好：`--fields`、`--ids-only`、`schema`、`sql --explain`。

### 5. WebUI 是探索型

POC WebUI 暴露了方向，但不够专业：

- 表格列宽不稳定。
- 长 evidence 会撑爆。
- 图视图缺少明确入口和限制说明。
- 部分页面性能依赖临时优化。

正式项目要求：

- React + shadcn + TanStack Table/Virtual。
- 所有大表服务端分页或虚拟滚动。
- 长文本/JSON 用 drawer。
- 局部图有明确 depth/limit。
- URL 保存状态。

### 6. reachable size 容易被误读

POC 中 reachable size 很有用，但如果不说明算法和限制，容易被当成精确归属。

正式项目要求：

- UI 和 CLI 标注 estimated。
- 保存 depth/limit/truncated/algorithm version。
- 文档说明循环、共享对象和截断。

### 7. 缺少 benchmark 和 golden fixtures

POC 只做了功能验证。

正式项目要求：

- golden dumps。
- import benchmarks。
- query benchmarks。
- WebUI Playwright 验证。

### 8. 缺少明确的运行安全边界

POC 是测试环境里的调试工具。

正式项目要求：

- producer 单飞。
- 默认不 collect。
- 默认不 repr。
- 本地 server 默认只 bind localhost。
- 错误可行动。

## 正式项目验收标准

第一版必须满足：

- 用 `pygco open dump-a.jsonl.gz dump-b.jsonl.gz` 一条命令完成分析入口。
- 支持多个 dump 进入同一个 fresh SQLite session。
- 支持 top types/modules/cohorts。
- 支持 object/referrers/referents/local graph。
- 支持 estimated reachable size。
- 支持 diff 和 diff-objects。
- 支持 SQL/schema/idset。
- WebUI 可以流畅浏览百万级对象索引。
- 文档覆盖用户使用、dump spec、WebUI、架构、性能、测试。
