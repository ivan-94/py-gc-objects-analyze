use std::{fs, path::PathBuf};
use tokio::time::{sleep, Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use pygco_api::{app, app_with_static_dir};
use pygco_importer::{import_dumps, ImportOptions};
use rusqlite::params;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/golden")
        .join(name)
}

fn import_fixture(inputs: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    import_dumps(
        inputs.iter().map(|name| fixture(name)).collect(),
        db.clone(),
        ImportOptions::default(),
    )
    .unwrap();
    (dir, db)
}

fn synthetic_reachability_db(object_count: i64) -> (tempfile::TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("analysis.sqlite");
    let mut conn = rusqlite::Connection::open(&db).unwrap();
    pygco_store::create_schema(&conn).unwrap();
    conn.execute(
        "
        INSERT INTO snapshots(
          source_uri, source_basename, dump_sha256, dump_format, dump_format_version,
          producer, producer_version, imported_at, import_options_json, object_count,
          edge_count, stub_count, missing_referent_count, shallow_size_sum
        ) VALUES (?1, ?2, ?3, 'pygco-dump-jsonl', 1, 'test', '0.1.0', ?4, '{}', ?5, ?6, 0, 0, ?7)
        ",
        params![
            "synthetic.jsonl.gz",
            "synthetic.jsonl.gz",
            format!("synthetic-{object_count}"),
            pygco_store::now_rfc3339(),
            object_count,
            object_count.saturating_sub(1),
            object_count * 8
        ],
    )
    .unwrap();
    let tx = conn.transaction().unwrap();
    {
        let mut object_stmt = tx
            .prepare(
                "
                INSERT INTO objects(
                  snapshot_id, object_id, type, module, qualname, shallow_size, gc_tracked, stub
                ) VALUES (1, ?1, 'Node', 'synthetic', 'synthetic.Node', 8, 1, 0)
                ",
            )
            .unwrap();
        for object_id in 1..=object_count {
            object_stmt.execute([object_id]).unwrap();
        }
    }
    {
        let mut edge_stmt = tx
            .prepare(
                "
                INSERT INTO edges(snapshot_id, from_id, edge_index, to_id)
                VALUES (1, ?1, 0, ?2)
                ",
            )
            .unwrap();
        for object_id in 1..object_count {
            edge_stmt
                .execute(params![object_id, object_id + 1])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    (dir, db)
}

async fn request_json(db: PathBuf, request: Request<Body>) -> (StatusCode, Value) {
    let response = app(db).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    (status, body)
}

async fn request_json_router(router: axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    (status, body)
}

async fn request_text(
    db: PathBuf,
    static_dir: Option<PathBuf>,
    request: Request<Body>,
) -> (StatusCode, String) {
    let response = app_with_static_dir(db, static_dir)
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn serves_session_summary_objects_and_graph_contracts() {
    let (_dir, db) = import_fixture(&["tiny-v1.jsonl.gz"]);

    let (status, session) = request_json(db.clone(), get("/api/session")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session["data"]["schema_version"], 1);

    let (status, summary) = request_json(db.clone(), get("/api/summary?snapshot_id=1")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["data"]["snapshot"]["object_count"], 4);

    let (status, objects) = request_json(
        db.clone(),
        get("/api/objects?snapshot_id=1&type=&module=&stub=&min_shallow_size=&sort=object-id&order=asc&limit=2&offset=0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(objects["data"].as_array().unwrap().len(), 2);
    assert_eq!(objects["meta"]["total"], 4);
    assert_eq!(objects["data"][0]["object_id"], "1");

    let (status, detail) = request_json(db.clone(), get("/api/objects/1?snapshot_id=1")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["data"]["object"]["object_id"], "1");

    let (status, graph) = request_json(
        db,
        get("/api/graph?snapshot_id=1&root_object_id=1&direction=both&depth=2&node_limit=500&edge_limit=2000"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!graph["data"]["nodes"].as_array().unwrap().is_empty());
    assert_eq!(graph["data"]["limits"]["depth"], 2);
}

#[tokio::test]
async fn serves_openapi_static_assets_and_graph_limit_errors() {
    let (dir, db) = import_fixture(&["tiny-v1.jsonl.gz"]);

    let (status, openapi) = request_json(db.clone(), get("/api/openapi.json")).await;
    assert_eq!(status, StatusCode::OK);
    for path in [
        "/api/session",
        "/api/snapshots",
        "/api/summary",
        "/api/objects",
        "/api/objects/{object_id}",
        "/api/objects/{object_id}/edges",
        "/api/objects/{object_id}/paths",
        "/api/graph",
        "/api/types",
        "/api/modules",
        "/api/cohorts",
        "/api/diff",
        "/api/diff/objects",
        "/api/findings",
        "/api/sql/query",
        "/api/sql/explain",
        "/api/reachability/recompute",
        "/api/jobs/{job_id}",
        "/api/jobs/{job_id}/cancel",
        "/api/idset",
        "/api/saved-idsets",
        "/api/saved-idsets/{idset_id}",
        "/api/schema",
        "/api/report.md",
        "/api/report.json",
    ] {
        assert!(
            openapi["data"]["paths"].get(path).is_some(),
            "missing {path}"
        );
    }
    assert_eq!(
        openapi["data"]["paths"]["/api/objects"]["get"]["x-client"]["responseMode"],
        "envelope"
    );
    assert_eq!(
        openapi["data"]["paths"]["/api/reachability/recompute"]["post"]["x-client"]["bodySchema"],
        "ReachabilityRecomputeRequest"
    );
    assert!(
        openapi["data"]["components"]["schemas"]["GraphData"]["x-typescript"]
            .as_str()
            .unwrap()
            .contains("missing_edges")
    );

    let (status, invalid_graph) =
        request_json(db.clone(), get("/api/graph?root_object_id=1&depth=99")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_graph["error"]["code"], "invalid_filter");
    assert!(invalid_graph["error"]["message"]
        .as_str()
        .unwrap()
        .contains("depth"));

    let static_dir = dir.path().join("dist");
    fs::create_dir_all(static_dir.join("assets")).unwrap();
    fs::write(
        static_dir.join("index.html"),
        r#"<!doctype html><div id="root">pygco web</div><script type="module" src="/assets/app.js"></script>"#,
    )
    .unwrap();
    fs::write(static_dir.join("assets/app.js"), "console.log('pygco');").unwrap();

    let (status, index) = request_text(db.clone(), Some(static_dir.clone()), get("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(index.contains("pygco web"));

    let (status, spa_fallback) =
        request_text(db.clone(), Some(static_dir.clone()), get("/objects")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(spa_fallback.contains("pygco web"));

    let (status, asset) = request_text(db.clone(), Some(static_dir), get("/assets/app.js")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(asset.contains("pygco"));

    let (status, embedded_index) = request_text(db.clone(), None, get("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(embedded_index.contains("id=\"root\""));
    let embedded_asset_path = embedded_index
        .split("src=\"")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .expect("embedded index references a script asset");
    let (status, embedded_asset) = request_text(db, None, get(embedded_asset_path)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(embedded_asset.contains("createRoot") || embedded_asset.contains("pygco"));
}

#[tokio::test]
async fn supports_async_sql_jobs_and_cancellation() {
    let (_dir, db) = import_fixture(&["tiny-v1.jsonl.gz"]);
    let router = app(db);

    let (status, submitted) = request_json_router(
        router.clone(),
        post(
            "/api/sql/query",
            json!({"query": "select cast(object_id as text) as object_id from objects order by object_id", "limit": 10, "async": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let job_id = submitted["data"]["job_id"].as_str().unwrap();
    assert_eq!(submitted["data"]["status"], "queued");

    let completed = wait_for_job(router.clone(), job_id).await;
    assert_eq!(completed["data"]["status"], "succeeded");
    assert_eq!(completed["data"]["result"]["rows"][0]["object_id"], "1");

    let (status, long_job) = request_json_router(
        router.clone(),
        post(
            "/api/sql/query",
            json!({
                "query": "with recursive cnt(x) as (select 0 union all select x + 1 from cnt where x < 100000000) select sum(x) as total from cnt",
                "limit": 10,
                "async": true
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let long_job_id = long_job["data"]["job_id"].as_str().unwrap();

    let (status, cancel) = request_json_router(
        router.clone(),
        post(&format!("/api/jobs/{long_job_id}/cancel"), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(
        cancel["data"]["status"].as_str().unwrap(),
        "canceling" | "canceled"
    ));

    let canceled = wait_for_job(router, long_job_id).await;
    assert_eq!(canceled["data"]["status"], "canceled");
}

#[tokio::test]
async fn supports_reachability_recompute_jobs_and_cancellation() {
    let (_dir, db) = import_fixture(&["tiny-v1.jsonl.gz"]);
    let router = app(db);

    let (status, submitted) = request_json_router(
        router.clone(),
        post(
            "/api/reachability/recompute",
            json!({"snapshot_id": 1, "depth": 2, "node_limit": 100, "fanout_limit": 10}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(submitted["data"]["kind"], "reachability_recompute");
    let job_id = submitted["data"]["job_id"].as_str().unwrap();

    let completed = wait_for_job(router, job_id).await;
    assert_eq!(completed["data"]["status"], "succeeded");
    assert_eq!(completed["data"]["result"]["snapshot_id"], 1);
    assert_eq!(completed["data"]["result"]["params"]["depth"], 2);

    let (_busy_dir, busy_db) = synthetic_reachability_db(8_000);
    let busy_router = app(busy_db.clone());
    let (status, busy_job) = request_json_router(
        busy_router.clone(),
        post(
            "/api/reachability/recompute",
            json!({"snapshot_id": 1, "depth": 8000, "node_limit": 8000, "fanout_limit": 1}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let busy_job_id = busy_job["data"]["job_id"].as_str().unwrap();

    let (status, cancel) = request_json_router(
        busy_router.clone(),
        post(&format!("/api/jobs/{busy_job_id}/cancel"), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(
        cancel["data"]["status"].as_str().unwrap(),
        "canceling" | "canceled"
    ));

    let canceled = wait_for_job(busy_router, busy_job_id).await;
    assert_eq!(canceled["data"]["status"], "canceled");

    let conn = pygco_store::connect(&busy_db).unwrap();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM object_reachability", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
}

async fn wait_for_job(router: axum::Router, job_id: &str) -> Value {
    for _ in 0..50 {
        let (status, body) =
            request_json_router(router.clone(), get(&format!("/api/jobs/{job_id}"))).await;
        assert_eq!(status, StatusCode::OK);
        match body["data"]["status"].as_str().unwrap() {
            "queued" | "running" | "canceling" => sleep(Duration::from_millis(20)).await,
            _ => return body,
        }
    }
    panic!("job did not finish: {job_id}");
}

#[tokio::test]
async fn serves_diff_sql_idset_report_and_schema_contracts() {
    let (_dir, db) = import_fixture(&["diff-before-v1.jsonl.gz", "diff-after-v1.jsonl.gz"]);

    let (status, diff) = request_json(
        db.clone(),
        get("/api/diff?from_snapshot_id=1&to_snapshot_id=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(diff["data"]["confidence"]["level"], "high");
    assert_eq!(diff["data"]["summary_delta"]["object_count"], 1);

    let (status, diff_objects) = request_json(
        db.clone(),
        get("/api/diff/objects?from_snapshot_id=1&to_snapshot_id=2&state=new"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(diff_objects["data"]["rows"][0]["object_id"], "102");

    let (status, cohorts) = request_json(db.clone(), get("/api/cohorts?snapshot_id=2")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cohorts["data"][0]["cohort"], "database_cache");
    assert!(cohorts["data"][0]["estimated_reachable_size_sum"].is_number());

    let (status, types_by_count) =
        request_json(db.clone(), get("/api/types?snapshot_id=2&sort=count&limit=2")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        types_by_count["data"][0]["count"].as_i64().unwrap()
            >= types_by_count["data"][1]["count"].as_i64().unwrap()
    );

    let (status, modules_by_reachable) = request_json(
        db.clone(),
        get("/api/modules?snapshot_id=2&sort=reachable-size&limit=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        modules_by_reachable["data"][0]["estimated_reachable_size_sum"]
            .as_i64()
            .unwrap()
            >= modules_by_reachable["data"][1]["estimated_reachable_size_sum"]
                .as_i64()
                .unwrap()
    );

    let (status, cohorts_by_shallow) = request_json(
        db.clone(),
        get("/api/cohorts?snapshot_id=2&sort=shallow-size&limit=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cohorts_by_shallow["data"][0]["shallow_size_sum"].is_number());
    if cohorts_by_shallow["data"].as_array().unwrap().len() > 1 {
        assert!(
            cohorts_by_shallow["data"][0]["shallow_size_sum"]
                .as_i64()
                .unwrap()
                >= cohorts_by_shallow["data"][1]["shallow_size_sum"]
                    .as_i64()
                    .unwrap()
        );
    }

    let (status, findings) = request_json(
        db.clone(),
        get("/api/findings?snapshot_id=2&kind=large_type&severity=info&limit=1&offset=0"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(findings["data"]["limit"], 1);
    assert_eq!(findings["data"]["offset"], 0);
    assert!(findings["data"]["total"].as_i64().unwrap() >= 1);
    assert_eq!(findings["data"]["rows"][0]["kind"], "large_type");
    assert_eq!(findings["data"]["rows"][0]["severity"], "info");
    assert_eq!(findings["data"]["rows"][0]["evidence"]["schema_version"], 1);
    assert!(findings["data"]["rows"][0]["links"][0]["href"]
        .as_str()
        .unwrap()
        .contains("page=objects"));

    let (status, sql) = request_json(
        db.clone(),
        post(
            "/api/sql/query",
            json!({"query": "select cast(object_id as text) as object_id from objects where snapshot_id = 2 order by object_id", "limit": 10}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sql["data"]["rows"][0]["object_id"], "100");

    let (status, explain) = request_json(
        db.clone(),
        post(
            "/api/sql/explain",
            json!({"query": "select object_id from objects where snapshot_id = 2", "limit": 10}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(explain["data"]["explain"], true);

    let (status, idset) = request_json(
        db.clone(),
        post(
            "/api/idset",
            json!({
                "snapshot_id": 2,
                "left_query": "select object_id from objects where snapshot_id = 2",
                "right_query": "select to_id as object_id from edges where snapshot_id = 2 and from_id = 100",
                "op": "intersect",
                "details": true,
                "limit": 10
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(idset["data"]["rows"].as_array().unwrap().len(), 2);

    let (status, saved) = request_json(
        db.clone(),
        post(
            "/api/saved-idsets",
            json!({
                "snapshot_id": 2,
                "name": "redis referents",
                "object_ids": ["101", "102", "102"],
                "source": {"kind": "test", "query": "select object_id from objects"}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let idset_id = saved["data"]["idset"]["idset_id"].as_i64().unwrap();
    assert_eq!(saved["data"]["idset"]["object_count"], 2);
    assert_eq!(saved["data"]["rows"][0]["object_id"], "101");

    let (status, saved_list) =
        request_json(db.clone(), get("/api/saved-idsets?snapshot_id=2")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_list["data"]["rows"][0]["name"], "redis referents");

    let (status, saved_detail) =
        request_json(db.clone(), get(&format!("/api/saved-idsets/{idset_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_detail["data"]["idset"]["source"]["kind"], "test");

    let (status, schema) = request_json(db.clone(), get("/api/schema")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(schema["data"]["tables"].as_array().unwrap().len() >= 5);

    let (status, report) = request_json(db, get("/api/report.json?snapshot_id=2")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["data"]["summary"]["snapshot"]["snapshot_id"], 2);
    assert_eq!(
        report["data"]["finding_evidence_schema"]["properties"]["kind"]["enum"][1],
        "large_type"
    );
    assert!(report["data"]["findings"]["rows"][0]["links"].is_array());
}

#[tokio::test]
async fn returns_documented_error_envelopes() {
    let (_dir, db) = import_fixture(&["tiny-v1.jsonl.gz"]);

    let (status, invalid_filter) =
        request_json(db.clone(), get("/api/objects?min_shallow_size=wat")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_filter["error"]["code"], "invalid_filter");
    assert!(invalid_filter["error"]["message"]
        .as_str()
        .unwrap()
        .contains("min_shallow_size"));
    assert_eq!(
        invalid_filter["error"]["details"]["field"],
        "min_shallow_size"
    );
    assert_eq!(invalid_filter["error"]["details"]["expected"], "integer");
    assert!(invalid_filter["error"]["details"]["next_step"]
        .as_str()
        .unwrap()
        .contains("min_shallow_size"));

    let (status, invalid_finding) =
        request_json(db.clone(), get("/api/findings?kind=unknown")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_finding["error"]["code"], "invalid_filter");
    assert!(invalid_finding["error"]["message"]
        .as_str()
        .unwrap()
        .contains("invalid finding kind"));
    assert!(invalid_finding["error"]["details"]["next_step"]
        .as_str()
        .unwrap()
        .contains("Adjust"));

    let (status, invalid_sql) = request_json(
        db,
        post("/api/sql/query", json!({"query": "delete from objects"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_sql["error"]["code"], "query_failed");
    assert!(invalid_sql["error"]["details"]["next_step"]
        .as_str()
        .unwrap()
        .contains("SELECT or WITH"));
}
