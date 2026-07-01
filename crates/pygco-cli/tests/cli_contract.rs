use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    time::Duration,
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

fn run_with_env(args: &[String], envs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(pygco());
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().unwrap()
}

fn run_open_until_database(args: &[String], envs: &[(&str, &Path)]) -> (Vec<String>, PathBuf) {
    let mut command = Command::new(pygco());
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });

    let mut lines = Vec::new();
    for _ in 0..100 {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if let Some(path) = line.strip_prefix("database: ") {
                    let database = PathBuf::from(path);
                    lines.push(line);
                    let _ = child.kill();
                    let _ = child.wait();
                    return (lines, database);
                }
                lines.push(line);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(status) = child.try_wait().unwrap() {
                    panic!(
                        "pygco open exited before printing database line: {status:?}\nstdout:\n{}",
                        lines.join("\n")
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait().unwrap();
                panic!(
                    "pygco open closed stdout before printing database line: {status:?}\nstdout:\n{}",
                    lines.join("\n")
                );
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!(
        "timed out waiting for pygco open database line\nstdout:\n{}",
        lines.join("\n")
    );
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

#[test]
fn open_creates_default_cache_session_with_manifest() {
    let cache = tempdir().unwrap();
    let (_lines, database) = run_open_until_database(
        &[
            "open".to_owned(),
            arg(fixture("tiny-v1.jsonl.gz")),
            "--no-browser".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    );

    let sessions = cache.path().join("sessions");
    assert!(
        database.starts_with(&sessions),
        "database should be under cache sessions dir, got {}",
        database.display()
    );
    assert_eq!(database.file_name().unwrap(), "analysis.sqlite");
    assert!(database.is_file());

    let session_dir = database.parent().unwrap();
    let session_id = session_dir.file_name().unwrap().to_string_lossy();
    assert!(session_id.contains('T'));
    assert!(session_dir.join("import.log").is_file());

    let manifest_path = session_dir.join("manifest.json");
    assert!(manifest_path.is_file());
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["session_id"], session_id.as_ref());
    assert_eq!(manifest["cache_root"], cache.path().display().to_string());
    assert_eq!(manifest["database_path"], database.display().to_string());
    assert_eq!(manifest["snapshots"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["snapshots"][0]["object_count"], 4);
}

#[test]
fn sessions_list_reports_cached_open_sessions() {
    let cache = tempdir().unwrap();
    let (_lines, database) = run_open_until_database(
        &[
            "open".to_owned(),
            arg(fixture("tiny-v1.jsonl.gz")),
            "--no-browser".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    );

    let listing = json_stdout(run_with_env(
        &[
            "sessions".to_owned(),
            "list".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    ));
    assert_eq!(listing["cache_root"], cache.path().display().to_string());
    let sessions = listing["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    let session = &sessions[0];
    assert_eq!(
        session["id"],
        database
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(session["status"], "ready");
    assert_eq!(session["database_path"], database.display().to_string());
    assert_eq!(session["snapshot_count"], 1);
    assert!(session["size_bytes"].as_u64().unwrap() > 0);
    assert!(session["source_dumps"][0]
        .as_str()
        .unwrap()
        .contains("tiny-v1.jsonl.gz"));
}

#[test]
fn open_respects_explicit_session_dir() {
    let base = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let session_dir = base.path().join("manual-session");
    let (_lines, database) = run_open_until_database(
        &[
            "open".to_owned(),
            arg(fixture("tiny-v1.jsonl.gz")),
            "--session-dir".to_owned(),
            arg(&session_dir),
            "--no-browser".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    );

    assert_eq!(database, session_dir.join("analysis.sqlite"));
    assert!(session_dir.join("import.log").is_file());
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(session_dir.join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["session_id"], "manual-session");
    assert!(manifest["cache_root"].is_null());
}

#[test]
fn sessions_list_tolerates_empty_and_damaged_cache_sessions() {
    let cache = tempdir().unwrap();
    let empty = json_stdout(run_with_env(
        &[
            "sessions".to_owned(),
            "list".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    ));
    assert_eq!(empty["sessions"].as_array().unwrap().len(), 0);

    let broken = cache.path().join("sessions").join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("manifest.json"), "{not json").unwrap();
    let listing = json_stdout(run_with_env(
        &[
            "sessions".to_owned(),
            "list".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    ));
    let sessions = listing["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "broken");
    assert_eq!(sessions[0]["status"], "invalid-manifest");
}

#[test]
fn sessions_list_rejects_relative_cache_root_env() {
    let output = run_with_env(
        &["sessions".to_owned(), "list".to_owned()],
        &[("PYGCO_HOME", Path::new("relative-cache"))],
    );
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("PYGCO_HOME must be an absolute path"),
        "stderr should explain the invalid cache root:\n{stderr}"
    );
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
