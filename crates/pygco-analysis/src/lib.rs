use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use pygco_store::{
    ensure_readonly_sql, now_rfc3339, resolve_snapshot_id, rows_to_json,
    schema_summary as store_schema_summary, value_ref_to_json, StoreError,
};

pub const REACHABILITY_ALGORITHM_VERSION: i64 = 1;
pub const DEFAULT_REACHABILITY_DEPTH: i64 = 3;
pub const DEFAULT_REACHABILITY_NODE_LIMIT: i64 = 10_000;
pub const DEFAULT_REACHABILITY_FANOUT_LIMIT: i64 = 1_000;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("operation canceled")]
    Canceled,
    #[error("object not found in snapshot {snapshot_id}: {object_id}")]
    ObjectNotFound { snapshot_id: i64, object_id: i64 },
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("invalid idset operation: {0}")]
    InvalidIdsetOp(String),
}

pub type Result<T> = std::result::Result<T, AnalysisError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReachabilityParams {
    pub algorithm_version: i64,
    pub depth: i64,
    pub node_limit: i64,
    pub fanout_limit: i64,
}

impl Default for ReachabilityParams {
    fn default() -> Self {
        Self {
            algorithm_version: REACHABILITY_ALGORITHM_VERSION,
            depth: DEFAULT_REACHABILITY_DEPTH,
            node_limit: DEFAULT_REACHABILITY_NODE_LIMIT,
            fanout_limit: DEFAULT_REACHABILITY_FANOUT_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    CohortSignal,
    LargeType,
    LargeObject,
    HighOutDegree,
    HighInDegree,
    MissingReferents,
    StubHeavyType,
    DiffGrowth,
}

impl FindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingKind::CohortSignal => "cohort_signal",
            FindingKind::LargeType => "large_type",
            FindingKind::LargeObject => "large_object",
            FindingKind::HighOutDegree => "high_out_degree",
            FindingKind::HighInDegree => "high_in_degree",
            FindingKind::MissingReferents => "missing_referents",
            FindingKind::StubHeavyType => "stub_heavy_type",
            FindingKind::DiffGrowth => "diff_growth",
        }
    }

    pub fn values() -> &'static [&'static str] {
        &[
            "cohort_signal",
            "large_type",
            "large_object",
            "high_out_degree",
            "high_in_degree",
            "missing_referents",
            "stub_heavy_type",
            "diff_growth",
        ]
    }

    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "cohort_signal" => Ok(FindingKind::CohortSignal),
            "large_type" => Ok(FindingKind::LargeType),
            "large_object" => Ok(FindingKind::LargeObject),
            "high_out_degree" => Ok(FindingKind::HighOutDegree),
            "high_in_degree" => Ok(FindingKind::HighInDegree),
            "missing_referents" => Ok(FindingKind::MissingReferents),
            "stub_heavy_type" => Ok(FindingKind::StubHeavyType),
            "diff_growth" => Ok(FindingKind::DiffGrowth),
            other => Err(AnalysisError::InvalidQuery(format!(
                "invalid finding kind: {other}; expected one of {}",
                FindingKind::values().join(", ")
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warn,
}

impl FindingSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingSeverity::Info => "info",
            FindingSeverity::Warn => "warn",
        }
    }

    pub fn values() -> &'static [&'static str] {
        &["info", "warn"]
    }

    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "info" => Ok(FindingSeverity::Info),
            "warn" => Ok(FindingSeverity::Warn),
            other => Err(AnalysisError::InvalidQuery(format!(
                "invalid finding severity: {other}; expected one of {}",
                FindingSeverity::values().join(", ")
            ))),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FindingsOptions {
    pub snapshot_id: Option<i64>,
    pub kind: Option<String>,
    pub severity: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl FindingsOptions {
    fn normalized(mut self) -> Self {
        self.kind = nonempty(self.kind);
        self.severity = nonempty(self.severity);
        if self.limit <= 0 {
            self.limit = 100;
        }
        if self.offset < 0 {
            self.offset = 0;
        }
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuspectsOptions {
    pub snapshot_id: Option<i64>,
    pub kinds: Vec<String>,
    pub min_reachable_size: i64,
    pub non_builtin: bool,
    pub include_stub: bool,
    pub limit: i64,
    pub offset: i64,
}

impl SuspectsOptions {
    fn normalized(mut self) -> Self {
        self.kinds = self
            .kinds
            .into_iter()
            .filter_map(|kind| nonempty(Some(kind)))
            .collect();
        if self.min_reachable_size < 0 {
            self.min_reachable_size = 0;
        }
        if self.min_reachable_size == 0 {
            self.min_reachable_size = 1024 * 1024;
        }
        if self.limit <= 0 {
            self.limit = 20;
        }
        if self.offset < 0 {
            self.offset = 0;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SuspectKind {
    OrphanRetained,
    HighRetainedRoot,
    TruncatedRoot,
    TypeFootprint,
    MetadataHeavy,
    CacheHeavy,
    AsyncBacklog,
    ConnectionHeavy,
}

impl SuspectKind {
    fn as_str(self) -> &'static str {
        match self {
            SuspectKind::OrphanRetained => "orphan_retained",
            SuspectKind::HighRetainedRoot => "high_retained_root",
            SuspectKind::TruncatedRoot => "truncated_root",
            SuspectKind::TypeFootprint => "type_footprint",
            SuspectKind::MetadataHeavy => "metadata_heavy",
            SuspectKind::CacheHeavy => "cache_heavy",
            SuspectKind::AsyncBacklog => "async_backlog",
            SuspectKind::ConnectionHeavy => "connection_heavy",
        }
    }

    fn values() -> &'static [&'static str] {
        &[
            "orphan_retained",
            "high_retained_root",
            "truncated_root",
            "type_footprint",
            "metadata_heavy",
            "cache_heavy",
            "async_backlog",
            "connection_heavy",
        ]
    }

    fn default_kinds() -> &'static [SuspectKind] {
        &[
            SuspectKind::OrphanRetained,
            SuspectKind::HighRetainedRoot,
            SuspectKind::MetadataHeavy,
            SuspectKind::CacheHeavy,
            SuspectKind::AsyncBacklog,
            SuspectKind::ConnectionHeavy,
            SuspectKind::TruncatedRoot,
        ]
    }

    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "orphan_retained" => Ok(SuspectKind::OrphanRetained),
            "high_retained_root" | "high_retained" => Ok(SuspectKind::HighRetainedRoot),
            "truncated_root" | "truncated" => Ok(SuspectKind::TruncatedRoot),
            "type_footprint" | "type" => Ok(SuspectKind::TypeFootprint),
            "metadata_heavy" | "metadata" => Ok(SuspectKind::MetadataHeavy),
            "cache_heavy" | "cache" => Ok(SuspectKind::CacheHeavy),
            "async_backlog" | "async" => Ok(SuspectKind::AsyncBacklog),
            "connection_heavy" | "connection" | "connections" => Ok(SuspectKind::ConnectionHeavy),
            other => Err(AnalysisError::InvalidQuery(format!(
                "invalid suspect kind: {other}; expected one of {}",
                SuspectKind::values().join(", ")
            ))),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectFilters {
    pub snapshot_id: Option<i64>,
    pub q: Option<String>,
    pub type_name: Option<String>,
    pub module: Option<String>,
    pub cohort: Option<String>,
    pub min_shallow_size: Option<i64>,
    pub min_reachable_size: Option<i64>,
    pub min_in_edges: Option<i64>,
    pub min_out_edges: Option<i64>,
    pub has_referrers: bool,
    pub missing_referents: bool,
    pub stub: Option<bool>,
    pub sort: String,
    pub order: String,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffObjectsOptions {
    pub from_snapshot_id: i64,
    pub to_snapshot_id: i64,
    pub state: String,
    pub type_name: Option<String>,
    pub module: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl ObjectFilters {
    pub fn normalized(mut self) -> Self {
        self.q = nonempty(self.q);
        self.type_name = nonempty(self.type_name);
        self.module = nonempty(self.module);
        self.cohort = nonempty(self.cohort);
        if self.sort.is_empty() {
            self.sort = "reachable_size".to_owned();
        }
        if self.order.is_empty() {
            self.order = "desc".to_owned();
        }
        if self.limit <= 0 {
            self.limit = 100;
        }
        if self.offset < 0 {
            self.offset = 0;
        }
        self
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

pub fn summary(conn: &Connection, snapshot_id: Option<i64>, limit: i64) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    let snapshot = snapshot(conn, sid)?;
    Ok(json!({
        "snapshot": snapshot,
        "top_types_by_count": top_types(conn, sid, "count", limit, false)?,
        "top_types_by_shallow_size": top_types(conn, sid, "shallow_size", limit, false)?,
        "top_non_builtin_types_by_count": top_types(conn, sid, "count", limit, true)?,
        "top_non_builtin_types_by_shallow_size": top_types(conn, sid, "shallow_size", limit, true)?,
        "top_modules_by_count": top_modules(conn, sid, "count", limit)?,
        "top_modules_by_shallow_size": top_modules(conn, sid, "shallow_size", limit)?,
        "top_reachable_types": top_reachable_types(conn, sid, limit, false)?,
        "top_non_builtin_reachable_types": top_reachable_types(conn, sid, limit, true)?,
        "cohorts": cohorts(conn, sid, "shallow_size", limit)?,
        "missing_stub_summary": missing_stub_summary(conn, sid)?,
        "import_warnings": import_warnings(conn, sid)?,
        "rss_gap_note": "GC object dumps only cover Python GC-tracked objects and do not explain full RSS."
    }))
}

pub fn snapshot(conn: &Connection, snapshot_id: i64) -> Result<Value> {
    pygco_store::snapshot_row(conn, snapshot_id).map_err(Into::into)
}

pub fn snapshots(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT * FROM snapshots ORDER BY snapshot_id")?;
    let mut rows = stmt.query([])?;
    Ok(json!({ "rows": rows_to_json(&mut rows)? }))
}

pub fn top_types(
    conn: &Connection,
    snapshot_id: i64,
    by: &str,
    limit: i64,
    exclude_builtins: bool,
) -> Result<Value> {
    let order = match by {
        "count" => "ts.count",
        "reachable_size" | "estimated_reachable" => "estimated_reachable_size_sum",
        "in_edges" => "ts.in_edges",
        "out_edges" => "ts.out_edges",
        _ => "ts.shallow_size_sum",
    };
    let noise = if exclude_builtins {
        "AND ts.module NOT IN ('builtins','abc','types','typing','weakref','enum','_thread')"
    } else {
        ""
    };
    let sql = format!(
        "
        SELECT ts.type,
               ts.module,
               ts.count,
               ts.shallow_size_sum,
               ts.in_edges,
               ts.out_edges,
               ts.stub_count,
               COALESCE(trs.reachable_size_sum, 0) AS estimated_reachable_size_sum,
               COALESCE(trs.reachable_size_avg, 0) AS estimated_reachable_size_avg,
               COALESCE(trs.reachable_size_max, 0) AS estimated_reachable_size_max,
               COALESCE(trs.truncated_count, 0) AS reachable_truncated_count
        FROM type_stats ts
        LEFT JOIN type_reachability_stats trs
          ON trs.snapshot_id = ts.snapshot_id
         AND trs.type = ts.type
         AND trs.algorithm_version = ?2
         AND trs.direction = 'referents'
         AND trs.depth = ?3
         AND trs.node_limit = ?4
         AND trs.fanout_limit = ?5
        WHERE ts.snapshot_id = ?1
        {noise}
        ORDER BY {order} DESC, ts.type ASC
        LIMIT ?6
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        snapshot_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT,
        limit
    ])?;
    Ok(Value::Array(rows_to_json(&mut rows)?))
}

pub fn top_modules(conn: &Connection, snapshot_id: i64, by: &str, limit: i64) -> Result<Value> {
    let order = match by {
        "count" => "ms.count",
        "reachable_size" => "estimated_reachable_size_sum",
        _ => "ms.shallow_size_sum",
    };
    let sql = format!(
        "
        WITH reachability AS (
          SELECT module,
                 SUM(reachable_size_sum) AS estimated_reachable_size_sum,
                 MAX(reachable_size_max) AS estimated_reachable_size_max,
                 SUM(truncated_count) AS reachable_truncated_count
          FROM type_reachability_stats
          WHERE snapshot_id = ?1
            AND algorithm_version = ?2
            AND direction = 'referents'
            AND depth = ?3
            AND node_limit = ?4
            AND fanout_limit = ?5
          GROUP BY module
        )
        SELECT ms.module,
               ms.count,
               ms.shallow_size_sum,
               ms.in_edges,
               ms.out_edges,
               COALESCE(r.estimated_reachable_size_sum, 0) AS estimated_reachable_size_sum,
               COALESCE(r.estimated_reachable_size_max, 0) AS estimated_reachable_size_max,
               COALESCE(r.reachable_truncated_count, 0) AS reachable_truncated_count
        FROM module_stats ms
        LEFT JOIN reachability r ON r.module = ms.module
        WHERE ms.snapshot_id = ?1
        ORDER BY {order} DESC, ms.module ASC
        LIMIT ?6
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        snapshot_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT,
        limit
    ])?;
    Ok(Value::Array(rows_to_json(&mut rows)?))
}

pub fn top_reachable_types(
    conn: &Connection,
    snapshot_id: i64,
    limit: i64,
    exclude_builtins: bool,
) -> Result<Value> {
    let noise = if exclude_builtins {
        "AND module NOT IN ('builtins','abc','types','typing','weakref','enum','_thread')"
    } else {
        ""
    };
    let sql = format!(
        "
        SELECT type,
               module,
               count,
               shallow_size_sum,
               reachable_size_sum AS estimated_reachable_size_sum,
               reachable_size_avg AS estimated_reachable_size_avg,
               reachable_size_max AS estimated_reachable_size_max,
               truncated_count AS reachable_truncated_count,
               depth,
               node_limit,
               fanout_limit,
               algorithm_version
        FROM type_reachability_stats
        WHERE snapshot_id = ?1
          AND algorithm_version = ?2
          AND direction = 'referents'
          AND depth = ?3
          AND node_limit = ?4
          AND fanout_limit = ?5
          {noise}
        ORDER BY reachable_size_sum DESC, reachable_size_max DESC, type ASC
        LIMIT ?6
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        snapshot_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT,
        limit
    ])?;
    Ok(Value::Array(rows_to_json(&mut rows)?))
}

pub fn cohorts(conn: &Connection, snapshot_id: i64, by: &str, limit: i64) -> Result<Value> {
    let order = match by {
        "count" => "cs.count",
        "reachable_size" | "estimated_reachable" => "estimated_reachable_size_sum",
        _ => "cs.shallow_size_sum",
    };
    let sql = format!(
        "
        WITH cohort_types AS (
          SELECT cs.cohort,
                 json_extract(value, '$.type') AS type
          FROM cohort_stats cs,
               json_each(json_extract(cs.details_json, '$.types'))
          WHERE cs.snapshot_id = ?1
        ),
        reachability AS (
          SELECT ct.cohort,
                 SUM(trs.reachable_size_sum) AS estimated_reachable_size_sum,
                 MAX(trs.reachable_size_max) AS estimated_reachable_size_max,
                 SUM(trs.truncated_count) AS reachable_truncated_count
          FROM cohort_types ct
          JOIN type_reachability_stats trs
            ON trs.snapshot_id = ?1
           AND trs.type = ct.type
           AND trs.algorithm_version = ?2
           AND trs.direction = 'referents'
           AND trs.depth = ?3
           AND trs.node_limit = ?4
           AND trs.fanout_limit = ?5
          GROUP BY ct.cohort
        )
        SELECT cs.cohort,
               cs.count,
               cs.shallow_size_sum,
               cs.type_count,
               cs.details_json,
               cs.rules_version,
               COALESCE(r.estimated_reachable_size_sum, 0) AS estimated_reachable_size_sum,
               COALESCE(r.estimated_reachable_size_max, 0) AS estimated_reachable_size_max,
               COALESCE(r.reachable_truncated_count, 0) AS reachable_truncated_count
        FROM cohort_stats cs
        LEFT JOIN reachability r ON r.cohort = cs.cohort
        WHERE cs.snapshot_id = ?1
        ORDER BY {order} DESC, cs.count DESC, cs.cohort ASC
        LIMIT ?6
        ",
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        snapshot_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT,
        limit
    ])?;
    let mut values = rows_to_json(&mut rows)?;
    for value in &mut values {
        if let Some(object) = value.as_object_mut() {
            if let Some(details) = object.remove("details_json") {
                let parsed = details
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                    .unwrap_or(Value::Null);
                object.insert("details".to_owned(), parsed);
            }
        }
    }
    Ok(Value::Array(values))
}

fn import_warnings(conn: &Connection, snapshot_id: i64) -> Result<Value> {
    let mut stmt = conn.prepare(
        "
        SELECT level, code, message, context_json, created_at
        FROM import_warnings
        WHERE snapshot_id = ?1 OR snapshot_id IS NULL
        ORDER BY warning_id
        ",
    )?;
    let mut rows = stmt.query([snapshot_id])?;
    Ok(Value::Array(rows_to_json(&mut rows)?))
}

fn missing_stub_summary(conn: &Connection, snapshot_id: i64) -> Result<Value> {
    let (stub_count, missing_count): (i64, i64) = conn.query_row(
        "
        SELECT stub_count, missing_referent_count
        FROM snapshots
        WHERE snapshot_id = ?1
        ",
        [snapshot_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(json!({
        "stub_count": stub_count,
        "missing_referent_count": missing_count,
    }))
}

pub fn list_objects(conn: &Connection, filters: ObjectFilters) -> Result<Value> {
    let filters = filters.normalized();
    let sid = resolve_snapshot_id(conn, filters.snapshot_id)?;
    if object_list_metrics_available(conn, sid)? {
        return list_objects_with_metrics(conn, &filters, sid);
    }
    if object_query_can_drive_from_reachability(conn, &filters, sid)? {
        return list_objects_sorted_by_reachability(conn, &filters, sid);
    }
    if object_query_needs_global_edge_stats(&filters) && object_edge_stats_available(conn, sid)? {
        return list_objects_with_materialized_edge_stats(conn, &filters, sid);
    }
    if object_query_needs_global_edge_stats(&filters) {
        return list_objects_with_global_edge_stats(conn, &filters, sid);
    }
    list_objects_fast(conn, &filters, sid)
}

fn list_objects_with_metrics(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<Value> {
    let (where_sql, params) = object_metrics_where_clause(filters, sid, 4);
    let order = object_metrics_order_for(&filters.sort);
    let tie_break = object_metrics_order_tie_break(order);
    let direction = if filters.order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let index_hint = object_metrics_index_hint(filters);
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               m.reachable_count AS estimated_reachable_count,
               m.reachable_size AS estimated_reachable_size,
               m.reachable_truncated AS reachable_truncated,
               m.in_edges,
               m.out_edges,
               m.missing_referents
        FROM object_list_metrics AS m {index_hint}
        JOIN objects o
          ON o.snapshot_id = m.snapshot_id
         AND o.object_id = m.object_id
        {where_sql}
        ORDER BY {order} {direction}{tie_break}
        LIMIT ?2 OFFSET ?3
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(filters.limit),
        SqlValue::Integer(filters.offset),
    ];
    all_params.extend(params);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(all_params.iter()))?;
    let data = rows_to_json(&mut rows)?;
    let total = count_objects_from_metrics(conn, filters, sid)?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": filters.limit,
        "offset": filters.offset,
    }))
}

fn list_objects_sorted_by_reachability(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<Value> {
    let (where_sql, params) = object_where_clause(filters, sid, 8);
    let where_sql = where_sql.replacen("WHERE o.snapshot_id = ?1", "WHERE r.snapshot_id = ?1", 1);
    let order = reachability_order(&filters.sort);
    let tie_break = object_order_tie_break(order);
    let direction = if filters.order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let edge_stats_join = if object_edge_stats_available(conn, sid)? {
        "
        LEFT JOIN object_edge_stats d
          ON d.snapshot_id = o.snapshot_id
         AND d.object_id = o.object_id
        "
    } else {
        ""
    };
    let (in_edges, out_edges, missing_referents) = if edge_stats_join.is_empty() {
        (
            "(SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.to_id = o.object_id)",
            "(SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.from_id = o.object_id)",
            "(SELECT COUNT(*) FROM edges e LEFT JOIN objects target ON target.snapshot_id = e.snapshot_id AND target.object_id = e.to_id WHERE e.snapshot_id = o.snapshot_id AND e.from_id = o.object_id AND target.object_id IS NULL)",
        )
    } else {
        (
            "COALESCE(d.in_edges, 0)",
            "COALESCE(d.out_edges, 0)",
            "COALESCE(d.missing_referents, 0)",
        )
    };
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               r.reachable_count AS estimated_reachable_count,
               r.reachable_size AS estimated_reachable_size,
               r.truncated AS reachable_truncated,
               {in_edges} AS in_edges,
               {out_edges} AS out_edges,
               {missing_referents} AS missing_referents
        FROM object_reachability AS r INDEXED BY idx_object_reachability_size
        JOIN objects o
          ON o.snapshot_id = r.snapshot_id
         AND o.object_id = r.object_id
        {edge_stats_join}
        {where_sql}
          AND r.algorithm_version = ?2
          AND r.direction = 'referents'
          AND r.depth = ?3
          AND r.node_limit = ?4
          AND r.fanout_limit = ?5
        ORDER BY {order} {direction}{tie_break}
        LIMIT ?6 OFFSET ?7
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        SqlValue::Integer(filters.limit),
        SqlValue::Integer(filters.offset),
    ];
    all_params.extend(params.clone());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(all_params.iter()))?;
    let data = rows_to_json(&mut rows)?;
    let total = count_objects_fast(conn, filters, sid)?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": filters.limit,
        "offset": filters.offset,
    }))
}

