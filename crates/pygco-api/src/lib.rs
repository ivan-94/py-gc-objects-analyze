#![recursion_limit = "512"]

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::{ServeDir, ServeFile};

use pygco_analysis::{DiffObjectsOptions, FindingsOptions, ObjectFilters, ReachabilityParams};

struct EmbeddedAsset {
    path: &'static str,
    content_type: &'static str,
    bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/embedded_web_assets.rs"));

#[derive(Clone)]
pub struct ApiState {
    pub database_path: PathBuf,
    jobs: Arc<JobRegistry>,
}

#[derive(Default)]
struct JobRegistry {
    next_id: AtomicU64,
    jobs: Mutex<BTreeMap<String, JobRecord>>,
}

#[derive(Clone)]
struct JobRecord {
    job_id: String,
    kind: String,
    status: JobStatus,
    progress: f64,
    message: Option<String>,
    result: Option<Value>,
    created_at: String,
    updated_at: String,
    interrupt: Option<Arc<rusqlite::InterruptHandle>>,
    cancel_requested: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceling,
    Canceled,
}

impl JobStatus {
    fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Canceling => "canceling",
            JobStatus::Canceled => "canceled",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Canceled
        )
    }
}

impl JobRegistry {
    fn create(&self, kind: &str) -> JobRecord {
        let number = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let now = pygco_store::now_rfc3339();
        let record = JobRecord {
            job_id: format!("job-{number}"),
            kind: kind.to_owned(),
            status: JobStatus::Queued,
            progress: 0.0,
            message: None,
            result: None,
            created_at: now.clone(),
            updated_at: now,
            interrupt: None,
            cancel_requested: Arc::new(AtomicBool::new(false)),
        };
        self.jobs
            .lock()
            .expect("job registry mutex poisoned")
            .insert(record.job_id.clone(), record.clone());
        record
    }

    fn get(&self, job_id: &str) -> Option<Value> {
        self.jobs
            .lock()
            .expect("job registry mutex poisoned")
            .get(job_id)
            .map(job_to_json)
    }

    fn start_running(
        &self,
        job_id: &str,
        interrupt: Option<Arc<rusqlite::InterruptHandle>>,
    ) -> bool {
        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        let Some(job) = jobs.get_mut(job_id) else {
            return false;
        };
        if matches!(job.status, JobStatus::Canceled | JobStatus::Canceling)
            || job.cancel_requested.load(Ordering::Relaxed)
        {
            if let Some(interrupt) = interrupt {
                interrupt.interrupt();
            }
            return false;
        }
        job.status = JobStatus::Running;
        job.progress = 0.1;
        job.updated_at = pygco_store::now_rfc3339();
        job.interrupt = interrupt;
        true
    }

    fn finish(&self, job_id: &str, result: std::result::Result<Value, String>) {
        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        let Some(job) = jobs.get_mut(job_id) else {
            return;
        };
        if matches!(job.status, JobStatus::Canceling | JobStatus::Canceled) {
            job.status = JobStatus::Canceled;
            job.progress = 1.0;
            job.message = Some("Job canceled.".to_owned());
            job.interrupt = None;
            job.updated_at = pygco_store::now_rfc3339();
            return;
        }
        match result {
            Ok(value) => {
                job.status = JobStatus::Succeeded;
                job.progress = 1.0;
                job.result = Some(value);
                job.message = None;
            }
            Err(message) => {
                job.status = JobStatus::Failed;
                job.progress = 1.0;
                job.message = Some(message);
            }
        }
        job.interrupt = None;
        job.updated_at = pygco_store::now_rfc3339();
    }

    fn cancel(&self, job_id: &str) -> Option<Value> {
        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        let job = jobs.get_mut(job_id)?;
        if job.status.is_terminal() {
            return Some(job_to_json(job));
        }
        job.cancel_requested.store(true, Ordering::Relaxed);
        if let Some(interrupt) = &job.interrupt {
            job.status = JobStatus::Canceling;
            job.message = Some("Cancellation requested.".to_owned());
            interrupt.interrupt();
        } else if matches!(job.status, JobStatus::Queued) {
            job.status = JobStatus::Canceled;
            job.progress = 1.0;
            job.message = Some("Job canceled before execution.".to_owned());
        } else {
            job.status = JobStatus::Canceling;
            job.message = Some("Cancellation requested.".to_owned());
        }
        job.updated_at = pygco_store::now_rfc3339();
        Some(job_to_json(job))
    }
}

fn job_to_json(job: &JobRecord) -> Value {
    let mut value = json!({
        "job_id": job.job_id,
        "kind": job.kind,
        "status": job.status.as_str(),
        "progress": job.progress,
        "message": job.message,
        "created_at": job.created_at,
        "updated_at": job.updated_at,
    });
    if let Some(result) = &job.result {
        value["result"] = result.clone();
    }
    value
}

fn start_sql_job(state: Arc<ApiState>, query: String, limit: i64, explain: bool) -> Value {
    let kind = if explain { "sql_explain" } else { "sql_query" };
    let job = state.jobs.create(kind);
    let job_id = job.job_id.clone();
    let database_path = state.database_path.clone();
    let jobs = state.jobs.clone();
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            let conn = pygco_store::connect(&database_path).map_err(|error| error.to_string())?;
            let interrupt = Arc::new(conn.get_interrupt_handle());
            if !jobs.start_running(&job_id, Some(interrupt)) {
                return Err("Job canceled.".to_owned());
            }
            pygco_analysis::readonly_sql(&conn, &query, limit, explain).map_err(|error| {
                if error.to_string().contains("interrupted") {
                    "Job canceled.".to_owned()
                } else {
                    error.to_string()
                }
            })
        })();
        jobs.finish(&job_id, result);
    });
    job_to_json(&job)
}

