# Local API 规范

Implementation contract: this document is the first-version contract between the Rust local API server and React Web UI.

本地 API 由 Rust `pygco-api` 提供，服务 React Web UI。默认绑定 `127.0.0.1`。

## 通用约定

Base URL：

```text
http://127.0.0.1:<port>/api
```

所有 object id 在 JSON 中序列化为 string。

成功响应：

```json
{
  "data": {},
  "meta": {}
}
```

错误响应：

```json
{
  "error": {
    "code": "invalid_filter",
    "message": "min_shallow_size must be an integer",
    "details": {
      "field": "min_shallow_size",
      "expected": "integer",
      "next_step": "Pass min_shallow_size as an integer or leave it empty."
    }
  }
}
```

`details` 必须保留 machine-readable context；当错误可由用户修正时，`details.next_step` 应给出 Web UI、脚本或自动化工具可直接展示的下一步行动。

常见错误场景和人工排查路径见 [Troubleshooting](troubleshooting.md)。

分页：

```json
{
  "data": [],
  "meta": {
    "limit": 100,
    "offset": 0,
    "total": 944384,
    "truncated": false
  }
}
```

## ID Serialization

Python `id(obj)` 可能超过 JavaScript safe integer。API 必须把所有 object id、from id、to id 序列化为 string：

```json
{
  "object_id": "140067815097616"
}
```

Rust 内部和 SQLite 仍使用 signed 64-bit integer。

## Endpoints

### GET `/session`

返回当前分析 session。

```json
{
  "data": {
    "database_path": ".../analysis.sqlite",
    "schema_version": 1,
    "tool_version": "0.1.0"
  }
}
```

### GET `/snapshots`

列出 snapshots。

### GET `/summary`

Query：

```text
snapshot_id=1
```

返回 overview 所需数据。

### GET `/objects`

Query：

```text
snapshot_id=1
q=
type=
module=
cohort=
min_shallow_size=
min_reachable_size=
min_in_edges=
min_out_edges=
stub=
missing_referents=
sort=reachable_size
order=desc
limit=100
offset=0
```

空字符串 filter 必须按未设置处理。

### GET `/objects/{object_id}`

Query：

```text
snapshot_id=1
```

返回 object detail。

### GET `/objects/{object_id}/edges`

Query：

```text
snapshot_id=1
direction=referents|referrers
limit=100
offset=0
```

### GET `/objects/{object_id}/paths`

Query：

```text
snapshot_id=1
direction=referrers|referents
depth=5
fanout_limit=30
limit=50
include_core=false
```

### GET `/graph`

Query：

```text
snapshot_id=1
root_object_id=140067815097616
direction=both
depth=2
node_limit=500
edge_limit=2000
```

响应必须包含：

- nodes
- edges
- missing_edges
- truncated

### GET `/types`

列出 type stats。

### GET `/modules`

列出 module stats。

### GET `/cohorts`

列出 cohort stats。

### GET `/diff`

Query：

```text
from_snapshot_id=1
to_snapshot_id=2
limit=100
```

### GET `/diff/objects`

Query：

```text
from_snapshot_id=1
to_snapshot_id=2
state=new|gone|retained|changed
type=
module=
sort=reachable_size
order=desc
limit=100
offset=0
```

### GET `/findings`

Query：

```text
snapshot_id=1
kind=
severity=
limit=100
offset=0
```

返回持久化 `findings` 表中的启发式线索。`kind` 和 `severity` 必须是已知枚举；空字符串按未设置处理。每行包含 `evidence` 和顶层 `links`，其中 `links` 从 `evidence.links` 提升而来。

### POST `/sql/query`

Request：

```json
{
  "query": "select object_id, type from objects limit 10",
  "limit": 1000
}
```

只允许只读 SQL。

### POST `/sql/explain`

返回 SQLite query plan。

### POST `/idset`

Request：

```json
{
  "snapshot_id": 1,
  "left_query": "select object_id from objects where type = 'cachetools.LRUCache'",
  "right_query": "select from_id as object_id from edges where snapshot_id = 1 and to_id = 140067815108736",
  "op": "intersect",
  "details": true,
  "limit": 100
}
```

两侧 query 必须返回一列 object id，列名推荐为 `object_id`。

### GET `/saved-idsets`

Query:

- `snapshot_id`

返回当前 snapshot 下已保存的 idset 列表。

### POST `/saved-idsets`

Request：

```json
{
  "snapshot_id": 1,
  "name": "cache candidates",
  "object_ids": ["1", "2", "3"],
  "source": {
    "kind": "sql",
    "query": "select object_id from objects where type like '%Cache%'"
  }
}
```

显式保存 Web UI 中的 SQL/idset 结果。`object_ids` 会去重后写入 `saved_idset_objects`。

### GET `/saved-idsets/{idset_id}`

返回保存的 idset metadata、object ids 和按当前 snapshot hydrate 后的对象行。

### GET `/schema`

返回 SQLite schema 摘要，供 SQL 页面使用。

### GET `/report.md`

返回 Markdown report。

### GET `/report.json`

返回结构化 report，包括：

- `summary`
- `findings`
- `finding_evidence_schema`
- `algorithm_parameters`

### POST `/reachability/recompute`

重新计算某个 snapshot 的默认 referents reachability cache，并返回 long-running job。请求：

```json
{
  "snapshot_id": 1,
  "depth": 3,
  "node_limit": 10000,
  "fanout_limit": 1000
}
```

`depth`、`node_limit`、`fanout_limit` 省略时使用默认值。取消 job 时，未完成的 recompute 不会删除已有 reachability cache。

## Long Running Jobs

超过 2 秒的操作应返回 job：

```json
{
  "data": {
    "job_id": "01HX...",
    "status": "running"
  }
}
```

Job endpoints：

```text
GET /api/jobs/{job_id}
POST /api/jobs/{job_id}/cancel
```

用于：

- full reachability recompute
- expensive SQL
- large export
- report generation

## OpenAPI

Rust API 必须能导出 OpenAPI JSON，供 Web typed client 生成类型。

开发期同步 Web typed client：

```bash
cd web/app
corepack pnpm generate:api
```

该命令通过 `pygco-api` 的 OpenAPI example 生成 `web/app/src/generated/openapi.json` 和 `web/app/src/generated/api-client.ts`。Web UI 只应通过生成客户端访问 `/api/*`，避免页面代码手写 endpoint 字符串。
