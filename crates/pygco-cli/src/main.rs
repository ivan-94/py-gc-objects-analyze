use std::{
    fs::{self, File},
    io::{IsTerminal, Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, Context};
use chrono::Utc;
use clap::{Args, ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use reqwest::header::{HeaderName, HeaderValue, CONTENT_DISPOSITION};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use pygco_analysis::{
    DiffObjectsOptions, FindingsOptions, ObjectFilters, ReachabilityParams, SuspectsOptions,
};
use pygco_importer::{import_dumps, ImportError, ImportOptions, ReachabilityMode};

mod cache_sessions;

const FINDINGS_TABLE_FIELDS: &[&str] = &["severity", "kind", "title", "action"];
const OVERVIEW_TABLE_FIELDS: &[&str] = &[
    "section",
    "rank",
    "kind",
    "subject",
    "count",
    "shallow_size",
    "estimated_reachable_size",
    "status",
    "next_command",
];
const SUSPECTS_TABLE_FIELDS: &[&str] = &[
    "rank",
    "kind",
    "severity",
    "confidence",
    "subject.object_id",
    "subject.type",
    "subject.module",
    "metrics.estimated_reachable_size",
    "reason",
    "next_command",
];
const ANNOTATED_PATHS_TABLE_FIELDS: &[&str] = &[
    "path_index",
    "depth",
    "object_id",
    "type",
    "module",
    "shallow_size",
    "estimated_reachable_size",
    "external_in_edges",
    "interpretation",
];
const CONTAINER_TABLE_FIELDS: &[&str] = &[
    "section",
    "rank",
    "object_id",
    "type",
    "module",
    "count",
    "shallow_size",
    "shallow_size_sum",
];

#[derive(Debug, Parser)]
#[command(
    name = "pygco",
    version,
    about = "Local Python GC object memory forensics",
    after_help = r#"Typical workflows:
  pygco open dump.jsonl.gz --no-browser
      Import one or more dumps into a cache session, then serve the local Web UI.
  pygco import before.jsonl.gz after.jsonl.gz -o analysis.sqlite --rebuild
      Build an explicit SQLite analysis database for repeatable CLI/API use.
  pygco sessions list --format table
      Find cached analysis sessions created by `pygco open`.
  pygco overview analysis.sqlite --format table
      Start leak triage from quality, top types, resource cohorts, and next commands.
  pygco objects analysis.sqlite --sort reachable-size --limit 20 --format table
      Start from the largest reachable objects when investigating leaks.

Use --format json for machine-readable output. Use --fields to project JSON/table rows for agent pipelines.
SQLite analysis files are rebuildable cache artifacts; keep the source dump files for durable evidence."#
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Disable ANSI color in errors and help output"
    )]
    no_color: bool,
    #[arg(
        long,
        global = true,
        help = "Print detailed error chains for debugging and agent logs"
    )]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        about = "Import dumps into a cache session and serve the local Web UI",
        after_help = r#"Examples:
  pygco open dump.jsonl.gz
  pygco open before.jsonl.gz after.jsonl.gz --no-browser
  pygco open dump.jsonl.gz --session-dir .pygco/sessions/manual --cleanup-on-exit

Notes:
  Without --session-dir, sessions are stored under PYGCO_HOME, XDG_CACHE_HOME/pygco, or ~/.cache/pygco.
  The session contains analysis.sqlite, import.log, and manifest.json.
  Use `pygco sessions list` to discover cache sessions later."#
    )]
    Open(OpenArgs),
    #[command(
        about = "Download a dump URL to a local file with hashing and redacted source metadata",
        after_help = r#"Examples:
  pygco fetch https://example.com/gc-heap-dump -o dump.jsonl.gz
  pygco fetch https://example.com/gc-heap-dump --header Authorization=Bearer... --format json

Fetch records original/final URL in redacted form and never prints secret header values."#
    )]
    Fetch(FetchArgs),
    #[command(
        about = "Import dumps into an explicit SQLite analysis database",
        after_help = r#"Examples:
  pygco import dump.jsonl.gz -o analysis.sqlite --rebuild
  pygco import before.jsonl.gz after.jsonl.gz -o comparison.sqlite --rebuild --profile
  pygco import dump.jsonl.gz -o fast.sqlite --rebuild --no-reachability

Writes a fresh SQLite analysis database for CLI, API, or Web UI commands.
Use --no-reachability for faster shallow analysis when reachable-size sorting is not needed."#
    )]
    Import(ImportArgs),
    #[command(about = "Inspect cached analysis sessions created by `pygco open`")]
    Sessions(SessionsArgs),
    #[command(
        about = "Show snapshot overview, top types/modules/cohorts, warnings, and findings",
        after_help = r#"Examples:
  pygco summary analysis.sqlite --format table
  pygco summary analysis.sqlite --snapshot 2 --limit 30 --format json

Useful first check:
  Confirm object_count, edge_count, top type/module growth, missing referents, and warnings."#
    )]
    Summary(DbArgs),
    #[command(
        about = "Compact leak triage entrypoint with quality, top cohorts, and next commands",
        after_help = r#"Examples:
  pygco overview analysis.sqlite --snapshot 1 --format table
  pygco overview analysis.sqlite --snapshot 1 --with-suspects --format json

By default overview avoids heavy suspect queries and prints the next command to run them."#
    )]
    Overview(OverviewArgs),
    #[command(
        about = "List objects with filters, sorting, pagination, and automation-friendly projections",
        after_help = r#"Examples:
  pygco objects analysis.sqlite --sort reachable-size --limit 20 --format table
  pygco objects analysis.sqlite --type dict --min-reachable-size 1mb --format json
  pygco objects analysis.sqlite --fields object_id,type,shallow_size,reachable_size --format jsonl

Sort keys:
  reachable-size, shallow-size, in-edges, out-edges, object-id, type, module.

Automation tip:
  Use --fields object_id,type,shallow_size,reachable_size,next_command to keep outputs compact."#
    )]
    Objects(ObjectsArgs),
    #[command(
        about = "Show one object's metadata, metrics, direct edges, and next investigation commands",
        after_help = r#"Examples:
  pygco object analysis.sqlite --id 281470886362416 --format json
  pygco object analysis.sqlite --snapshot 2 --id 100 --format markdown

Object ids are emitted as strings in JSON to preserve JavaScript precision."#
    )]
    Object(ObjectArgs),
    #[command(
        about = "List direct referents or referrers for one object",
        after_help = r#"Examples:
  pygco edges analysis.sqlite --from 100 --limit 50 --format table
  pygco edges analysis.sqlite --to 100 --snapshot 2 --format json

Use exactly one of --from or --to."#
    )]
    Edges(EdgesArgs),
    #[command(
        about = "Sample bounded owner/reference paths around an object",
        after_help = r#"Examples:
  pygco paths analysis.sqlite --id 100 --direction referrers --depth 5 --fanout 30 --format json
  pygco paths analysis.sqlite --id 100 --direction referents --limit 20 --format table

