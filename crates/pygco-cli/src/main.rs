use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use chrono::Utc;
use clap::{Args, ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use serde_json::{Map, Value};

use pygco_analysis::{
    DiffObjectsOptions, FindingsOptions, ObjectFilters, ReachabilityParams, SuspectsOptions,
};
use pygco_importer::{import_dumps, ImportError, ImportOptions, ReachabilityMode};

#[derive(Debug, Parser)]
#[command(
    name = "pygco",
    version,
    about = "Local Python GC object memory forensics"
)]
struct Cli {
    #[arg(long, global = true)]
    no_color: bool,
    #[arg(long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Open(OpenArgs),
    Import(ImportArgs),
    Summary(DbArgs),
    Objects(ObjectsArgs),
    Object(ObjectArgs),
    Edges(EdgesArgs),
    Paths(PathsArgs),
    Diff(DiffArgs),
    DiffObjects(DiffObjectsArgs),
    Findings(FindingsArgs),
    Suspects(SuspectsArgs),
    Idset(IdsetArgs),
    Sql(SqlArgs),
    Schema(DbArgs),
    ExportSubgraph(ExportSubgraphArgs),
    Report(DbArgs),
    Doctor(DbArgs),
    Web(WebArgs),
    Api(WebArgs),
    Version,
}

#[derive(Debug, Clone, Args)]
struct OutputArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long)]
    fields: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Jsonl,
    Table,
    Markdown,
}

#[derive(Debug, Args)]
struct ImportArgs {
    #[arg(required = true)]
    dumps: Vec<PathBuf>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    rebuild: bool,
    #[arg(long)]
    no_reachability: bool,
    #[arg(long, value_enum, default_value_t = ReachabilityModeArg::Full)]
    reachability_mode: ReachabilityModeArg,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_DEPTH)]
    reachability_depth: i64,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_NODE_LIMIT)]
    reachability_node_limit: i64,
    #[arg(long, default_value_t = pygco_analysis::DEFAULT_REACHABILITY_FANOUT_LIMIT)]
    reachability_fanout_limit: i64,
    #[arg(long)]
    rules: Option<PathBuf>,
    #[arg(long)]
    profile: bool,
    #[command(flatten)]
    output_args: OutputArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReachabilityModeArg {
    Full,
    Off,
}

#[derive(Debug, Args)]
struct DbArgs {
    db: PathBuf,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 20)]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ObjectsArgs {
    db: PathBuf,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long)]
    q: Option<String>,
    #[arg(long = "type")]
    type_name: Option<String>,
    #[arg(long)]
    module: Option<String>,
    #[arg(long)]
    cohort: Option<String>,
    #[arg(long)]
    min_shallow_size: Option<i64>,
    #[arg(long)]
    min_reachable_size: Option<i64>,
    #[arg(long)]
    min_in_edges: Option<i64>,
    #[arg(long)]
    min_out_edges: Option<i64>,
    #[arg(long)]
    has_referrers: bool,
    #[arg(long)]
    missing_referents: bool,
    #[arg(long)]
    stub: Option<bool>,
    #[arg(long, default_value = "reachable-size")]
    sort: String,
    #[arg(long, default_value = "desc")]
    order: String,
    #[arg(long, default_value_t = 100)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ObjectArgs {
    db: PathBuf,
    #[arg(long, alias = "object-id")]
    id: i64,
    #[arg(long)]
    snapshot: Option<i64>,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct EdgesArgs {
    db: PathBuf,
    #[arg(long = "from")]
    from_id: Option<i64>,
    #[arg(long = "to")]
    to_id: Option<i64>,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 100)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct PathsArgs {
    db: PathBuf,
    #[arg(long)]
    id: i64,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long, default_value = "referrers")]
    direction: String,
    #[arg(long, default_value_t = 5)]
    depth: i64,
    #[arg(long, default_value_t = 30)]
    fanout: i64,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct DiffArgs {
    db: PathBuf,
    #[arg(long = "from")]
    from_snapshot: i64,
    #[arg(long = "to")]
    to_snapshot: i64,
    #[arg(long, default_value_t = 100)]
    limit: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct DiffObjectsArgs {
    db: PathBuf,
    #[arg(long = "from")]
    from_snapshot: i64,
    #[arg(long = "to")]
    to_snapshot: i64,
    #[arg(long, default_value = "new")]
    state: String,
    #[arg(long = "type")]
    type_name: Option<String>,
    #[arg(long)]
    module: Option<String>,
    #[arg(long, default_value_t = 100)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
    #[arg(long)]
    ids_only: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct FindingsArgs {
    db: PathBuf,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long)]
    kind: Option<String>,
    #[arg(long)]
    severity: Option<String>,
    #[arg(long, default_value_t = 100)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct SuspectsArgs {
    db: PathBuf,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long = "kind")]
    kinds: Vec<String>,
    #[arg(long, default_value = "1mb")]
    min_reachable: String,
    #[arg(long)]
    non_builtin: bool,
    #[arg(long)]
    include_stub: bool,
    #[arg(long, default_value_t = 20)]
    limit: i64,
    #[arg(long, default_value_t = 0)]
    offset: i64,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct SqlArgs {
    db: PathBuf,
    #[arg(long, short)]
    query: String,
    #[arg(long, default_value_t = 1000)]
    limit: i64,
    #[arg(long)]
    explain: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct IdsetArgs {
    db: PathBuf,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long)]
    left_query: String,
    #[arg(long)]
    right_query: String,
    #[arg(long, default_value = "intersect")]
    op: String,
    #[arg(long)]
    details: bool,
    #[arg(long, default_value_t = 1000)]
    limit: i64,
    #[arg(long)]
    ids_only: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ExportSubgraphArgs {
    db: PathBuf,
    #[arg(long, alias = "object-id")]
    id: i64,
    #[arg(long)]
    snapshot: Option<i64>,
    #[arg(long, default_value_t = 2)]
    depth: i64,
    #[arg(long, default_value = "both")]
    direction: String,
    #[arg(long, default_value_t = 500)]
    node_limit: i64,
    #[arg(long, default_value_t = 2000)]
    edge_limit: i64,
    #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
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
    db: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long)]
    no_browser: bool,
    #[arg(
        long,
        help = "Open the React dev server and let it proxy /api to this server"
    )]
    dev: bool,
    #[arg(long, requires = "dev", default_value = "http://127.0.0.1:5173/")]
    dev_server_url: String,
}

