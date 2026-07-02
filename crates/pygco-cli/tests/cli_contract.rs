use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    time::Duration,
};

use flate2::{write::GzEncoder, Compression};
use rusqlite::Connection;
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

fn legacy_db_without_object_list_metrics() -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("legacy.sqlite");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE snapshots (
          snapshot_id INTEGER PRIMARY KEY,
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
        CREATE TABLE objects (
          snapshot_id INTEGER NOT NULL,
          object_id INTEGER NOT NULL,
          type TEXT NOT NULL,
          module TEXT NOT NULL,
          qualname TEXT NOT NULL,
          shallow_size INTEGER,
          gc_tracked INTEGER,
          stub INTEGER NOT NULL DEFAULT 0,
          repr TEXT,
          PRIMARY KEY (snapshot_id, object_id)
        );
        CREATE TABLE edges (
          snapshot_id INTEGER NOT NULL,
          from_id INTEGER NOT NULL,
          edge_index INTEGER NOT NULL,
          to_id INTEGER NOT NULL,
          PRIMARY KEY (snapshot_id, from_id, edge_index)
        );
        CREATE TABLE type_stats (
          snapshot_id INTEGER NOT NULL,
          type TEXT NOT NULL,
          module TEXT NOT NULL,
          count INTEGER NOT NULL,
          shallow_size_sum INTEGER NOT NULL,
          in_edges INTEGER NOT NULL,
          out_edges INTEGER NOT NULL,
          stub_count INTEGER NOT NULL,
          PRIMARY KEY (snapshot_id, type)
        );
        CREATE TABLE module_stats (
          snapshot_id INTEGER NOT NULL,
          module TEXT NOT NULL,
          count INTEGER NOT NULL,
          shallow_size_sum INTEGER NOT NULL,
          in_edges INTEGER NOT NULL,
          out_edges INTEGER NOT NULL,
          PRIMARY KEY (snapshot_id, module)
        );
        CREATE TABLE cohort_stats (
          snapshot_id INTEGER NOT NULL,
          cohort TEXT NOT NULL,
          count INTEGER NOT NULL,
          shallow_size_sum INTEGER NOT NULL,
          type_count INTEGER NOT NULL,
          details_json TEXT NOT NULL,
          rules_version TEXT NOT NULL,
          PRIMARY KEY (snapshot_id, cohort)
        );
        CREATE TABLE object_reachability (
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
          PRIMARY KEY (snapshot_id, object_id, algorithm_version, direction, depth, node_limit, fanout_limit)
        );
        CREATE TABLE type_reachability_stats (
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
        CREATE TABLE findings (
          finding_id INTEGER PRIMARY KEY AUTOINCREMENT,
          snapshot_id INTEGER NOT NULL,
          kind TEXT NOT NULL,
          severity TEXT NOT NULL,
          title TEXT NOT NULL,
          message TEXT NOT NULL,
          action TEXT NOT NULL,
          evidence_json TEXT NOT NULL,
          algorithm_version INTEGER NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE import_warnings (
          warning_id INTEGER PRIMARY KEY AUTOINCREMENT,
          snapshot_id INTEGER,
          level TEXT NOT NULL,
          code TEXT NOT NULL,
          message TEXT NOT NULL,
          context_json TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        INSERT INTO snapshots(
          snapshot_id, source_uri, source_basename, dump_sha256, dump_format,
          dump_format_version, producer, producer_version, imported_at,
          import_options_json, object_count, edge_count, shallow_size_sum
        ) VALUES (
          1, 'legacy.jsonl.gz', 'legacy.jsonl.gz', 'abc', 'pygco-dump-jsonl',
          1, 'pygco_dump', '0.1.0', '2026-07-02T00:00:00Z',
          '{}', 1, 0, 64
        );
        INSERT INTO objects(snapshot_id, object_id, type, module, qualname, shallow_size, gc_tracked, stub, repr)
        VALUES (1, 1, 'dict', 'builtins', 'dict', 64, 1, 0, NULL);
        INSERT INTO type_stats(snapshot_id, type, module, count, shallow_size_sum, in_edges, out_edges, stub_count)
        VALUES (1, 'dict', 'builtins', 1, 64, 0, 0, 0);
        INSERT INTO module_stats(snapshot_id, module, count, shallow_size_sum, in_edges, out_edges)
        VALUES (1, 'builtins', 1, 64, 0, 0);
        INSERT INTO findings(snapshot_id, kind, severity, title, message, action, evidence_json, algorithm_version, created_at)
        VALUES (
          1, 'large_type', 'info', 'Legacy finding', 'Existing finding row',
          'Inspect objects', '{"schema_version":1,"kind":"large_type","subject":{"type":"dict"},"metrics":{},"links":[]}',
          1, '2026-07-02T00:00:00Z'
        );
        "#,
    )
    .unwrap();
    (dir, db)
}

fn import_single_dump(path: &Path) -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let import = json_stdout(run(&[
        "import".to_owned(),
        arg(path),
        "-o".to_owned(),
        arg(&db),
        "--rebuild".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(import["snapshots"].as_array().unwrap().len(), 1);
    (dir, db)
}

fn write_large_orphan_dump(path: &Path) {
    let file = File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::fast());
    writeln!(
        encoder,
        r#"{{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":1,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"large-orphan","dump_sequence":1,"created_at":"2026-07-02T00:00:00Z","process_started_at":"2026-07-02T00:00:00Z","host_id":"fixture-host","container_id":null,"pid":4242,"python_version":"3.12.0","platform":"fixture","collect_before_dump":false,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"gc_count":[0,0,0],"gc_stats":null,"object_count":1}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"object","id":700,"type":"app.Buffer","module":"app","qualname":"Buffer","size":2097152,"gc_tracked":true,"stub":false,"referents":[]}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"metadata","phase":"end","dumped_count":1,"stub_count":0,"total_object_records":1,"elapsed_ms":1}}"#
    )
    .unwrap();
    encoder.finish().unwrap();
}

fn write_container_dump(path: &Path) {
    let file = File::create(path).unwrap();
    let mut encoder = GzEncoder::new(file, Compression::fast());
    writeln!(
        encoder,
        r#"{{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":1,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"container-fixture","dump_sequence":1,"created_at":"2026-07-02T00:00:00Z","process_started_at":"2026-07-02T00:00:00Z","host_id":"fixture-host","container_id":null,"pid":4242,"python_version":"3.12.0","platform":"fixture","collect_before_dump":true,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"gc_count":[0,0,0],"gc_stats":null,"object_count":4}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"object","id":900,"type":"collections.deque","module":"collections","qualname":"deque","size":760,"gc_tracked":true,"stub":false,"referents":[901,902,903]}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"object","id":901,"type":"str","module":"builtins","qualname":"str","size":5500,"gc_tracked":false,"stub":false,"referents":[]}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"object","id":902,"type":"str","module":"builtins","qualname":"str","size":5600,"gc_tracked":false,"stub":false,"referents":[]}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"object","id":903,"type":"bytes","module":"builtins","qualname":"bytes","size":2048,"gc_tracked":false,"stub":false,"referents":[]}}"#
    )
    .unwrap();
    writeln!(
        encoder,
        r#"{{"record_type":"metadata","phase":"end","dumped_count":4,"stub_count":0,"total_object_records":4,"elapsed_ms":1}}"#
    )
    .unwrap();
    encoder.finish().unwrap();
}

fn serve_dump_once(bytes: Vec<u8>, filename: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/gzip\r\nContent-Disposition: attachment; filename=\"{filename}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(&bytes).unwrap();
    });
    format!("http://{addr}/dump?token=SECRET")
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
    assert!(
        high_root_table
            .lines()
            .next()
            .unwrap_or_default()
            .contains("kind"),
        "table output should have a header row:\n{high_root_table}"
    );
    assert!(
        !high_root_table.contains("kind="),
        "table output should be aligned columns, not key=value logs:\n{high_root_table}"
    );
}

#[test]
fn p0_leak_workflow_surfaces_quality_suspects_and_annotations() {
    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);

    let summary = json_stdout(run(&[
        "summary".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    let quality_warnings = summary["quality"]["warnings"].as_array().unwrap();
    assert!(quality_warnings
        .iter()
        .any(|warning| warning["code"] == "collect_before_dump_false"));
    assert!(quality_warnings
        .iter()
        .any(|warning| warning["code"] == "repr_unavailable"));
    assert!(quality_warnings
        .iter()
        .any(|warning| warning["code"] == "edge_labels_unavailable"));

    let report = json_stdout(run(&[
        "report".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(report["quality"]["status"], "warning");
    assert!(report["suspects"]["rows"].is_array());
    assert_eq!(
        report["quality"]["warnings"],
        report["summary"]["quality"]["warnings"]
    );

    let report_markdown = assert_success(run(&[
        "report".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "markdown".to_owned(),
    ]));
    assert!(report_markdown.contains("## Quality"));
    assert!(report_markdown.contains("## Top Suspects"));
    assert!(report_markdown.contains("collect_before_dump_false"));

    let object = json_stdout(run(&[
        "object".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--id".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(object["object"]["self_edges"], 0);
    assert_eq!(object["object"]["external_in_edges"], 0);
    assert_eq!(object["object"]["is_orphan_retained_candidate"], false);
    assert!(object["object"]["orphan_retained_reason"]
        .as_str()
        .unwrap()
        .contains("external incoming edge"));

    let annotated_paths = json_stdout(run(&[
        "paths".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--id".to_owned(),
        "1".to_owned(),
        "--direction".to_owned(),
        "referents".to_owned(),
        "--annotate".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(annotated_paths["annotated"], true);
    let first_path = &annotated_paths["paths"].as_array().unwrap()[0];
    assert!(first_path["nodes"].is_array());
    assert_eq!(first_path["nodes"][0]["object_id"], "1");
    assert_eq!(first_path["nodes"][0]["external_in_edges"], 0);
    assert!(first_path["interpretation"].is_array());

    let annotated_table = assert_success(run(&[
        "paths".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--id".to_owned(),
        "1".to_owned(),
        "--direction".to_owned(),
        "referents".to_owned(),
        "--annotate".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    assert!(annotated_table
        .lines()
        .next()
        .unwrap_or_default()
        .contains("path_index"));
    assert!(annotated_table.contains("external_in_edges"));
    assert!(annotated_table.contains("Root node has no external incoming edge."));
}

#[test]
fn p0_report_and_tables_have_stable_degraded_contracts() {
    let fixture_dir = tempdir().unwrap();
    let large_dump = fixture_dir.path().join("large-orphan.jsonl.gz");
    write_large_orphan_dump(&large_dump);
    let (_large_dir, large_db) = import_single_dump(&large_dump);

    let suspects = json_stdout(run(&[
        "suspects".to_owned(),
        arg(&large_db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    let report = json_stdout(run(&[
        "report".to_owned(),
        arg(&large_db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(
        report["suspects"]["rows"][0]["kind"], suspects["rows"][0]["kind"],
        "report top suspect should reuse the same ordered suspect facts as pygco suspects"
    );
    assert_eq!(
        report["suspects"]["rows"][0]["subject"], suspects["rows"][0]["subject"],
        "report top suspect subject should match pygco suspects"
    );

    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);
    let suspects_table = assert_success(run(&[
        "suspects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--kind".to_owned(),
        "orphan-retained".to_owned(),
        "--min-reachable".to_owned(),
        "100b".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    let header = suspects_table.lines().next().unwrap_or_default();
    assert!(
        header.starts_with("rank  kind"),
        "suspects table should use diagnostic default columns, got:\n{suspects_table}"
    );
    assert!(
        !header.split_whitespace().any(|column| matches!(
            column,
            "metrics" | "limitations" | "subject"
        )),
        "suspects table should flatten human fields instead of dumping nested JSON columns:\n{suspects_table}"
    );

    let projected_table = assert_success(run(&[
        "suspects".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--kind".to_owned(),
        "orphan-retained".to_owned(),
        "--min-reachable".to_owned(),
        "100b".to_owned(),
        "--fields".to_owned(),
        "kind,subject.object_id,metrics.estimated_reachable_size".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    let projected_header = projected_table.lines().next().unwrap_or_default();
    let projected_columns: Vec<&str> = projected_header.split_whitespace().collect();
    assert_eq!(
        projected_columns,
        vec![
            "kind",
            "subject.object_id",
            "metrics.estimated_reachable_size"
        ],
        "--fields should preserve nested projection order for table output:\n{projected_table}"
    );
    assert!(projected_table.contains("orphan_retained"));

    let (_legacy_dir, legacy_db) = legacy_db_without_object_list_metrics();
    let legacy_report = json_stdout(run(&[
        "report".to_owned(),
        arg(&legacy_db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert!(legacy_report["suspects"]["rows"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(legacy_report["suspects"]["status"], "unavailable");
    assert!(legacy_report["suspects"]["limitations"][0]
        .as_str()
        .unwrap()
        .contains("object_list_metrics"));
}

#[test]
fn p1_overview_is_a_compact_triage_entrypoint() {
    let (_dir, db) = import_db(&["tiny-v1.jsonl.gz"]);

    let overview = json_stdout(run(&[
        "overview".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(overview["snapshot"]["snapshot_id"], 1);
    assert_eq!(overview["quality"]["status"], "warning");
    assert_eq!(overview["heavy_suspects"]["status"], "omitted");
    assert!(overview["heavy_suspects"]["next_command"]
        .as_str()
        .unwrap()
        .contains("pygco suspects"));
    assert!(overview["sections"]["top_non_builtin_types"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["module"] == "app"));
    assert!(overview["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["section"] == "quality"));
    assert!(overview["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("Heavy suspect queries")));

    let overview_table = assert_success(run(&[
        "overview".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    let header: Vec<&str> = overview_table
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    assert_eq!(
        header,
        vec![
            "section",
            "rank",
            "kind",
            "subject",
            "count",
            "shallow_size",
            "estimated_reachable_size",
            "status",
            "next_command"
        ]
    );
    assert!(overview_table.contains("quality"));
    assert!(overview_table.contains("top_non_builtin_type"));
    assert!(overview_table.contains("pygco suspects"));
    assert!(
        !overview_table.contains("section="),
        "overview table should use aligned columns:\n{overview_table}"
    );
}

#[test]
fn p1_container_explains_common_container_contents() {
    let fixture_dir = tempdir().unwrap();
    let dump = fixture_dir.path().join("container.jsonl.gz");
    write_container_dump(&dump);
    let (_dir, db) = import_single_dump(&dump);

    let container = json_stdout(run(&[
        "container".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--id".to_owned(),
        "900".to_owned(),
        "--top-items".to_owned(),
        "--item-types".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]));
    assert_eq!(container["container"]["object_id"], "900");
    assert_eq!(container["container_kind"], "deque");
    assert_eq!(container["direct_referent_count"], 3);
    assert_eq!(container["item_types"]["rows"][0]["type"], "str");
    assert_eq!(container["item_types"]["rows"][0]["count"], 2);
    assert_eq!(
        container["item_types"]["rows"][0]["shallow_size_sum"],
        11100
    );
    assert_eq!(container["top_items"]["rows"][0]["object_id"], "902");
    assert!(container["limitations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str().unwrap().contains("edge labels")));

    let table = assert_success(run(&[
        "container".to_owned(),
        arg(&db),
        "--snapshot".to_owned(),
        "1".to_owned(),
        "--id".to_owned(),
        "900".to_owned(),
        "--top-items".to_owned(),
        "--item-types".to_owned(),
        "--format".to_owned(),
        "table".to_owned(),
    ]));
    let header: Vec<&str> = table
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    assert_eq!(
        header,
        vec![
            "section",
            "rank",
            "object_id",
            "type",
            "module",
            "count",
            "shallow_size",
            "shallow_size_sum"
        ]
    );
    assert!(table.contains("item_type"));
    assert!(table.contains("top_item"));
    assert!(table.contains("str"));
    assert!(
        !table.contains("type="),
        "container table should use aligned columns:\n{table}"
    );
}

#[test]
fn p2_fetch_and_open_url_record_redacted_source_manifest() {
    let fixture_dir = tempdir().unwrap();
    let dump = fixture_dir.path().join("url-dump.jsonl.gz");
    write_large_orphan_dump(&dump);
    let bytes = fs::read(&dump).unwrap();

    let fetch_url = serve_dump_once(bytes.clone(), "heap.jsonl.gz");
    let fetched = fixture_dir.path().join("fetched.jsonl.gz");
    let fetch = run(&[
        "fetch".to_owned(),
        fetch_url,
        "-o".to_owned(),
        arg(&fetched),
        "--header".to_owned(),
        "Authorization=Bearer SECRET".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    assert!(
        !text(&fetch.stdout).contains("SECRET") && !text(&fetch.stderr).contains("SECRET"),
        "fetch output must redact URL query and secret headers\nstdout:\n{}\nstderr:\n{}",
        text(&fetch.stdout),
        text(&fetch.stderr)
    );
    let fetch_json: Value = serde_json::from_str(&assert_success(fetch)).unwrap();
    assert_eq!(fetch_json["local_path"], fetched.display().to_string());
    assert_eq!(fetch_json["bytes"], bytes.len() as i64);
    assert_eq!(
        fetch_json["source"]["original_url"],
        "http://127.0.0.1:<redacted>/dump?<redacted>"
    );
    assert!(fetch_json["sha256"].as_str().unwrap().len() >= 64);
    assert!(fetched.is_file());

    let cache = tempdir().unwrap();
    let open_url = serve_dump_once(bytes, "heap.jsonl.gz");
    let (_lines, database) = run_open_until_database(
        &[
            "open".to_owned(),
            open_url,
            "--header".to_owned(),
            "Authorization=Bearer SECRET".to_owned(),
            "--no-browser".to_owned(),
        ],
        &[("PYGCO_HOME", cache.path())],
    );
    assert!(database.is_file());
    let manifest_path = database.parent().unwrap().join("manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path).unwrap();
    assert!(
        !manifest_raw.contains("SECRET"),
        "manifest must not leak URL query or secret headers:\n{manifest_raw}"
    );
    let manifest: Value = serde_json::from_str(&manifest_raw).unwrap();
    assert_eq!(
        manifest["fetched_sources"][0]["source"]["original_url"],
        "http://127.0.0.1:<redacted>/dump?<redacted>"
    );
    assert!(manifest["fetched_sources"][0]["local_path"]
        .as_str()
        .unwrap()
        .ends_with("heap.jsonl.gz"));
    assert!(manifest["source_dumps"][0]
        .as_str()
        .unwrap()
        .ends_with("heap.jsonl.gz"));
}

#[test]
fn p2_import_progress_and_profile_are_machine_safe() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let quiet = run(&[
        "import".to_owned(),
        arg(fixture("tiny-v1.jsonl.gz")),
        "-o".to_owned(),
        arg(&db),
        "--rebuild".to_owned(),
        "--progress".to_owned(),
        "never".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    assert_eq!(text(&quiet.stderr), "");
    let quiet_json: Value = serde_json::from_str(&assert_success(quiet)).unwrap();
    assert_eq!(quiet_json["snapshots"][0]["object_count"], 4);

    let profiled_db = dir.path().join("profiled.sqlite");
    let profiled = run(&[
        "import".to_owned(),
        arg(fixture("tiny-v1.jsonl.gz")),
        "-o".to_owned(),
        arg(&profiled_db),
        "--rebuild".to_owned(),
        "--profile".to_owned(),
        "--progress".to_owned(),
        "always".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    let stderr = text(&profiled.stderr);
    assert!(
        stderr.contains("pygco import: start") && stderr.contains("pygco import: finished"),
        "progress should be written to stderr, got:\n{stderr}"
    );
    let profiled_json: Value = serde_json::from_str(&assert_success(profiled)).unwrap();
    let profile = profiled_json["profile"].as_array().unwrap();
    assert!(profile
        .iter()
        .any(|event| event["phase"] == "build_indexes"));
    let first = &profile[0];
    assert!(first["elapsed_ms"].is_number());
    assert!(first["wall_time_ms"].is_number());
    assert!(first["self_time_ms"].is_number());
    assert!(first["nested"].is_boolean());
    assert!(first["phase_kind"].is_string());
    assert!(profile
        .iter()
        .any(|event| event["snapshot_id"] == 1 && event["nested"] == true));
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
fn cli_help_is_agent_friendly() {
    let root_help = assert_success(run(&["--help".to_owned()]));
    assert!(root_help.contains("Typical workflows"));
    assert!(root_help.contains("pygco open dump.jsonl.gz --no-browser"));
    assert!(root_help.contains("Use --format json for machine-readable output"));
    assert!(root_help.contains("sessions"));

    let import_help = assert_success(run(&["import".to_owned(), "--help".to_owned()]));
    assert!(import_help.contains("Examples:"));
    assert!(import_help.contains("pygco import before.jsonl.gz after.jsonl.gz"));
    assert!(import_help.contains("--no-reachability"));
    assert!(import_help.contains("Writes a fresh SQLite analysis database"));

    let objects_help = assert_success(run(&["objects".to_owned(), "--help".to_owned()]));
    assert!(objects_help.contains("Sort keys:"));
    assert!(objects_help.contains("reachable-size"));
    assert!(objects_help.contains("--fields object_id,type,shallow_size"));

    let sql_help = assert_success(run(&["sql".to_owned(), "--help".to_owned()]));
    assert!(sql_help.contains("Read-only SQL workbench"));
    assert!(sql_help.contains("--explain"));
    assert!(sql_help.contains("Only SELECT-style read queries are accepted"));

    let sessions_help = assert_success(run(&[
        "sessions".to_owned(),
        "list".to_owned(),
        "--help".to_owned(),
    ]));
    assert!(sessions_help.contains("Cache root order"));
    assert!(sessions_help.contains("status=ready"));
    assert!(sessions_help.contains("pygco sessions list --format table"));
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