This is a bounded exploration helper, not an exhaustive graph traversal."#
    )]
    Paths(PathsArgs),
    #[command(
        about = "Compare aggregate changes between two snapshots",
        after_help = r#"Examples:
  pygco diff analysis.sqlite --from 1 --to 2 --format markdown
  pygco diff analysis.sqlite --from 1 --to 2 --limit 50 --format json

Best for before/after dump pairs imported into the same SQLite database."#
    )]
    Diff(DiffArgs),
    #[command(
        about = "Compare object lifecycle rows between two snapshots",
        after_help = r#"Examples:
  pygco diff-objects analysis.sqlite --from 1 --to 2 --state new --format table
  pygco diff-objects analysis.sqlite --from 1 --to 2 --state retained --type dict --ids-only

Object-level lifecycle confidence is highest for dumps from the same Python process run."#
    )]
    DiffObjects(DiffObjectsArgs),
    #[command(
        about = "List persisted diagnostic findings produced during import",
        after_help = r#"Examples:
  pygco findings analysis.sqlite --snapshot 1 --format table
  pygco findings analysis.sqlite --kind large-type --severity warn --format json

Findings are leads, not final conclusions. Use object/path commands to verify."#
    )]
    Findings(FindingsArgs),
    #[command(
        about = "Generate heuristic memory investigation leads without writing SQL",
        after_help = r#"Examples:
  pygco suspects analysis.sqlite --snapshot 1 --kind orphan-retained --min-reachable 1mb --format table
  pygco suspects analysis.sqlite --kind cache --kind async --kind connection --format json

Kinds include orphan-retained, high-retained-root, truncated-root, type-footprint,
metadata-heavy, cache-heavy, async-backlog, and connection-heavy."#
    )]
    Suspects(SuspectsArgs),
    #[command(
        about = "Explain direct contents of common containers such as deque, queue, cache, dict, list, and set",
        after_help = r#"Examples:
  pygco container analysis.sqlite --id 281470886362416 --top-items --item-types --format table
  pygco container analysis.sqlite --snapshot 1 --id 100 --format json

Container analysis uses direct referents from the dump graph. Field names, dict keys, and queue internals require richer dump data."#
    )]
    Container(ContainerArgs),
    #[command(
        about = "Run set operations over two read-only object-id SQL queries",
        after_help = r#"Examples:
  pygco idset analysis.sqlite --left-query 'select object_id from objects' --right-query 'select to_id as object_id from edges' --op intersect --details
  pygco idset analysis.sqlite --snapshot 1 --op left-only --ids-only

Both SQL queries must return an object_id column."#
    )]
    Idset(IdsetArgs),
    #[command(
        about = "Run read-only SQL or EXPLAIN QUERY PLAN against the analysis database",
        after_help = r#"Examples:
  pygco sql analysis.sqlite --query 'select type, count(*) from objects group by type order by count(*) desc limit 20' --format table
  pygco sql analysis.sqlite --query 'select object_id from objects limit 10' --explain --format json

Read-only SQL workbench:
  Only SELECT-style read queries are accepted.
  Use --explain to inspect SQLite query plans before expensive probes."#
    )]
    Sql(SqlArgs),
    #[command(
        about = "Print SQLite schema summary for query planning and agent discovery",
        after_help = r#"Examples:
  pygco schema analysis.sqlite --format table
  pygco schema analysis.sqlite --format json

Use this before writing ad hoc SQL against unfamiliar databases."#
    )]
    Schema(DbArgs),
    #[command(
        about = "Export a bounded object neighborhood as JSON, JSONL, or DOT",
        after_help = r#"Examples:
  pygco export-subgraph analysis.sqlite --id 100 --depth 2 --direction both --graph-format dot
  pygco export-subgraph analysis.sqlite --id 100 --node-limit 500 --edge-limit 2000 --format json

Use DOT for graph visualization and JSON for agent-side post-processing."#
    )]
    ExportSubgraph(ExportSubgraphArgs),
    #[command(
        about = "Generate a human-readable or JSON memory forensics report",
        after_help = r#"Examples:
  pygco report analysis.sqlite --snapshot 1 --format markdown
  pygco report analysis.sqlite --format json

Markdown reports are suitable for issues, PRs, and release acceptance notes."#
    )]
    Report(DbArgs),
    #[command(
        about = "Check database health, schema version, indexes, and snapshot availability",
        after_help = r#"Examples:
  pygco doctor analysis.sqlite --format table
  pygco doctor analysis.sqlite --format json

Run this when a database fails to open, queries are unexpectedly slow, or a session looks incomplete."#
    )]
    Doctor(DbArgs),
    #[command(
        about = "Serve the Web UI for an existing SQLite analysis database",
        after_help = r#"Examples:
  pygco web analysis.sqlite --host 127.0.0.1 --port 3791
  pygco web analysis.sqlite --dev --no-browser

Use this after an explicit `pygco import -o` workflow."#
    )]
    Web(WebArgs),
    #[command(
        about = "Serve the local API for an existing SQLite analysis database",
        after_help = r#"Examples:
  pygco api analysis.sqlite --host 127.0.0.1 --port 5174 --no-browser

The API is local-first and binds to 127.0.0.1 by default."#
    )]
    Api(WebArgs),
    #[command(about = "Print the pygco CLI version")]
    Version,
}

#[derive(Debug, Clone, Args)]
struct OutputArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json, help = "Output format: json for agents, jsonl for streams, table for humans, markdown for reports")]
    format: OutputFormat,
    #[arg(
        long,
        value_name = "FIELDS",
        help = "Comma-separated projection for row/object fields, for example object_id,type,shallow_size"
    )]
    fields: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
    Table,
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProgressArg {
    Auto,
    Always,
    Never,
}

impl ProgressArg {
    fn enabled(self) -> bool {
        match self {
            ProgressArg::Always => true,
            ProgressArg::Never => false,
            ProgressArg::Auto => std::io::stderr().is_terminal(),
        }
    }
}

