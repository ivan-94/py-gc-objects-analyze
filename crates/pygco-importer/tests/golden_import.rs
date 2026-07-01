use std::{fs::File, io::Write, path::PathBuf};

use flate2::{write::GzEncoder, Compression};
use pygco_analysis::{
    diff, diff_objects, idset, list_objects, object_edges, readonly_sql, summary,
    DiffObjectsOptions, ObjectFilters, ReachabilityParams,
};
use pygco_importer::{import_dumps, ImportError, ImportOptions};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::tempdir;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/golden")
        .join(name)
}

fn import_into_temp(inputs: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let paths = inputs.iter().map(|name| fixture(name)).collect();
    import_dumps(paths, db.clone(), ImportOptions::default()).unwrap();
    (dir, db)
}

#[test]
fn import_profile_includes_required_pipeline_phases() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let summary = import_dumps(
        vec![fixture("tiny-v1.jsonl.gz")],
        db,
        ImportOptions {
            profile: true,
            ..ImportOptions::default()
        },
    )
    .unwrap();
    for phase in [
        "snapshot:1:decode",
        "snapshot:1:parse",
        "snapshot:1:insert_objects",
        "snapshot:1:insert_edges",
        "snapshot:1:build_stats",
        "build_indexes",
        "reachability",
        "findings",
    ] {
        assert!(
            summary.profile.iter().any(|event| event.phase == phase),
            "missing profile phase {phase}: {:?}",
            summary.profile
        );
    }
}

#[test]
fn imports_tiny_fixture_and_matches_expected_summary() {
    let (_dir, db) = import_into_temp(&["tiny-v1.jsonl.gz"]);
    let conn = Connection::open(db).unwrap();

    let overview = summary(&conn, Some(1), 20).unwrap();
    let snapshot = &overview["snapshot"];
    assert_eq!(snapshot["object_count"], 4);
    assert_eq!(snapshot["edge_count"], 5);
    assert_eq!(snapshot["stub_count"], 0);
    assert_eq!(snapshot["missing_referent_count"], 0);
    assert_eq!(snapshot["shallow_size_sum"], 600);

    let rows = list_objects(
        &conn,
        ObjectFilters {
            snapshot_id: Some(1),
            sort: "object-id".to_owned(),
            order: "asc".to_owned(),
            limit: 10,
            ..ObjectFilters::default()
        },
    )
    .unwrap();
    let ids: Vec<_> = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["object_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["1", "2", "3", "4"]);

    let reachability = readonly_sql(
        &conn,
        "select cast(object_id as text) as object_id, reachable_count, reachable_size, truncated from object_reachability order by object_id",
        10,
        false,
    )
    .unwrap();
    assert_eq!(
        reachability["rows"],
        json!([
            { "object_id": "1", "reachable_count": 4, "reachable_size": 600, "truncated": 0 },
            { "object_id": "2", "reachable_count": 2, "reachable_size": 160, "truncated": 0 },
            { "object_id": "3", "reachable_count": 1, "reachable_size": 40, "truncated": 0 },
            { "object_id": "4", "reachable_count": 2, "reachable_size": 200, "truncated": 0 }
        ])
    );

    let persisted_findings = readonly_sql(
        &conn,
        "select kind, severity, evidence_json from findings where snapshot_id = 1 order by finding_id",
        10,
        false,
    )
    .unwrap();
    let finding_rows = persisted_findings["rows"].as_array().unwrap();
    assert!(!finding_rows.is_empty());
    assert_eq!(finding_rows[0]["kind"], "large_type");
    assert_eq!(finding_rows[0]["severity"], "info");
    let evidence: Value =
        serde_json::from_str(finding_rows[0]["evidence_json"].as_str().unwrap()).unwrap();
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["kind"], "large_type");
    assert!(evidence["links"][0]["href"]
        .as_str()
        .unwrap()
        .contains("page=objects"));
}

#[test]
fn covers_stub_missing_and_cycle_semantics() {
    let (_stub_dir, stub_db) = import_into_temp(&["stubs-v1.jsonl.gz"]);
    let stub_conn = Connection::open(stub_db).unwrap();
    let stubs = list_objects(
        &stub_conn,
        ObjectFilters {
            snapshot_id: Some(1),
            stub: Some(true),
            sort: "object-id".to_owned(),
            order: "asc".to_owned(),
            limit: 10,
            ..ObjectFilters::default()
        },
    )
    .unwrap();
    assert_eq!(stubs["rows"][0]["object_id"], "11");
    assert_eq!(stubs["rows"][0]["stub"], 1);

    let (_missing_dir, missing_db) = import_into_temp(&["missing-referents-v1.jsonl.gz"]);
    let missing_conn = Connection::open(missing_db).unwrap();
    let edges = object_edges(&missing_conn, Some(1), 20, "referents", 10, 0).unwrap();
    assert_eq!(edges["rows"][0]["from_id"], "20");
    assert_eq!(edges["rows"][0]["to_id"], "999");
    assert_eq!(edges["rows"][0]["missing"], 1);

    let (_cycle_dir, cycle_db) = import_into_temp(&["cycles-v1.jsonl.gz"]);
    let cycle_conn = Connection::open(cycle_db).unwrap();
    let reachable = readonly_sql(
        &cycle_conn,
        "select cast(object_id as text) as object_id, reachable_count, reachable_size, truncated from object_reachability order by object_id",
        10,
        false,
    )
    .unwrap();
    assert_eq!(
        reachable["rows"],
        json!([
            { "object_id": "30", "reachable_count": 2, "reachable_size": 100, "truncated": 0 },
            { "object_id": "31", "reachable_count": 2, "reachable_size": 100, "truncated": 0 }
        ])
    );
}

