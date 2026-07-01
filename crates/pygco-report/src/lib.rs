use rusqlite::Connection;
use serde_json::Value;

pub fn build_json(conn: &Connection, snapshot_id: Option<i64>) -> pygco_analysis::Result<Value> {
    pygco_analysis::report_json(conn, snapshot_id)
}

pub fn build_markdown(
    conn: &Connection,
    snapshot_id: Option<i64>,
) -> pygco_analysis::Result<String> {
    pygco_analysis::report_markdown(conn, snapshot_id)
}