#[derive(Debug, Args)]
struct ImportArgs {
    #[arg(
        required = true,
        value_name = "DUMPS",
        help = "One or more gzip JSONL dump files from pygco_dump"
    )]
    dumps: Vec<PathBuf>,
    #[arg(
        short,
        long,
        value_name = "SQLITE",
        help = "Output SQLite analysis database path"
    )]
    output: PathBuf,
    #[arg(long, help = "Replace the output database if it already exists")]
    rebuild: bool,
    #[arg(
        long,
        help = "Skip reachable-size computation for faster shallow imports"
    )]
    no_reachability: bool,
    #[arg(long, value_enum, default_value_t = ReachabilityModeArg::Full, help = "Reachability computation mode")]
    reachability_mode: ReachabilityModeArg,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_DEPTH, help = "Maximum depth for bounded reachable-size estimation")]
    reachability_depth: i64,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_NODE_LIMIT, help = "Maximum nodes visited per reachable-size computation")]
    reachability_node_limit: i64,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_FANOUT_LIMIT, help = "Maximum outgoing edges explored per node during reachable-size estimation")]
    reachability_fanout_limit: i64,
    #[arg(
        long,
        value_name = "TOML",
        help = "Optional cohort rules TOML file for cache/async/connection classification"
    )]
    rules: Option<PathBuf>,
    #[arg(
        long,
        help = "Include import phase timings in the JSON output and import log"
    )]
    profile: bool,
    #[arg(
        long,
        value_enum,
        default_value_t = ProgressArg::Auto,
        help = "Import progress on stderr: auto, always, or never"
    )]
    progress: ProgressArg,
    #[command(flatten)]
    output_args: OutputArgs,
}

#[derive(Debug, Args)]
struct FetchArgs {
    #[arg(value_name = "URL", help = "HTTP or HTTPS dump URL to download")]
    url: String,
    #[arg(
        short = 'o',
        long = "output",
        value_name = "PATH",
        help = "Local output dump path; defaults to a filename inferred from HTTP headers or URL"
    )]
    output_file: Option<PathBuf>,
    #[arg(
        long = "header",
        value_name = "KEY=VALUE",
        help = "HTTP request header; repeat for multiple headers. Secret values are not logged."
    )]
    headers: Vec<String>,
    #[arg(long, default_value_t = 30, help = "HTTP request timeout in seconds")]
    timeout: u64,
    #[arg(
        long,
        value_name = "BYTES",
        help = "Maximum response bytes, for example 100mb"
    )]
    max_bytes: Option<String>,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct SessionsArgs {
    #[command(subcommand)]
    command: SessionsCommand,
}

#[derive(Debug, Subcommand)]
enum SessionsCommand {
    #[command(
        about = "List cached analysis sessions",
        after_help = r#"Examples:
  pygco sessions list --format table
  pygco sessions list --format json --fields id,status,size_bytes,database_path

Cache root order:
  1. PYGCO_HOME
  2. XDG_CACHE_HOME/pygco
  3. ~/.cache/pygco

Statuses:
  status=ready means analysis.sqlite and manifest.json are present.
  status=missing-db, missing-manifest, or invalid-manifest marks an incomplete cache session."#
    )]
    List(SessionsListArgs),
}