fn list_objects_with_materialized_edge_stats(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<Value> {
    let (where_sql, params) = object_where_clause(filters, sid, 8);
    let reachability_available = object_reachability_available(conn, sid)?;
    let order = object_order_for(&filters.sort, reachability_available);
    let tie_break = object_order_tie_break(order);
    let edge_sort_index = edge_stats_sort_index(&filters.sort);
    let from_sql = if object_query_sorts_by_edge_stats(filters) {
        format!(
            "
        FROM object_edge_stats AS d INDEXED BY {edge_sort_index}
        JOIN objects o
          ON o.snapshot_id = d.snapshot_id
         AND o.object_id = d.object_id
        "
        )
    } else if object_query_filters_by_edge_stats(filters) {
        "
        FROM objects o
        CROSS JOIN object_edge_stats d
          ON d.snapshot_id = o.snapshot_id
         AND d.object_id = o.object_id
        "
        .to_owned()
    } else {
        "
        FROM objects o
        LEFT JOIN object_edge_stats d
          ON d.snapshot_id = o.snapshot_id
         AND d.object_id = o.object_id
        "
        .to_owned()
    };
    let direction = if filters.order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               COALESCE(r.reachable_count, 0) AS estimated_reachable_count,
               COALESCE(r.reachable_size, 0) AS estimated_reachable_size,
               COALESCE(r.truncated, 0) AS reachable_truncated,
               COALESCE(d.in_edges, 0) AS in_edges,
               COALESCE(d.out_edges, 0) AS out_edges,
               COALESCE(d.missing_referents, 0) AS missing_referents
        {from_sql}
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        {where_sql}
        ORDER BY {order} {direction}{tie_break}
        LIMIT ?6 OFFSET ?7
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        SqlValue::Integer(filters.limit),
        SqlValue::Integer(filters.offset),
    ];
    all_params.extend(params.clone());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(all_params.iter()))?;
    let data = rows_to_json(&mut rows)?;
    let total = count_objects_with_materialized_edge_stats(conn, filters, sid)?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": filters.limit,
        "offset": filters.offset,
    }))
}

fn list_objects_fast(conn: &Connection, filters: &ObjectFilters, sid: i64) -> Result<Value> {
    let (where_sql, params) = object_where_clause(filters, sid, 8);
    let reachability_available = object_reachability_available(conn, sid)?;
    let order = object_order_for(&filters.sort, reachability_available);
    let tie_break = object_order_tie_break(order);
    let direction = if filters.order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               COALESCE(r.reachable_count, 0) AS estimated_reachable_count,
               COALESCE(r.reachable_size, 0) AS estimated_reachable_size,
               COALESCE(r.truncated, 0) AS reachable_truncated,
               (SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.to_id = o.object_id) AS in_edges,
               (SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.from_id = o.object_id) AS out_edges,
               (SELECT COUNT(*)
                FROM edges e
                LEFT JOIN objects target
                  ON target.snapshot_id = e.snapshot_id
                 AND target.object_id = e.to_id
                WHERE e.snapshot_id = o.snapshot_id
                  AND e.from_id = o.object_id
                  AND target.object_id IS NULL) AS missing_referents
        FROM objects o
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        {where_sql}
        ORDER BY {order} {direction}{tie_break}
        LIMIT ?6 OFFSET ?7
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        SqlValue::Integer(filters.limit),
        SqlValue::Integer(filters.offset),
    ];
    all_params.extend(params.clone());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(all_params.iter()))?;
    let data = rows_to_json(&mut rows)?;
    let total = count_objects_fast(conn, filters, sid)?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": filters.limit,
        "offset": filters.offset,
    }))
}

fn list_objects_with_global_edge_stats(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<Value> {
    let (where_sql, params) = object_where_clause(filters, sid, 8);
    let reachability_available = object_reachability_available(conn, sid)?;
    let order = object_order_for(&filters.sort, reachability_available);
    let tie_break = object_order_tie_break(order);
    let direction = if filters.order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sql = format!(
        "
        WITH degrees AS (
          SELECT o.object_id,
                 COALESCE(i.in_edges, 0) AS in_edges,
                 COALESCE(out.out_edges, 0) AS out_edges,
                 COALESCE(missing.missing_referents, 0) AS missing_referents
          FROM objects o
          LEFT JOIN (
            SELECT to_id AS object_id, COUNT(*) AS in_edges
            FROM edges
            WHERE snapshot_id = ?1
            GROUP BY to_id
          ) i ON i.object_id = o.object_id
          LEFT JOIN (
            SELECT from_id AS object_id, COUNT(*) AS out_edges
            FROM edges
            WHERE snapshot_id = ?1
            GROUP BY from_id
          ) out ON out.object_id = o.object_id
          LEFT JOIN (
            SELECT e.from_id AS object_id, COUNT(*) AS missing_referents
            FROM edges e
            LEFT JOIN objects target
              ON target.snapshot_id = e.snapshot_id
             AND target.object_id = e.to_id
            WHERE e.snapshot_id = ?1
              AND target.object_id IS NULL
            GROUP BY e.from_id
          ) missing ON missing.object_id = o.object_id
          WHERE o.snapshot_id = ?1
        )
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               COALESCE(r.reachable_count, 0) AS estimated_reachable_count,
               COALESCE(r.reachable_size, 0) AS estimated_reachable_size,
               COALESCE(r.truncated, 0) AS reachable_truncated,
               d.in_edges,
               d.out_edges,
               d.missing_referents
        FROM objects o
        JOIN degrees d ON d.object_id = o.object_id
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        {where_sql}
        ORDER BY {order} {direction}{tie_break}
        LIMIT ?6 OFFSET ?7
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        SqlValue::Integer(filters.limit),
        SqlValue::Integer(filters.offset),
    ];
    all_params.extend(params.clone());
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(all_params.iter()))?;
    let data = rows_to_json(&mut rows)?;
    let total = count_objects_with_global_edge_stats(conn, filters, sid)?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": filters.limit,
        "offset": filters.offset,
    }))
}