fn start_reachability_job(
    state: Arc<ApiState>,
    snapshot_id: Option<i64>,
    params: ReachabilityParams,
) -> Value {
    let job = state.jobs.create("reachability_recompute");
    let job_id = job.job_id.clone();
    let cancel_requested = job.cancel_requested.clone();
    let database_path = state.database_path.clone();
    let jobs = state.jobs.clone();
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            let conn = pygco_store::connect(&database_path).map_err(|error| error.to_string())?;
            if !jobs.start_running(&job_id, None) {
                return Err("Job canceled.".to_owned());
            }
            pygco_analysis::compute_reachability_with_cancel(&conn, snapshot_id, params, || {
                cancel_requested.load(Ordering::Relaxed)
            })
            .map_err(|error| error.to_string())
        })();
        jobs.finish(&job_id, result);
    });
    job_to_json(&job)
}

#[derive(Debug, Serialize)]
pub struct Envelope {
    data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<Value>,
}

impl Envelope {
    fn data(data: Value) -> Self {
        Self { data, meta: None }
    }

    fn with_meta(data: Value, meta: Value) -> Self {
        Self {
            data,
            meta: Some(meta),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    code: String,
    message: String,
    details: Value,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: String,
    message: String,
    details: Value,
}

impl ApiError {
    fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self::bad_request_with_details(
            code,
            message,
            json!({ "next_step": "Review the request parameters and try again." }),
        )
    }

    fn bad_request_with_details(code: &str, message: impl Into<String>, details: Value) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: code.to_owned(),
            message: message.into(),
            details,
        }
    }

    fn not_found(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: code.to_owned(),
            message: message.into(),
            details: json!({ "next_step": "Refresh the current view or choose an existing resource." }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                    details: self.details,
                },
            }),
        )
            .into_response()
    }
}

impl From<pygco_analysis::AnalysisError> for ApiError {
    fn from(error: pygco_analysis::AnalysisError) -> Self {
        match error {
            pygco_analysis::AnalysisError::InvalidQuery(_) => ApiError::bad_request_with_details(
                "invalid_filter",
                error.to_string(),
                json!({ "next_step": "Adjust the filter or query parameter and retry." }),
            ),
            pygco_analysis::AnalysisError::Store(pygco_store::StoreError::NotReadOnly) => {
                ApiError::bad_request_with_details(
                    "query_failed",
                    error.to_string(),
                    json!({
                        "next_step": "Use a SELECT or WITH query. Writes are intentionally blocked.",
                    }),
                )
            }
            _ => ApiError::bad_request_with_details(
                "query_failed",
                error.to_string(),
                json!({ "next_step": "Inspect the query, object id, or snapshot id and retry." }),
            ),
        }
    }
}

impl From<pygco_store::StoreError> for ApiError {
    fn from(error: pygco_store::StoreError) -> Self {
        ApiError::bad_request_with_details(
            "store_error",
            error.to_string(),
            json!({ "next_step": "Reopen or rebuild the analysis database, then retry." }),
        )
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        ApiError::bad_request_with_details(
            "sqlite_error",
            error.to_string(),
            json!({ "next_step": "Check the SQLite database and retry the operation." }),
        )
    }
}

pub fn app(database_path: PathBuf) -> Router {
    app_with_static_dir(database_path, default_static_dir())
}

pub fn app_with_static_dir(database_path: PathBuf, static_dir: Option<PathBuf>) -> Router {
    let state = Arc::new(ApiState {
        database_path,
        jobs: Arc::new(JobRegistry::default()),
    });
    let router = Router::new()
        .route("/api/session", get(session))
        .route("/api/snapshots", get(snapshots))
        .route("/api/summary", get(summary))
        .route("/api/objects", get(objects))
        .route("/api/objects/:object_id", get(object_detail))
        .route("/api/objects/:object_id/edges", get(object_edges))
        .route("/api/objects/:object_id/paths", get(object_paths))
        .route("/api/graph", get(graph))
        .route("/api/types", get(types))
        .route("/api/modules", get(modules))
        .route("/api/cohorts", get(cohorts))
        .route("/api/diff", get(diff))
        .route("/api/diff/objects", get(diff_objects))
        .route("/api/findings", get(findings))
        .route("/api/sql/query", post(sql_query))
        .route("/api/sql/explain", post(sql_explain))
        .route("/api/reachability/recompute", post(recompute_reachability))
        .route("/api/jobs/:job_id", get(job_status))
        .route("/api/jobs/:job_id/cancel", post(cancel_job))
        .route("/api/idset", post(idset))
        .route("/api/saved-idsets", get(saved_idsets).post(save_idset))
        .route("/api/saved-idsets/:idset_id", get(saved_idset_detail))
        .route("/api/schema", get(schema))
        .route("/api/report.md", get(report_md))
        .route("/api/report.json", get(report_json))
        .route("/api/openapi.json", get(openapi))
        .with_state(state);
    if let Some(static_dir) = static_dir {
        let index = static_dir.join("index.html");
        router.fallback_service(
            ServeDir::new(static_dir)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(index)),
        )
    } else {
        router.fallback(embedded_asset)
    }
}