#[derive(Debug, Args)]
struct SessionsListArgs {
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReachabilityModeArg {
    Full,
    Off,
}

#[derive(Debug, Args)]
struct DbArgs {
    #[arg(
        value_name = "DB",
        help = "SQLite analysis database produced by `pygco import` or `pygco open`"
    )]
    db: PathBuf,
    #[arg(
        long,
        help = "Snapshot id to query; defaults to the latest/only snapshot when supported"
    )]
    snapshot: Option<i64>,
    #[arg(
        long,
        default_value_t = 20,
        help = "Maximum rows or top-N entries returned by commands that support limits"
    )]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct OverviewArgs {
    #[arg(
        value_name = "DB",
        help = "SQLite analysis database produced by `pygco import` or `pygco open`"
    )]
    db: PathBuf,
    #[arg(
        long,
        help = "Snapshot id to query; defaults to the latest/only snapshot"
    )]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 20, help = "Maximum rows per overview section")]
    limit: i64,
    #[arg(long, help = "Run heavier suspects analysis inside overview")]
    with_suspects: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ObjectsArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(
        long,
        value_name = "TEXT",
        help = "Case-insensitive search over type, module, qualname, repr, and labels"
    )]
    q: Option<String>,
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Filter by exact or pattern-like Python type name"
    )]
    type_name: Option<String>,
    #[arg(long, value_name = "MODULE", help = "Filter by module name")]
    module: Option<String>,
    #[arg(
        long,
        value_name = "COHORT",
        help = "Filter by analysis cohort such as cache-heavy or async-backlog"
    )]
    cohort: Option<String>,
    #[arg(long, value_name = "BYTES", help = "Minimum shallow size in bytes")]
    min_shallow_size: Option<i64>,
    #[arg(
        long,
        value_name = "BYTES",
        help = "Minimum estimated reachable size in bytes"
    )]
    min_reachable_size: Option<i64>,
    #[arg(long, value_name = "N", help = "Minimum incoming edge count")]
    min_in_edges: Option<i64>,
    #[arg(long, value_name = "N", help = "Minimum outgoing edge count")]
    min_out_edges: Option<i64>,
    #[arg(long, help = "Only include objects with at least one referrer")]
    has_referrers: bool,
    #[arg(long, help = "Only include objects with missing referent records")]
    missing_referents: bool,
    #[arg(
        long,
        help = "Filter stub objects: true for stubs, false for non-stubs"
    )]
    stub: Option<bool>,
    #[arg(
        long,
        default_value = "reachable-size",
        help = "Sort key such as reachable-size, shallow-size, in-edges, out-edges, object-id, type, or module"
    )]
    sort: String,
    #[arg(long, default_value = "desc", help = "Sort order: asc or desc")]
    order: String,
    #[arg(long, default_value_t = 100, help = "Maximum object rows to return")]
    limit: i64,
    #[arg(long, default_value_t = 0, help = "Pagination offset")]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ObjectArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, alias = "object-id", help = "Object id to inspect")]
    id: i64,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct EdgesArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(
        long = "from",
        value_name = "OBJECT_ID",
        help = "List referents of this object id"
    )]
    from_id: Option<i64>,
    #[arg(
        long = "to",
        value_name = "OBJECT_ID",
        help = "List referrers pointing to this object id"
    )]
    to_id: Option<i64>,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 100, help = "Maximum edge rows to return")]
    limit: i64,
    #[arg(long, default_value_t = 0, help = "Pagination offset")]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct PathsArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, help = "Root object id for bounded path sampling")]
    id: i64,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(
        long,
        default_value = "referrers",
        help = "Path direction: referrers, referents, or both when supported"
    )]
    direction: String,
    #[arg(long, default_value_t = 5, help = "Maximum path depth")]
    depth: i64,
    #[arg(long, default_value_t = 30, help = "Maximum branches sampled per node")]
    fanout: i64,
    #[arg(long, default_value_t = 50, help = "Maximum paths to return")]
    limit: i64,
    #[arg(
        long,
        help = "Annotate every path node with object summary and diagnostic facts"
    )]
    annotate: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct DiffArgs {
    #[arg(
        value_name = "DB",
        help = "SQLite analysis database containing both snapshots"
    )]
    db: PathBuf,
    #[arg(
        long = "from",
        value_name = "SNAPSHOT_ID",
        help = "Baseline snapshot id"
    )]
    from_snapshot: i64,
    #[arg(
        long = "to",
        value_name = "SNAPSHOT_ID",
        help = "Comparison snapshot id"
    )]
    to_snapshot: i64,
    #[arg(long, default_value_t = 100, help = "Maximum rows per diff section")]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct DiffObjectsArgs {
    #[arg(
        value_name = "DB",
        help = "SQLite analysis database containing both snapshots"
    )]
    db: PathBuf,
    #[arg(
        long = "from",
        value_name = "SNAPSHOT_ID",
        help = "Baseline snapshot id"
    )]
    from_snapshot: i64,
    #[arg(
        long = "to",
        value_name = "SNAPSHOT_ID",
        help = "Comparison snapshot id"
    )]
    to_snapshot: i64,
    #[arg(
        long,
        default_value = "new",
        help = "Lifecycle state: new, gone, retained, or changed when supported"
    )]
    state: String,
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Filter lifecycle rows by type"
    )]
    type_name: Option<String>,
    #[arg(long, value_name = "MODULE", help = "Filter lifecycle rows by module")]
    module: Option<String>,
    #[arg(long, default_value_t = 100, help = "Maximum object rows to return")]
    limit: i64,
    #[arg(long, default_value_t = 0, help = "Pagination offset")]
    offset: i64,
    #[arg(long, help = "Print only object ids, one per line")]
    ids_only: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct FindingsArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(
        long,
        value_name = "KIND",
        help = "Finding kind filter, for example large-type"
    )]
    kind: Option<String>,
    #[arg(
        long,
        value_name = "SEVERITY",
        help = "Severity filter such as info, warn, or error"
    )]
    severity: Option<String>,
    #[arg(long, default_value_t = 100, help = "Maximum finding rows to return")]
    limit: i64,
    #[arg(long, default_value_t = 0, help = "Pagination offset")]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct SuspectsArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(
        long = "kind",
        value_name = "KIND",
        help = "Suspect kind; repeat for multiple kinds"
    )]
    kinds: Vec<String>,
    #[arg(
        long,
        default_value = "1mb",
        help = "Minimum estimated reachable size, for example 100b, 1mb, 2gb"
    )]
    min_reachable: String,
    #[arg(long, help = "Prefer non-builtin/module-owned suspects")]
    non_builtin: bool,
    #[arg(long, help = "Include stub objects in suspect generation")]
    include_stub: bool,
    #[arg(long, default_value_t = 20, help = "Maximum suspect rows to return")]
    limit: i64,
    #[arg(long, default_value_t = 0, help = "Pagination offset")]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ContainerArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, alias = "object-id", help = "Container object id to inspect")]
    id: i64,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(long, help = "Include largest direct referent items")]
    top_items: bool,
    #[arg(long, help = "Include direct referent type aggregation")]
    item_types: bool,
    #[arg(long, default_value_t = 20, help = "Maximum item rows per section")]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct SqlArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, short, value_name = "SQL", help = "Read-only SQL query to run")]
    query: String,
    #[arg(long, default_value_t = 1000, help = "Maximum result rows to return")]
    limit: i64,
    #[arg(
        long,
        help = "Return SQLite EXPLAIN QUERY PLAN output instead of query rows"
    )]
    explain: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct IdsetArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(long, help = "Snapshot id to apply to details output when relevant")]
    snapshot: Option<i64>,
    #[arg(
        long,
        value_name = "SQL",
        help = "Left read-only SQL query returning object_id"
    )]
    left_query: String,
    #[arg(
        long,
        value_name = "SQL",
        help = "Right read-only SQL query returning object_id"
    )]
    right_query: String,
    #[arg(
        long,
        default_value = "intersect",
        help = "Set operation: intersect, union, left-only, or right-only"
    )]
    op: String,
    #[arg(long, help = "Return object details for the resulting id set")]
    details: bool,
    #[arg(
        long,
        default_value_t = 1000,
        help = "Maximum ids or detail rows to return"
    )]
    limit: i64,
    #[arg(long, help = "Print only object ids, one per line")]
    ids_only: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ExportSubgraphArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database")]
    db: PathBuf,
    #[arg(
        long,
        alias = "object-id",
        help = "Root object id for the exported neighborhood"
    )]
    id: i64,
    #[arg(long, help = "Snapshot id to inspect")]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 2, help = "Maximum graph traversal depth")]
    depth: i64,
    #[arg(
        long,
        default_value = "both",
        help = "Traversal direction: referents, referrers, or both"
    )]
    direction: String,
    #[arg(long, default_value_t = 500, help = "Maximum graph nodes to export")]
    node_limit: i64,
    #[arg(long, default_value_t = 2000, help = "Maximum graph edges to export")]
    edge_limit: i64,
    #[arg(long, value_enum, default_value_t = GraphFormat::Json, help = "Graph payload format")]
    graph_format: GraphFormat,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
    Jsonl,
    Dot,
}

#[derive(Debug, Args)]
struct WebArgs {
    #[arg(value_name = "DB", help = "SQLite analysis database to serve")]
    db: PathBuf,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Host interface to bind; keep 127.0.0.1 for local-only use"
    )]
    host: String,
    #[arg(
        long,
        default_value_t = 0,
        help = "Port to bind; 0 asks the OS for a free port"
    )]
    port: u16,
    #[arg(long, help = "Do not open a browser; print the URL instead")]
    no_browser: bool,
    #[arg(
        long,
        help = "Open the React dev server and let it proxy /api to this server"
    )]
    dev: bool,
    #[arg(
        long,
        requires = "dev",
        default_value = "http://127.0.0.1:5173/",
        help = "React dev server URL used with --dev"
    )]
    dev_server_url: String,
}

#[derive(Debug, Args)]
struct OpenArgs {
    #[arg(
        required = true,
        value_name = "DUMPS",
        help = "One or more gzip JSONL dump files or HTTP(S) dump URLs from pygco_dump"
    )]
    dumps: Vec<String>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Use an explicit session directory instead of the user cache root"
    )]
    session_dir: Option<PathBuf>,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Host interface to bind; keep 127.0.0.1 for local-only use"
    )]
    host: String,
    #[arg(
        long,
        default_value_t = 0,
        help = "Port to bind; 0 asks the OS for a free port"
    )]
    port: u16,
    #[arg(long, help = "Do not open a browser; print the URL instead")]
    no_browser: bool,
    #[arg(
        long,
        help = "Open the React dev server and let it proxy /api to this server"
    )]
    dev: bool,
    #[arg(
        long,
        requires = "dev",
        default_value = "http://127.0.0.1:5173/",
        help = "React dev server URL used with --dev"
    )]
    dev_server_url: String,
    #[arg(long, help = "Delete the session directory after the server exits")]
    cleanup_on_exit: bool,
    #[arg(
        long,
        help = "Include import phase timings in import.log and JSON summaries"
    )]
    profile: bool,
    #[arg(
        long,
        value_enum,
        default_value_t = ProgressArg::Auto,
        help = "Import progress on stderr: auto, always, or never"
    )]
    progress: ProgressArg,
    #[arg(
        long = "header",
        value_name = "KEY=VALUE",
        help = "HTTP request header for URL dumps; repeat for multiple headers. Secret values are not logged."
    )]
    headers: Vec<String>,
    #[arg(
        long,
        default_value_t = 30,
        help = "HTTP request timeout in seconds for URL dumps"
    )]
    timeout: u64,
    #[arg(
        long,
        value_name = "BYTES",
        help = "Maximum response bytes per URL dump, for example 100mb"
    )]
    max_bytes: Option<String>,
}