fn object_query_needs_global_edge_stats(filters: &ObjectFilters) -> bool {
    object_query_sorts_by_edge_stats(filters) || object_query_filters_by_edge_stats(filters)
}

fn object_query_can_drive_from_reachability(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<bool> {
    if !object_query_sorts_by_reachability(filters) || !object_reachability_available(conn, sid)? {
        return Ok(false);
    }
    Ok(filters.q.is_none()
        && filters.type_name.is_none()
        && filters.module.is_none()
        && filters.cohort.is_none()
        && filters.min_shallow_size.is_none()
        && filters.stub.is_none()
        && !object_query_needs_global_edge_stats(filters))
}

fn object_query_sorts_by_reachability(filters: &ObjectFilters) -> bool {
    matches!(
        filters.sort.as_str(),
        "reachable-size" | "reachable_size" | "reachable-count" | "reachable_count"
    ) || !matches!(
        filters.sort.as_str(),
        "object-id"
            | "object_id"
            | "type"
            | "module"
            | "shallow-size"
            | "shallow_size"
            | "in-edges"
            | "in_edges"
            | "out-edges"
            | "out_edges"
    )
}

fn object_query_filters_by_edge_stats(filters: &ObjectFilters) -> bool {
    filters.min_in_edges.is_some()
        || filters.min_out_edges.is_some()
        || filters.has_referrers
        || filters.missing_referents
}

fn object_query_sorts_by_edge_stats(filters: &ObjectFilters) -> bool {
    matches!(
        filters.sort.as_str(),
        "in-edges" | "in_edges" | "out-edges" | "out_edges"
    )
}

fn edge_stats_sort_index(sort: &str) -> &'static str {
    match sort {
        "out-edges" | "out_edges" => "idx_object_edge_stats_out",
        _ => "idx_object_edge_stats_in",
    }
}

fn object_edge_stats_available(conn: &Connection, sid: i64) -> Result<bool> {
    let table_exists: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM sqlite_master
        WHERE type = 'table'
          AND name = 'object_edge_stats'
        ",
        [],
        |row| row.get(0),
    )?;
    if table_exists == 0 {
        return Ok(false);
    }
    let rows_exist: i64 = conn.query_row(
        "
        SELECT EXISTS(
          SELECT 1
          FROM object_edge_stats
          WHERE snapshot_id = ?1
          LIMIT 1
        )
        ",
        [sid],
        |row| row.get(0),
    )?;
    Ok(rows_exist != 0)
}

fn object_list_metrics_available(conn: &Connection, sid: i64) -> Result<bool> {
    let table_exists: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM sqlite_master
        WHERE type = 'table'
          AND name = 'object_list_metrics'
        ",
        [],
        |row| row.get(0),
    )?;
    if table_exists == 0 {
        return Ok(false);
    }
    let rows_exist: i64 = conn.query_row(
        "
        SELECT EXISTS(
          SELECT 1
          FROM object_list_metrics
          WHERE snapshot_id = ?1
          LIMIT 1
        )
        ",
        [sid],
        |row| row.get(0),
    )?;
    Ok(rows_exist != 0)
}

fn object_reachability_available(conn: &Connection, sid: i64) -> Result<bool> {
    let found: i64 = conn.query_row(
        "
        SELECT EXISTS(
          SELECT 1
          FROM object_reachability
          WHERE snapshot_id = ?1
            AND algorithm_version = ?2
            AND direction = 'referents'
            AND depth = ?3
            AND node_limit = ?4
            AND fanout_limit = ?5
          LIMIT 1
        )
        ",
        params![
            sid,
            REACHABILITY_ALGORITHM_VERSION,
            DEFAULT_REACHABILITY_DEPTH,
            DEFAULT_REACHABILITY_NODE_LIMIT,
            DEFAULT_REACHABILITY_FANOUT_LIMIT
        ],
        |row| row.get(0),
    )?;
    Ok(found != 0)
}

fn object_metrics_where_clause(
    filters: &ObjectFilters,
    snapshot_id: i64,
    start_index: usize,
) -> (String, Vec<SqlValue>) {
    let mut clauses = vec!["m.snapshot_id = ?1".to_owned()];
    let mut params = Vec::new();
    let mut index = start_index;
    if let Some(q) = &filters.q {
        let a = placeholder(&mut index);
        let b = placeholder(&mut index);
        let c = placeholder(&mut index);
        clauses.push(format!(
            "(m.type LIKE {a} OR m.module LIKE {b} OR CAST(m.object_id AS TEXT) LIKE {c})"
        ));
        let pattern = format!("%{q}%");
        params.push(SqlValue::Text(pattern.clone()));
        params.push(SqlValue::Text(pattern.clone()));
        params.push(SqlValue::Text(pattern));
    }
    if let Some(type_name) = &filters.type_name {
        let p = placeholder(&mut index);
        clauses.push(format!("m.type = {p}"));
        params.push(SqlValue::Text(type_name.clone()));
    }
    if let Some(module) = &filters.module {
        let p = placeholder(&mut index);
        clauses.push(format!("m.module = {p}"));
        params.push(SqlValue::Text(module.clone()));
    }
    if let Some(cohort) = &filters.cohort {
        let snapshot = placeholder(&mut index);
        let cohort_param = placeholder(&mut index);
        clauses.push(format!("m.type IN (SELECT json_extract(value, '$.type') FROM cohort_stats cs, json_each(json_extract(cs.details_json, '$.types')) WHERE cs.snapshot_id = {snapshot} AND cs.cohort = {cohort_param})"));
        params.push(SqlValue::Integer(snapshot_id));
        params.push(SqlValue::Text(cohort.clone()));
    }
    if let Some(size) = filters.min_shallow_size {
        let p = placeholder(&mut index);
        clauses.push(format!("m.shallow_size >= {p}"));
        params.push(SqlValue::Integer(size));
    }
    if let Some(size) = filters.min_reachable_size {
        let p = placeholder(&mut index);
        clauses.push(format!("m.reachable_size >= {p}"));
        params.push(SqlValue::Integer(size));
    }
    if let Some(edges) = filters.min_in_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("m.in_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if let Some(edges) = filters.min_out_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("m.out_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if filters.has_referrers {
        clauses.push("m.in_edges > 0".to_owned());
    }
    if filters.missing_referents {
        clauses.push("m.missing_referents > 0".to_owned());
    }
    if let Some(stub) = filters.stub {
        let p = placeholder(&mut index);
        clauses.push(format!("m.stub = {p}"));
        params.push(SqlValue::Integer(bool_i64(stub)));
    }
    (format!("WHERE {}", clauses.join(" AND ")), params)
}

fn object_where_clause(
    filters: &ObjectFilters,
    snapshot_id: i64,
    start_index: usize,
) -> (String, Vec<SqlValue>) {
    let mut clauses = vec!["WHERE o.snapshot_id = ?1".to_owned()];
    let mut params = Vec::new();
    let mut index = start_index;
    if let Some(q) = &filters.q {
        let a = placeholder(&mut index);
        let b = placeholder(&mut index);
        let c = placeholder(&mut index);
        clauses.push(format!(
            "(o.type LIKE {a} OR o.module LIKE {b} OR CAST(o.object_id AS TEXT) LIKE {c})"
        ));
        let pattern = format!("%{q}%");
        params.push(SqlValue::Text(pattern.clone()));
        params.push(SqlValue::Text(pattern.clone()));
        params.push(SqlValue::Text(pattern));
    }
    if let Some(type_name) = &filters.type_name {
        let p = placeholder(&mut index);
        clauses.push(format!("o.type = {p}"));
        params.push(SqlValue::Text(type_name.clone()));
    }
    if let Some(module) = &filters.module {
        let p = placeholder(&mut index);
        clauses.push(format!("o.module = {p}"));
        params.push(SqlValue::Text(module.clone()));
    }
    if let Some(cohort) = &filters.cohort {
        let snapshot = placeholder(&mut index);
        let cohort_param = placeholder(&mut index);
        clauses.push(format!("o.type IN (SELECT json_extract(value, '$.type') FROM cohort_stats cs, json_each(json_extract(cs.details_json, '$.types')) WHERE cs.snapshot_id = {snapshot} AND cs.cohort = {cohort_param})"));
        params.push(SqlValue::Integer(snapshot_id));
        params.push(SqlValue::Text(cohort.clone()));
    }
    if let Some(size) = filters.min_shallow_size {
        let p = placeholder(&mut index);
        clauses.push(format!("COALESCE(o.shallow_size, 0) >= {p}"));
        params.push(SqlValue::Integer(size));
    }
    if let Some(size) = filters.min_reachable_size {
        let p = placeholder(&mut index);
        clauses.push(format!("COALESCE(r.reachable_size, 0) >= {p}"));
        params.push(SqlValue::Integer(size));
    }
    if let Some(edges) = filters.min_in_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("d.in_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if let Some(edges) = filters.min_out_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("d.out_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if filters.has_referrers {
        clauses.push("d.in_edges > 0".to_owned());
    }
    if filters.missing_referents {
        clauses.push("COALESCE(d.missing_referents, 0) > 0".to_owned());
    }
    if let Some(stub) = filters.stub {
        let p = placeholder(&mut index);
        clauses.push(format!("o.stub = {p}"));
        params.push(SqlValue::Integer(bool_i64(stub)));
    }
    (clauses.join(" AND "), params)
}

fn placeholder(index: &mut usize) -> String {
    let value = format!("?{}", *index);
    *index += 1;
    value
}

fn count_objects_fast(conn: &Connection, filters: &ObjectFilters, sid: i64) -> Result<i64> {
    let needs_reachability_join = filters.min_reachable_size.is_some();
    let (where_sql, params) =
        object_where_clause(filters, sid, if needs_reachability_join { 6 } else { 2 });
    let reachability_join = if needs_reachability_join {
        "
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        "
    } else {
        ""
    };
    let sql = format!("SELECT COUNT(*) FROM objects o {reachability_join} {where_sql}");
    let mut all_params = vec![SqlValue::Integer(sid)];
    if needs_reachability_join {
        all_params.extend([
            SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
            SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
            SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
            SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        ]);
    }
    all_params.extend(params);
    Ok(conn.query_row(&sql, params_from_iter(all_params.iter()), |row| row.get(0))?)
}

fn count_objects_with_materialized_edge_stats(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<i64> {
    if object_count_can_use_edge_stats_only(filters) {
        return count_objects_from_edge_stats(conn, filters, sid);
    }
    let needs_reachability_join = filters.min_reachable_size.is_some();
    let (where_sql, params) =
        object_where_clause(filters, sid, if needs_reachability_join { 6 } else { 2 });
    let reachability_join = if needs_reachability_join {
        "
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        "
    } else {
        ""
    };
    let sql = format!(
        "
        SELECT COUNT(*)
        FROM objects o
        LEFT JOIN object_edge_stats d
          ON d.snapshot_id = o.snapshot_id
         AND d.object_id = o.object_id
        LEFT JOIN object_edge_stats m
          ON m.snapshot_id = o.snapshot_id
         AND m.object_id = o.object_id
        {reachability_join}
        {where_sql}
        "
    );
    let mut all_params = vec![SqlValue::Integer(sid)];
    if needs_reachability_join {
        all_params.extend([
            SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
            SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
            SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
            SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        ]);
    }
    all_params.extend(params);
    Ok(conn.query_row(&sql, params_from_iter(all_params.iter()), |row| row.get(0))?)
}

fn object_count_can_use_edge_stats_only(filters: &ObjectFilters) -> bool {
    filters.q.is_none()
        && filters.type_name.is_none()
        && filters.module.is_none()
        && filters.cohort.is_none()
        && filters.min_shallow_size.is_none()
        && filters.min_reachable_size.is_none()
        && filters.stub.is_none()
}

fn count_objects_from_edge_stats(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<i64> {
    let mut clauses = vec!["snapshot_id = ?1".to_owned()];
    let mut params = vec![SqlValue::Integer(sid)];
    let mut index = 2;
    if let Some(edges) = filters.min_in_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("in_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if let Some(edges) = filters.min_out_edges {
        let p = placeholder(&mut index);
        clauses.push(format!("out_edges >= {p}"));
        params.push(SqlValue::Integer(edges));
    }
    if filters.has_referrers {
        clauses.push("in_edges > 0".to_owned());
    }
    if filters.missing_referents {
        clauses.push("missing_referents > 0".to_owned());
    }
    let sql = format!(
        "SELECT COUNT(*) FROM object_edge_stats WHERE {}",
        clauses.join(" AND ")
    );
    Ok(conn.query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))?)
}

fn count_objects_from_metrics(conn: &Connection, filters: &ObjectFilters, sid: i64) -> Result<i64> {
    let (where_sql, params) = object_metrics_where_clause(filters, sid, 2);
    let sql = format!("SELECT COUNT(*) FROM object_list_metrics m {where_sql}");
    let mut all_params = vec![SqlValue::Integer(sid)];
    all_params.extend(params);
    Ok(conn.query_row(&sql, params_from_iter(all_params.iter()), |row| row.get(0))?)
}

fn count_objects_with_global_edge_stats(
    conn: &Connection,
    filters: &ObjectFilters,
    sid: i64,
) -> Result<i64> {
    let (where_sql, params) = object_where_clause(filters, sid, 6);
    let sql = format!(
        "
        WITH degrees AS (
          SELECT o.object_id,
                 COALESCE(i.in_edges, 0) AS in_edges,
                 COALESCE(out.out_edges, 0) AS out_edges,
                 COALESCE(missing.missing_referents, 0) AS missing_referents
          FROM objects o
          LEFT JOIN (SELECT to_id AS object_id, COUNT(*) AS in_edges FROM edges WHERE snapshot_id = ?1 GROUP BY to_id) i ON i.object_id = o.object_id
          LEFT JOIN (SELECT from_id AS object_id, COUNT(*) AS out_edges FROM edges WHERE snapshot_id = ?1 GROUP BY from_id) out ON out.object_id = o.object_id
          LEFT JOIN (
            SELECT e.from_id AS object_id, COUNT(*) AS missing_referents
            FROM edges e
            LEFT JOIN objects target ON target.snapshot_id = e.snapshot_id AND target.object_id = e.to_id
            WHERE e.snapshot_id = ?1 AND target.object_id IS NULL
            GROUP BY e.from_id
          ) missing ON missing.object_id = o.object_id
          WHERE o.snapshot_id = ?1
        )
        SELECT COUNT(*)
        FROM objects o
        JOIN degrees d ON d.object_id = o.object_id
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?2
         AND r.direction = 'referents'
         AND r.depth = ?3
         AND r.node_limit = ?4
         AND r.fanout_limit = ?5
        {where_sql}
        "
    );
    let mut all_params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
    ];
    all_params.extend(params);
    Ok(conn.query_row(&sql, params_from_iter(all_params.iter()), |row| row.get(0))?)
}

fn object_order_for(sort: &str, reachability_available: bool) -> &'static str {
    match sort {
        "object-id" | "object_id" => "o.object_id",
        "type" => "o.type",
        "module" => "o.module",
        "shallow-size" | "shallow_size" => "COALESCE(o.shallow_size, 0)",
        "reachable-count" | "reachable_count" if reachability_available => {
            "COALESCE(r.reachable_count, 0)"
        }
        "reachable-count" | "reachable_count" => "o.object_id",
        "in-edges" | "in_edges" => "d.in_edges",
        "out-edges" | "out_edges" => "d.out_edges",
        _ if reachability_available => "COALESCE(r.reachable_size, 0)",
        _ => "o.object_id",
    }
}