#[test]
fn imports_multiple_snapshots_and_diffs_lifecycle() {
    let (_dir, db) = import_into_temp(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);
    let conn = Connection::open(db).unwrap();

    let delta = diff(&conn, 1, 2, 20).unwrap();
    assert_eq!(delta["confidence"]["level"], "high");
    assert_eq!(
        delta["summary_delta"],
        json!({
            "object_count": 1,
            "edge_count": 1,
            "stub_count": 0,
            "missing_referent_count": 0,
            "shallow_size_sum": 112
        })
    );
    assert_eq!(delta["object_lifecycle"]["new_count"], 1);
    assert_eq!(delta["object_lifecycle"]["retained_count"], 2);

    let new_objects = diff_objects(
        &conn,
        DiffObjectsOptions {
            from_snapshot_id: 1,
            to_snapshot_id: 2,
            state: "new".to_owned(),
            type_name: None,
            module: None,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(new_objects["rows"][0]["object_id"], "102");

    let changed_objects = diff_objects(
        &conn,
        DiffObjectsOptions {
            from_snapshot_id: 1,
            to_snapshot_id: 2,
            state: "changed".to_owned(),
            type_name: None,
            module: None,
            limit: 10,
            offset: 0,
        },
    )
    .unwrap();
    assert_eq!(changed_objects["rows"][0]["object_id"], "101");
}

#[test]
fn rejects_writes_and_supports_idset_operations() {
    let (_dir, db) = import_into_temp(&["tiny-v1.jsonl.gz"]);
    let conn = Connection::open(db).unwrap();

    let write_error = readonly_sql(&conn, "delete from objects", 10, false).unwrap_err();
    assert!(write_error.to_string().contains("not read-only"));

    let result = idset(
        &conn,
        1,
        "select object_id from objects where object_id in (1, 2, 3)",
        "select to_id as object_id from edges where snapshot_id = 1 and from_id = 1",
        "intersect",
        true,
        10,
    )
    .unwrap();
    let ids: Vec<&str> = result["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["object_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["2", "3"]);
}

#[test]
fn duplicate_object_id_fails_and_removes_tmp_database() {
    let dir = tempdir().unwrap();
    let dump = dir.path().join("duplicate.jsonl.gz");
    write_dump(
        &dump,
        &[
            json!({
                "record_type": "metadata",
                "phase": "start",
                "format": "pygco-dump-jsonl",
                "format_version": 1,
                "producer": "pygco_dump",
                "producer_version": "0.1.0",
                "producer_run_id": "duplicate",
                "dump_sequence": 1,
                "created_at": "2026-07-01T00:00:00Z",
                "pid": 1,
                "python_version": "3.12",
                "platform": "test",
                "collect_before_dump": false,
                "include_referents": true,
                "include_referent_stubs": true,
                "include_repr": false,
                "repr_limit": 0,
                "object_count": 2
            }),
            json!({"record_type": "object", "id": 1, "type": "dict", "size": 64, "referents": []}),
            json!({"record_type": "object", "id": 1, "type": "list", "size": 80, "referents": []}),
            json!({"record_type": "metadata", "phase": "end", "dumped_count": 2, "stub_count": 0, "total_object_records": 2, "elapsed_ms": 1}),
        ],
    );
    let output = dir.path().join("analysis.sqlite");
    let err = import_dumps(vec![dump], output.clone(), ImportOptions::default()).unwrap_err();
    assert!(matches!(
        err,
        ImportError::DuplicateObjectId { object_id: 1, .. }
    ));
    assert!(!output.exists());
    assert!(!output.with_file_name("analysis.sqlite.tmp.sqlite").exists());
}

#[test]
fn custom_reachability_params_are_part_of_cache_key() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    import_dumps(
        vec![fixture("tiny-v1.jsonl.gz")],
        db.clone(),
        ImportOptions {
            reachability_params: ReachabilityParams {
                algorithm_version: 7,
                depth: 1,
                node_limit: 2,
                fanout_limit: 2,
            },
            ..ImportOptions::default()
        },
    )
    .unwrap();
    let conn = Connection::open(db).unwrap();
    let row = readonly_sql(
        &conn,
        "select algorithm_version, depth, node_limit, fanout_limit, truncated from object_reachability where object_id = 1",
        10,
        false,
    )
    .unwrap();
    assert_eq!(
        row["rows"][0],
        json!({
            "algorithm_version": 7,
            "depth": 1,
            "node_limit": 2,
            "fanout_limit": 2,
            "truncated": 1
        })
    );
}

fn write_dump(path: &std::path::Path, records: &[Value]) {
    let file = File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::fast());
    for record in records {
        writeln!(
            encoder,
            "{}",
            serde_json::to_string(record).expect("fixture json serializes")
        )
        .unwrap();
    }
    encoder.finish().unwrap();
}
