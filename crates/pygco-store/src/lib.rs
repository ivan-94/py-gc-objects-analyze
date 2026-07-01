use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

pub const SCHEMA_VERSION: i64 = 1;
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("query is not read-only")]
    NotReadOnly,
    #[error("database has no snapshots")]
    NoSnapshots,
    #[error("snapshot not found: {0}")]
    SnapshotNotFound(i64),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub fn connect(path: impl AsRef<Path>) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn connect_readonly(path: impl AsRef<Path>) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "query_only", "ON")?;
    Ok(conn)
}

pub fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    let now = now_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('tool_version', ?1)",
        [TOOL_VERSION],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('created_at', ?1)",
        [now],
    )?;
    Ok(())
}

pub fn apply_import_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = OFF;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        ",
    )?;
    Ok(())
}

pub fn finalize_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA optimize;
        ",
    )?;
    Ok(())
}

pub fn create_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(INDEX_SQL)?;
    Ok(())
}

pub fn latest_snapshot_id(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT snapshot_id FROM snapshots ORDER BY snapshot_id DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => StoreError::NoSnapshots,
        other => StoreError::Sqlite(other),
    })
}

pub fn resolve_snapshot_id(conn: &Connection, snapshot_id: Option<i64>) -> Result<i64> {
    match snapshot_id {
        Some(id) => {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM snapshots WHERE snapshot_id = ?1",
                [id],
                |row| row.get(0),
            )?;
            if exists == 0 {
                Err(StoreError::SnapshotNotFound(id))
            } else {
                Ok(id)
            }
        }
        None => latest_snapshot_id(conn),
    }
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn is_readonly_sql(query: &str) -> bool {
    let trimmed = query.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with("select") || lowered.starts_with("with")
}

pub fn ensure_readonly_sql(query: &str) -> Result<()> {
    if is_readonly_sql(query) {
        Ok(())
    } else {
        Err(StoreError::NotReadOnly)
    }
}