fn object_order_tie_break(order: &str) -> &'static str {
    if order == "o.object_id" {
        ""
    } else {
        ", o.object_id ASC"
    }
}

fn object_metrics_order_for(sort: &str) -> &'static str {
    match sort {
        "object-id" | "object_id" => "m.object_id",
        "type" => "m.type",
        "module" => "m.module",
        "shallow-size" | "shallow_size" => "m.shallow_size",
        "reachable-count" | "reachable_count" => "m.reachable_count",
        "in-edges" | "in_edges" => "m.in_edges",
        "out-edges" | "out_edges" => "m.out_edges",
        _ => "m.reachable_size",
    }
}

fn object_metrics_order_tie_break(order: &str) -> &'static str {
    if order == "m.object_id" {
        ""
    } else {
        ", m.object_id ASC"
    }
}

fn object_metrics_index_hint(filters: &ObjectFilters) -> &'static str {
    if object_query_sorts_by_reachability(filters) {
        if filters.type_name.is_some()
            && filters.module.is_none()
            && filters.q.is_none()
            && filters.cohort.is_none()
        {
            return "INDEXED BY idx_object_list_metrics_type_reachable";
        }
        if filters.module.is_some()
            && filters.type_name.is_none()
            && filters.q.is_none()
            && filters.cohort.is_none()
        {
            return "INDEXED BY idx_object_list_metrics_module_reachable";
        }
        if filters.type_name.is_none()
            && filters.module.is_none()
            && filters.q.is_none()
            && filters.cohort.is_none()
        {
            return "INDEXED BY idx_object_list_metrics_reachable";
        }
    }
    if matches!(filters.sort.as_str(), "in-edges" | "in_edges")
        && filters.type_name.is_none()
        && filters.module.is_none()
        && filters.q.is_none()
        && filters.cohort.is_none()
    {
        return "INDEXED BY idx_object_list_metrics_in_edges";
    }
    if matches!(filters.sort.as_str(), "out-edges" | "out_edges")
        && filters.type_name.is_none()
        && filters.module.is_none()
        && filters.q.is_none()
        && filters.cohort.is_none()
    {
        return "INDEXED BY idx_object_list_metrics_out_edges";
    }
    ""
}

fn reachability_order(sort: &str) -> &'static str {
    match sort {
        "reachable-count" | "reachable_count" => "r.reachable_count",
        _ => "r.reachable_size",
    }
}

pub fn object_detail(conn: &Connection, snapshot_id: Option<i64>, object_id: i64) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    let mut stmt = conn.prepare(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               o.shallow_size,
               o.gc_tracked,
               o.stub,
               o.repr,
               COALESCE(r.reachable_count, 0) AS estimated_reachable_count,
               COALESCE(r.reachable_size, 0) AS estimated_reachable_size,
               COALESCE(r.truncated, 0) AS reachable_truncated,
               (SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.from_id = o.object_id) AS out_edges,
               (SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = o.snapshot_id AND e.to_id = o.object_id) AS in_edges,
               (SELECT COUNT(*)
                FROM edges e
                LEFT JOIN objects target
                  ON target.snapshot_id = e.snapshot_id
                 AND target.object_id = e.to_id
                WHERE e.snapshot_id = o.snapshot_id
                  AND e.from_id = o.object_id
                  AND target.object_id IS NULL) AS missing_referents
        FROM objects o
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?3
         AND r.direction = 'referents'
         AND r.depth = ?4
         AND r.node_limit = ?5
         AND r.fanout_limit = ?6
        WHERE o.snapshot_id = ?1 AND o.object_id = ?2
        ",
    )?;
    let mut rows = stmt.query(params![
        sid,
        object_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT
    ])?;
    let object =
        rows_to_json(&mut rows)?
            .into_iter()
            .next()
            .ok_or(AnalysisError::ObjectNotFound {
                snapshot_id: sid,
                object_id,
            })?;
    Ok(json!({
        "object": object,
        "top_referents": object_edges(conn, Some(sid), object_id, "referents", 20, 0)?["rows"].clone(),
        "top_referrers": object_edges(conn, Some(sid), object_id, "referrers", 20, 0)?["rows"].clone(),
        "actions": ["copy_object_id", "open_referents", "open_referrers", "export_subgraph", "query_same_type"]
    }))
}

pub fn object_edges(
    conn: &Connection,
    snapshot_id: Option<i64>,
    object_id: i64,
    direction: &str,
    limit: i64,
    offset: i64,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    let (predicate, join_id, edge_id, order) = if direction == "referrers" || direction == "to" {
        ("e.to_id = ?2", "e.from_id", "e.from_id", "e.from_id")
    } else {
        ("e.from_id = ?2", "e.to_id", "e.to_id", "e.edge_index")
    };
    let sql = format!(
        "
        SELECT CAST(e.from_id AS TEXT) AS from_id,
               CAST(e.to_id AS TEXT) AS to_id,
               e.edge_index,
               CASE WHEN target.object_id IS NULL THEN 1 ELSE 0 END AS missing,
               CAST({edge_id} AS TEXT) AS object_id,
               target.type,
               target.module,
               target.qualname,
               target.shallow_size,
               target.stub
        FROM edges e
        LEFT JOIN objects target
          ON target.snapshot_id = e.snapshot_id
         AND target.object_id = {join_id}
        WHERE e.snapshot_id = ?1 AND {predicate}
        ORDER BY {order}
        LIMIT ?3 OFFSET ?4
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![sid, object_id, limit, offset])?;
    let data = rows_to_json(&mut rows)?;
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM edges e WHERE e.snapshot_id = ?1 AND {predicate}"),
        params![sid, object_id],
        |row| row.get(0),
    )?;
    Ok(json!({
        "rows": data,
        "total": total,
        "limit": limit,
        "offset": offset,
    }))
}

pub fn paths(
    conn: &Connection,
    snapshot_id: Option<i64>,
    object_id: i64,
    direction: &str,
    depth: i64,
    fanout_limit: i64,
    limit: i64,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    ensure_object_exists(conn, sid, object_id)?;
    let mut paths = Vec::new();
    let mut queue = VecDeque::from([(object_id, vec![object_id], 0_i64)]);
    while let Some((current, path, current_depth)) = queue.pop_front() {
        if current_depth >= depth || paths.len() as i64 >= limit {
            continue;
        }
        for next in neighbors(conn, sid, current, direction, fanout_limit)? {
            let mut next_path = path.clone();
            next_path.push(next);
            paths.push(json!(next_path
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()));
            if !path.contains(&next) {
                queue.push_back((next, next_path, current_depth + 1));
            }
            if paths.len() as i64 >= limit {
                break;
            }
        }
    }
    Ok(json!({
        "object_id": object_id.to_string(),
        "direction": direction,
        "depth": depth,
        "fanout_limit": fanout_limit,
        "paths": paths,
    }))
}

pub fn subgraph(
    conn: &Connection,
    snapshot_id: Option<i64>,
    root_object_id: i64,
    direction: &str,
    depth: i64,
    node_limit: i64,
    edge_limit: i64,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    ensure_object_exists(conn, sid, root_object_id)?;
    let mut nodes: BTreeSet<i64> = BTreeSet::from([root_object_id]);
    let mut edges = Vec::new();
    let mut missing_edges = Vec::new();
    let mut queue = VecDeque::from([(root_object_id, 0_i64)]);
    let mut truncated = false;
    while let Some((current, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        let dirs: &[&str] = if direction == "both" {
            &["referents", "referrers"]
        } else {
            &[direction]
        };
        for dir in dirs {
            for edge in edge_neighbors(conn, sid, current, dir, edge_limit)? {
                if edges.len() as i64 >= edge_limit || nodes.len() as i64 >= node_limit {
                    truncated = true;
                    break;
                }
                let next = edge.next_id;
                if edge.missing {
                    missing_edges.push(json!({
                        "from_id": edge.from_id.to_string(),
                        "to_id": edge.to_id.to_string(),
                    }));
                } else {
                    edges.push(json!({
                        "from_id": edge.from_id.to_string(),
                        "to_id": edge.to_id.to_string(),
                    }));
                    if nodes.insert(next) {
                        queue.push_back((next, current_depth + 1));
                    }
                }
            }
        }
    }
    let node_values = load_nodes(conn, sid, &nodes)?;
    Ok(json!({
        "root_object_id": root_object_id.to_string(),
        "nodes": node_values,
        "edges": edges,
        "missing_edges": missing_edges,
        "truncated": truncated,
        "limits": { "depth": depth, "node_limit": node_limit, "edge_limit": edge_limit }
    }))
}

fn ensure_object_exists(conn: &Connection, snapshot_id: i64, object_id: i64) -> Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM objects WHERE snapshot_id = ?1 AND object_id = ?2",
        params![snapshot_id, object_id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        Err(AnalysisError::ObjectNotFound {
            snapshot_id,
            object_id,
        })
    } else {
        Ok(())
    }
}