fn default_static_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PYGCO_WEB_DIST") {
        let path = PathBuf::from(path);
        if path.join("index.html").is_file() {
            return Some(path);
        }
    }
    let source_tree_dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/app/dist");
    if source_tree_dist.join("index.html").is_file() {
        Some(source_tree_dist)
    } else {
        None
    }
}

pub async fn serve(database_path: PathBuf, host: String, port: u16) -> anyhow::Result<SocketAddr> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_addr = listener.local_addr()?;
    axum::serve(listener, app(database_path)).await?;
    Ok(actual_addr)
}

async fn embedded_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let asset = embedded_asset_for_path(path).or_else(|| embedded_asset_for_path("index.html"));
    match asset {
        Some(asset) => embedded_asset_response(asset),
        None => (
            StatusCode::NOT_FOUND,
            Html("<!doctype html><div id=\"root\">pygco embedded web assets not found</div>"),
        )
            .into_response(),
    }
}

fn embedded_asset_for_path(path: &str) -> Option<&'static EmbeddedAsset> {
    let clean = if path.is_empty() { "index.html" } else { path };
    if clean.contains("..") || clean.starts_with('/') {
        return None;
    }
    EMBEDDED_WEB_ASSETS.iter().find(|asset| asset.path == clean)
}

fn embedded_asset_response(asset: &'static EmbeddedAsset) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(asset.content_type),
    );
    (headers, asset.bytes).into_response()
}

fn conn(state: &ApiState) -> Result<rusqlite::Connection, ApiError> {
    pygco_store::connect(&state.database_path).map_err(Into::into)
}

async fn session(State(state): State<Arc<ApiState>>) -> Result<Json<Envelope>, ApiError> {
    let _conn = conn(&state)?;
    let schema_version = pygco_store::SCHEMA_VERSION;
    Ok(Json(Envelope::data(json!({
        "database_path": state.database_path,
        "schema_version": schema_version,
        "tool_version": pygco_store::TOOL_VERSION,
    }))))
}

async fn snapshots(State(state): State<Arc<ApiState>>) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::snapshots(&conn(
        &state,
    )?)?)))
}

#[derive(Debug, Deserialize)]
struct SummaryQuery {
    snapshot_id: Option<i64>,
    limit: Option<i64>,
}

async fn summary(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SummaryQuery>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::summary(
        &conn(&state)?,
        query.snapshot_id,
        query.limit.unwrap_or(20),
    )?)))
}

