use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use flate2::{write::GzEncoder, Compression};
use serde_json::Value;
use tempfile::{tempdir, TempDir};

fn pygco() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pygco"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/golden")
        .join(name)
}

fn run(args: &[String]) -> Output {
    Command::new(pygco()).args(args).output().unwrap()
}

fn arg(value: impl AsRef<Path>) -> String {
    value.as_ref().display().to_string()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn assert_success(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "expected command success\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            text(&output.stdout),
            text(&output.stderr)
        );
    }
    text(&output.stdout)
}

fn assert_failure(output: Output, code: i32, error_code: &str) -> String {
    assert_eq!(output.status.code(), Some(code));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains(&format!("code={error_code}")),
        "stderr did not contain code={error_code}:\n{stderr}"
    );
    stderr
}

fn json_stdout(output: Output) -> Value {
    serde_json::from_str(&assert_success(output)).unwrap()
}

fn assert_snapshot(name: &str, output: Output) {
    let actual = assert_success(output);
    let expected = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(name),
    )
    .unwrap();
    assert_eq!(actual.trim_end(), expected.trim_end(), "snapshot {name}");
}

fn jsonl_stdout(output: Output) -> Vec<Value> {
    assert_success(output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn assert_json_snapshot(actual: Value, snapshot: &str) {
    let expected: Value = serde_json::from_str(snapshot).unwrap();
    assert_eq!(actual, expected);
}

fn import_db(fixtures: &[&str]) -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let mut args = vec!["import".to_owned()];
    args.extend(fixtures.iter().map(|name| arg(fixture(name))));
    args.extend([
        "-o".to_owned(),
        arg(&db),
        "--rebuild".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    let import = json_stdout(run(&args));
    assert_eq!(
        import["snapshots"].as_array().unwrap().len(),
        fixtures.len()
    );
    (dir, db)
}

#[test]
fn stable_json_command_outputs_match_snapshots() {
    let (_dir, db) = import_db(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);

    assert_snapshot(
        "objects_snapshot2.json",
        run(&[
            "objects".to_owned(),
            arg(&db),
            "--snapshot".to_owned(),
            "2".to_owned(),
            "--sort".to_owned(),
            "object-id".to_owned(),
            "--order".to_owned(),
            "asc".to_owned(),
            "--limit".to_owned(),
            "2".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]),
    );
    assert_snapshot(
        "diff_1_2.json",
        run(&[
            "diff".to_owned(),
            arg(&db),
            "--from".to_owned(),
            "1".to_owned(),
            "--to".to_owned(),
            "2".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]),
    );
    assert_snapshot(
        "paths_snapshot2_referrers_101.json",
        run(&[
            "paths".to_owned(),
            arg(&db),
            "--snapshot".to_owned(),
            "2".to_owned(),
            "--id".to_owned(),
            "101".to_owned(),
            "--direction".to_owned(),
            "referrers".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]),
    );
    assert_snapshot(
        "subgraph_snapshot2_root100.json",
        run(&[
            "export-subgraph".to_owned(),
            arg(&db),
            "--snapshot".to_owned(),
            "2".to_owned(),
            "--id".to_owned(),
            "100".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]),
    );
}

#[test]
fn object_and_analysis_commands_support_json_and_jsonl_formats() {
    let (_dir, db) = import_db(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);

    let objects_json = json_stdout(run(&[
        "objects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--sort".to_owned(),
        "object-id".to_owned(),
        "--order".to_owned(),
        "asc".to_owned(),
        "--limit".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(objects_json["rows"][0]["object_id"], "100");
    let objects_jsonl = jsonl_stdout(run(&[
        "objects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--sort".to_owned(),
        "object-id".to_owned(),
        "--order".to_owned(),
        "asc".to_owned(),
        "--limit".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(objects_jsonl[0]["object_id"], "100");

    let object_json = json_stdout(run(&[
        "object".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(object_json["object"]["object_id"], "100");
    let object_jsonl = jsonl_stdout(run(&[
        "object".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(object_jsonl[0]["object"]["object_id"], "100");

    let edges_json = json_stdout(run(&[
        "edges".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--from".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(edges_json["rows"].as_array().unwrap().len(), 2);
    let edges_jsonl = jsonl_stdout(run(&[
        "edges".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--from".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(edges_jsonl.len(), 2);

    let paths_json = json_stdout(run(&[
        "paths".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "101".to_owned(),
        "--direction".to_owned(),
        "referrers".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(paths_json["object_id"], "101");
    let paths_jsonl = jsonl_stdout(run(&[
        "paths".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "101".to_owned(),
        "--direction".to_owned(),
        "referrers".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(paths_jsonl[0]["object_id"], "101");

    let diff_json = json_stdout(run(&[
        "diff".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(diff_json["confidence"]["level"], "high");
    let diff_jsonl = jsonl_stdout(run(&[
        "diff".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(diff_jsonl[0]["confidence"]["level"], "high");

    let diff_objects_json = json_stdout(run(&[
        "diff-objects".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--state".to_owned(),
        "new".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(diff_objects_json["rows"][0]["object_id"], "102");
    let diff_objects_jsonl = jsonl_stdout(run(&[
        "diff-objects".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--state".to_owned(),
        "new".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(diff_objects_jsonl[0]["object_id"], "102");

    let idset_args = [
        "idset".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--left-query".to_owned(),
        "select object_id from objects where snapshot_id = 2".to_owned(),
        "--right-query".to_owned(),
        "select to_id as object_id from edges where snapshot_id = 2 and from_id = 100".to_owned(),
        "--op".to_owned(),
        "intersect".to_owned(),
        "--details".to_owned(),
    ];
    let mut idset_json_args = idset_args.to_vec();
    idset_json_args.extend(["--format".to_owned(), "json".to_owned()]);
    let idset_json = json_stdout(run(&idset_json_args));
    assert_eq!(idset_json["rows"].as_array().unwrap().len(), 2);
    let mut idset_jsonl_args = idset_args.to_vec();
    idset_jsonl_args.extend(["--format".to_owned(), "jsonl".to_owned()]);
    let idset_jsonl = jsonl_stdout(run(&idset_jsonl_args));
    assert_eq!(idset_jsonl.len(), 2);

    let sql_json = json_stdout(run(&[
        "sql".to_owned(),
        arg(&db),
        "--query".to_owned(),
        "select cast(object_id as text) as object_id from objects where snapshot_id = 2 order by object_id limit 1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(sql_json["rows"][0]["object_id"], "100");
    let sql_jsonl = jsonl_stdout(run(&[
        "sql".to_owned(),
        arg(&db),
        "--query".to_owned(),
        "select cast(object_id as text) as object_id from objects where snapshot_id = 2 order by object_id limit 1".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(sql_jsonl[0]["object_id"], "100");

    let schema_json = json_stdout(run(&[
        "schema".to_owned(),
        arg(&db),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert!(schema_json["tables"].as_array().unwrap().len() >= 5);
    let schema_jsonl = jsonl_stdout(run(&[
        "schema".to_owned(),
        arg(&db),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert!(schema_jsonl[0]["tables"].as_array().unwrap().len() >= 5);

    let graph_json = json_stdout(run(&[
        "export-subgraph".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert!(!graph_json["nodes"].as_array().unwrap().is_empty());
    let graph_jsonl = jsonl_stdout(run(&[
        "export-subgraph".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert!(!graph_jsonl[0]["nodes"].as_array().unwrap().is_empty());

    let report_json = json_stdout(run(&[
        "report".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(report_json["summary"]["snapshot"]["snapshot_id"], 2);
    let report_jsonl = jsonl_stdout(run(&[
        "report".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    assert_eq!(report_jsonl[0]["summary"]["snapshot"]["snapshot_id"], 2);
}

#[test]
fn representative_cli_json_outputs_match_snapshots() {
    let (_dir, db) = import_db(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);

    let objects = json_stdout(run(&[
        "objects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--sort".to_owned(),
        "object-id".to_owned(),
        "--order".to_owned(),
        "asc".to_owned(),
        "--limit".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_json_snapshot(objects, include_str!("snapshots/objects_diff_before.json"));

    let diff = json_stdout(run(&[
        "diff".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_json_snapshot(diff, include_str!("snapshots/diff_summary.json"));
}

#[test]
fn import_summary_objects_and_core_analysis_commands_work() {
    let (_dir, db) = import_db(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);

    let summary = json_stdout(run(&[
        "summary".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(summary["snapshot"]["object_count"], 3);

    let objects_output = assert_success(run(&[
        "objects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--sort".to_owned(),
        "object-id".to_owned(),
        "--order".to_owned(),
        "asc".to_owned(),
        "--limit".to_owned(),
        "2".to_owned(),
        "--fields".to_owned(),
        "object_id,type".to_owned(),
        "--format".to_owned(),
        "jsonl".to_owned(),
    ]));
    let object_lines: Vec<Value> = objects_output
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(object_lines[0]["object_id"], "100");
    assert_eq!(object_lines[0]["type"], "dict");

    let object = json_stdout(run(&[
        "object".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(object["object"]["object_id"], "100");

    let edges = json_stdout(run(&[
        "edges".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--from".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(edges["rows"].as_array().unwrap().len(), 2);

    let paths = json_stdout(run(&[
        "paths".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "2".to_owned(),
        "--id".to_owned(),
        "101".to_owned(),
        "--direction".to_owned(),
        "referrers".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(paths["object_id"], "101");

    let diff = json_stdout(run(&[
        "diff".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(diff["confidence"]["level"], "high");
    assert_eq!(diff["summary_delta"]["object_count"], 1);

    let diff_ids = assert_success(run(&[
        "diff-objects".to_owned(),
        arg(&db),
        "--from".to_owned(),
        "1".to_owned(),
        "--to".to_owned(),
        "2".to_owned(),
        "--state".to_owned(),
        "new".to_owned(),
        "--ids-only".to_owned(),
    ]));
    assert_eq!(diff_ids.trim(), "102");
}

#[test]
fn sql_idset_schema_report_doctor_and_subgraph_commands_work() {
    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);

    let sql = json_stdout(run(&[
        "sql".to_owned(),
        arg(&db),
        "--query".to_owned(),
        "select cast(object_id as text) as object_id from objects order by object_id".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(sql["rows"][0]["object_id"], "1");
    assert!(sql["elapsed_ms"].is_number());

    let explain = json_stdout(run(&[
        "sql".to_owned(),
        arg(&db),
        "--query".to_owned(),
        "select object_id from objects".to_owned(),
        "--explain".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(explain["explain"], true);

    let idset = json_stdout(run(&[
        "idset".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--left-query".to_owned(),
        "select object_id from objects where object_id in (1,2,3)".to_owned(),
        "--right-query".to_owned(),
        "select to_id as object_id from edges where snapshot_id = 1 and from_id = 1".to_owned(),
        "--op".to_owned(),
        "intersect".to_owned(),
        "--details".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(idset["rows"].as_array().unwrap().len(), 2);

    let schema = json_stdout(run(&[
        "schema".to_owned(),
        arg(&db),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert!(schema["tables"].as_array().unwrap().len() >= 5);

    let dot = assert_success(run(&[
        "export-subgraph".to_owned(),
        arg(&db),
        "--id".to_owned(),
        "1".to_owned(),
        "--graph-format".to_owned(),
        "dot".to_owned(),
    ]));
    assert!(dot.contains("digraph pygco"));

    let report = assert_success(run(&[
        "report".to_owned(),
        arg(&db),
        "--format".to_owned(),
        "markdown".to_owned(),
    ]));
    assert!(report.contains("# Memory Forensics Report"));
    assert!(report.contains("## Algorithm Parameters"));
    assert!(report.contains("/?page=objects"));

    let doctor = json_stdout(run(&[
        "doctor".to_owned(),
        arg(&db),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(doctor["snapshot_count"], 1);
    assert_eq!(doctor["indexes_ok"], true);

    let version = assert_success(run(&["version".to_owned()]));
    assert!(!version.trim().is_empty());
}

#[test]
fn findings_and_suspects_expose_diagnostic_leads_without_sql() {
    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);

    let findings = json_stdout(run(&[
        "findings".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    let finding_rows = findings["rows"].as_array().unwrap();
    assert!(finding_rows.iter().any(|row| row["kind"] == "large_type"));
    assert!(finding_rows
        .iter()
        .any(|row| row["evidence"]["subject"]["type"] == "cachetools.LRUCache"));

    let findings_table = assert_success(run(&[
        "findings".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--kind".to_owned(),
        "large-type".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    assert!(findings_table.contains("Large type candidate"));

    let orphan_suspects = json_stdout(run(&[
        "suspects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--kind".to_owned(),
        "orphan-retained".to_owned(),
        "--min-reachable".to_owned(),
        "100".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(orphan_suspects["rows"][0]["kind"], "orphan_retained");
    assert_eq!(orphan_suspects["rows"][0]["subject"]["object_id"], "1");
    assert!(orphan_suspects["rows"][0]["next_command"]
        .as_str()
        .unwrap()
        .contains("pygco object"));
    assert!(!orphan_suspects["rows"][0]["next_command"]
        .as_str()
        .unwrap()
        .contains(" DB "));

    let high_root_table = assert_success(run(&[
        "suspects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--kind".to_owned(),
        "high-retained-root".to_owned(),
        "--min-reachable".to_owned(),
        "100b".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    assert!(high_root_table.contains("high_retained_root"));
    assert!(high_root_table.contains("pygco object"));
}

#[test]
fn open_and_web_help_document_dev_proxy_flags() {
    let open_help = assert_success(run(&["open".to_owned(), "--help".to_owned()]));
    assert!(open_help.contains("--dev"));
    assert!(open_help.contains("--dev-server-url"));

    let web_help = assert_success(run(&["web".to_owned(), "--help".to_owned()]));
    assert!(web_help.contains("--dev"));
    assert!(web_help.contains("--dev-server-url"));
}

#[test]
fn cli_errors_use_documented_exit_codes() {
    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);

    assert_failure(
        run(&[
            "edges".to_owned(),
            arg(&db),
            "--from".to_owned(),
            "1".to_owned(),
            "--to".to_owned(),
            "2".to_owned(),
        ]),
        2,
        "argument_error",
    );

    assert_failure(
        run(&[
            "sql".to_owned(),
            arg(&db),
            "--query".to_owned(),
            "delete from objects".to_owned(),
        ]),
        20,
        "query_failed",
    );
    let verbose_sql_error = assert_failure(
        run(&[
            "--no-color".to_owned(),
            "--verbose".to_owned(),
            "sql".to_owned(),
            arg(&db),
            "--query".to_owned(),
            "delete from objects".to_owned(),
        ]),
        20,
        "query_failed",
    );
    assert!(verbose_sql_error.contains("exit_code=20"));
    assert!(verbose_sql_error.contains("details:"));
    assert!(!verbose_sql_error.contains('\u{1b}'));

    let existing_dir = tempdir().unwrap();
    let existing = existing_dir.path().join("existing.sqlite");
    File::create(&existing).unwrap();
    assert_failure(
        run(&[
            "import".to_owned(),
            arg(fixture("tiny-v1.jsonl.gz")),
            "-o".to_owned(),
            arg(&existing),
        ]),
        11,
        "import_failed",
    );

    let malformed_dir = tempdir().unwrap();
    let malformed = malformed_dir.path().join("malformed.jsonl.gz");
    write_malformed_dump(&malformed);
    assert_failure(
        run(&[
            "import".to_owned(),
            arg(&malformed),
            "-o".to_owned(),
            arg(malformed_dir.path().join("out.sqlite")),
        ]),
        10,
        "dump_format_error",
    );

    let no_color_version = assert_success(run(&["--no-color".to_owned(), "version".to_owned()]));
    assert!(!no_color_version.trim().is_empty());
}

fn write_malformed_dump(path: &Path) {
    let file = File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::fast());
    writeln!(
        encoder,
        r#"{{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":1,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"bad","dump_sequence":1,"created_at":"2026-07-01T00:00:00Z","pid":1,"python_version":"3.12","platform":"test","collect_before_dump":false,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"object_count":1}}"#
    )
    .unwrap();
    writeln!(encoder, "{{").unwrap();
    encoder.finish().unwrap();
}