pub fn rows_to_json(rows: &mut rusqlite::Rows<'_>) -> rusqlite::Result<Vec<Value>> {
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let row_ref = row.as_ref();
        let mut object = Map::new();
        for index in 0..row_ref.column_count() {
            let name = row_ref.column_name(index)?.to_owned();
            let value = value_ref_to_json(row.get_ref(index)?);
            object.insert(name, value);
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}

pub fn value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => json!(format!("<{} bytes>", value.len())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRow {
    pub snapshot_id: i64,
    pub source_uri: String,
    pub source_basename: String,
    pub dump_sha256: String,
    pub dump_format: String,
    pub dump_format_version: i64,
    pub producer: String,
    pub producer_version: String,
    pub producer_run_id: Option<String>,
    pub dump_sequence: Option<i64>,
    pub process_started_at: Option<String>,
    pub host_id: Option<String>,
    pub container_id: Option<String>,
    pub pid: Option<i64>,
    pub python_version: Option<String>,
    pub platform: Option<String>,
    pub created_at: Option<String>,
    pub imported_at: String,
    pub import_options_json: String,
    pub object_count: i64,
    pub edge_count: i64,
    pub stub_count: i64,
    pub missing_referent_count: i64,
    pub shallow_size_sum: i64,
}

pub fn snapshot_row(conn: &Connection, snapshot_id: i64) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT * FROM snapshots WHERE snapshot_id = ?1")?;
    let mut rows = stmt.query([snapshot_id])?;
    let rows = rows_to_json(&mut rows)?;
    rows.into_iter()
        .next()
        .ok_or(StoreError::SnapshotNotFound(snapshot_id))
}

pub fn schema_summary(conn: &Connection) -> Result<Value> {
    let mut tables_stmt = conn.prepare(
        "
        SELECT name, sql
        FROM sqlite_master
        WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
        ORDER BY name
        ",
    )?;
    let mut table_rows = tables_stmt.query([])?;
    let tables = rows_to_json(&mut table_rows)?;
    let mut columns = Map::new();
    for table in &tables {
        let name = table
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_identifier(name)))?;
        let mut rows = stmt.query([])?;
        columns.insert(name.to_owned(), Value::Array(rows_to_json(&mut rows)?));
    }
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "tables": tables,
        "columns": columns,
    }))
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS snapshots (
  snapshot_id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_uri TEXT NOT NULL,
  source_basename TEXT NOT NULL,
  dump_sha256 TEXT NOT NULL,
  dump_format TEXT NOT NULL,
  dump_format_version INTEGER NOT NULL,
  producer TEXT NOT NULL,
  producer_version TEXT NOT NULL,
  producer_run_id TEXT,
  dump_sequence INTEGER,
  process_started_at TEXT,
  host_id TEXT,
  container_id TEXT,
  pid INTEGER,
  python_version TEXT,
  platform TEXT,
  created_at TEXT,
  imported_at TEXT NOT NULL,
  import_options_json TEXT NOT NULL,
  object_count INTEGER NOT NULL DEFAULT 0,
  edge_count INTEGER NOT NULL DEFAULT 0,
  stub_count INTEGER NOT NULL DEFAULT 0,
  missing_referent_count INTEGER NOT NULL DEFAULT 0,
  shallow_size_sum INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS objects (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  qualname TEXT NOT NULL,
  shallow_size INTEGER,
  gc_tracked INTEGER,
  stub INTEGER NOT NULL DEFAULT 0,
  repr TEXT,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS edges (
  snapshot_id INTEGER NOT NULL,
  from_id INTEGER NOT NULL,
  edge_index INTEGER NOT NULL,
  to_id INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, from_id, edge_index),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS object_edge_stats (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  missing_referents INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS object_list_metrics (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  shallow_size INTEGER NOT NULL,
  stub INTEGER NOT NULL,
  reachable_count INTEGER NOT NULL,
  reachable_size INTEGER NOT NULL,
  reachable_truncated INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  missing_referents INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, object_id),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS type_stats (
  snapshot_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  stub_count INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, type),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS module_stats (
  snapshot_id INTEGER NOT NULL,
  module TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  in_edges INTEGER NOT NULL,
  out_edges INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, module),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS cohort_stats (
  snapshot_id INTEGER NOT NULL,
  cohort TEXT NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  type_count INTEGER NOT NULL,
  details_json TEXT NOT NULL,
  rules_version TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, cohort),
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS object_reachability (
  snapshot_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  algorithm_version INTEGER NOT NULL,
  direction TEXT NOT NULL,
  depth INTEGER NOT NULL,
  node_limit INTEGER NOT NULL,
  fanout_limit INTEGER NOT NULL,
  reachable_count INTEGER NOT NULL,
  reachable_size INTEGER NOT NULL,
  truncated INTEGER NOT NULL,
  computed_at TEXT NOT NULL,
  PRIMARY KEY (
    snapshot_id,
    object_id,
    algorithm_version,
    direction,
    depth,
    node_limit,
    fanout_limit
  ),
  FOREIGN KEY (snapshot_id, object_id) REFERENCES objects(snapshot_id, object_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS type_reachability_stats (
  snapshot_id INTEGER NOT NULL,
  type TEXT NOT NULL,
  module TEXT NOT NULL,
  algorithm_version INTEGER NOT NULL,
  direction TEXT NOT NULL,
  depth INTEGER NOT NULL,
  node_limit INTEGER NOT NULL,
  fanout_limit INTEGER NOT NULL,
  count INTEGER NOT NULL,
  shallow_size_sum INTEGER NOT NULL,
  reachable_size_sum INTEGER NOT NULL,
  reachable_size_avg REAL NOT NULL,
  reachable_size_max INTEGER NOT NULL,
  truncated_count INTEGER NOT NULL,
  PRIMARY KEY (snapshot_id, type, algorithm_version, direction, depth, node_limit, fanout_limit)
);

CREATE TABLE IF NOT EXISTS findings (
  finding_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  kind TEXT NOT NULL,
  severity TEXT NOT NULL,
  title TEXT NOT NULL,
  message TEXT NOT NULL,
  action TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  algorithm_version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS saved_idsets (
  idset_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  source_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (snapshot_id) REFERENCES snapshots(snapshot_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS saved_idset_objects (
  idset_id INTEGER NOT NULL,
  object_id INTEGER NOT NULL,
  PRIMARY KEY (idset_id, object_id),
  FOREIGN KEY (idset_id) REFERENCES saved_idsets(idset_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS import_warnings (
  warning_id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_id INTEGER,
  level TEXT NOT NULL,
  code TEXT NOT NULL,
  message TEXT NOT NULL,
  context_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
"#;

const INDEX_SQL: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_snapshots_dump_sha256 ON snapshots(dump_sha256);
CREATE INDEX IF NOT EXISTS idx_snapshots_producer_run ON snapshots(producer_run_id, dump_sequence);
CREATE INDEX IF NOT EXISTS idx_objects_snapshot_type ON objects(snapshot_id, type);
CREATE INDEX IF NOT EXISTS idx_objects_snapshot_type_object ON objects(snapshot_id, type, object_id);
CREATE INDEX IF NOT EXISTS idx_objects_snapshot_module ON objects(snapshot_id, module);
CREATE INDEX IF NOT EXISTS idx_objects_snapshot_size ON objects(snapshot_id, shallow_size DESC);
CREATE INDEX IF NOT EXISTS idx_objects_snapshot_stub ON objects(snapshot_id, stub);
CREATE INDEX IF NOT EXISTS idx_edges_snapshot_from ON edges(snapshot_id, from_id);
CREATE INDEX IF NOT EXISTS idx_edges_snapshot_to ON edges(snapshot_id, to_id);
CREATE INDEX IF NOT EXISTS idx_edges_snapshot_from_to ON edges(snapshot_id, from_id, to_id);
CREATE INDEX IF NOT EXISTS idx_object_edge_stats_in ON object_edge_stats(snapshot_id, in_edges DESC);
CREATE INDEX IF NOT EXISTS idx_object_edge_stats_out ON object_edge_stats(snapshot_id, out_edges DESC);
CREATE INDEX IF NOT EXISTS idx_object_edge_stats_missing ON object_edge_stats(snapshot_id, missing_referents DESC);
CREATE INDEX IF NOT EXISTS idx_object_list_metrics_reachable
  ON object_list_metrics(snapshot_id, reachable_size DESC, object_id);
CREATE INDEX IF NOT EXISTS idx_object_list_metrics_type_reachable
  ON object_list_metrics(snapshot_id, type, reachable_size DESC, object_id);
CREATE INDEX IF NOT EXISTS idx_object_list_metrics_module_reachable
  ON object_list_metrics(snapshot_id, module, reachable_size DESC, object_id);
CREATE INDEX IF NOT EXISTS idx_object_list_metrics_in_edges
  ON object_list_metrics(snapshot_id, in_edges DESC, object_id);
CREATE INDEX IF NOT EXISTS idx_object_list_metrics_out_edges
  ON object_list_metrics(snapshot_id, out_edges DESC, object_id);
CREATE INDEX IF NOT EXISTS idx_object_reachability_size
  ON object_reachability(snapshot_id, algorithm_version, direction, depth, node_limit, fanout_limit, reachable_size DESC);
"#;