fn neighbors(
    conn: &Connection,
    snapshot_id: i64,
    object_id: i64,
    direction: &str,
    limit: i64,
) -> Result<Vec<i64>> {
    let sql = if direction == "referrers" {
        "SELECT from_id FROM edges WHERE snapshot_id = ?1 AND to_id = ?2 LIMIT ?3"
    } else {
        "SELECT to_id FROM edges WHERE snapshot_id = ?1 AND from_id = ?2 LIMIT ?3"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(params![snapshot_id, object_id, limit], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(rows)
}

struct EdgeNeighbor {
    from_id: i64,
    to_id: i64,
    next_id: i64,
    missing: bool,
}

fn edge_neighbors(
    conn: &Connection,
    snapshot_id: i64,
    object_id: i64,
    direction: &str,
    limit: i64,
) -> Result<Vec<EdgeNeighbor>> {
    let (predicate, next_expr) = if direction == "referrers" {
        ("e.to_id = ?2", "e.from_id")
    } else {
        ("e.from_id = ?2", "e.to_id")
    };
    let sql = format!(
        "
        SELECT e.from_id,
               e.to_id,
               {next_expr} AS next_id,
               CASE WHEN o.object_id IS NULL THEN 1 ELSE 0 END AS missing
        FROM edges e
        LEFT JOIN objects o
          ON o.snapshot_id = e.snapshot_id
         AND o.object_id = {next_expr}
        WHERE e.snapshot_id = ?1 AND {predicate}
        LIMIT ?3
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![snapshot_id, object_id, limit], |row| {
            Ok(EdgeNeighbor {
                from_id: row.get(0)?,
                to_id: row.get(1)?,
                next_id: row.get(2)?,
                missing: row.get::<_, i64>(3)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn load_nodes(conn: &Connection, snapshot_id: i64, nodes: &BTreeSet<i64>) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut stmt = conn.prepare(
        "
        SELECT CAST(object_id AS TEXT) AS object_id, type, module, qualname, shallow_size, stub
        FROM objects
        WHERE snapshot_id = ?1 AND object_id = ?2
        ",
    )?;
    for id in nodes {
        let mut rows = stmt.query(params![snapshot_id, id])?;
        if let Some(row) = rows_to_json(&mut rows)?.into_iter().next() {
            out.push(row);
        }
    }
    Ok(out)
}

#[derive(Debug)]
struct ObjectMeta {
    object_id: i64,
    type_name: String,
    module: String,
    shallow_size: i64,
    stub: bool,
}

pub fn compute_reachability(
    conn: &Connection,
    snapshot_id: Option<i64>,
    params: ReachabilityParams,
) -> Result<Value> {
    compute_reachability_with_cancel(conn, snapshot_id, params, || false)
}

pub fn compute_reachability_with_cancel(
    conn: &Connection,
    snapshot_id: Option<i64>,
    params: ReachabilityParams,
    mut should_cancel: impl FnMut() -> bool,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    check_canceled(&mut should_cancel)?;
    let objects = load_object_meta(conn, sid)?;
    check_canceled(&mut should_cancel)?;
    let adjacency = load_adjacency(conn, sid, &objects)?;
    check_canceled(&mut should_cancel)?;

    let mut type_totals: BTreeMap<String, (String, i64, i64, i64, i64, i64)> = BTreeMap::new();
    let mut object_rows = Vec::new();
    let computed_at = now_rfc3339();
    let mut truncated_count = 0;
    for object in objects.values().filter(|object| !object.stub) {
        check_canceled(&mut should_cancel)?;
        let result = reachable_from(
            object.object_id,
            &adjacency,
            &objects,
            params,
            &mut should_cancel,
        )?;
        if result.truncated {
            truncated_count += 1;
        }
        object_rows.push((
            sid,
            object.object_id,
            params.algorithm_version,
            "referents".to_owned(),
            params.depth,
            params.node_limit,
            params.fanout_limit,
            result.count,
            result.size,
            bool_i64(result.truncated),
            computed_at.clone(),
        ));
        let entry = type_totals.entry(object.type_name.clone()).or_insert((
            object.module.clone(),
            0,
            0,
            0,
            0,
            0,
        ));
        entry.1 += 1;
        entry.2 += object.shallow_size;
        entry.3 += result.size;
        entry.4 = entry.4.max(result.size);
        entry.5 += bool_i64(result.truncated);
    }
    check_canceled(&mut should_cancel)?;

    let object_count = objects.values().filter(|object| !object.stub).count();
    conn.execute_batch("BEGIN")?;
    let write_result = (|| {
        conn.execute(
            "
            DELETE FROM object_reachability
            WHERE snapshot_id = ?1
              AND algorithm_version = ?2
              AND direction = 'referents'
              AND depth = ?3
              AND node_limit = ?4
              AND fanout_limit = ?5
            ",
            params![
                sid,
                params.algorithm_version,
                params.depth,
                params.node_limit,
                params.fanout_limit
            ],
        )?;
        conn.execute(
            "
            DELETE FROM type_reachability_stats
            WHERE snapshot_id = ?1
              AND algorithm_version = ?2
              AND direction = 'referents'
              AND depth = ?3
              AND node_limit = ?4
              AND fanout_limit = ?5
            ",
            params![
                sid,
                params.algorithm_version,
                params.depth,
                params.node_limit,
                params.fanout_limit
            ],
        )?;
        let mut stmt = conn.prepare(
            "
            INSERT OR REPLACE INTO object_reachability(
              snapshot_id, object_id, algorithm_version, direction, depth, node_limit,
              fanout_limit, reachable_count, reachable_size, truncated, computed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
        )?;
        for row in object_rows {
            stmt.execute(params![
                row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10
            ])?;
        }

        let mut stmt = conn.prepare(
            "
            INSERT OR REPLACE INTO type_reachability_stats(
              snapshot_id, type, module, algorithm_version, direction, depth, node_limit,
              fanout_limit, count, shallow_size_sum, reachable_size_sum,
              reachable_size_avg, reachable_size_max, truncated_count
            ) VALUES (?1, ?2, ?3, ?4, 'referents', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ",
        )?;
        for (type_name, (module, count, shallow_sum, reachable_sum, reachable_max, truncated)) in
            type_totals
        {
            let avg = if count > 0 {
                reachable_sum as f64 / count as f64
            } else {
                0.0
            };
            stmt.execute(params![
                sid,
                type_name,
                module,
                params.algorithm_version,
                params.depth,
                params.node_limit,
                params.fanout_limit,
                count,
                shallow_sum,
                reachable_sum,
                avg,
                reachable_max,
                truncated
            ])?;
        }
        Ok(())
    })();
    match write_result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error);
        }
    }
    Ok(json!({
        "snapshot_id": sid,
        "object_count": object_count,
        "truncated_count": truncated_count,
        "params": params,
    }))
}

pub fn refresh_object_list_metrics(
    conn: &Connection,
    snapshot_id: i64,
    params: ReachabilityParams,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, Some(snapshot_id))?;
    conn.execute_batch("BEGIN")?;
    let write_result = (|| {
        conn.execute(
            "DELETE FROM object_list_metrics WHERE snapshot_id = ?1",
            [sid],
        )?;
        conn.execute(
            "
            INSERT INTO object_list_metrics(
              snapshot_id,
              object_id,
              type,
              module,
              shallow_size,
              stub,
              reachable_count,
              reachable_size,
              reachable_truncated,
              in_edges,
              out_edges,
              missing_referents
            )
            SELECT o.snapshot_id,
                   o.object_id,
                   o.type,
                   o.module,
                   COALESCE(o.shallow_size, 0) AS shallow_size,
                   o.stub,
                   COALESCE(r.reachable_count, 0) AS reachable_count,
                   COALESCE(r.reachable_size, 0) AS reachable_size,
                   COALESCE(r.truncated, 0) AS reachable_truncated,
                   COALESCE(es.in_edges, 0) AS in_edges,
                   COALESCE(es.out_edges, 0) AS out_edges,
                   COALESCE(es.missing_referents, 0) AS missing_referents
            FROM objects o
            LEFT JOIN object_edge_stats es
              ON es.snapshot_id = o.snapshot_id
             AND es.object_id = o.object_id
            LEFT JOIN object_reachability r
              ON r.snapshot_id = o.snapshot_id
             AND r.object_id = o.object_id
             AND r.algorithm_version = ?2
             AND r.direction = 'referents'
             AND r.depth = ?3
             AND r.node_limit = ?4
             AND r.fanout_limit = ?5
            WHERE o.snapshot_id = ?1
            ",
            params![
                sid,
                params.algorithm_version,
                params.depth,
                params.node_limit,
                params.fanout_limit
            ],
        )?;
        Ok(())
    })();
    match write_result {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(error);
        }
    }
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM object_list_metrics WHERE snapshot_id = ?1",
        [sid],
        |row| row.get(0),
    )?;
    Ok(json!({
        "snapshot_id": sid,
        "object_count": count,
        "params": params,
    }))
}

fn check_canceled(should_cancel: &mut impl FnMut() -> bool) -> Result<()> {
    if should_cancel() {
        Err(AnalysisError::Canceled)
    } else {
        Ok(())
    }
}

struct ReachabilityResult {
    count: i64,
    size: i64,
    truncated: bool,
}

fn reachable_from(
    root_id: i64,
    adjacency: &HashMap<i64, Vec<i64>>,
    objects: &HashMap<i64, ObjectMeta>,
    params: ReachabilityParams,
    should_cancel: &mut impl FnMut() -> bool,
) -> Result<ReachabilityResult> {
    let mut visited = HashSet::from([root_id]);
    let mut frontier = vec![root_id];
    let mut size = objects.get(&root_id).map(|o| o.shallow_size).unwrap_or(0);
    let mut truncated = false;
    for _ in 0..params.depth {
        check_canceled(should_cancel)?;
        let mut next_frontier = Vec::new();
        for current in frontier {
            check_canceled(should_cancel)?;
            let mut fanout = 0;
            for referent in adjacency.get(&current).into_iter().flatten() {
                check_canceled(should_cancel)?;
                fanout += 1;
                if fanout > params.fanout_limit {
                    truncated = true;
                    break;
                }
                if visited.contains(referent) {
                    continue;
                }
                if visited.len() as i64 >= params.node_limit {
                    truncated = true;
                    break;
                }
                if let Some(meta) = objects.get(referent) {
                    visited.insert(*referent);
                    size += meta.shallow_size;
                    if !meta.stub {
                        next_frontier.push(*referent);
                    }
                }
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        frontier = next_frontier;
    }
    Ok(ReachabilityResult {
        count: visited.len() as i64,
        size,
        truncated,
    })
}

fn load_object_meta(conn: &Connection, snapshot_id: i64) -> Result<HashMap<i64, ObjectMeta>> {
    let mut stmt = conn.prepare(
        "
        SELECT object_id, type, module, COALESCE(shallow_size, 0), stub
        FROM objects
        WHERE snapshot_id = ?1
        ",
    )?;
    let rows = stmt
        .query_map([snapshot_id], |row| {
            Ok(ObjectMeta {
                object_id: row.get(0)?,
                type_name: row.get(1)?,
                module: row.get(2)?,
                shallow_size: row.get(3)?,
                stub: row.get::<_, i64>(4)? != 0,
            })
        })?
        .map(|row| row.map(|meta| (meta.object_id, meta)))
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    Ok(rows)
}

fn load_adjacency(
    conn: &Connection,
    snapshot_id: i64,
    objects: &HashMap<i64, ObjectMeta>,
) -> Result<HashMap<i64, Vec<i64>>> {
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut stmt = conn.prepare(
        "
        SELECT from_id, to_id
        FROM edges
        WHERE snapshot_id = ?1
        ORDER BY from_id, edge_index
        ",
    )?;
    let mut rows = stmt.query([snapshot_id])?;
    while let Some(row) = rows.next()? {
        let from_id: i64 = row.get(0)?;
        let to_id: i64 = row.get(1)?;
        if objects.contains_key(&from_id) && objects.contains_key(&to_id) {
            adjacency.entry(from_id).or_default().push(to_id);
        }
    }
    Ok(adjacency)
}

pub fn diff(
    conn: &Connection,
    from_snapshot_id: i64,
    to_snapshot_id: i64,
    limit: i64,
) -> Result<Value> {
    let confidence = lifecycle_confidence(conn, from_snapshot_id, to_snapshot_id)?;
    Ok(json!({
        "from_snapshot_id": from_snapshot_id,
        "to_snapshot_id": to_snapshot_id,
        "confidence": confidence,
        "summary_delta": summary_delta(conn, from_snapshot_id, to_snapshot_id)?,
        "type_delta": stats_delta(conn, "type_stats", "type", from_snapshot_id, to_snapshot_id, limit)?,
        "module_delta": stats_delta(conn, "module_stats", "module", from_snapshot_id, to_snapshot_id, limit)?,
        "cohort_delta": stats_delta(conn, "cohort_stats", "cohort", from_snapshot_id, to_snapshot_id, limit)?,
        "object_lifecycle": lifecycle_summary(conn, from_snapshot_id, to_snapshot_id)?,
    }))
}

fn summary_delta(conn: &Connection, from_id: i64, to_id: i64) -> Result<Value> {
    let from = snapshot(conn, from_id)?;
    let to = snapshot(conn, to_id)?;
    Ok(json!({
        "object_count": int_field(&to, "object_count") - int_field(&from, "object_count"),
        "edge_count": int_field(&to, "edge_count") - int_field(&from, "edge_count"),
        "stub_count": int_field(&to, "stub_count") - int_field(&from, "stub_count"),
        "missing_referent_count": int_field(&to, "missing_referent_count") - int_field(&from, "missing_referent_count"),
        "shallow_size_sum": int_field(&to, "shallow_size_sum") - int_field(&from, "shallow_size_sum"),
    }))
}

fn int_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn stats_delta(
    conn: &Connection,
    table: &str,
    key: &str,
    from_id: i64,
    to_id: i64,
    limit: i64,
) -> Result<Value> {
    let table = match table {
        "type_stats" => "type_stats",
        "module_stats" => "module_stats",
        "cohort_stats" => "cohort_stats",
        _ => return Err(AnalysisError::InvalidQuery(table.to_owned())),
    };
    let key = match key {
        "type" => "type",
        "module" => "module",
        "cohort" => "cohort",
        _ => return Err(AnalysisError::InvalidQuery(key.to_owned())),
    };
    let sql = format!(
        "
        WITH old AS (SELECT * FROM {table} WHERE snapshot_id = ?1),
             new AS (SELECT * FROM {table} WHERE snapshot_id = ?2),
             keys AS (
               SELECT {key} AS k FROM old
               UNION
               SELECT {key} AS k FROM new
             )
        SELECT keys.k AS {key},
               COALESCE(new.count, 0) - COALESCE(old.count, 0) AS count_delta,
               COALESCE(new.shallow_size_sum, 0) - COALESCE(old.shallow_size_sum, 0) AS shallow_size_delta,
               COALESCE(new.count, 0) AS to_count,
               COALESCE(old.count, 0) AS from_count,
               COALESCE(new.shallow_size_sum, 0) AS to_shallow_size,
               COALESCE(old.shallow_size_sum, 0) AS from_shallow_size
        FROM keys
        LEFT JOIN old ON old.{key} = keys.k
        LEFT JOIN new ON new.{key} = keys.k
        ORDER BY shallow_size_delta DESC, count_delta DESC, keys.k ASC
        LIMIT ?3
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![from_id, to_id, limit])?;
    Ok(Value::Array(rows_to_json(&mut rows)?))
}

fn lifecycle_summary(conn: &Connection, from_id: i64, to_id: i64) -> Result<Value> {
    let new_count: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM objects n
        LEFT JOIN objects o ON o.snapshot_id = ?1 AND o.object_id = n.object_id
        WHERE n.snapshot_id = ?2 AND o.object_id IS NULL
        ",
        params![from_id, to_id],
        |row| row.get(0),
    )?;
    let gone_count: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM objects o
        LEFT JOIN objects n ON n.snapshot_id = ?2 AND n.object_id = o.object_id
        WHERE o.snapshot_id = ?1 AND n.object_id IS NULL
        ",
        params![from_id, to_id],
        |row| row.get(0),
    )?;
    let retained_count: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM objects o
        JOIN objects n ON n.snapshot_id = ?2 AND n.object_id = o.object_id
        WHERE o.snapshot_id = ?1
        ",
        params![from_id, to_id],
        |row| row.get(0),
    )?;
    Ok(json!({
        "new_count": new_count,
        "gone_count": gone_count,
        "retained_count": retained_count,
    }))
}

fn lifecycle_confidence(conn: &Connection, from_id: i64, to_id: i64) -> Result<Value> {
    let from = snapshot(conn, from_id)?;
    let to = snapshot(conn, to_id)?;
    let same_run = from.get("producer_run_id") == to.get("producer_run_id")
        && from
            .get("producer_run_id")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty());
    let ordered = int_field(&to, "dump_sequence") > int_field(&from, "dump_sequence");
    let confidence = if same_run && ordered {
        "high"
    } else if from.get("host_id") == to.get("host_id")
        && from.get("container_id") == to.get("container_id")
        && from.get("pid") == to.get("pid")
        && from.get("process_started_at") == to.get("process_started_at")
    {
        "medium"
    } else if from.get("pid") == to.get("pid") {
        "low"
    } else {
        "aggregate-only"
    };
    Ok(json!({
        "level": confidence,
        "message": "Object id lifecycle is strongest for consecutive dumps from the same process."
    }))
}