#[derive(Debug, Args)]
struct OpenArgs {
    #[arg(required = true)]
    dumps: Vec<PathBuf>,
    #[arg(long)]
    session_dir: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long)]
    no_browser: bool,
    #[arg(
        long,
        help = "Open the React dev server and let it proxy /api to this server"
    )]
    dev: bool,
    #[arg(long, requires = "dev", default_value = "http://127.0.0.1:5173/")]
    dev_server_url: String,
    #[arg(long)]
    cleanup_on_exit: bool,
    #[arg(long)]
    profile: bool,
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
        Command::Import(args) => cmd_import(args),
        Command::Summary(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::summary(conn, args.snapshot, args.limit)?,
                args.output,
            )
        }),
        Command::Objects(args) => cmd_objects(args),
        Command::Object(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::object_detail(conn, args.snapshot, args.id)?,
                args.output,
            )
        }),
        Command::Edges(args) => cmd_edges(args),
        Command::Paths(args) => with_conn(&args.db, |conn| {
            emit(
                pygco_analysis::paths(
                    conn,
                    args.snapshot,
                    args.id,
                    &args.direction,
                    args.depth,
                    args.fanout,
                    args.limit,
                )?,
                args.output,
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

fn cmd_import(args: ImportArgs) -> anyhow::Result<()> {
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
    let summary = import_dumps(args.dumps, args.output, options)?;
    emit(serde_json::to_value(summary)?, args.output_args)
}

async fn cmd_open(args: OpenArgs) -> anyhow::Result<()> {
    let session_root = args.session_dir.unwrap_or_else(|| {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        PathBuf::from(".pygco").join("sessions").join(timestamp)
    });
    fs::create_dir_all(&session_root)?;
    let db = session_root.join("analysis.sqlite");
    let import_log = session_root.join("import.log");
    let summary = import_dumps(
        args.dumps,
        db.clone(),
        ImportOptions {
            rebuild: true,
            profile: args.profile,
            ..ImportOptions::default()
        },
    )?;
    fs::write(&import_log, serde_json::to_string_pretty(&summary)?)?;
    let result = serve_web(WebArgs {
        db: db.clone(),
        host: args.host,
        port: args.port,
        no_browser: args.no_browser,
        dev: args.dev,
        dev_server_url: args.dev_server_url,
    })
    .await;
    if args.cleanup_on_exit {
        let _ = fs::remove_dir_all(&session_root);
    }
    result
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
        emit(
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
        emit(value, args.output)
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
    let value = apply_fields(value, output.fields.as_deref());
    match output.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&value)?),
        OutputFormat::Jsonl => emit_jsonl(&value),
        OutputFormat::Table => emit_table(&value),
        OutputFormat::Markdown => emit_markdown(&value),
    }
    Ok(())
}

fn apply_fields(value: Value, fields: Option<&str>) -> Value {
    let Some(fields) = fields else {
        return value;
    };
    let fields: Vec<&str> = fields
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect();
    if fields.is_empty() {
        return value;
    }
    match value {
        Value::Array(rows) => Value::Array(
            rows.into_iter()
                .map(|row| select_fields(row, &fields))
                .collect(),
        ),
        Value::Object(mut object) => {
            if let Some(rows) = object.remove("rows") {
                object.insert(
                    "rows".to_owned(),
                    apply_fields(rows, Some(&fields.join(","))),
                );
                Value::Object(object)
            } else {
                select_fields(Value::Object(object), &fields)
            }
        }
        other => other,
    }
}

fn select_fields(value: Value, fields: &[&str]) -> Value {
    let Value::Object(object) = value else {
        return value;
    };
    let mut selected = Map::new();
    for field in fields {
        if let Some(value) = object.get(*field) {
            selected.insert((*field).to_owned(), value.clone());
        }
    }
    Value::Object(selected)
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

fn emit_table(value: &Value) {
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
    for row in rows {
        if let Some(object) = row.as_object() {
            let line = object
                .iter()
                .map(|(key, value)| format!("{key}={}", compact(value)))
                .collect::<Vec<_>>()
                .join("\t");
            println!("{line}");
        }
    }
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