#[tokio::main]
async fn main() {
    let no_color = std::env::args_os().any(|arg| arg == "--no-color");
    let mut command = Cli::command();
    if no_color {
        command = command.color(ColorChoice::Never);
    }
    let cli = Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|error| error.exit());
    let verbose = cli.verbose;
    let _no_color = cli.no_color;
    if let Err(error) = run(cli).await {
        let classified = classify_error(&error);
        print_error(&error, classified, verbose);
        std::process::exit(classified.exit_code);
    }
}

#[derive(Debug, Clone, Copy)]
struct ClassifiedError {
    code: &'static str,
    exit_code: i32,
}

fn classify_error(error: &anyhow::Error) -> ClassifiedError {
    for cause in error.chain() {
        if let Some(import_error) = cause.downcast_ref::<ImportError>() {
            return match import_error {
                ImportError::DumpFormat(_) => ClassifiedError {
                    code: "dump_format_error",
                    exit_code: 10,
                },
                _ => ClassifiedError {
                    code: "import_failed",
                    exit_code: 11,
                },
            };
        }
        if cause
            .downcast_ref::<pygco_analysis::AnalysisError>()
            .is_some()
            || cause.downcast_ref::<pygco_store::StoreError>().is_some()
            || cause.downcast_ref::<rusqlite::Error>().is_some()
        {
            return ClassifiedError {
                code: "query_failed",
                exit_code: 20,
            };
        }
    }
    if error
        .to_string()
        .contains("provide exactly one of --from or --to")
    {
        return ClassifiedError {
            code: "argument_error",
            exit_code: 2,
        };
    }
    ClassifiedError {
        code: "internal_error",
        exit_code: 70,
    }
}

fn print_error(error: &anyhow::Error, classified: ClassifiedError, verbose: bool) {
    if verbose {
        eprintln!(
            "pygco: error: code={} exit_code={} message={error}",
            classified.code, classified.exit_code
        );
        eprintln!("details:");
        for (index, cause) in error.chain().enumerate() {
            eprintln!("  {index}: {cause}");
        }
    } else {
        eprintln!("pygco: error: code={} message={error}", classified.code);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Open(args) => cmd_open(args).await,
        Command::Fetch(args) => cmd_fetch(args),
        Command::Import(args) => cmd_import(args),
        Command::Sessions(args) => cmd_sessions(args),
        Command::Summary(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::summary(conn, args.snapshot, args.limit)?,
                args.output,
            )
        }),
        Command::Overview(args) => cmd_overview(args),
        Command::Objects(args) => cmd_objects(args),
        Command::Object(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::object_detail(conn, args.snapshot, args.id)?,
                args.output,
            )
        }),
        Command::Edges(args) => cmd_edges(args),
        Command::Paths(args) => with_conn(&args.db, |conn| {
            let annotate = args.annotate;
            let value = if args.annotate {
                pygco_analysis::annotated_paths(
                    conn,
                    args.snapshot,
                    args.id,
                    &args.direction,
                    args.depth,
                    args.fanout,
                    args.limit,
                )?
            } else {
                pygco_analysis::paths(
                    conn,
                    args.snapshot,
                    args.id,
                    &args.direction,
                    args.depth,
                    args.fanout,
                    args.limit,
                )?
            };
            emit_with_table_fields(
                value,
                args.output,
                annotate.then_some(ANNOTATED_PATHS_TABLE_FIELDS),
            )
        }),
        Command::Diff(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::diff(conn, args.from_snapshot, args.to_snapshot, args.limit)?,
                args.output,
            )
        }),
        Command::DiffObjects(args) => cmd_diff_objects(args),
        Command::Findings(args) => cmd_findings(args),
        Command::Suspects(args) => cmd_suspects(args),
        Command::Container(args) => cmd_container(args),
        Command::Idset(args) => cmd_idset(args),
        Command::Sql(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::readonly_sql(conn, &args.query, args.limit, args.explain)?,
                args.output,
            )
        }),
        Command::Schema(args) => with_conn(&args.db, |conn| {
            emit(pygco_analysis::schema_summary(conn)?, args.output)
        }),
        Command::ExportSubgraph(args) => cmd_export_subgraph(args),
        Command::Report(args) => with_conn(&args.db, |conn| match args.output.format {
            OutputFormat::Markdown => {
                println!("{}", pygco_report::build_markdown(conn, args.snapshot)?);
                Ok(())
            }
            _ => emit(pygco_report::build_json(conn, args.snapshot)?, args.output),
        }),
        Command::Doctor(args) => with_conn(&args.db, |conn| {
            emit(pygco_analysis::doctor(conn)?, args.output)
        }),
        Command::Web(args) | Command::Api(args) => serve_web(args).await,
        Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn cmd_sessions(args: SessionsArgs) -> anyhow::Result<()> {
    match args.command {
        SessionsCommand::List(args) => {
            let value = cache_sessions::list_sessions()?;
            match args.output.format {
                OutputFormat::Json => emit(value, args.output),
                OutputFormat::Jsonl | OutputFormat::Table | OutputFormat::Markdown => {
                    let rows = value
                        .get("sessions")
                        .cloned()
                        .unwrap_or_else(|| Value::Array(Vec::new()));
                    emit(rows, args.output)
                }
            }
        }
    }
}

fn cmd_import(args: ImportArgs) -> anyhow::Result<()> {
    let progress = args.progress.enabled();
    let options = ImportOptions {
        rebuild: args.rebuild,
        reachability_mode: if args.no_reachability
            || matches!(args.reachability_mode, ReachabilityModeArg::Off)
        {
            ReachabilityMode::Off
        } else {
            ReachabilityMode::Full
        },
        reachability_params: ReachabilityParams {
            algorithm_version: pygco_analysis::REACHABILITY_ALGORITHM_VERSION,
            depth: args.reachability_depth,
            node_limit: args.reachability_node_limit,
            fanout_limit: args.reachability_fanout_limit,
        },
        cohort_rules_path: args.rules,
        profile: args.profile,
    };
    progress_log(progress, "start");
    let summary = import_dumps(args.dumps, args.output, options)?;
    progress_log(progress, "finished");
    emit(serde_json::to_value(summary)?, args.output_args)
}

