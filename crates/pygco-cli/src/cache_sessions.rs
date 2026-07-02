use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use pygco_importer::{ImportOptions, ImportSummary};

#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub session_id: String,
    pub cache_root: Option<PathBuf>,
    pub session_dir: PathBuf,
    pub database_path: PathBuf,
    pub import_log_path: PathBuf,
    pub manifest_path: PathBuf,
}

impl SessionPaths {
    pub fn new_default() -> anyhow::Result<Self> {
        let cache_root = cache_root()?;
        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let suffix = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        let session_id = format!("{timestamp}-{suffix}");
        let session_dir = cache_root.join("sessions").join(&session_id);
        Ok(Self::from_parts(session_id, Some(cache_root), session_dir))
    }

    pub fn explicit(session_dir: PathBuf) -> Self {
        let session_id = session_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("session")
            .to_owned();
        Self::from_parts(session_id, None, session_dir)
    }

    fn from_parts(session_id: String, cache_root: Option<PathBuf>, session_dir: PathBuf) -> Self {
        Self {
            session_id,
            cache_root,
            database_path: session_dir.join("analysis.sqlite"),
            import_log_path: session_dir.join("import.log"),
            manifest_path: session_dir.join("manifest.json"),
            session_dir,
        }
    }
}

pub fn cache_root() -> anyhow::Result<PathBuf> {
    if let Some(path) = absolute_env_path("PYGCO_HOME")? {
        return Ok(path);
    }
    if let Some(path) = absolute_env_path("XDG_CACHE_HOME")? {
        return Ok(path.join("pygco"));
    }
    let home = home_dir().context(
        "could not resolve home directory; set PYGCO_HOME to an absolute cache directory",
    )?;
    Ok(home.join(".cache").join("pygco"))
}

#[allow(dead_code)]
pub fn write_manifest(
    paths: &SessionPaths,
    dumps: &[PathBuf],
    summary: &ImportSummary,
    options: &ImportOptions,
) -> anyhow::Result<()> {
    write_manifest_with_fetched_sources(paths, dumps, summary, options, &[])
}

pub fn write_manifest_with_fetched_sources(
    paths: &SessionPaths,
    dumps: &[PathBuf],
    summary: &ImportSummary,
    options: &ImportOptions,
    fetched_sources: &[Value],
) -> anyhow::Result<()> {
    let manifest = manifest_value(paths, dumps, summary, options, fetched_sources);
    fs::write(
        &paths.manifest_path,
        serde_json::to_string_pretty(&manifest)?,
    )
    .with_context(|| format!("write manifest {}", paths.manifest_path.display()))
}

pub fn list_sessions() -> anyhow::Result<Value> {
    let cache_root = cache_root()?;
    let sessions_dir = cache_root.join("sessions");
    let mut sessions = Vec::new();
    if sessions_dir.is_dir() {
        for entry in fs::read_dir(&sessions_dir)
            .with_context(|| format!("read sessions dir {}", sessions_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                sessions.push(session_list_entry(&path));
            }
        }
    }
    sessions.sort_by(|left, right| {
        let left_created = left.get("created_at").and_then(Value::as_str).unwrap_or("");
        let right_created = right
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or("");
        right_created.cmp(left_created)
    });
    Ok(json!({
        "cache_root": path_string(cache_root),
        "sessions": sessions,
    }))
}

fn manifest_value(
    paths: &SessionPaths,
    dumps: &[PathBuf],
    summary: &ImportSummary,
    options: &ImportOptions,
    fetched_sources: &[Value],
) -> Value {
    let created_at = Utc::now().to_rfc3339();
    json!({
        "schema_version": 1,
        "session_id": paths.session_id,
        "created_at": created_at,
        "last_opened_at": created_at,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "cache_root": paths.cache_root.as_ref().map(path_string),
        "session_dir": path_string(&paths.session_dir),
        "database_path": path_string(&paths.database_path),
        "import_log_path": path_string(&paths.import_log_path),
        "source_dumps": dumps.iter().map(path_string).collect::<Vec<_>>(),
        "fetched_sources": fetched_sources,
        "import_options": {
            "reachability_mode": format!("{:?}", options.reachability_mode).to_ascii_lowercase(),
            "profile": options.profile,
        },
        "snapshots": summary.snapshots,
    })
}

fn session_list_entry(session_dir: &Path) -> Value {
    let id = session_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("session")
        .to_owned();
    let manifest_path = session_dir.join("manifest.json");
    let size_bytes = recursive_dir_size(session_dir);

    let Ok(raw) = fs::read_to_string(&manifest_path) else {
        let database_path = session_dir.join("analysis.sqlite");
        return json!({
            "id": id,
            "created_at": Value::Null,
            "last_opened_at": Value::Null,
            "size_bytes": size_bytes,
            "database_path": path_string(database_path),
            "snapshot_count": 0,
            "object_count": Value::Null,
            "source_dumps": [],
            "status": "missing-manifest",
        });
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&raw) else {
        let database_path = session_dir.join("analysis.sqlite");
        return json!({
            "id": id,
            "created_at": Value::Null,
            "last_opened_at": Value::Null,
            "size_bytes": size_bytes,
            "database_path": path_string(database_path),
            "snapshot_count": 0,
            "object_count": Value::Null,
            "source_dumps": [],
            "status": "invalid-manifest",
        });
    };

    let database_path = manifest
        .get("database_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| session_dir.join("analysis.sqlite"));
    let snapshots = manifest
        .get("snapshots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let object_count = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.get("object_count").and_then(Value::as_i64))
        .sum::<i64>();
    let status = if database_path.is_file() {
        "ready"
    } else {
        "missing-db"
    };

    json!({
        "id": manifest.get("session_id").and_then(Value::as_str).unwrap_or(&id),
        "created_at": manifest.get("created_at").cloned().unwrap_or(Value::Null),
        "last_opened_at": manifest.get("last_opened_at").cloned().unwrap_or(Value::Null),
        "size_bytes": size_bytes,
        "database_path": path_string(database_path),
        "snapshot_count": snapshots.len(),
        "object_count": object_count,
        "source_dumps": manifest.get("source_dumps").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        "status": status,
    })
}

fn recursive_dir_size(path: &Path) -> Option<u64> {
    let mut total = 0_u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(path).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.checked_add(metadata.len())?;
            }
        }
    }
    Some(total)
}

fn absolute_env_path(name: &str) -> anyhow::Result<Option<PathBuf>> {
    let Some(value) = env::var_os(name) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        bail!("{name} must be an absolute path, got {}", path.display());
    }
    Ok(Some(path))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("USERPROFILE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
}

fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}
