use std::{
    collections::{BTreeMap, HashSet},
    fs,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::Instant,
};

use flate2::read::MultiGzDecoder;
use rusqlite::{params, Connection, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use pygco_analysis::{
    compute_reachability, refresh_findings, refresh_object_list_metrics, ReachabilityParams,
};
use pygco_dump_format::{
    parse_line, split_type, DumpRecord, MetadataRecord, MetadataStart, ObjectRecord,
};
use pygco_store::{
    apply_import_pragmas, create_indexes, create_schema, finalize_pragmas, now_rfc3339,
};

const OBJECT_BATCH_SIZE: usize = 50_000;
const EDGE_BATCH_SIZE: usize = 100_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ReachabilityMode {
    #[default]
    Full,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub rebuild: bool,
    pub reachability_mode: ReachabilityMode,
    pub reachability_params: ReachabilityParams,
    pub cohort_rules_path: Option<PathBuf>,
    pub profile: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            rebuild: false,
            reachability_mode: ReachabilityMode::Full,
            reachability_params: ReachabilityParams::default(),
            cohort_rules_path: None,
            profile: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub output: PathBuf,
    pub snapshots: Vec<SnapshotImportSummary>,
    pub profile: Vec<ProfileEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotImportSummary {
    pub snapshot_id: i64,
    pub source_uri: String,
    pub dump_sha256: String,
    pub object_count: i64,
    pub edge_count: i64,
    pub stub_count: i64,
    pub missing_referent_count: i64,
    pub shallow_size_sum: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEvent {
    pub phase: String,
    pub elapsed_ms: u128,
    pub wall_time_ms: u128,
    pub self_time_ms: u128,
    pub nested: bool,
    pub snapshot_id: Option<i64>,
    pub phase_kind: String,
}

impl ProfileEvent {
    fn new(
        phase: impl Into<String>,
        wall_time_ms: u128,
        self_time_ms: u128,
        nested: bool,
        snapshot_id: Option<i64>,
        phase_kind: impl Into<String>,
    ) -> Self {
        Self {
            phase: phase.into(),
            elapsed_ms: wall_time_ms,
            wall_time_ms,
            self_time_ms,
            nested,
            snapshot_id,
            phase_kind: phase_kind.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("dump format error: {0}")]
    DumpFormat(#[from] pygco_dump_format::DumpFormatError),
    #[error("analysis error: {0}")]
    Analysis(#[from] pygco_analysis::AnalysisError),
    #[error("store error: {0}")]
    Store(#[from] pygco_store::StoreError),
    #[error("output database already exists: {0}")]
    OutputExists(String),
    #[error("dump did not start with metadata phase=start: {0}")]
    MissingStart(String),
    #[error("dump did not end with metadata phase=end: {0}")]
    MissingEnd(String),
    #[error("duplicate object id {object_id} in snapshot imported from {source_uri}")]
    DuplicateObjectId { source_uri: String, object_id: i64 },
    #[error("cohort rules error: {0}")]
    CohortRules(String),
}

pub type Result<T> = std::result::Result<T, ImportError>;

pub fn import_dumps(
    inputs: Vec<PathBuf>,
    output: PathBuf,
    options: ImportOptions,
) -> Result<ImportSummary> {
    if inputs.is_empty() {
        return Err(ImportError::MissingStart("no input dumps".to_owned()));
    }
    if output.exists() {
        if options.rebuild {
            fs::remove_file(&output)?;
        } else {
            return Err(ImportError::OutputExists(output.display().to_string()));
        }
    }
    let tmp = tmp_path(&output);
    if tmp.exists() {
        fs::remove_file(&tmp)?;
    }
    let result = import_to_tmp(&inputs, &tmp, &output, &options);
    match result {
        Ok(summary) => {
            if output.exists() {
                fs::remove_file(&output)?;
            }
            fs::rename(&tmp, &output)?;
            Ok(summary)
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error)
        }
    }
}

fn import_to_tmp(
    inputs: &[PathBuf],
    tmp: &Path,
    output: &Path,
    options: &ImportOptions,
) -> Result<ImportSummary> {
    let conn = Connection::open(tmp)?;
    apply_import_pragmas(&conn)?;
    create_schema(&conn)?;
    let rules = CohortRules::load(options.cohort_rules_path.as_deref())?;
    let mut snapshots = Vec::new();
    let mut profile = Vec::new();
    for input in inputs {
        let started = Instant::now();
        let (summary, events) = import_one(&conn, input, options, &rules)?;
        let wall_time_ms = started.elapsed().as_millis();
        let nested_time_ms = events.iter().map(|event| event.wall_time_ms).sum::<u128>();
        profile.push(ProfileEvent::new(
            format!("import:{}", input.display()),
            wall_time_ms,
            wall_time_ms.saturating_sub(nested_time_ms),
            false,
            Some(summary.snapshot_id),
            "snapshot_import",
        ));
        profile.extend(events);
        snapshots.push(summary);
    }
    let started = Instant::now();
    create_indexes(&conn)?;
    let elapsed = started.elapsed().as_millis();
    profile.push(ProfileEvent::new(
        "build_indexes",
        elapsed,
        elapsed,
        false,
        None,
        "build_indexes",
    ));
    if matches!(options.reachability_mode, ReachabilityMode::Full) {
        let started = Instant::now();
        for snapshot in &snapshots {
            let has_edges = snapshot.edge_count > 0;
            if has_edges {
                compute_reachability(
                    &conn,
                    Some(snapshot.snapshot_id),
                    options.reachability_params,
                )?;
            } else {
                insert_warning(
                    &conn,
                    Some(snapshot.snapshot_id),
                    "warn",
                    "reachability_unavailable",
                    "Dump has no referent edges; reachable size is unavailable.",
                    json!({ "source_uri": snapshot.source_uri }),
                )?;
            }
        }
        let elapsed = started.elapsed().as_millis();
        profile.push(ProfileEvent::new(
            "reachability",
            elapsed,
            elapsed,
            false,
            None,
            "reachability",
        ));
    }
    let started = Instant::now();
    for snapshot in &snapshots {
        refresh_object_list_metrics(&conn, snapshot.snapshot_id, options.reachability_params)?;
    }
    let elapsed = started.elapsed().as_millis();
    profile.push(ProfileEvent::new(
        "object_list_metrics",
        elapsed,
        elapsed,
        false,
        None,
        "object_list_metrics",
    ));
    let started = Instant::now();
    for snapshot in &snapshots {
        refresh_findings(&conn, snapshot.snapshot_id)?;
    }
    let elapsed = started.elapsed().as_millis();
    profile.push(ProfileEvent::new(
        "findings", elapsed, elapsed, false, None, "findings",
    ));
    finalize_pragmas(&conn)?;
    Ok(ImportSummary {
        output: output.to_path_buf(),
        snapshots,
        profile,
    })
}

fn import_one(
    conn: &Connection,
    input: &Path,
    options: &ImportOptions,
    rules: &CohortRules,
) -> Result<(SnapshotImportSummary, Vec<ProfileEvent>)> {
    let source_uri = input.display().to_string();
    let source_basename = input
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&source_uri)
        .to_owned();
    let dump_sha256 = sha256_file(input)?;
    let file = File::open(input)?;
    let decoder = MultiGzDecoder::new(file);
    let mut reader = BufReader::new(decoder);
    let mut line_number = 0_usize;
    let mut timings = ImportTimings::default();

    let first = match read_dump_record(&mut reader, &mut line_number, &mut timings)? {
        Some(record) => record.1,
        None => return Err(ImportError::MissingStart(source_uri)),
    };
    let start = match first {
        DumpRecord::Metadata(MetadataRecord::Start(start)) => start,
        _ => return Err(ImportError::MissingStart(source_uri)),
    };
    let snapshot_id = insert_snapshot(
        conn,
        &source_uri,
        &source_basename,
        &dump_sha256,
        &start,
        options,
    )?;
    let mut batch_seen = HashSet::with_capacity(OBJECT_BATCH_SIZE);
    let mut object_batch = Vec::with_capacity(OBJECT_BATCH_SIZE);
    let mut edge_batch = Vec::with_capacity(EDGE_BATCH_SIZE);
    let mut saw_end = false;
    let mut dumped_count = 0_i64;
    let mut stub_count = 0_i64;
    let mut edge_count = 0_i64;
    let mut shallow_size_sum = 0_i64;

    conn.execute_batch("BEGIN")?;
    while let Some((_, record)) = read_dump_record(&mut reader, &mut line_number, &mut timings)? {
        match record {
            DumpRecord::Object(object) => {
                if !batch_seen.insert(object.id) {
                    conn.execute_batch("ROLLBACK")?;
                    return Err(ImportError::DuplicateObjectId {
                        source_uri,
                        object_id: object.id,
                    });
                }
                append_object(snapshot_id, &object, &mut object_batch);
                dumped_count += bool_i64(!object.stub);
                stub_count += bool_i64(object.stub);
                shallow_size_sum += object.size.unwrap_or(0).max(0);
                for (edge_index, to_id) in object.referents.iter().enumerate() {
                    edge_batch.push((snapshot_id, object.id, edge_index as i64, *to_id));
                    edge_count += 1;
                    if edge_batch.len() >= EDGE_BATCH_SIZE {
                        let started = Instant::now();
                        flush_edges(conn, &edge_batch)?;
                        timings.insert_edges_ms += started.elapsed().as_millis();
                        edge_batch.clear();
                    }
                }
                if object_batch.len() >= OBJECT_BATCH_SIZE {
                    let started = Instant::now();
                    flush_objects(conn, &source_uri, &object_batch)?;
                    timings.insert_objects_ms += started.elapsed().as_millis();
                    object_batch.clear();
                    batch_seen.clear();
                }
            }
            DumpRecord::Metadata(MetadataRecord::End(_end)) => {
                saw_end = true;
                break;
            }
            DumpRecord::Metadata(MetadataRecord::Start(_)) => {
                conn.execute_batch("ROLLBACK")?;
                return Err(ImportError::MissingEnd(format!(
                    "unexpected second start metadata in {source_uri}"
                )));
            }
        }
    }
    if !object_batch.is_empty() {
        let started = Instant::now();
        flush_objects(conn, &source_uri, &object_batch)?;
        timings.insert_objects_ms += started.elapsed().as_millis();
    }
    if !edge_batch.is_empty() {
        let started = Instant::now();
        flush_edges(conn, &edge_batch)?;
        timings.insert_edges_ms += started.elapsed().as_millis();
    }
    conn.execute_batch("COMMIT")?;
    if !saw_end {
        return Err(ImportError::MissingEnd(source_uri));
    }
    let started = Instant::now();
    build_stats(conn, snapshot_id, rules)?;
    timings.build_stats_ms += started.elapsed().as_millis();
    let missing_referent_count = missing_referent_count(conn, snapshot_id)?;
    conn.execute(
        "
        UPDATE snapshots
        SET object_count = ?2,
            edge_count = ?3,
            stub_count = ?4,
            missing_referent_count = ?5,
            shallow_size_sum = ?6
        WHERE snapshot_id = ?1
        ",
        params![
            snapshot_id,
            dumped_count + stub_count,
            edge_count,
            stub_count,
            missing_referent_count,
            shallow_size_sum
        ],
    )?;
    if !start.include_referents && matches!(options.reachability_mode, ReachabilityMode::Full) {
        insert_warning(
            conn,
            Some(snapshot_id),
            "warn",
            "reachability_unavailable",
            "Dump metadata says include_referents=false; reachable size is unavailable.",
            json!({ "source_uri": source_uri }),
        )?;
    }
    if !start.collect_before_dump {
        insert_warning(
            conn,
            Some(snapshot_id),
            "warn",
            "collect_before_dump_false",
            "Dump was captured without forcing GC; orphan-retained candidates may include GC-pending garbage.",
            json!({ "source_uri": source_uri }),
        )?;
    }
    if !start.include_repr {
        insert_warning(
            conn,
            Some(snapshot_id),
            "info",
            "repr_unavailable",
            "Dump metadata says include_repr=false; string contents, repr snippets, and some dict-key clues are unavailable.",
            json!({ "source_uri": source_uri }),
        )?;
    }
    insert_warning(
        conn,
        Some(snapshot_id),
        "info",
        "edge_labels_unavailable",
        "Current dump format has referent edges but no field names, dict keys, list indexes, or local variable names.",
        json!({ "source_uri": source_uri }),
    )?;
    if !start.include_referent_stubs {
        insert_warning(
            conn,
            Some(snapshot_id),
            "info",
            "referent_stubs_unavailable",
            "Dump metadata says include_referent_stubs=false; missing referents may reduce graph completeness.",
            json!({ "source_uri": source_uri }),
        )?;
    }
    Ok((
        SnapshotImportSummary {
            snapshot_id,
            source_uri,
            dump_sha256,
            object_count: dumped_count + stub_count,
            edge_count,
            stub_count,
            missing_referent_count,
            shallow_size_sum,
        },
        timings.profile_events(snapshot_id),
    ))
}

fn read_dump_record<R: BufRead>(
    reader: &mut R,
    line_number: &mut usize,
    timings: &mut ImportTimings,
) -> Result<Option<(usize, DumpRecord)>> {
    loop {
        let mut line = String::new();
        let started = Instant::now();
        let bytes = reader.read_line(&mut line)?;
        timings.decode_ms += started.elapsed().as_millis();
        if bytes == 0 {
            return Ok(None);
        }
        *line_number += 1;
        if line.trim().is_empty() {
            continue;
        }
        let started = Instant::now();
        let record = parse_line(&line, *line_number)?;
        timings.parse_ms += started.elapsed().as_millis();
        return Ok(Some((*line_number, record)));
    }
}

fn insert_snapshot(
    conn: &Connection,
    source_uri: &str,
    source_basename: &str,
    dump_sha256: &str,
    start: &MetadataStart,
    options: &ImportOptions,
) -> Result<i64> {
    conn.execute(
        "
        INSERT INTO snapshots(
          source_uri, source_basename, dump_sha256, dump_format, dump_format_version,
          producer, producer_version, producer_run_id, dump_sequence, process_started_at,
          host_id, container_id, pid, python_version, platform, created_at, imported_at,
          import_options_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ",
        params![
            source_uri,
            source_basename,
            dump_sha256,
            start.format,
            start.format_version as i64,
            start.producer,
            start.producer_version,
            start.producer_run_id,
            start.dump_sequence as i64,
            start.process_started_at,
            start.host_id,
            start.container_id,
            start.pid as i64,
            start.python_version,
            start.platform,
            start.created_at,
            now_rfc3339(),
            serde_json::to_string(options).unwrap_or_else(|_| "{}".to_owned()),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

type ObjectRow = (
    i64,
    i64,
    String,
    String,
    String,
    Option<i64>,
    Option<i64>,
    i64,
    Option<String>,
);

fn append_object(snapshot_id: i64, object: &ObjectRecord, batch: &mut Vec<ObjectRow>) {
    let (module, qualname) = split_type(
        &object.type_name,
        object.module.as_deref(),
        object.qualname.as_deref(),
    );
    batch.push((
        snapshot_id,
        object.id,
        object.type_name.clone(),
        module,
        qualname,
        object.size,
        object.gc_tracked.map(bool_i64),
        bool_i64(object.stub),
        object.repr.clone(),
    ));
}

fn flush_objects(conn: &Connection, source_uri: &str, rows: &[ObjectRow]) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        INSERT INTO objects(
          snapshot_id, object_id, type, module, qualname, shallow_size, gc_tracked, stub, repr
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
    )?;
    for row in rows {
        if let Err(error) = stmt.execute(params![
            row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8
        ]) {
            if is_constraint_violation(&error) {
                return Err(ImportError::DuplicateObjectId {
                    source_uri: source_uri.to_owned(),
                    object_id: row.1,
                });
            }
            return Err(error.into());
        }
    }
    Ok(())
}

fn is_constraint_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == ErrorCode::ConstraintViolation
    )
}

fn flush_edges(conn: &Connection, rows: &[(i64, i64, i64, i64)]) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        INSERT INTO edges(snapshot_id, from_id, edge_index, to_id)
        VALUES (?1, ?2, ?3, ?4)
        ",
    )?;
    for row in rows {
        stmt.execute(params![row.0, row.1, row.2, row.3])?;
    }
    Ok(())
}

fn build_stats(conn: &Connection, snapshot_id: i64, rules: &CohortRules) -> Result<()> {
    conn.execute(
        "DELETE FROM object_edge_stats WHERE snapshot_id = ?1",
        [snapshot_id],
    )?;
    conn.execute(
        "DELETE FROM type_stats WHERE snapshot_id = ?1",
        [snapshot_id],
    )?;
    conn.execute(
        "DELETE FROM module_stats WHERE snapshot_id = ?1",
        [snapshot_id],
    )?;
    conn.execute(
        "DELETE FROM cohort_stats WHERE snapshot_id = ?1",
        [snapshot_id],
    )?;
    conn.execute(
        "
        INSERT INTO object_edge_stats(snapshot_id, object_id, in_edges, out_edges, missing_referents)
        WITH in_counts AS (
          SELECT to_id AS object_id, COUNT(*) AS n
          FROM edges
          WHERE snapshot_id = ?1
          GROUP BY to_id
        ),
        out_counts AS (
          SELECT from_id AS object_id, COUNT(*) AS n
          FROM edges
          WHERE snapshot_id = ?1
          GROUP BY from_id
        ),
        missing_counts AS (
          SELECT e.from_id AS object_id, COUNT(*) AS n
          FROM edges e
          LEFT JOIN objects target
            ON target.snapshot_id = e.snapshot_id
           AND target.object_id = e.to_id
          WHERE e.snapshot_id = ?1
            AND target.object_id IS NULL
          GROUP BY e.from_id
        )
        SELECT o.snapshot_id,
               o.object_id,
               COALESCE(in_counts.n, 0) AS in_edges,
               COALESCE(out_counts.n, 0) AS out_edges,
               COALESCE(missing_counts.n, 0) AS missing_referents
        FROM objects o
        LEFT JOIN in_counts ON in_counts.object_id = o.object_id
        LEFT JOIN out_counts ON out_counts.object_id = o.object_id
        LEFT JOIN missing_counts ON missing_counts.object_id = o.object_id
        WHERE o.snapshot_id = ?1
        ",
        [snapshot_id],
    )?;
    conn.execute(
        "
        INSERT INTO type_stats(snapshot_id, type, module, count, shallow_size_sum, in_edges, out_edges, stub_count)
        SELECT o.snapshot_id,
               o.type,
               MIN(o.module) AS module,
               COUNT(*) AS count,
               COALESCE(SUM(o.shallow_size), 0) AS shallow_size_sum,
               COALESCE(SUM(es.in_edges), 0) AS in_edges,
               COALESCE(SUM(es.out_edges), 0) AS out_edges,
               SUM(o.stub) AS stub_count
        FROM objects o
        LEFT JOIN object_edge_stats es
          ON es.snapshot_id = o.snapshot_id
         AND es.object_id = o.object_id
        WHERE o.snapshot_id = ?1
        GROUP BY o.snapshot_id, o.type
        ",
        [snapshot_id],
    )?;
    conn.execute(
        "
        INSERT INTO module_stats(snapshot_id, module, count, shallow_size_sum, in_edges, out_edges)
        SELECT snapshot_id,
               module,
               SUM(count),
               SUM(shallow_size_sum),
               SUM(in_edges),
               SUM(out_edges)
        FROM type_stats
        WHERE snapshot_id = ?1
        GROUP BY snapshot_id, module
        ",
        [snapshot_id],
    )?;
    build_cohort_stats(conn, snapshot_id, rules)?;
    Ok(())
}

fn build_cohort_stats(conn: &Connection, snapshot_id: i64, rules: &CohortRules) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        SELECT type, module, count, shallow_size_sum
        FROM type_stats
        WHERE snapshot_id = ?1
        ",
    )?;
    let rows = stmt
        .query_map([snapshot_id], |row| {
            Ok(TypeStat {
                type_name: row.get(0)?,
                module: row.get(1)?,
                count: row.get(2)?,
                shallow_size_sum: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut cohorts: BTreeMap<String, CohortAccumulator> = BTreeMap::new();
    for stat in rows {
        for name in rules.matches(&stat) {
            cohorts.entry(name).or_default().add(&stat);
        }
    }
    let mut insert = conn.prepare(
        "
        INSERT INTO cohort_stats(
          snapshot_id, cohort, count, shallow_size_sum, type_count, details_json, rules_version
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
    )?;
    for (name, accumulator) in cohorts {
        insert.execute(params![
            snapshot_id,
            name,
            accumulator.count,
            accumulator.shallow_size_sum,
            accumulator.types.len() as i64,
            serde_json::to_string(&json!({ "types": accumulator.types }))
                .unwrap_or_else(|_| "{\"types\":[]}".to_owned()),
            rules.version,
        ])?;
    }
    Ok(())
}

fn missing_referent_count(conn: &Connection, snapshot_id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "
        SELECT COALESCE(SUM(missing_referents), 0)
        FROM object_edge_stats
        WHERE snapshot_id = ?1
        ",
        [snapshot_id],
        |row| row.get(0),
    )?)
}

fn insert_warning(
    conn: &Connection,
    snapshot_id: Option<i64>,
    level: &str,
    code: &str,
    message: &str,
    context: Value,
) -> Result<()> {
    conn.execute(
        "
        INSERT INTO import_warnings(snapshot_id, level, code, message, context_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            snapshot_id,
            level,
            code,
            message,
            context.to_string(),
            now_rfc3339()
        ],
    )?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 64];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn tmp_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("analysis.sqlite");
    output.with_file_name(format!("{file_name}.tmp.sqlite"))
}

fn bool_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[derive(Debug, Default)]
struct ImportTimings {
    decode_ms: u128,
    parse_ms: u128,
    insert_objects_ms: u128,
    insert_edges_ms: u128,
    build_stats_ms: u128,
}

impl ImportTimings {
    fn profile_events(self, snapshot_id: i64) -> Vec<ProfileEvent> {
        [
            ("decode", self.decode_ms),
            ("parse", self.parse_ms),
            ("insert_objects", self.insert_objects_ms),
            ("insert_edges", self.insert_edges_ms),
            ("build_stats", self.build_stats_ms),
        ]
        .into_iter()
        .map(|(phase, elapsed_ms)| {
            ProfileEvent::new(
                format!("snapshot:{snapshot_id}:{phase}"),
                elapsed_ms,
                elapsed_ms,
                true,
                Some(snapshot_id),
                phase,
            )
        })
        .collect()
    }
}

#[derive(Debug, Clone)]
struct TypeStat {
    type_name: String,
    module: String,
    count: i64,
    shallow_size_sum: i64,
}

#[derive(Debug, Clone, Default)]
struct CohortAccumulator {
    count: i64,
    shallow_size_sum: i64,
    types: Vec<Value>,
}

impl CohortAccumulator {
    fn add(&mut self, stat: &TypeStat) {
        self.count += stat.count;
        self.shallow_size_sum += stat.shallow_size_sum;
        self.types.push(json!({
            "type": stat.type_name,
            "module": stat.module,
            "count": stat.count,
            "shallow_size_sum": stat.shallow_size_sum,
        }));
    }
}

#[derive(Debug, Clone)]
struct CohortRules {
    version: String,
    rules: Vec<CohortRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CohortRulesFile {
    #[serde(default)]
    cohort: Vec<CohortRuleFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CohortRuleFile {
    name: String,
    #[serde(default)]
    type_contains: Vec<String>,
    #[serde(default)]
    module_prefix: Vec<String>,
    #[serde(default)]
    type_prefix: Vec<String>,
}

#[derive(Debug, Clone)]
struct CohortRule {
    name: String,
    type_contains: Vec<String>,
    module_prefix: Vec<String>,
    type_prefix: Vec<String>,
}

impl CohortRules {
    fn load(path: Option<&Path>) -> Result<Self> {
        let mut rules = builtin_rules();
        let mut version = "builtin-v1".to_owned();
        if let Some(path) = path {
            let content = fs::read_to_string(path)?;
            let parsed: CohortRulesFile = toml::from_str(&content)
                .map_err(|error| ImportError::CohortRules(error.to_string()))?;
            for rule in parsed.cohort {
                rules.push(CohortRule {
                    name: rule.name,
                    type_contains: rule.type_contains,
                    module_prefix: rule.module_prefix,
                    type_prefix: rule.type_prefix,
                });
            }
            version = format!("builtin-v1+{}", path.display());
        }
        Ok(Self { version, rules })
    }

    fn matches(&self, stat: &TypeStat) -> Vec<String> {
        self.rules
            .iter()
            .filter(|rule| rule.matches(stat))
            .map(|rule| rule.name.clone())
            .collect()
    }
}

impl CohortRule {
    fn matches(&self, stat: &TypeStat) -> bool {
        self.type_contains
            .iter()
            .any(|needle| stat.type_name.contains(needle))
            || self
                .type_prefix
                .iter()
                .any(|prefix| stat.type_name.starts_with(prefix))
            || self
                .module_prefix
                .iter()
                .any(|prefix| stat.module.starts_with(prefix))
    }
}

fn builtin_rules() -> Vec<CohortRule> {
    vec![
        CohortRule {
            name: "threading".to_owned(),
            type_contains: vec!["Thread".to_owned(), "Lock".to_owned()],
            module_prefix: vec![
                "threading".to_owned(),
                "_thread".to_owned(),
                "concurrent.futures".to_owned(),
            ],
            type_prefix: Vec::new(),
        },
        CohortRule {
            name: "async_runtime".to_owned(),
            type_contains: vec![
                "coroutine".to_owned(),
                "Task".to_owned(),
                "async_generator".to_owned(),
            ],
            module_prefix: vec!["asyncio".to_owned(), "_asyncio".to_owned()],
            type_prefix: Vec::new(),
        },
        CohortRule {
            name: "network_io".to_owned(),
            type_contains: vec!["Response".to_owned(), "Socket".to_owned()],
            module_prefix: vec![
                "socket".to_owned(),
                "ssl".to_owned(),
                "httpx".to_owned(),
                "httpcore".to_owned(),
                "urllib3".to_owned(),
                "requests".to_owned(),
            ],
            type_prefix: Vec::new(),
        },
        CohortRule {
            name: "database_cache".to_owned(),
            type_contains: vec!["ConnectionPool".to_owned(), "Cache".to_owned()],
            module_prefix: vec![
                "sqlalchemy".to_owned(),
                "redis".to_owned(),
                "pymysql".to_owned(),
                "psycopg".to_owned(),
            ],
            type_prefix: Vec::new(),
        },
        CohortRule {
            name: "streaming".to_owned(),
            type_contains: vec![
                "Stream".to_owned(),
                "Streaming".to_owned(),
                "ResponseStream".to_owned(),
            ],
            module_prefix: Vec::new(),
            type_prefix: Vec::new(),
        },
        CohortRule {
            name: "observability".to_owned(),
            type_contains: Vec::new(),
            module_prefix: vec![
                "logging".to_owned(),
                "opentelemetry".to_owned(),
                "prometheus_client".to_owned(),
            ],
            type_prefix: Vec::new(),
        },
    ]
}