fn cmd_fetch(args: FetchArgs) -> anyhow::Result<()> {
    let max_bytes = parse_optional_byte_size(args.max_bytes.as_deref())?;
    let fetched = fetch_dump_url(
        &args.url,
        args.output_file,
        None,
        &args.headers,
        args.timeout,
        max_bytes,
    )?;
    emit(fetched.manifest, args.output)
}

async fn cmd_open(args: OpenArgs) -> anyhow::Result<()> {
    let progress = args.progress.enabled();
    let session = match args.session_dir {
        Some(session_dir) => cache_sessions::SessionPaths::explicit(session_dir),
        None => cache_sessions::SessionPaths::new_default()?,
    };
    fs::create_dir_all(&session.session_dir)?;
    let max_bytes = parse_optional_byte_size(args.max_bytes.as_deref())?;
    let (dumps, fetched_sources) = localize_open_dumps(
        &args.dumps,
        &session.session_dir,
        &args.headers,
        args.timeout,
        max_bytes,
    )?;
    let options = ImportOptions {
        rebuild: true,
        profile: args.profile,
        ..ImportOptions::default()
    };
    progress_log(progress, "start");
    let summary = import_dumps(
        dumps.clone(),
        session.database_path.clone(),
        options.clone(),
    )?;
    progress_log(progress, "finished");
    fs::write(
        &session.import_log_path,
        serde_json::to_string_pretty(&summary)?,
    )?;
    cache_sessions::write_manifest_with_fetched_sources(
        &session,
        &dumps,
        &summary,
        &options,
        &fetched_sources,
    )?;
    let result = serve_web(WebArgs {
        db: session.database_path.clone(),
        host: args.host,
        port: args.port,
        no_browser: args.no_browser,
        dev: args.dev,
        dev_server_url: args.dev_server_url,
    })
    .await;
    if args.cleanup_on_exit {
        let _ = fs::remove_dir_all(&session.session_dir);
    }
    result
}

fn progress_log(enabled: bool, message: &str) {
    if enabled {
        eprintln!("pygco import: {message}");
    }
}

#[derive(Debug, Clone)]
struct FetchedDump {
    local_path: PathBuf,
    manifest: Value,
}

fn localize_open_dumps(
    inputs: &[String],
    session_dir: &Path,
    headers: &[String],
    timeout_secs: u64,
    max_bytes: Option<u64>,
) -> anyhow::Result<(Vec<PathBuf>, Vec<Value>)> {
    let mut dumps = Vec::new();
    let mut fetched_sources = Vec::new();
    let download_dir = session_dir.join("downloads");
    for input in inputs {
        if is_url(input) {
            let fetched = fetch_dump_url(
                input,
                None,
                Some(&download_dir),
                headers,
                timeout_secs,
                max_bytes,
            )?;
            dumps.push(fetched.local_path);
            fetched_sources.push(fetched.manifest);
        } else {
            dumps.push(PathBuf::from(input));
        }
    }
    Ok((dumps, fetched_sources))
}

fn fetch_dump_url(
    url: &str,
    output_file: Option<PathBuf>,
    output_dir: Option<&Path>,
    headers: &[String],
    timeout_secs: u64,
    max_bytes: Option<u64>,
) -> anyhow::Result<FetchedDump> {
    let url = url.to_owned();
    let output_dir = output_dir.map(Path::to_path_buf);
    let headers = headers.to_vec();
    std::thread::spawn(move || {
        fetch_dump_url_blocking(
            &url,
            output_file,
            output_dir.as_deref(),
            &headers,
            timeout_secs,
            max_bytes,
        )
    })
    .join()
    .map_err(|_| anyhow!("fetch worker panicked"))?
}

fn fetch_dump_url_blocking(
    url: &str,
    output_file: Option<PathBuf>,
    output_dir: Option<&Path>,
    headers: &[String],
    timeout_secs: u64,
    max_bytes: Option<u64>,
) -> anyhow::Result<FetchedDump> {
    if !is_url(url) {
        return Err(anyhow!("fetch expects an http or https URL"));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("build HTTP client")?;
    let mut request = client.get(url);
    for header in headers {
        let (name, value) = parse_header_arg(header)?;
        request = request.header(name, value);
    }
    let mut response = request
        .send()
        .with_context(|| format!("fetch {}", redact_url(url)))?;
    let status = response.status();
    let final_url = response.url().to_string();
    if !status.is_success() {
        return Err(anyhow!(
            "fetch {} failed with HTTP status {status}",
            redact_url(url)
        ));
    }
    let header_filename = response
        .headers()
        .get(CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(content_disposition_filename);
    let local_path = output_file.unwrap_or_else(|| {
        let filename = header_filename
            .or_else(|| filename_from_url(&final_url))
            .unwrap_or_else(|| "dump.jsonl.gz".to_owned());
        output_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join(sanitize_filename(&filename))
    });
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    let mut file = File::create(&local_path)
        .with_context(|| format!("create fetched dump {}", local_path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .with_context(|| format!("read {}", redact_url(url)))?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| anyhow!("download is too large"))?;
        if let Some(max_bytes) = max_bytes {
            if bytes > max_bytes {
                let _ = fs::remove_file(&local_path);
                return Err(anyhow!("fetch {} exceeded --max-bytes", redact_url(url)));
            }
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .with_context(|| format!("write fetched dump {}", local_path.display()))?;
    }
    let sha256 = format!("{:x}", hasher.finalize());
    let manifest = json!({
        "source": {
            "original_url": redact_url(url),
            "final_url": redact_url(&final_url),
        },
        "local_path": local_path.display().to_string(),
        "filename": local_path.file_name().and_then(|value| value.to_str()).unwrap_or("dump.jsonl.gz"),
        "sha256": sha256,
        "bytes": bytes,
        "fetched_at": Utc::now().to_rfc3339(),
    });
    Ok(FetchedDump {
        local_path,
        manifest,
    })
}

fn is_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn parse_header_arg(header: &str) -> anyhow::Result<(HeaderName, HeaderValue)> {
    let Some((name, value)) = header.split_once('=') else {
        return Err(anyhow!("invalid --header; expected KEY=VALUE"));
    };
    let name = HeaderName::from_bytes(name.trim().as_bytes())
        .map_err(|_| anyhow!("invalid --header name"))?;
    let value = HeaderValue::from_str(value.trim())
        .map_err(|_| anyhow!("invalid --header value for {name}"))?;
    Ok((name, value))
}

fn content_disposition_filename(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(filename) = part.strip_prefix("filename=") {
            return Some(filename.trim_matches('"').to_owned());
        }
    }
    None
}

fn filename_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed
        .path_segments()?
        .rfind(|segment| !segment.is_empty())
        .map(str::to_owned)
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "dump.jsonl.gz".to_owned()
    } else {
        sanitized
    }
}

