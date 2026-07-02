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

## Internal-only endpoint patterns

生产或类生产环境中，dump endpoint 必须作为内部诊断入口管理。推荐模式：

- 默认关闭：通过 feature flag、环境变量或只在 debug build 中注册路由。
- 只绑定内网：不要通过公网 ingress、CDN、API gateway public route 暴露。
- 复用现有管理面鉴权：如果组织已有 admin plane，放在同一权限边界内。
- 限制调用者：只允许受信 VPN、bastion、Kubernetes exec、临时 port-forward 或内部 SSO 管理用户触发。
- 记录审计：记录触发人、时间、参数、PID、输出文件路径、对象数量和耗时。
- 控制参数：默认 `collect=false`、`include_repr=false`，并为响应大小、超时和并发设置上限。
- 明确保留策略：dump 文件和 SQLite 分析库默认视为敏感诊断产物，用完删除或放入受控存储。

反模式：

- 把 dump endpoint 放在业务 public API 下。
- 允许任意用户传 `include_repr=true`。
- 把 dump 文件上传到公共 issue、聊天工具或无访问控制的对象存储。
- 在高峰流量中无审批地启用 `collect=true`。

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