#[derive(Debug, Deserialize)]
struct ObjectsQuery {
    snapshot_id: Option<i64>,
    q: Option<String>,
    #[serde(rename = "type")]
    type_name: Option<String>,
    module: Option<String>,
    cohort: Option<String>,
    min_shallow_size: Option<String>,
    min_reachable_size: Option<String>,
    min_in_edges: Option<String>,
    min_out_edges: Option<String>,
    has_referrers: Option<bool>,
    missing_referents: Option<bool>,
    stub: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn objects(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ObjectsQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let filters = ObjectFilters {
        snapshot_id: query.snapshot_id,
        q: query.q,
        type_name: query.type_name,
        module: query.module,
        cohort: query.cohort,
        min_shallow_size: parse_optional_i64(query.min_shallow_size, "min_shallow_size")?,
        min_reachable_size: parse_optional_i64(query.min_reachable_size, "min_reachable_size")?,
        min_in_edges: parse_optional_i64(query.min_in_edges, "min_in_edges")?,
        min_out_edges: parse_optional_i64(query.min_out_edges, "min_out_edges")?,
        has_referrers: query.has_referrers.unwrap_or(false),
        missing_referents: query.missing_referents.unwrap_or(false),
        stub: parse_optional_bool(query.stub, "stub")?,
        sort: query.sort.unwrap_or_else(|| "reachable_size".to_owned()),
        order: query.order.unwrap_or_else(|| "desc".to_owned()),
        limit: query.limit.unwrap_or(100),
        offset: query.offset.unwrap_or(0),
    };
    let data = pygco_analysis::list_objects(&conn(&state)?, filters)?;
    let meta = json!({
        "limit": data["limit"],
        "offset": data["offset"],
        "total": data["total"],
        "truncated": false
    });
    Ok(Json(Envelope::with_meta(data["rows"].clone(), meta)))
}

async fn object_detail(
    State(state): State<Arc<ApiState>>,
    Path(object_id): Path<String>,
    Query(query): Query<SnapshotOnly>,
) -> Result<Json<Envelope>, ApiError> {
    let object_id = parse_object_id(&object_id)?;
    Ok(Json(Envelope::data(pygco_analysis::object_detail(
        &conn(&state)?,
        query.snapshot_id,
        object_id,
    )?)))
}

#[derive(Debug, Deserialize)]
struct EdgesQuery {
    snapshot_id: Option<i64>,
    direction: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn object_edges(
    State(state): State<Arc<ApiState>>,
    Path(object_id): Path<String>,
    Query(query): Query<EdgesQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let object_id = parse_object_id(&object_id)?;
    let data = pygco_analysis::object_edges(
        &conn(&state)?,
        query.snapshot_id,
        object_id,
        query.direction.as_deref().unwrap_or("referents"),
        query.limit.unwrap_or(100),
        query.offset.unwrap_or(0),
    )?;
    Ok(Json(Envelope::data(data)))
}

#[derive(Debug, Deserialize)]
struct PathsQuery {
    snapshot_id: Option<i64>,
    direction: Option<String>,
    depth: Option<i64>,
    fanout_limit: Option<i64>,
    limit: Option<i64>,
    include_core: Option<bool>,
}

async fn object_paths(
    State(state): State<Arc<ApiState>>,
    Path(object_id): Path<String>,
    Query(query): Query<PathsQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let _ = query.include_core;
    let object_id = parse_object_id(&object_id)?;
    let depth = parse_bounded_i64(query.depth, "depth", 5, 0, 10)?;
    let fanout_limit = parse_bounded_i64(query.fanout_limit, "fanout_limit", 30, 1, 5000)?;
    let limit = parse_bounded_i64(query.limit, "limit", 50, 1, 1000)?;
    Ok(Json(Envelope::data(pygco_analysis::paths(
        &conn(&state)?,
        query.snapshot_id,
        object_id,
        query.direction.as_deref().unwrap_or("referrers"),
        depth,
        fanout_limit,
        limit,
    )?)))
}

#[derive(Debug, Deserialize)]
struct GraphQuery {
    snapshot_id: Option<i64>,
    root_object_id: String,
    direction: Option<String>,
    depth: Option<i64>,
    node_limit: Option<i64>,
    edge_limit: Option<i64>,
}

async fn graph(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let root = parse_object_id(&query.root_object_id)?;
    let depth = parse_bounded_i64(query.depth, "depth", 2, 0, 10)?;
    let node_limit = parse_bounded_i64(query.node_limit, "node_limit", 500, 1, 5000)?;
    let edge_limit = parse_bounded_i64(query.edge_limit, "edge_limit", 2000, 1, 20000)?;
    Ok(Json(Envelope::data(pygco_analysis::subgraph(
        &conn(&state)?,
        query.snapshot_id,
        root,
        query.direction.as_deref().unwrap_or("both"),
        depth,
        node_limit,
        edge_limit,
    )?)))
}

#[derive(Debug, Deserialize)]
struct SnapshotOnly {
    snapshot_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AggregateQuery {
    snapshot_id: Option<i64>,
    limit: Option<i64>,
    sort: Option<String>,
}

async fn types(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let conn = conn(&state)?;
    let sid = pygco_store::resolve_snapshot_id(&conn, query.snapshot_id)?;
    Ok(Json(Envelope::data(pygco_analysis::top_types(
        &conn,
        sid,
        aggregate_sort(query.sort.as_deref()),
        query.limit.unwrap_or(100),
        false,
    )?)))
}

async fn modules(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let conn = conn(&state)?;
    let sid = pygco_store::resolve_snapshot_id(&conn, query.snapshot_id)?;
    Ok(Json(Envelope::data(pygco_analysis::top_modules(
        &conn,
        sid,
        aggregate_sort(query.sort.as_deref()),
        query.limit.unwrap_or(100),
    )?)))
}

async fn cohorts(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<AggregateQuery>,
) -> Result<Json<Envelope>, ApiError> {
    let conn = conn(&state)?;
    let sid = pygco_store::resolve_snapshot_id(&conn, query.snapshot_id)?;
    Ok(Json(Envelope::data(pygco_analysis::cohorts(
        &conn,
        sid,
        aggregate_sort(query.sort.as_deref()),
        query.limit.unwrap_or(100),
    )?)))
}

fn aggregate_sort(value: Option<&str>) -> &'static str {
    match value {
        Some("count") => "count",
        Some("reachable-size") | Some("reachable_size") | Some("estimated-reachable") => {
            "reachable_size"
        }
        _ => "shallow_size",
    }
}

#[derive(Debug, Deserialize)]
struct DiffQuery {
    from_snapshot_id: i64,
    to_snapshot_id: i64,
    limit: Option<i64>,
}

async fn diff(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<DiffQuery>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::diff(
        &conn(&state)?,
        query.from_snapshot_id,
        query.to_snapshot_id,
        query.limit.unwrap_or(100),
    )?)))
}

#[derive(Debug, Deserialize)]
struct DiffObjectsQuery {
    from_snapshot_id: i64,
    to_snapshot_id: i64,
    state: Option<String>,
    #[serde(rename = "type")]
    type_name: Option<String>,
    module: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn diff_objects(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<DiffObjectsQuery>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::diff_objects(
        &conn(&state)?,
        DiffObjectsOptions {
            from_snapshot_id: query.from_snapshot_id,
            to_snapshot_id: query.to_snapshot_id,
            state: query.state.unwrap_or_else(|| "new".to_owned()),
            type_name: query.type_name,
            module: query.module,
            limit: query.limit.unwrap_or(100),
            offset: query.offset.unwrap_or(0),
        },
    )?)))
}

async fn findings(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<FindingsQuery>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::findings(
        &conn(&state)?,
        FindingsOptions {
            snapshot_id: query.snapshot_id,
            kind: query.kind,
            severity: query.severity,
            limit: query.limit.unwrap_or(100),
            offset: query.offset.unwrap_or(0),
        },
    )?)))
}