fn redact_url(url: &str) -> String {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return "<redacted-url>".to_owned();
    };
    let host = parsed.host_str().unwrap_or("<unknown>");
    let port = if parsed.port().is_some() {
        ":<redacted>"
    } else {
        ""
    };
    let query = if parsed.query().is_some() {
        "?<redacted>"
    } else {
        ""
    };
    format!(
        "{}://{}{}{}{}",
        parsed.scheme(),
        host,
        port,
        parsed.path(),
        query
    )
}

fn cmd_objects(args: ObjectsArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        emit(
            pygco_analysis::list_objects(
                conn,
                ObjectFilters {
                    snapshot_id: args.snapshot,
                    q: args.q,
                    type_name: args.type_name,
                    module: args.module,
                    cohort: args.cohort,
                    min_shallow_size: args.min_shallow_size,
                    min_reachable_size: args.min_reachable_size,
                    min_in_edges: args.min_in_edges,
                    min_out_edges: args.min_out_edges,
                    has_referrers: args.has_referrers,
                    missing_referents: args.missing_referents,
                    stub: args.stub,
                    sort: args.sort,
                    order: args.order,
                    limit: args.limit,
                    offset: args.offset,
                },
            )?,
            args.output,
        )
    })
}

fn cmd_overview(args: OverviewArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        let mut value =
            pygco_analysis::overview(conn, args.snapshot, args.limit, args.with_suspects)?;
        rewrite_next_command_db(&mut value, &args.db);
        emit_with_table_fields(value, args.output, Some(OVERVIEW_TABLE_FIELDS))
    })
}

fn cmd_edges(args: EdgesArgs) -> anyhow::Result<()> {
    let (object_id, direction) = match (args.from_id, args.to_id) {
        (Some(id), None) => (id, "referents"),
        (None, Some(id)) => (id, "referrers"),
        _ => return Err(anyhow!("provide exactly one of --from or --to")),
    };
    with_conn(&args.db, |conn| {
        emit(
            pygco_analysis::object_edges(
                conn,
                args.snapshot,
                object_id,
                direction,
                args.limit,
                args.offset,
            )?,
            args.output,
        )
    })
}

fn cmd_diff_objects(args: DiffObjectsArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        let value = pygco_analysis::diff_objects(
            conn,
            DiffObjectsOptions {
                from_snapshot_id: args.from_snapshot,
                to_snapshot_id: args.to_snapshot,
                state: args.state,
                type_name: args.type_name,
                module: args.module,
                limit: args.limit,
                offset: args.offset,
            },
        )?;
        if args.ids_only {
            if let Some(rows) = value.get("rows").and_then(Value::as_array) {
                for row in rows {
                    if let Some(id) = row.get("object_id").and_then(Value::as_str) {
                        println!("{id}");
                    }
                }
            }
            Ok(())
        } else {
            emit(value, args.output)
        }
    })
}

fn cmd_findings(args: FindingsArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        emit_with_table_fields(
            pygco_analysis::findings(
                conn,
                FindingsOptions {
                    snapshot_id: args.snapshot,
                    kind: args.kind,
                    severity: args.severity,
                    limit: args.limit,
                    offset: args.offset,
                },
            )?,
            args.output,
            Some(FINDINGS_TABLE_FIELDS),
        )
    })
}

fn cmd_suspects(args: SuspectsArgs) -> anyhow::Result<()> {
    let min_reachable_size = parse_byte_size(&args.min_reachable)?;
    with_conn(&args.db, |conn| {
        let mut value = pygco_analysis::suspects(
            conn,
            SuspectsOptions {
                snapshot_id: args.snapshot,
                kinds: args.kinds,
                min_reachable_size,
                non_builtin: args.non_builtin,
                include_stub: args.include_stub,
                limit: args.limit,
                offset: args.offset,
            },
        )?;
        rewrite_next_command_db(&mut value, &args.db);
        emit_with_table_fields(value, args.output, Some(SUSPECTS_TABLE_FIELDS))
    })
}

fn cmd_container(args: ContainerArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        let mut value = pygco_analysis::container_facts(
            conn,
            args.snapshot,
            args.id,
            args.top_items,
            args.item_types,
            args.limit,
        )?;
        rewrite_next_command_db(&mut value, &args.db);
        emit_with_table_fields(value, args.output, Some(CONTAINER_TABLE_FIELDS))
    })
}

fn cmd_idset(args: IdsetArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        let sid = pygco_store::resolve_snapshot_id(conn, args.snapshot)?;
        let value = pygco_analysis::idset(
            conn,
            sid,
            &args.left_query,
            &args.right_query,
            &args.op,
            args.details,
            args.limit,
        )?;
        if args.ids_only {
            let ids = value
                .get("object_ids")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for id in ids {
                println!("{}", id.as_str().unwrap_or_default());
            }
            Ok(())
        } else {
            emit(value, args.output)
        }
    })
}

fn cmd_export_subgraph(args: ExportSubgraphArgs) -> anyhow::Result<()> {
    with_conn(&args.db, |conn| {
        let graph = pygco_analysis::subgraph(
            conn,
            args.snapshot,
            args.id,
            &args.direction,
            args.depth,
            args.node_limit,
            args.edge_limit,
        )?;
        match args.graph_format {
            GraphFormat::Dot => {
                print!("{}", pygco_analysis::export_subgraph_dot(&graph));
                Ok(())
            }
            GraphFormat::Jsonl => {
                emit_jsonl(&graph);
                Ok(())
            }
            GraphFormat::Json => emit(graph, args.output),
        }
    })
}

async fn serve_web(args: WebArgs) -> anyhow::Result<()> {
    let port = if args.dev && args.port == 0 {
        5174
    } else {
        args.port
    };
    let addr: SocketAddr = format!("{}:{}", args.host, port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual = listener.local_addr()?;
    let api_url = format!("http://{actual}/");
    let web_url = if args.dev {
        ensure_trailing_slash(&args.dev_server_url)
    } else {
        api_url.clone()
    };
    println!("pygco web: {web_url}");
    if args.dev {
        println!("api server: {api_url}");
    }
    println!("database: {}", args.db.display());
    if !args.no_browser {
        let _ = webbrowser::open(&web_url);
    }
    axum::serve(listener, pygco_api::app(args.db)).await?;
    Ok(())
}

fn ensure_trailing_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    }
}