pub fn diff_objects(conn: &Connection, options: DiffObjectsOptions) -> Result<Value> {
    let state_sql = match options.state.as_str() {
        "new" => "old.object_id IS NULL",
        "gone" => "new.object_id IS NULL",
        "retained" => "old.object_id IS NOT NULL AND new.object_id IS NOT NULL",
        "changed" => "old.object_id IS NOT NULL AND new.object_id IS NOT NULL AND (old.shallow_size IS NOT new.shallow_size OR old.type IS NOT new.type)",
        other => return Err(AnalysisError::InvalidQuery(other.to_owned())),
    };
    let mut params = vec![
        SqlValue::Integer(options.from_snapshot_id),
        SqlValue::Integer(options.to_snapshot_id),
    ];
    let mut filters = vec![state_sql.to_owned()];
    if let Some(type_name) = nonempty(options.type_name) {
        filters.push("COALESCE(new.type, old.type) = ?".to_owned());
        params.push(SqlValue::Text(type_name));
    }
    if let Some(module) = nonempty(options.module) {
        filters.push("COALESCE(new.module, old.module) = ?".to_owned());
        params.push(SqlValue::Text(module));
    }
    let where_sql = filters.join(" AND ");
    let mut row_params = params.clone();
    row_params.push(SqlValue::Integer(options.limit));
    row_params.push(SqlValue::Integer(options.offset));
    let sql = format!(
        "
        WITH old AS (SELECT * FROM objects WHERE snapshot_id = ?),
             new AS (SELECT * FROM objects WHERE snapshot_id = ?),
             ids AS (
               SELECT object_id FROM old
               UNION
               SELECT object_id FROM new
             )
        SELECT CAST(ids.object_id AS TEXT) AS object_id,
               COALESCE(new.type, old.type) AS type,
               COALESCE(new.module, old.module) AS module,
               old.shallow_size AS from_shallow_size,
               new.shallow_size AS to_shallow_size,
               COALESCE(new.shallow_size, 0) - COALESCE(old.shallow_size, 0) AS shallow_size_delta,
               CASE
                 WHEN old.object_id IS NULL THEN 'new'
                 WHEN new.object_id IS NULL THEN 'gone'
                 WHEN old.shallow_size IS NOT new.shallow_size OR old.type IS NOT new.type THEN 'changed'
                 ELSE 'retained'
               END AS state
        FROM ids
        LEFT JOIN old ON old.object_id = ids.object_id
        LEFT JOIN new ON new.object_id = ids.object_id
        WHERE {where_sql}
        ORDER BY shallow_size_delta DESC, object_id ASC
        LIMIT ? OFFSET ?
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(row_params.iter()))?;
    let rows = rows_to_json(&mut rows)?;
    let count_sql = format!(
        "
        WITH old AS (SELECT * FROM objects WHERE snapshot_id = ?),
             new AS (SELECT * FROM objects WHERE snapshot_id = ?),
             ids AS (
               SELECT object_id FROM old
               UNION
               SELECT object_id FROM new
             )
        SELECT COUNT(*)
        FROM ids
        LEFT JOIN old ON old.object_id = ids.object_id
        LEFT JOIN new ON new.object_id = ids.object_id
        WHERE {where_sql}
        "
    );
    let total: i64 = conn.query_row(&count_sql, params_from_iter(params.iter()), |row| {
        row.get(0)
    })?;
    Ok(json!({
        "confidence": lifecycle_confidence(conn, options.from_snapshot_id, options.to_snapshot_id)?,
        "rows": rows,
        "total": total,
        "limit": options.limit,
        "offset": options.offset,
    }))
}

pub fn readonly_sql(conn: &Connection, query: &str, limit: i64, explain: bool) -> Result<Value> {
    ensure_readonly_sql(query)?;
    conn.pragma_update(None, "query_only", "ON")?;
    let started = std::time::Instant::now();
    let sql = if explain {
        format!("EXPLAIN QUERY PLAN {query}")
    } else {
        query.to_owned()
    };
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut values = rows_to_json(&mut rows)?;
    if !explain && values.len() > limit as usize {
        values.truncate(limit as usize);
    }
    Ok(json!({
        "rows": values,
        "elapsed_ms": started.elapsed().as_millis(),
        "explain": explain,
        "limit": limit,
    }))
}

pub fn idset(
    conn: &Connection,
    snapshot_id: i64,
    left_query: &str,
    right_query: &str,
    op: &str,
    details: bool,
    limit: i64,
) -> Result<Value> {
    let left = query_idset(conn, left_query)?;
    let right = query_idset(conn, right_query)?;
    let result: BTreeSet<i64> = match op {
        "intersect" => left.intersection(&right).copied().collect(),
        "union" => left.union(&right).copied().collect(),
        "left-diff" => left.difference(&right).copied().collect(),
        "right-diff" => right.difference(&left).copied().collect(),
        "symdiff" => left.symmetric_difference(&right).copied().collect(),
        other => return Err(AnalysisError::InvalidIdsetOp(other.to_owned())),
    };
    let ids: Vec<i64> = result.into_iter().take(limit as usize).collect();
    if details {
        let rows = hydrate_ids(conn, snapshot_id, &ids)?;
        Ok(json!({ "op": op, "total": ids.len(), "rows": rows }))
    } else {
        Ok(json!({
            "op": op,
            "total": ids.len(),
            "object_ids": ids.iter().map(ToString::to_string).collect::<Vec<_>>()
        }))
    }
}

pub fn save_idset(
    conn: &mut Connection,
    snapshot_id: i64,
    name: &str,
    source: Value,
    object_ids: Vec<i64>,
) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, Some(snapshot_id))?;
    let clean_name =
        nonempty(Some(name.to_owned())).unwrap_or_else(|| format!("idset {}", now_rfc3339()));
    let mut unique_ids = BTreeSet::new();
    for id in object_ids {
        unique_ids.insert(id);
    }
    let created_at = now_rfc3339();
    let tx = conn.transaction()?;
    tx.execute(
        "
        INSERT INTO saved_idsets(snapshot_id, name, source_json, created_at)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![sid, clean_name, source.to_string(), created_at],
    )?;
    let idset_id = tx.last_insert_rowid();
    {
        let mut stmt = tx.prepare(
            "
            INSERT INTO saved_idset_objects(idset_id, object_id)
            VALUES (?1, ?2)
            ",
        )?;
        for object_id in &unique_ids {
            stmt.execute(params![idset_id, object_id])?;
        }
    }
    tx.commit()?;
    saved_idset_detail(conn, idset_id)
}

pub fn saved_idsets(conn: &Connection, snapshot_id: Option<i64>) -> Result<Value> {
    let mut sql = "
        SELECT s.idset_id,
               s.snapshot_id,
               s.name,
               s.source_json,
               s.created_at,
               COUNT(o.object_id) AS object_count
        FROM saved_idsets s
        LEFT JOIN saved_idset_objects o ON o.idset_id = s.idset_id
    "
    .to_owned();
    let mut params = Vec::new();
    if let Some(snapshot_id) = snapshot_id {
        let sid = resolve_snapshot_id(conn, Some(snapshot_id))?;
        sql.push_str(" WHERE s.snapshot_id = ?1");
        params.push(SqlValue::Integer(sid));
    }
    sql.push_str(
        "
        GROUP BY s.idset_id
        ORDER BY s.created_at DESC, s.idset_id DESC
        ",
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(params.iter()))?;
    let mut values = rows_to_json(&mut rows)?;
    for value in &mut values {
        parse_source_json(value);
    }
    Ok(json!({ "rows": values }))
}

pub fn saved_idset_detail(conn: &Connection, idset_id: i64) -> Result<Value> {
    let mut stmt = conn.prepare(
        "
        SELECT s.idset_id,
               s.snapshot_id,
               s.name,
               s.source_json,
               s.created_at,
               COUNT(o.object_id) AS object_count
        FROM saved_idsets s
        LEFT JOIN saved_idset_objects o ON o.idset_id = s.idset_id
        WHERE s.idset_id = ?1
        GROUP BY s.idset_id
        ",
    )?;
    let mut rows = stmt.query(params![idset_id])?;
    let mut values = rows_to_json(&mut rows)?;
    let Some(mut metadata) = values.pop() else {
        return Err(AnalysisError::InvalidQuery(format!(
            "saved idset not found: {idset_id}"
        )));
    };
    parse_source_json(&mut metadata);
    let snapshot_id = metadata
        .get("snapshot_id")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut ids_stmt = conn.prepare(
        "
        SELECT CAST(object_id AS TEXT) AS object_id
        FROM saved_idset_objects
        WHERE idset_id = ?1
        ORDER BY object_id ASC
        ",
    )?;
    let mut id_rows = ids_stmt.query(params![idset_id])?;
    let ids = rows_to_json(&mut id_rows)?;
    let int_ids: Vec<i64> = ids
        .iter()
        .filter_map(|row| row.get("object_id").and_then(Value::as_str))
        .filter_map(|id| id.parse::<i64>().ok())
        .collect();
    let hydrated = hydrate_ids(conn, snapshot_id, &int_ids)?;
    Ok(json!({
        "idset": metadata,
        "object_ids": ids,
        "rows": hydrated,
    }))
}

fn parse_source_json(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        if let Some(source) = object.remove("source_json") {
            let parsed = source
                .as_str()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .unwrap_or(Value::Null);
            object.insert("source".to_owned(), parsed);
        }
    }
}

fn query_idset(conn: &Connection, query: &str) -> Result<BTreeSet<i64>> {
    ensure_readonly_sql(query)?;
    let mut stmt = conn.prepare(query)?;
    let mut rows = stmt.query([])?;
    let mut set = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let value = row.get_ref(0)?;
        let id = match value_ref_to_json(value) {
            Value::Number(number) => number.as_i64().ok_or_else(|| {
                AnalysisError::InvalidQuery("object id must be an integer".to_owned())
            })?,
            Value::String(text) => text.parse::<i64>().map_err(|_| {
                AnalysisError::InvalidQuery("object id must parse as integer".to_owned())
            })?,
            _ => {
                return Err(AnalysisError::InvalidQuery(
                    "object id query returned non-scalar".to_owned(),
                ))
            }
        };
        set.insert(id);
    }
    Ok(set)
}

fn hydrate_ids(conn: &Connection, snapshot_id: i64, ids: &[i64]) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let mut stmt = conn.prepare(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.shallow_size,
               COALESCE(r.reachable_size, 0) AS estimated_reachable_size
        FROM objects o
        LEFT JOIN object_reachability r
          ON r.snapshot_id = o.snapshot_id
         AND r.object_id = o.object_id
         AND r.algorithm_version = ?3
         AND r.direction = 'referents'
         AND r.depth = ?4
         AND r.node_limit = ?5
         AND r.fanout_limit = ?6
        WHERE o.snapshot_id = ?1 AND o.object_id = ?2
        ",
    )?;
    for id in ids {
        let mut query_rows = stmt.query(params![
            snapshot_id,
            id,
            REACHABILITY_ALGORITHM_VERSION,
            DEFAULT_REACHABILITY_DEPTH,
            DEFAULT_REACHABILITY_NODE_LIMIT,
            DEFAULT_REACHABILITY_FANOUT_LIMIT
        ])?;
        if let Some(row) = rows_to_json(&mut query_rows)?.into_iter().next() {
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn schema_summary(conn: &Connection) -> Result<Value> {
    Ok(store_schema_summary(conn)?)
}

pub fn doctor(conn: &Connection) -> Result<Value> {
    let snapshot_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    let object_count: i64 = conn.query_row("SELECT COUNT(*) FROM objects", [], |row| row.get(0))?;
    let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
    let warning_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM import_warnings", [], |row| row.get(0))?;
    let reachability_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM object_reachability", [], |row| {
            row.get(0)
        })?;
    let indexes = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index'")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({
        "schema_version": pygco_store::SCHEMA_VERSION,
        "snapshot_count": snapshot_count,
        "object_count": object_count,
        "edge_count": edge_count,
        "warning_count": warning_count,
        "reachability_cache_rows": reachability_count,
        "indexes_ok": indexes.iter().any(|name| name == "idx_edges_snapshot_to") && indexes.iter().any(|name| name == "idx_objects_snapshot_type"),
        "indexes": indexes,
    }))
}