#[derive(Debug, Deserialize)]
struct FindingsQuery {
    snapshot_id: Option<i64>,
    kind: Option<String>,
    severity: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SqlRequest {
    query: String,
    limit: Option<i64>,
    #[serde(default, rename = "async")]
    async_job: bool,
}

#[derive(Debug, Deserialize)]
struct ReachabilityRecomputeRequest {
    snapshot_id: Option<i64>,
    depth: Option<i64>,
    node_limit: Option<i64>,
    fanout_limit: Option<i64>,
}

async fn sql_query(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SqlRequest>,
) -> Result<Json<Envelope>, ApiError> {
    if request.async_job {
        let job = start_sql_job(
            state.clone(),
            request.query,
            request.limit.unwrap_or(1000),
            false,
        );
        Ok(Json(Envelope::data(job)))
    } else {
        Ok(Json(Envelope::data(pygco_analysis::readonly_sql(
            &conn(&state)?,
            &request.query,
            request.limit.unwrap_or(1000),
            false,
        )?)))
    }
}

async fn sql_explain(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SqlRequest>,
) -> Result<Json<Envelope>, ApiError> {
    if request.async_job {
        let job = start_sql_job(
            state.clone(),
            request.query,
            request.limit.unwrap_or(1000),
            true,
        );
        Ok(Json(Envelope::data(job)))
    } else {
        Ok(Json(Envelope::data(pygco_analysis::readonly_sql(
            &conn(&state)?,
            &request.query,
            request.limit.unwrap_or(1000),
            true,
        )?)))
    }
}

async fn recompute_reachability(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ReachabilityRecomputeRequest>,
) -> Result<Json<Envelope>, ApiError> {
    let params = reachability_params_from_request(&request)?;
    let job = start_reachability_job(state, request.snapshot_id, params);
    Ok(Json(Envelope::data(job)))
}

async fn job_status(
    State(state): State<Arc<ApiState>>,
    Path(job_id): Path<String>,
) -> Result<Json<Envelope>, ApiError> {
    state
        .jobs
        .get(&job_id)
        .map(Envelope::data)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("job_not_found", format!("job not found: {job_id}")))
}

async fn cancel_job(
    State(state): State<Arc<ApiState>>,
    Path(job_id): Path<String>,
) -> Result<Json<Envelope>, ApiError> {
    state
        .jobs
        .cancel(&job_id)
        .map(Envelope::data)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("job_not_found", format!("job not found: {job_id}")))
}

#[derive(Debug, Deserialize)]
struct IdsetRequest {
    snapshot_id: i64,
    left_query: String,
    right_query: String,
    op: String,
    details: Option<bool>,
    limit: Option<i64>,
}

async fn idset(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IdsetRequest>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::idset(
        &conn(&state)?,
        request.snapshot_id,
        &request.left_query,
        &request.right_query,
        &request.op,
        request.details.unwrap_or(false),
        request.limit.unwrap_or(1000),
    )?)))
}

#[derive(Debug, Deserialize)]
struct SaveIdsetRequest {
    snapshot_id: i64,
    name: String,
    object_ids: Vec<String>,
    #[serde(default)]
    source: Value,
}

async fn saved_idsets(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SnapshotOnly>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::saved_idsets(
        &conn(&state)?,
        query.snapshot_id,
    )?)))
}

async fn save_idset(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SaveIdsetRequest>,
) -> Result<Json<Envelope>, ApiError> {
    let object_ids = request
        .object_ids
        .iter()
        .map(|id| {
            id.parse::<i64>().map_err(|_| {
                ApiError::bad_request("invalid_filter", format!("invalid object id: {id}"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut connection = conn(&state)?;
    Ok(Json(Envelope::data(pygco_analysis::save_idset(
        &mut connection,
        request.snapshot_id,
        &request.name,
        request.source,
        object_ids,
    )?)))
}

async fn saved_idset_detail(
    State(state): State<Arc<ApiState>>,
    Path(idset_id): Path<i64>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::saved_idset_detail(
        &conn(&state)?,
        idset_id,
    )?)))
}

async fn schema(State(state): State<Arc<ApiState>>) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_analysis::schema_summary(
        &conn(&state)?,
    )?)))
}

async fn report_json(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SnapshotOnly>,
) -> Result<Json<Envelope>, ApiError> {
    Ok(Json(Envelope::data(pygco_report::build_json(
        &conn(&state)?,
        query.snapshot_id,
    )?)))
}

async fn report_md(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SnapshotOnly>,
) -> Result<impl IntoResponse, ApiError> {
    let markdown = pygco_report::build_markdown(&conn(&state)?, query.snapshot_id)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    Ok((headers, markdown))
}

async fn openapi() -> Json<Envelope> {
    Json(Envelope::data(openapi_document()))
}