fn parse_byte_size(value: &str) -> anyhow::Result<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("byte size cannot be empty"));
    }
    let lower = trimmed.to_ascii_lowercase();
    let (number, multiplier) = if let Some(number) = lower.strip_suffix("kib") {
        (number, 1024_i64)
    } else if let Some(number) = lower.strip_suffix("kb") {
        (number, 1024_i64)
    } else if let Some(number) = lower.strip_suffix('k') {
        (number, 1024_i64)
    } else if let Some(number) = lower.strip_suffix("mib") {
        (number, 1024_i64 * 1024)
    } else if let Some(number) = lower.strip_suffix("mb") {
        (number, 1024_i64 * 1024)
    } else if let Some(number) = lower.strip_suffix('m') {
        (number, 1024_i64 * 1024)
    } else if let Some(number) = lower.strip_suffix("gib") {
        (number, 1024_i64 * 1024 * 1024)
    } else if let Some(number) = lower.strip_suffix("gb") {
        (number, 1024_i64 * 1024 * 1024)
    } else if let Some(number) = lower.strip_suffix('g') {
        (number, 1024_i64 * 1024 * 1024)
    } else if let Some(number) = lower.strip_suffix('b') {
        (number, 1_i64)
    } else {
        (lower.as_str(), 1_i64)
    };
    let integer = number
        .trim()
        .parse::<i64>()
        .map_err(|_| anyhow!("invalid byte size: {value}"))?;
    if integer < 0 {
        return Err(anyhow!("byte size must be non-negative: {value}"));
    }
    integer
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow!("byte size is too large: {value}"))
}

fn parse_optional_byte_size(value: Option<&str>) -> anyhow::Result<Option<u64>> {
    value
        .map(parse_byte_size)
        .transpose()?
        .map(|size| {
            u64::try_from(size).map_err(|_| anyhow!("byte size must be non-negative: {size}"))
        })
        .transpose()
}

fn rewrite_next_command_db(value: &mut Value, db: &Path) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(command)) = object.get_mut("next_command") {
                *command = command.replace(
                    " DB ",
                    &format!(" {} ", shell_quote_arg(&db.to_string_lossy())),
                );
            }
            for nested in object.values_mut() {
                rewrite_next_command_db(nested, db);
            }
        }
        Value::Array(rows) => {
            for row in rows {
                rewrite_next_command_db(row, db);
            }
        }
        _ => {}
    }
}

fn shell_quote_arg(value: &str) -> String {
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':')
    }) {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn with_conn(
    db: &Path,
    f: impl FnOnce(&rusqlite::Connection) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let conn = pygco_store::connect(db).with_context(|| format!("open sqlite {}", db.display()))?;
    f(&conn)
}

fn emit(value: Value, output: OutputArgs) -> anyhow::Result<()> {
    emit_with_table_fields(value, output, None)
}

fn emit_with_table_fields(
    value: Value,
    output: OutputArgs,
    default_table_fields: Option<&[&str]>,
) -> anyhow::Result<()> {
    let explicit_fields = parse_fields(output.fields.as_deref());
    let table_fields = if output.format == OutputFormat::Table {
        explicit_fields
            .clone()
            .or_else(|| default_table_fields.map(fields_to_strings))
    } else {
        explicit_fields.clone()
    };
    let value = apply_field_list(value, table_fields.as_deref());
    match output.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&value)?),
        OutputFormat::Jsonl => emit_jsonl(&value),
        OutputFormat::Table => emit_table(&value, table_fields.as_deref()),
        OutputFormat::Markdown => emit_markdown(&value),
    }
    Ok(())
}

fn parse_fields(fields: Option<&str>) -> Option<Vec<String>> {
    let fields = fields?;
    let fields: Vec<String> = fields
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn fields_to_strings(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|field| (*field).to_owned()).collect()
}

fn apply_field_list(value: Value, fields: Option<&[String]>) -> Value {
    let Some(fields) = fields else {
        return value;
    };
    if fields.is_empty() {
        return value;
    }
    match value {
        Value::Array(rows) => Value::Array(
            rows.into_iter()
                .map(|row| select_fields(row, fields))
                .collect(),
        ),
        Value::Object(mut object) => {
            if let Some(rows) = object.remove("rows") {
                object.insert("rows".to_owned(), apply_field_list(rows, Some(fields)));
                Value::Object(object)
            } else {
                select_fields(Value::Object(object), fields)
            }
        }
        other => other,
    }
}

fn select_fields(value: Value, fields: &[String]) -> Value {
    let Value::Object(object) = value else {
        return value;
    };
    let mut selected = Map::new();
    for field in fields {
        if let Some(value) = get_field_path(&Value::Object(object.clone()), field) {
            selected.insert(field.clone(), value.clone());
        }
    }
    Value::Object(selected)
}

fn get_field_path<'a>(value: &'a Value, field: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in field.split('.') {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn emit_jsonl(value: &Value) {
    if let Some(rows) = value.get("rows").and_then(Value::as_array) {
        for row in rows {
            println!(
                "{}",
                serde_json::to_string(row).unwrap_or_else(|_| "{}".to_owned())
            );
        }
    } else if let Some(rows) = value.as_array() {
        for row in rows {
            println!(
                "{}",
                serde_json::to_string(row).unwrap_or_else(|_| "{}".to_owned())
            );
        }
    } else {
        println!(
            "{}",
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned())
        );
    }
}

fn emit_table(value: &Value, columns_hint: Option<&[String]>) {
    let rows = value
        .get("rows")
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    let Some(rows) = rows else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        );
        return;
    };
    let objects = rows.iter().filter_map(Value::as_object).collect::<Vec<_>>();
    if objects.is_empty() {
        return;
    }
    let mut columns = columns_hint
        .map(|columns| columns.to_vec())
        .unwrap_or_default();
    if columns.is_empty() {
        for object in &objects {
            for key in object.keys() {
                if !columns.iter().any(|column| column == key) {
                    columns.push(key.clone());
                }
            }
        }
        columns.sort();
    }
    let rendered = objects
        .iter()
        .map(|object| {
            columns
                .iter()
                .map(|column| object.get(column).map(compact).unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let widths = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            rendered
                .iter()
                .map(|row| row[index].chars().count())
                .max()
                .unwrap_or(0)
                .max(column.chars().count())
        })
        .collect::<Vec<_>>();
    print_table_row(&columns, &widths, None);
    let separator = widths
        .iter()
        .map(|width| "-".repeat(*width))
        .collect::<Vec<_>>();
    print_table_row(&separator, &widths, None);
    for row in rendered {
        print_table_row(&row, &widths, Some(&columns));
    }
}

fn print_table_row(values: &[String], widths: &[usize], columns: Option<&[String]>) {
    let cells = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let width = widths[index];
            if columns
                .and_then(|columns| columns.get(index))
                .map(|column| is_numeric_column(column))
                .unwrap_or(false)
            {
                format!("{value:>width$}")
            } else {
                format!("{value:<width$}")
            }
        })
        .collect::<Vec<_>>();
    println!("{}", cells.join("  "));
}

fn is_numeric_column(column: &str) -> bool {
    column.ends_with("_size")
        || column.ends_with("_bytes")
        || column.ends_with("_count")
        || column.ends_with("_edges")
        || matches!(column, "limit" | "offset" | "total" | "rank" | "depth")
}

fn emit_markdown(value: &Value) {
    println!("```json");
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
    println!("```");
}

fn compact(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}