#[derive(Debug, Clone)]
struct FindingDraft {
    kind: FindingKind,
    severity: FindingSeverity,
    title: String,
    message: String,
    action: String,
    evidence: Value,
}

pub fn finding_evidence_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "pygco finding evidence",
        "type": "object",
        "required": ["schema_version", "kind", "subject", "metrics", "links"],
        "properties": {
            "schema_version": { "type": "integer", "const": 1 },
            "kind": { "type": "string", "enum": FindingKind::values() },
            "subject": { "type": "object" },
            "metrics": { "type": "object" },
            "links": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["label", "href"],
                    "properties": {
                        "label": { "type": "string" },
                        "href": { "type": "string" }
                    }
                }
            }
        }
    })
}

pub fn refresh_findings(conn: &Connection, snapshot_id: i64) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, Some(snapshot_id))?;
    let drafts = generate_findings(conn, sid)?;
    conn.execute(
        "DELETE FROM findings WHERE snapshot_id = ?1 AND algorithm_version = ?2",
        params![sid, REACHABILITY_ALGORITHM_VERSION],
    )?;
    let created_at = now_rfc3339();
    let mut stmt = conn.prepare(
        "
        INSERT INTO findings(
          snapshot_id, kind, severity, title, message, action,
          evidence_json, algorithm_version, created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
    )?;
    let mut written = 0;
    for draft in drafts {
        stmt.execute(params![
            sid,
            draft.kind.as_str(),
            draft.severity.as_str(),
            draft.title,
            draft.message,
            draft.action,
            draft.evidence.to_string(),
            REACHABILITY_ALGORITHM_VERSION,
            created_at.as_str()
        ])?;
        written += 1;
    }
    Ok(json!({
        "snapshot_id": sid,
        "algorithm_version": REACHABILITY_ALGORITHM_VERSION,
        "written": written
    }))
}

pub fn findings(conn: &Connection, options: FindingsOptions) -> Result<Value> {
    let options = options.normalized();
    let sid = resolve_snapshot_id(conn, options.snapshot_id)?;
    ensure_findings(conn, sid)?;
    let kind = options
        .kind
        .as_deref()
        .map(FindingKind::parse)
        .transpose()?;
    let severity = options
        .severity
        .as_deref()
        .map(FindingSeverity::parse)
        .transpose()?;

    let mut clauses = vec![
        "snapshot_id = ?".to_owned(),
        "algorithm_version = ?".to_owned(),
    ];
    let mut params = vec![
        SqlValue::Integer(sid),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
    ];
    if let Some(kind) = kind {
        clauses.push("kind = ?".to_owned());
        params.push(SqlValue::Text(kind.as_str().to_owned()));
    }
    if let Some(severity) = severity {
        clauses.push("severity = ?".to_owned());
        params.push(SqlValue::Text(severity.as_str().to_owned()));
    }
    let where_sql = clauses.join(" AND ");
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM findings WHERE {where_sql}"),
        params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    let mut select_params = params.clone();
    select_params.push(SqlValue::Integer(options.limit));
    select_params.push(SqlValue::Integer(options.offset));
    let mut stmt = conn.prepare(&format!(
        "
        SELECT finding_id,
               snapshot_id,
               kind,
               severity,
               title,
               message,
               action,
               evidence_json,
               algorithm_version,
               created_at
        FROM findings
        WHERE {where_sql}
        ORDER BY CASE severity WHEN 'warn' THEN 0 ELSE 1 END, finding_id ASC
        LIMIT ? OFFSET ?
        "
    ))?;
    let mut rows = stmt.query(params_from_iter(select_params.iter()))?;
    let mut values = rows_to_json(&mut rows)?;
    for row in &mut values {
        inflate_finding_row(row);
    }
    Ok(json!({
        "rows": values,
        "total": total,
        "limit": options.limit,
        "offset": options.offset,
        "kind": options.kind,
        "severity": options.severity
    }))
}

pub fn suspects(conn: &Connection, options: SuspectsOptions) -> Result<Value> {
    let options = options.normalized();
    let sid = resolve_snapshot_id(conn, options.snapshot_id)?;
    if !object_list_metrics_available(conn, sid)? {
        return Err(AnalysisError::InvalidQuery(
            "suspects requires object_list_metrics; re-import the dump with current pygco"
                .to_owned(),
        ));
    }
    let kinds: Vec<SuspectKind> = if options.kinds.is_empty() {
        SuspectKind::default_kinds().to_vec()
    } else {
        options
            .kinds
            .iter()
            .map(|kind| SuspectKind::parse(kind))
            .collect::<Result<Vec<_>>>()?
    };
    let query_limit = options
        .limit
        .saturating_add(options.offset)
        .max(options.limit);
    let mut rows = Vec::new();
    for kind in &kinds {
        let mut kind_rows = match kind {
            SuspectKind::OrphanRetained => orphan_retained_suspects(conn, sid, &options, query_limit)?,
            SuspectKind::HighRetainedRoot => high_retained_root_suspects(conn, sid, &options, query_limit)?,
            SuspectKind::TruncatedRoot => truncated_root_suspects(conn, sid, &options, query_limit)?,
            SuspectKind::TypeFootprint => type_footprint_suspects(conn, sid, &options, query_limit)?,
            SuspectKind::MetadataHeavy => metadata_heavy_suspects(conn, sid, &options, query_limit)?,
            SuspectKind::CacheHeavy => pattern_type_suspects(
                conn,
                sid,
                &options,
                query_limit,
                SuspectKind::CacheHeavy,
                &["%cache%", "%cached%", "%lru%", "%ttl%", "%pool%", "%inmemory%"],
                "Cache-like types have a high footprint. Check configured bounds and eviction behavior.",
                "Cache cohorts are pattern-based in this phase and may include legitimate bounded caches.",
            )?,
            SuspectKind::AsyncBacklog => pattern_type_suspects(
                conn,
                sid,
                &options,
                query_limit,
                SuspectKind::AsyncBacklog,
                &["%asyncio%", "%_asyncio%", "%task%", "%future%", "%async_generator%", "%anyio%"],
                "Async-related types have a high footprint. Check for pending tasks, futures, callbacks, or unclosed async generators.",
                "Async state is inferred from type/module names; task state is not available in the current dump format.",
            )?,
            SuspectKind::ConnectionHeavy => pattern_type_suspects(
                conn,
                sid,
                &options,
                query_limit,
                SuspectKind::ConnectionHeavy,
                &["%connection%", "%connectionpool%", "%redis%", "%socket%", "%poolmanager%", "%httpconnection%"],
                "Connection or pool types have a high footprint. Check pool sizing and resource lifecycle.",
                "Connection state is inferred from type/module names; live socket state is not available in the current dump format.",
            )?,
        };
        rows.append(&mut kind_rows);
    }
    let total = rows.len();
    let rows: Vec<Value> = rows
        .into_iter()
        .skip(options.offset as usize)
        .take(options.limit as usize)
        .collect();
    Ok(json!({
        "snapshot_id": sid,
        "rows": rows,
        "total": total,
        "limit": options.limit,
        "offset": options.offset,
        "kinds": kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(),
        "min_reachable_size": options.min_reachable_size,
        "meta": {
            "estimated_reachable": true,
            "confidence_note": "suspects are investigation leads, not confirmed leaks",
            "algorithm_version": REACHABILITY_ALGORITHM_VERSION,
            "reachability": ReachabilityParams::default()
        }
    }))
}

fn orphan_retained_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
) -> Result<Vec<Value>> {
    let stub_clause = suspect_stub_clause(options);
    let module_clause = suspect_module_clause(options);
    let sql = format!(
        "
        WITH candidates AS (
          SELECT CAST(o.object_id AS TEXT) AS object_id,
                 o.type,
                 o.module,
                 o.qualname,
                 m.shallow_size,
                 m.reachable_size AS estimated_reachable_size,
                 m.reachable_count AS estimated_reachable_count,
                 m.reachable_truncated,
                 m.in_edges,
                 m.out_edges,
                 m.missing_referents,
                 (
                   SELECT COUNT(*)
                   FROM edges se INDEXED BY idx_edges_snapshot_from_to
                   WHERE se.snapshot_id = m.snapshot_id
                     AND se.from_id = m.object_id
                     AND se.to_id = m.object_id
                 ) AS self_edges
          FROM object_list_metrics m INDEXED BY idx_object_list_metrics_reachable
          JOIN objects o
            ON o.snapshot_id = m.snapshot_id
           AND o.object_id = m.object_id
          WHERE m.snapshot_id = ?1
            AND m.reachable_size >= ?2
            {stub_clause}
            {module_clause}
        )
        SELECT *,
               (in_edges - self_edges) AS external_in_edges
        FROM candidates
        WHERE (in_edges - self_edges) = 0
        ORDER BY estimated_reachable_size DESC
        LIMIT ?3
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![snapshot_id, options.min_reachable_size, limit])?;
    let values = rows_to_json(&mut rows)?;
    Ok(values
        .into_iter()
        .map(|row| object_suspect(
            SuspectKind::OrphanRetained,
            "warn",
            "medium",
            "Object has no external referrers and retains a large estimated subgraph.",
            vec![
                "This can be GC-pending cyclic garbage or a temporary dump-time island, not proof of a leak.",
                "Current dump data does not expose owner field names, dict keys, or frame locals.",
            ],
            snapshot_id,
            row,
        ))
        .collect())
}

fn high_retained_root_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
) -> Result<Vec<Value>> {
    let stub_clause = suspect_stub_clause(options);
    let module_clause = suspect_module_clause(options);
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               m.shallow_size,
               m.reachable_size AS estimated_reachable_size,
               m.reachable_count AS estimated_reachable_count,
               m.reachable_truncated,
               m.in_edges,
               m.out_edges,
               m.missing_referents
        FROM object_list_metrics m
        JOIN objects o
          ON o.snapshot_id = m.snapshot_id
         AND o.object_id = m.object_id
        WHERE m.snapshot_id = ?1
          AND m.reachable_size >= ?2
          {stub_clause}
          {module_clause}
        ORDER BY m.reachable_size DESC, m.shallow_size DESC, o.object_id ASC
        LIMIT ?3
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![snapshot_id, options.min_reachable_size, limit])?;
    let values = rows_to_json(&mut rows)?;
    Ok(values
        .into_iter()
        .map(|row| object_suspect(
            SuspectKind::HighRetainedRoot,
            severity_for_size(int_field(&row, "estimated_reachable_size")),
            "medium",
            "Object has a large estimated reachable subgraph and is worth inspecting as a potential root.",
            vec![
                "Estimated reachable sizes overlap across roots and must not be summed as retained memory.",
                "Use diff between same-process snapshots before calling this a leak.",
            ],
            snapshot_id,
            row,
        ))
        .collect())
}

fn truncated_root_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
) -> Result<Vec<Value>> {
    let stub_clause = suspect_stub_clause(options);
    let module_clause = suspect_module_clause(options);
    let sql = format!(
        "
        SELECT CAST(o.object_id AS TEXT) AS object_id,
               o.type,
               o.module,
               o.qualname,
               m.shallow_size,
               m.reachable_size AS estimated_reachable_size,
               m.reachable_count AS estimated_reachable_count,
               m.reachable_truncated,
               m.in_edges,
               m.out_edges,
               m.missing_referents
        FROM object_list_metrics m
        JOIN objects o
          ON o.snapshot_id = m.snapshot_id
         AND o.object_id = m.object_id
        WHERE m.snapshot_id = ?1
          AND m.reachable_truncated != 0
          {stub_clause}
          {module_clause}
        ORDER BY m.reachable_size DESC, o.object_id ASC
        LIMIT ?2
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![snapshot_id, limit])?;
    let values = rows_to_json(&mut rows)?;
    Ok(values
        .into_iter()
        .map(|row| {
            object_suspect(
                SuspectKind::TruncatedRoot,
                "info",
                "low",
                "Reachability traversal hit configured limits for this root.",
                vec![
                    "The estimated reachable size is a lower-bound sample for this root.",
                    "Increase reachability depth/node/fanout limits or inspect a smaller subgraph.",
                ],
                snapshot_id,
                row,
            )
        })
        .collect())
}