pub fn openapi_document() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": { "title": "pygco local API", "version": pygco_store::TOOL_VERSION },
        "paths": {
            "/api/session": { "get": client_operation("getSession", "session", "SessionInfo", "data", None, None, vec![]) },
            "/api/snapshots": { "get": client_operation("listSnapshots", "snapshots", "SnapshotsResponse", "data", None, None, vec![]) },
            "/api/summary": { "get": client_operation("getSummary", "summary", "Summary", "data", Some("SnapshotOnlyParams"), None, vec![]) },
            "/api/objects": { "get": client_operation("listObjects", "objects", "ObjectRow[]", "envelope", Some("ObjectListParams"), None, vec![]) },
            "/api/objects/{object_id}": { "get": client_operation("getObjectDetail", "objectDetail", "ObjectDetailResponse", "data", Some("SnapshotOnlyParams"), None, vec![path_param("objectId", "object_id", "string")]) },
            "/api/objects/{object_id}/edges": { "get": client_operation("getObjectEdges", "objectEdges", "ObjectEdgesResponse", "data", Some("EdgesParams"), None, vec![path_param("objectId", "object_id", "string")]) },
            "/api/objects/{object_id}/paths": { "get": client_operation("getObjectPaths", "objectPaths", "ObjectPathsResponse", "data", Some("ObjectPathsParams"), None, vec![path_param("objectId", "object_id", "string")]) },
            "/api/graph": { "get": client_operation("getGraph", "graph", "GraphData", "data", Some("GraphParams"), None, vec![]) },
            "/api/types": { "get": client_operation("listTypes", "types", "StatRow[]", "data", Some("AggregateParams"), None, vec![]) },
            "/api/modules": { "get": client_operation("listModules", "modules", "ModuleRow[]", "data", Some("AggregateParams"), None, vec![]) },
            "/api/cohorts": { "get": client_operation("listCohorts", "cohorts", "CohortRow[]", "data", Some("AggregateParams"), None, vec![]) },
            "/api/diff": { "get": client_operation("getDiff", "diff", "DiffSummary", "data", Some("DiffParams"), None, vec![]) },
            "/api/diff/objects": { "get": client_operation("listDiffObjects", "diffObjects", "DiffObjectsResponse", "data", Some("DiffObjectsParams"), None, vec![]) },
            "/api/findings": { "get": client_operation("listFindings", "findings", "FindingsResponse", "data", Some("FindingsParams"), None, vec![]) },
            "/api/sql/query": { "post": client_operation("runSqlQuery", "sqlQuery", "SqlQueryResponse", "data", None, Some("SqlRequest"), vec![]) },
            "/api/sql/explain": { "post": client_operation("explainSqlQuery", "sqlExplain", "SqlQueryResponse", "data", None, Some("SqlRequest"), vec![]) },
            "/api/reachability/recompute": { "post": client_operation("recomputeReachability", "recomputeReachability", "JobData", "data", None, Some("ReachabilityRecomputeRequest"), vec![]) },
            "/api/jobs/{job_id}": { "get": client_operation("getJobStatus", "jobStatus", "JobData", "data", None, None, vec![path_param("jobId", "job_id", "string")]) },
            "/api/jobs/{job_id}/cancel": { "post": client_operation("cancelJob", "cancelJob", "JobData", "data", None, None, vec![path_param("jobId", "job_id", "string")]) },
            "/api/idset": { "post": client_operation("runIdset", "idset", "IdsetResponse", "data", None, Some("IdsetRequest"), vec![]) },
            "/api/saved-idsets": {
                "get": client_operation("listSavedIdsets", "savedIdsets", "SavedIdsetsResponse", "data", Some("SnapshotOnlyParams"), None, vec![]),
                "post": client_operation("saveIdset", "saveIdset", "SaveIdsetResponse", "data", None, Some("SaveIdsetRequest"), vec![])
            },
            "/api/saved-idsets/{idset_id}": { "get": client_operation("getSavedIdset", "savedIdsetDetail", "SaveIdsetResponse", "data", None, None, vec![path_param("idsetId", "idset_id", "number")]) },
            "/api/schema": { "get": client_operation("getSchema", "schema", "SchemaSummary", "data", None, None, vec![]) },
            "/api/report.md": { "get": client_operation("getReportMarkdown", "reportMarkdown", "string", "text", Some("SnapshotOnlyParams"), None, vec![]) },
            "/api/report.json": { "get": client_operation("getReportJson", "reportJson", "ReportJson", "data", Some("SnapshotOnlyParams"), None, vec![]) },
            "/api/openapi.json": { "get": client_operation("getOpenApi", "openapi", "OpenApiDocument", "data", None, None, vec![]) }
        },
        "components": {
            "schemas": {
                "AggregateDelta": ts_schema(r#"export type AggregateDelta = {
  count_delta: number;
  shallow_size_delta: number;
  type?: string;
  module?: string;
  cohort?: string;
};"#),
                "CohortRow": ts_schema(r#"export type CohortRow = {
  cohort: string;
  count: number;
  shallow_size_sum: number;
  estimated_reachable_size_sum?: number;
  estimated_reachable_size_max?: number;
  reachable_truncated_count?: number;
  type_count: number;
  details?: { types?: { type: string; module: string; count: number; shallow_size_sum: number }[] };
  rules_version: string;
};"#),
                "DiffObjectRow": ts_schema(r#"export type DiffObjectRow = {
  object_id: string;
  type: string;
  module: string;
  from_shallow_size: number | null;
  to_shallow_size: number | null;
  shallow_size_delta: number;
  state: string;
};"#),
                "DiffObjectsParams": ts_schema(r#"export type DiffObjectsParams = {
  from_snapshot_id: number;
  to_snapshot_id: number;
  state?: string;
  type?: string;
  module?: string;
  limit?: number;
  offset?: number;
};"#),
                "DiffObjectsResponse": ts_schema(r#"export type DiffObjectsResponse = {
  rows: DiffObjectRow[];
  total: number;
};"#),
                "DiffParams": ts_schema(r#"export type DiffParams = {
  from_snapshot_id: number;
  to_snapshot_id: number;
  limit?: number;
};"#),
                "DiffSummary": ts_schema(r#"export type DiffSummary = {
  confidence: { level: string; message: string };
  summary_delta: Record<string, number>;
  object_lifecycle: { new_count: number; gone_count: number; retained_count: number };
  type_delta?: AggregateDelta[];
  module_delta?: AggregateDelta[];
  cohort_delta?: AggregateDelta[];
};"#),
                "EdgesParams": ts_schema(r#"export type EdgesParams = {
  snapshot_id?: number;
  direction?: string;
  limit?: number;
  offset?: number;
};"#),
                "FindingRow": ts_schema(r#"export type FindingRow = {
  severity: string;
  kind: string;
  title: string;
  message: string;
  action: string;
  evidence: unknown;
};"#),
                "FindingsParams": ts_schema(r#"export type FindingsParams = {
  snapshot_id?: number;
  kind?: string;
  severity?: string;
  limit?: number;
  offset?: number;
};"#),
                "FindingsResponse": ts_schema(r#"export type FindingsResponse = {
  rows: FindingRow[];
};"#),
                "GraphData": ts_schema(r#"export type GraphData = {
  nodes: ObjectRow[];
  edges: { from_id: string; to_id: string }[];
  missing_edges: { from_id: string; to_id: string }[];
  limits: Record<string, unknown>;
  truncated?: boolean;
};"#),
                "GraphParams": ts_schema(r#"export type GraphParams = {
  snapshot_id?: number;
  root_object_id: string;
  direction?: string;
  depth?: number;
  node_limit?: number;
  edge_limit?: number;
};"#),
                "IdsetRequest": ts_schema(r#"export type IdsetRequest = {
  snapshot_id: number;
  left_query: string;
  right_query: string;
  op: string;
  details?: boolean;
  limit?: number;
};"#),
                "IdsetResponse": ts_schema(r#"export type IdsetResponse = Record<string, unknown>;"#),
                "JobData": ts_schema(r#"export type JobData = {
  job_id: string;
  kind: string;
  status: "queued" | "running" | "succeeded" | "failed" | "canceling" | "canceled";
  progress: number;
  message?: string | null;
  result?: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};"#),
                "ModuleRow": ts_schema(r#"export type ModuleRow = {
  module: string;
  count: number;
  shallow_size_sum: number;
  in_edges?: number;
  out_edges?: number;
  estimated_reachable_size_sum?: number;
  estimated_reachable_size_max?: number;
  reachable_truncated_count?: number;
};"#),
                "ObjectDetailResponse": ts_schema(r#"export type ObjectDetailResponse = {
  object: ObjectRow;
  top_referents: ObjectRow[];
  top_referrers: ObjectRow[];
};"#),
                "ObjectEdgesResponse": ts_schema(r#"export type ObjectEdgesResponse = Record<string, unknown>;"#),
                "ObjectListParams": ts_schema(r#"export type ObjectListParams = {
  snapshot_id?: number;
  q?: string;
  type?: string;
  module?: string;
  cohort?: string;
  min_shallow_size?: string;
  min_reachable_size?: string;
  min_in_edges?: string;
  min_out_edges?: string;
  has_referrers?: boolean;
  missing_referents?: boolean;
  stub?: string;
  sort?: string;
  order?: string;
  limit?: number;
  offset?: number;
};"#),
                "ObjectPathsParams": ts_schema(r#"export type ObjectPathsParams = {
  snapshot_id?: number;
  direction?: string;
  depth?: number;
  fanout_limit?: number;
  limit?: number;
  include_core?: boolean;
};"#),
                "ObjectPathsResponse": ts_schema(r#"export type ObjectPathsResponse = {
  paths: string[][];
};"#),
                "ObjectRow": ts_schema(r#"export type ObjectRow = {
  object_id: string;
  type: string;
  module: string;
  shallow_size: number;
  estimated_reachable_size: number;
  reachable_truncated: number;
  in_edges: number;
  out_edges: number;
  stub: number;
  missing_referents: number;
};"#),
                "OpenApiDocument": ts_schema(r#"export type OpenApiDocument = Record<string, unknown>;"#),
                "ReachabilityRecomputeRequest": ts_schema(r#"export type ReachabilityRecomputeRequest = {
  snapshot_id?: number;
  depth?: number;
  node_limit?: number;
  fanout_limit?: number;
};"#),
                "ReportJson": ts_schema(r#"export type ReportJson = Record<string, unknown>;"#),
                "SaveIdsetRequest": ts_schema(r#"export type SaveIdsetRequest = {
  snapshot_id: number;
  name: string;
  object_ids: string[];
  source?: unknown;
};"#),
                "SaveIdsetResponse": ts_schema(r#"export type SaveIdsetResponse = {
  idset: SavedIdset;
  rows: ObjectRow[];
};"#),
                "SavedIdset": ts_schema(r#"export type SavedIdset = {
  idset_id: number;
  snapshot_id: number;
  name: string;
  object_count: number;
  created_at: string;
  source?: unknown;
};"#),
                "SavedIdsetsResponse": ts_schema(r#"export type SavedIdsetsResponse = {
  rows: SavedIdset[];
};"#),
                "SchemaSummary": ts_schema(r#"export type SchemaSummary = {
  tables: { name: string; sql: string }[];
  columns: Record<string, { name: string; type: string; notnull: number; pk: number }[]>;
};"#),
                "SessionInfo": ts_schema(r#"export type SessionInfo = {
  database_path: string;
  schema_version: number;
  tool_version: string;
};"#),
                "Snapshot": ts_schema(r#"export type Snapshot = {
  snapshot_id: number;
  source_basename: string;
  object_count: number;
  edge_count: number;
  shallow_size_sum: number;
};"#),
                "AggregateParams": ts_schema(r#"export type AggregateParams = {
  snapshot_id?: number;
  limit?: number;
  sort?: "count" | "shallow-size" | "reachable-size";
};"#),
                "SnapshotOnlyParams": ts_schema(r#"export type SnapshotOnlyParams = {
  snapshot_id?: number;
  limit?: number;
};"#),
                "SnapshotsResponse": ts_schema(r#"export type SnapshotsResponse = {
  rows: Snapshot[];
};"#),
                "SqlQueryResponse": ts_schema(r#"export type SqlQueryResponse = Record<string, unknown> | JobData;"#),
                "SqlRequest": ts_schema(r#"export type SqlRequest = {
  query: string;
  limit?: number;
  async?: boolean;
};"#),
                "StatRow": ts_schema(r#"export type StatRow = {
  type: string;
  module: string;
  count: number;
  shallow_size_sum: number;
  in_edges?: number;
  out_edges?: number;
  stub_count?: number;
  estimated_reachable_size_sum?: number;
  estimated_reachable_size_max?: number;
  reachable_truncated_count?: number;
};"#),
                "Summary": ts_schema(r#"export type Summary = {
  snapshot: Snapshot;
  top_types_by_shallow_size: StatRow[];
  top_modules_by_shallow_size: ModuleRow[];
  top_reachable_types: StatRow[];
  missing_stub_summary: { stub_count: number; missing_referent_count: number };
  import_warnings: unknown[];
};"#)
            }
        }
    })
}

fn client_operation(
    operation_id: &str,
    method_name: &str,
    response_schema: &str,
    response_mode: &str,
    query_schema: Option<&str>,
    body_schema: Option<&str>,
    path_params: Vec<Value>,
) -> Value {
    let mut client = json!({
        "methodName": method_name,
        "responseSchema": response_schema,
        "responseMode": response_mode,
        "pathParams": path_params
    });
    if let Some(schema) = query_schema {
        client["querySchema"] = json!(schema);
    }
    if let Some(schema) = body_schema {
        client["bodySchema"] = json!(schema);
    }
    json!({
        "operationId": operation_id,
        "responses": { "200": { "description": "OK" } },
        "x-client": client
    })
}

fn path_param(name: &str, placeholder: &str, kind: &str) -> Value {
    json!({ "name": name, "placeholder": placeholder, "type": kind })
}

fn ts_schema(source: &str) -> Value {
    json!({ "x-typescript": source })
}

fn parse_optional_i64(value: Option<String>, field: &str) -> Result<Option<i64>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    value.parse::<i64>().map(Some).map_err(|_| {
        ApiError::bad_request_with_details(
            "invalid_filter",
            format!("{field} must be an integer"),
            json!({
                "field": field,
                "expected": "integer",
                "next_step": format!("Pass {field} as an integer or leave it empty.")
            }),
        )
    })
}

fn reachability_params_from_request(
    request: &ReachabilityRecomputeRequest,
) -> Result<ReachabilityParams, ApiError> {
    Ok(ReachabilityParams {
        algorithm_version: pygco_analysis::REACHABILITY_ALGORITHM_VERSION,
        depth: parse_bounded_i64(
            request.depth,
            "depth",
            pygco_analysis::DEFAULT_REACHABILITY_DEPTH,
            0,
            100_000,
        )?,
        node_limit: parse_bounded_i64(
            request.node_limit,
            "node_limit",
            pygco_analysis::DEFAULT_REACHABILITY_NODE_LIMIT,
            1,
            1_000_000,
        )?,
        fanout_limit: parse_bounded_i64(
            request.fanout_limit,
            "fanout_limit",
            pygco_analysis::DEFAULT_REACHABILITY_FANOUT_LIMIT,
            1,
            1_000_000,
        )?,
    })
}

fn parse_optional_bool(value: Option<String>, field: &str) -> Result<Option<bool>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.trim().is_empty() {
        return Ok(None);
    }
    match value.as_str() {
        "true" | "1" => Ok(Some(true)),
        "false" | "0" => Ok(Some(false)),
        _ => Err(ApiError::bad_request_with_details(
            "invalid_filter",
            format!("{field} must be true or false"),
            json!({
                "field": field,
                "expected": "boolean",
                "next_step": format!("Pass {field}=true or {field}=false, or leave it empty.")
            }),
        )),
    }
}

fn parse_object_id(value: &str) -> Result<i64, ApiError> {
    value.parse::<i64>().map_err(|_| {
        ApiError::bad_request_with_details(
            "invalid_object_id",
            "object id must be an integer string",
            json!({
                "field": "object_id",
                "expected": "integer string",
                "next_step": "Copy an object_id from the Objects table or graph and retry."
            }),
        )
    })
}

fn parse_bounded_i64(
    value: Option<i64>,
    field: &str,
    default: i64,
    min: i64,
    max: i64,
) -> Result<i64, ApiError> {
    let value = value.unwrap_or(default);
    if value < min || value > max {
        return Err(ApiError::bad_request_with_details(
            "invalid_filter",
            format!("{field} must be between {min} and {max}"),
            json!({
                "field": field,
                "min": min,
                "max": max,
                "value": value,
                "next_step": format!("Set {field} to a value between {min} and {max}.")
            }),
        ));
    }
    Ok(value)
}
