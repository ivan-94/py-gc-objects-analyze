# 测试策略

`pygco` 必须用测试保证 dump 兼容、导入正确、分析语义稳定、Web UI 可用。

## Golden Dumps

维护一组固定 dump：

```text
fixtures/golden/
  tiny-v1.jsonl.gz
  stubs-v1.jsonl.gz
  missing-referents-v1.jsonl.gz
  cycles-v1.jsonl.gz
  diff-before-v1.jsonl.gz
  diff-after-v1.jsonl.gz
```

每个 golden dump 必须有 expected 文件：

```text
expected/
  summary.json
  objects.json
  diff.json
  reachability.json
```

## Dump Format Tests

覆盖：

- start metadata 必填字段。
- object record 必填字段。
- end metadata 必填字段。
- unknown version 拒绝。
- optional field forward compatibility。
- malformed JSONL 行号报错。
- duplicate object id 报错。

## Import Tests

覆盖：

- objects count。
- edges count。
- stub count。
- missing referent count。
- type/module stats。
- snapshot metadata。
- sha256 记录。
- rebuild 行为。
- import failure cleanup。

## Analysis Tests

覆盖：

- shallow size aggregation。
- reachable size with cycles。
- reachable truncation。
- one-hop edges。
- referrers/referents。
- owner path sampling。
- idset operations。
- SQL read-only guard。
- diff semantics。

## CLI Tests

每个 CLI command 至少有：

- JSON 输出测试。
- JSONL 输出测试。
- 参数错误测试。
- smoke test。

`pygco open` 需要有不打开浏览器的 headless 测试模式。

## Web UI Tests

使用 Playwright 覆盖：

- Overview 渲染。
- Objects 筛选和排序。
- Object detail drawer。
- Findings evidence drawer。
- Diff 页面。
- Object Graph 局部图。
- SQL explain。
- URL state roundtrip。

视觉要求：

- 表格文本不挤压成单词竖排。
- 长 JSON 不撑破列宽。
- 图页面不空白。
- loading/error/empty 状态可见。

## Benchmark Tests

CI 可以跑小型 benchmark。大型 benchmark 手动或 nightly 跑。

必须记录：

- import throughput。
- query latency。
- Web API latency。
- bundle size。