fn type_footprint_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
) -> Result<Vec<Value>> {
    let module_clause = suspect_type_module_clause(options);
    let sql = format!(
        "
        SELECT ts.type,
               ts.module,
               ts.count,
               ts.shallow_size_sum,
               ts.in_edges,
               ts.out_edges,
               ts.stub_count,
               COALESCE(trs.reachable_size_sum, 0) AS estimated_reachable_size_sum,
               COALESCE(trs.reachable_size_max, 0) AS estimated_reachable_size_max,
               COALESCE(trs.truncated_count, 0) AS reachable_truncated_count
        FROM type_stats ts
        LEFT JOIN type_reachability_stats trs
          ON trs.snapshot_id = ts.snapshot_id
         AND trs.type = ts.type
         AND trs.algorithm_version = ?2
         AND trs.direction = 'referents'
         AND trs.depth = ?3
         AND trs.node_limit = ?4
         AND trs.fanout_limit = ?5
        WHERE ts.snapshot_id = ?1
          AND (ts.shallow_size_sum >= ?6 OR COALESCE(trs.reachable_size_sum, 0) >= ?6)
          {module_clause}
        ORDER BY ts.shallow_size_sum DESC, COALESCE(trs.reachable_size_sum, 0) DESC, ts.type ASC
        LIMIT ?7
        "
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![
        snapshot_id,
        REACHABILITY_ALGORITHM_VERSION,
        DEFAULT_REACHABILITY_DEPTH,
        DEFAULT_REACHABILITY_NODE_LIMIT,
        DEFAULT_REACHABILITY_FANOUT_LIMIT,
        options.min_reachable_size,
        limit
    ])?;
    let values = rows_to_json(&mut rows)?;
    Ok(values
        .into_iter()
        .map(|row| {
            type_suspect(
            SuspectKind::TypeFootprint,
            severity_for_size(
                int_field(&row, "estimated_reachable_size_sum")
                    .max(int_field(&row, "shallow_size_sum")),
            ),
            "medium",
            "Type has a high shallow or estimated reachable footprint.",
            "Type-level reachable sums overlap and are best used for ranking, not exact totals.",
            snapshot_id,
            row,
        )
        })
        .collect())
}

fn metadata_heavy_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
) -> Result<Vec<Value>> {
    pattern_type_suspects(
        conn,
        snapshot_id,
        options,
        limit,
        SuspectKind::MetadataHeavy,
        &[
            "pydantic%",
            "pydantic_core%",
            "fastapi%",
            "starlette%",
            "sqlalchemy%",
            "typing%",
            "typing_extensions%",
            "%pydantic%",
            "%sqlalchemy%",
        ],
        "Framework metadata types have a high footprint. This is often steady-state framework cost.",
        "Single-dump metadata footprint is not leak evidence; compare same-process snapshots for growth.",
    )
}

fn pattern_type_suspects(
    conn: &Connection,
    snapshot_id: i64,
    options: &SuspectsOptions,
    limit: i64,
    kind: SuspectKind,
    patterns: &[&str],
    reason: &str,
    limitation: &str,
) -> Result<Vec<Value>> {
    let module_clause = suspect_type_module_clause(options);
    let pattern_clause = patterns
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let param = index + 7;
            format!("LOWER(ts.module || ' ' || ts.type) LIKE ?{param}")
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    let sql = format!(
        "
        SELECT ts.type,
               ts.module,
               ts.count,
               ts.shallow_size_sum,
               ts.in_edges,
               ts.out_edges,
               ts.stub_count,
               COALESCE(trs.reachable_size_sum, 0) AS estimated_reachable_size_sum,
               COALESCE(trs.reachable_size_max, 0) AS estimated_reachable_size_max,
               COALESCE(trs.truncated_count, 0) AS reachable_truncated_count
        FROM type_stats ts
        LEFT JOIN type_reachability_stats trs
          ON trs.snapshot_id = ts.snapshot_id
         AND trs.type = ts.type
         AND trs.algorithm_version = ?2
         AND trs.direction = 'referents'
         AND trs.depth = ?3
         AND trs.node_limit = ?4
         AND trs.fanout_limit = ?5
        WHERE ts.snapshot_id = ?1
          AND (ts.shallow_size_sum >= ?6 OR COALESCE(trs.reachable_size_sum, 0) >= ?6)
          AND ({pattern_clause})
          {module_clause}
        ORDER BY ts.shallow_size_sum DESC, COALESCE(trs.reachable_size_sum, 0) DESC, ts.type ASC
        LIMIT ?{}
        ",
        patterns.len() + 7
    );
    let mut query_params = vec![
        SqlValue::Integer(snapshot_id),
        SqlValue::Integer(REACHABILITY_ALGORITHM_VERSION),
        SqlValue::Integer(DEFAULT_REACHABILITY_DEPTH),
        SqlValue::Integer(DEFAULT_REACHABILITY_NODE_LIMIT),
        SqlValue::Integer(DEFAULT_REACHABILITY_FANOUT_LIMIT),
        SqlValue::Integer(options.min_reachable_size),
    ];
    query_params.extend(
        patterns
            .iter()
            .map(|pattern| SqlValue::Text(pattern.to_ascii_lowercase())),
    );
    query_params.push(SqlValue::Integer(limit));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(query_params.iter()))?;
    let values = rows_to_json(&mut rows)?;
    Ok(values
        .into_iter()
        .map(|row| {
            type_suspect(
                kind,
                severity_for_size(
                    int_field(&row, "estimated_reachable_size_sum")
                        .max(int_field(&row, "shallow_size_sum")),
                ),
                "medium",
                reason,
                limitation,
                snapshot_id,
                row,
            )
        })
        .collect())
}

fn object_suspect(
    kind: SuspectKind,
    severity: &str,
    confidence: &str,
    reason: &str,
    limitations: Vec<&str>,
    snapshot_id: i64,
    row: Value,
) -> Value {
    let object_id = string_field(&row, "object_id");
    json!({
        "kind": kind.as_str(),
        "severity": severity,
        "confidence": confidence,
        "subject": {
            "kind": "object",
            "object_id": object_id,
            "type": string_field(&row, "type"),
            "module": string_field(&row, "module"),
            "qualname": string_field(&row, "qualname"),
        },
        "metrics": row,
        "reason": reason,
        "limitations": limitations,
        "next_command": format!("pygco object DB --snapshot {snapshot_id} --id {object_id} --format json")
    })
}

fn type_suspect(
    kind: SuspectKind,
    severity: &str,
    confidence: &str,
    reason: &str,
    limitation: &str,
    snapshot_id: i64,
    row: Value,
) -> Value {
    let type_name = string_field(&row, "type");
    json!({
        "kind": kind.as_str(),
        "severity": severity,
        "confidence": confidence,
        "subject": {
            "kind": "type",
            "type": type_name,
            "module": string_field(&row, "module"),
        },
        "metrics": row,
        "reason": reason,
        "limitations": [limitation],
        "next_command": format!("pygco objects DB --snapshot {snapshot_id} --type {} --sort reachable-size --limit 20 --format table", shell_quote(&type_name))
    })
}

fn suspect_stub_clause(options: &SuspectsOptions) -> &'static str {
    if options.include_stub {
        ""
    } else {
        "AND m.stub = 0"
    }
}

fn suspect_module_clause(options: &SuspectsOptions) -> &'static str {
    if options.non_builtin {
        "AND m.module NOT IN ('builtins','abc','types','typing','weakref','enum','_thread')"
    } else {
        ""
    }
}

fn suspect_type_module_clause(options: &SuspectsOptions) -> &'static str {
    if options.non_builtin {
        "AND ts.module NOT IN ('builtins','abc','types','typing','weakref','enum','_thread')"
    } else {
        ""
    }
}

fn severity_for_size(size: i64) -> &'static str {
    if size >= 10 * 1024 * 1024 {
        "warn"
    } else {
        "info"
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn ensure_findings(conn: &Connection, snapshot_id: i64) -> Result<()> {
    let count: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM findings
        WHERE snapshot_id = ?1 AND algorithm_version = ?2
        ",
        params![snapshot_id, REACHABILITY_ALGORITHM_VERSION],
        |row| row.get(0),
    )?;
    if count == 0 {
        refresh_findings(conn, snapshot_id)?;
    }
    Ok(())
}

fn generate_findings(conn: &Connection, snapshot_id: i64) -> Result<Vec<FindingDraft>> {
    let mut findings = Vec::new();
    let top_large = top_types(conn, snapshot_id, "shallow_size", 5, true)?;
    for item in top_large.as_array().into_iter().flatten() {
        let type_name = item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let module = item.get("module").and_then(Value::as_str).unwrap_or("");
        let links = json!([
            {
                "label": "Open Objects filtered by type",
                "href": format!(
                    "/?page=objects&snapshot={snapshot_id}&type={}",
                    percent_encode_query(type_name)
                )
            }
        ]);
        findings.push(FindingDraft {
            kind: FindingKind::LargeType,
            severity: FindingSeverity::Info,
            title: format!("Large type candidate: {type_name}"),
            message: "This type is worth inspecting because its shallow size total is high."
                .to_owned(),
            action: "Open Objects filtered by this type and inspect referents/referrers."
                .to_owned(),
            evidence: finding_evidence(
                FindingKind::LargeType,
                json!({ "type": type_name, "module": module }),
                item.clone(),
                links,
            ),
        });
    }
    let missing = missing_stub_summary(conn, snapshot_id)?;
    if int_field(&missing, "missing_referent_count") > 0 {
        let links = json!([
            {
                "label": "Open Objects for this snapshot",
                "href": format!("/?page=objects&snapshot={snapshot_id}")
            }
        ]);
        findings.push(FindingDraft {
            kind: FindingKind::MissingReferents,
            severity: FindingSeverity::Warn,
            title: "Missing referents detected".to_owned(),
            message: "Some edges point to object ids without object records; graph exploration may be incomplete."
                .to_owned(),
            action: "Enable referent stubs in the producer or compare with another dump."
                .to_owned(),
            evidence: finding_evidence(
                FindingKind::MissingReferents,
                json!({ "snapshot_id": snapshot_id }),
                missing,
                links,
            ),
        });
    }
    Ok(findings)
}

fn finding_evidence(kind: FindingKind, subject: Value, metrics: Value, links: Value) -> Value {
    json!({
        "schema_version": 1,
        "kind": kind.as_str(),
        "subject": subject,
        "metrics": metrics,
        "links": links
    })
}

fn inflate_finding_row(row: &mut Value) {
    let Some(object) = row.as_object_mut() else {
        return;
    };
    let evidence = match object.remove("evidence_json") {
        Some(Value::String(text)) => serde_json::from_str::<Value>(&text).unwrap_or(Value::Null),
        Some(value) => value,
        None => Value::Null,
    };
    let links = evidence
        .get("links")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    object.insert("links".to_owned(), links);
    object.insert("evidence".to_owned(), evidence);
}

fn percent_encode_query(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn report_json(conn: &Connection, snapshot_id: Option<i64>) -> Result<Value> {
    let sid = resolve_snapshot_id(conn, snapshot_id)?;
    Ok(json!({
        "snapshot_id": sid,
        "summary": summary(conn, Some(sid), 10)?,
        "findings": findings(conn, FindingsOptions { snapshot_id: Some(sid), limit: 10, ..FindingsOptions::default() })?,
        "finding_evidence_schema": finding_evidence_schema(),
        "algorithm_parameters": ReachabilityParams::default(),
    }))
}

pub fn report_markdown(conn: &Connection, snapshot_id: Option<i64>) -> Result<String> {
    let report = report_json(conn, snapshot_id)?;
    let snapshot = report["summary"]["snapshot"].clone();
    let mut out = String::new();
    out.push_str("# Memory Forensics Report\n\n");
    out.push_str("## Snapshot\n\n");
    out.push_str(&format!(
        "- Snapshot: {}\n- Objects: {}\n- Edges: {}\n- Shallow size: {}\n\n",
        snapshot["snapshot_id"],
        snapshot["object_count"],
        snapshot["edge_count"],
        snapshot["shallow_size_sum"]
    ));
    out.push_str("## Top Leads\n\n");
    if let Some(rows) = report["findings"]["rows"].as_array() {
        for row in rows {
            out.push_str(&format!(
                "- **{}** (`{}`): {}\n",
                row["title"].as_str().unwrap_or("Lead"),
                row["severity"].as_str().unwrap_or("info"),
                row["action"].as_str().unwrap_or("")
            ));
            if let Some(links) = row["links"].as_array() {
                for link in links {
                    let label = link["label"].as_str().unwrap_or("Open related view");
                    let href = link["href"].as_str().unwrap_or("#");
                    out.push_str(&format!("  - [{label}]({href})\n"));
                }
            }
        }
    }
    out.push_str("\n## Algorithm Parameters\n\n");
    out.push_str("- Reachable size is estimated.\n");
    out.push_str("- direction: referents\n");
    out.push_str(&format!("- depth: {}\n", DEFAULT_REACHABILITY_DEPTH));
    out.push_str(&format!(
        "- node_limit: {}\n",
        DEFAULT_REACHABILITY_NODE_LIMIT
    ));
    out.push_str(&format!(
        "- fanout_limit: {}\n",
        DEFAULT_REACHABILITY_FANOUT_LIMIT
    ));
    Ok(out)
}

pub fn export_subgraph_dot(graph: &Value) -> String {
    let mut out = String::from("digraph pygco {\n");
    if let Some(nodes) = graph.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            let id = node.get("object_id").and_then(Value::as_str).unwrap_or("?");
            let label = node.get("type").and_then(Value::as_str).unwrap_or("object");
            out.push_str(&format!("  \"{id}\" [label=\"{id}\\n{label}\"];\n"));
        }
    }
    if let Some(edges) = graph.get("edges").and_then(Value::as_array) {
        for edge in edges {
            let from = edge.get("from_id").and_then(Value::as_str).unwrap_or("?");
            let to = edge.get("to_id").and_then(Value::as_str).unwrap_or("?");
            out.push_str(&format!("  \"{from}\" -> \"{to}\";\n"));
        }
    }
    if let Some(edges) = graph.get("missing_edges").and_then(Value::as_array) {
        for edge in edges {
            let from = edge.get("from_id").and_then(Value::as_str).unwrap_or("?");
            let to = edge.get("to_id").and_then(Value::as_str).unwrap_or("?");
            out.push_str(&format!("  \"{from}\" -> \"{to}\" [style=dashed];\n"));
        }
    }
    out.push_str("}\n");
    out
}

fn bool_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}
