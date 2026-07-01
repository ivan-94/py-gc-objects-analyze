# 运行安全边界

`pygco` 是内部分析工具，不做过度安全、合规、权限、多租户设计。

本文件只定义必要的运行安全边界：不要误暴露 dump endpoint，不要打挂被分析进程，不要让本地分析工具影响主流程。

## Python Producer

producer 必须：

- 默认流式输出 gzip。
- 同一进程只允许一个 dump 运行。
- 默认 `collect=false`。
- 默认 `include_repr=false`。
- 支持超时或调用方取消。
- 在 dump 开始和结束记录日志。

producer 不做：

- 鉴权系统。
- 脱敏系统。
- 聚合分析。
- 后台定时 dump。

是否暴露 endpoint 由接入项目自己控制。

## 为什么默认关闭 repr

`repr` 默认关闭主要是运行稳定性考虑：

- 可能很慢。
- 可能输出巨大字符串。
- 可能触发用户自定义逻辑。
- 可能让 dump 文件膨胀到不可用。

如需打开，必须显式传参，并设置 `repr_limit`。

## GC collect

`collect=true` 会触发 GC，可能影响被分析进程延迟。

默认行为：

- endpoint 默认 `collect=false`。
- 文档中明确说明 `collect=true` 的影响。
- CLI/WebUI 不假设 dump 一定是在 collect 后生成。

## 本地 API Server

本地 server 默认只绑定：

```text
127.0.0.1
```

第一版不做：

- login
- RBAC
- remote sharing
- multi-user workspace

## 临时文件

`pygco open` 创建 session：

```text
<cache-root>/sessions/<timestamp-random>/
  analysis.sqlite
  import.log
  manifest.json
```

要求：

- 默认 cache root 解析顺序是 `PYGCO_HOME`、`XDG_CACHE_HOME/pygco`、`~/.cache/pygco`。
- 显式 `--session-dir <path>` 使用用户提供的目录。
- 导入中的 SQLite 使用 `.tmp.sqlite`。
- 成功后 rename。
- 失败后清理半成品。
- 用户可以通过 `pygco sessions list` 发现缓存 session，并手动删除对应 session 目录。

## 错误处理

错误信息必须可行动：

- dump 格式错误：指出行号、字段、原因。
- import 错误：指出阶段。
- query 错误：指出 SQL 或参数。
- graph 截断：展示 limit 和如何调整。
- API 错误：包含 `code`、`message`、`details`；可由用户修正时，`details.next_step` 给出下一步行动。
