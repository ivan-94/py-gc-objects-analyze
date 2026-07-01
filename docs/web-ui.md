# Web UI 规范

Web UI 是 `pygco` 的主要探索界面。它必须面向高密度数据分析，而不是展示型 landing page。

技术栈：

- React
- TypeScript
- shadcn/ui
- Tailwind CSS
- TanStack Router
- TanStack Table
- TanStack Virtual
- TanStack Query
- Apache ECharts
- Cytoscape.js 或等价专业图可视化库

## 设计原则

- 第一屏就是分析工作台，不做营销页。
- 页面信息密度高，但必须可扫描。
- 所有大表必须虚拟滚动或服务端分页。
- 所有图必须是局部图，不渲染全量对象图。
- URL 必须保存筛选、排序、分页、tab、selected object。
- 所有 estimated 值必须标注。
- 所有长 JSON/text 必须通过 drawer/dialog 展开。

## 布局

整体布局：

```text
Top Bar
  Snapshot selector
  Search
  Import/session status

Left Nav
  Overview
  Objects
  Types
  Modules
  Cohorts
  Diff
  Object Graph
  Findings
  SQL
  Report

Content Area
  Toolbar
  Main table/chart/graph
  Detail drawer
```

页面不使用大型 hero、不使用装饰性背景、不使用营销式卡片堆叠。

## 页面列表

### Overview

用途：快速判断 dump 规模和首要调查方向。

必须包含：

- snapshot metadata
- object count
- edge count
- total shallow size
- top types by count
- top types by shallow size
- top non-builtin types
- top modules
- top reachable types
- missing/stub summary
- import warnings

交互：

- 点击 type/module/cohort 进入过滤后的 Objects 页面。
- 点击 metric 查看解释。

### Objects

用途：高性能对象列表。

要求：

- 服务端分页或虚拟滚动。
- 支持 URL query state。
- 支持列选择。
- 支持排序：shallow size、reachable size、in/out edges、type、module。
- 支持筛选：type、module、cohort、size range、degree range、stub、missing referents。
- 每行必须可以打开 object detail drawer。

默认列：

```text
object_id
type
module
shallow_size
estimated_reachable_size
reachable_truncated
in_edges
out_edges
stub
missing_referents
```

禁止：

- 把长 type/module 文本挤成窄列。
- 让空筛选参数造成 API 解析错误。
- 一次请求加载全量 objects。

### Object Detail

用途：查看单个对象和局部引用关系。

必须包含：

- object metadata
- shallow size
- estimated reachable size
- in/out degree
- missing/stub status
- top referents table
- top referrers table
- local graph preview
- owner path samples
- actions

actions：

- copy object id
- open referents
- open referrers
- export subgraph
- add to idset
- query same type

### Types / Modules / Cohorts

用途：聚合维度分析。

必须包含：

- count
- shallow size sum
- estimated reachable size sum
- max reachable size
- in/out edge totals
- top example objects
- diff delta if two snapshots selected

### Diff

用途：比较两个 snapshot。

必须包含：

- snapshot selector: from/to
- summary delta
- type delta
- module delta
- cohort delta
- object lifecycle summary
- link to `diff-objects`

对象级 diff 必须显示可信度提示：

```text
Object id lifecycle is strongest for consecutive dumps from the same process.
```

### Object Graph

用途：局部引用图探索。

要求：

- 只加载局部图。
- 默认 depth <= 2。
- 默认 node limit <= 500。
- missing edge 用不同线型展示。
- stub node 用不同样式展示。
- 支持按 type/module 聚合视图。
- 支持 focus node。
- 支持 expand selected node。

禁止：

- 渲染全量对象图。
- 默认启用重型 force layout。
- 在图里展示不可读的密集文字。

### Findings

用途：启发式线索列表。

表格列：

```text
severity
kind
title
message
action
evidence_summary
```

要求：

- `message` 和 `action` 使用合理宽度，不挤成单词竖排。
- `evidence` 默认显示摘要。
- 完整 evidence 通过 drawer 展开。
- 每条 finding 提供跳转 action。

### SQL

用途：高级只读查询。

必须包含：

- schema browser
- SQL editor
- run button
- explain button
- elapsed time
- result table
- save as idset

约束：

- 只允许只读 query。
- 默认 limit。
- 长查询显示进度和取消。

### Report

用途：生成可复制的 Markdown/JSON 报告。

必须包含：

- snapshot summary
- diff summary
- top leads
- links to relevant UI route
- algorithm parameter section

## URL 状态

以下状态必须进入 URL：

- selected snapshot
- selected diff snapshots
- route
- filters
- sort
- pagination
- selected object id
- graph depth/limit/direction
- SQL query id or saved query id

不要把大型 JSON evidence 放入 URL。

## 表格规范

所有大表必须：

- 服务端排序。
- 服务端过滤。
- 服务端分页或虚拟滚动。
- 支持 column resize。
- 支持 sticky header。
- 数字右对齐。
- size 字段使用人类可读格式，同时保留 raw bytes tooltip。

## Server State

Web UI 使用 TanStack Query 或等价 server-state 库管理 API 请求。

API 访问必须走 `src/generated/api-client.ts`。当 Rust API contract 变化时，在 `web/app` 目录运行 `corepack pnpm generate:api` 从 OpenAPI 重新生成 typed client。

要求：

- pagination/filter/sort 变化时自动取消过期请求。
- 同一 query key 去重。
- snapshot 切换时失效相关 cache。
- SQL、graph、diff 等长请求支持 loading、cancel、retry。
- URL state 属于 TanStack Router，server-state cache 属于 TanStack Query，两者职责分离。

## Drawer/Dialog 规范

用于展示：

- object detail
- full evidence JSON
- long text
- SQL explain plan
- export preview

Drawer 必须支持：

- copy JSON
- open linked route
- close with Escape
- stable width

## 图表规范

聚合图使用 Apache ECharts 或等价库。

推荐图：

- top types bar chart
- module shallow/reachable comparison
- diff waterfall
- cohort size comparison
- import time breakdown

图表不替代表格。每个图都必须能跳转到对应表格过滤结果。

## 性能预算

默认目标：

| 操作 | 目标 |
| --- | --- |
| 首屏 skeleton | < 500 ms |
| Overview API | < 1 s |
| Objects 翻页 | < 300 ms |
| Objects 排序/过滤 | < 1 s |
| Object detail | < 500 ms |
| 局部图加载 | < 1.5 s |
| SQL explain | < 500 ms |

超出预算时 UI 必须显示 loading、progress 或 explainable wait，不允许空白卡死。

## 空状态和错误状态

必须设计：

- no dump imported
- no rows matched
- no reachability cache
- truncated graph
- missing object
- invalid SQL
- long query canceled
- import failed

错误文案要告诉用户下一步行动。

## 发布形态

开发期：

```text
cd web/app && corepack pnpm dev        # React dev server on 127.0.0.1:5173
pygco web analysis.sqlite --dev        # Rust API server on 127.0.0.1:5174
pygco open dump.jsonl.gz --dev         # Import, then serve API for the React dev server
```

发布期：

```text
pygco open dump.jsonl.gz
```

发布版 React build 产物嵌入 Rust binary，由本地 API server 提供静态资源。
