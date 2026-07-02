# Generated CLI Help

Do not edit command help text in this file by hand; regenerate it from the binary.

## `pygco`

```text
Local Python GC object memory forensics

Usage: pygco [OPTIONS] <COMMAND>

Commands:
  open             Import dumps into a cache session and serve the local Web UI
  fetch            Download a dump URL to a local file with hashing and redacted source metadata
  import           Import dumps into an explicit SQLite analysis database
  sessions         Inspect cached analysis sessions created by `pygco open`
  summary          Show snapshot overview, top types/modules/cohorts, warnings, and findings
  overview         Compact leak triage entrypoint with quality, top cohorts, and next commands
  objects          List objects with filters, sorting, pagination, and agent-friendly projections
  object           Show one object's metadata, metrics, direct edges, and next investigation commands
  edges            List direct referents or referrers for one object
  paths            Sample bounded owner/reference paths around an object
  diff             Compare aggregate changes between two snapshots
  diff-objects     Compare object lifecycle rows between two snapshots
  findings         List persisted diagnostic findings produced during import
  suspects         Generate heuristic memory investigation leads without writing SQL
  container        Explain direct contents of common containers such as deque, queue, cache, dict, list, and set
  idset            Run set operations over two read-only object-id SQL queries
  sql              Run read-only SQL or EXPLAIN QUERY PLAN against the analysis database
  schema           Print SQLite schema summary for query planning and agent discovery
  export-subgraph  Export a bounded object neighborhood as JSON, JSONL, or DOT
  report           Generate a human-readable or JSON memory forensics report
  doctor           Check database health, schema version, indexes, and snapshot availability
  web              Serve the Web UI for an existing SQLite analysis database
  api              Serve the local API for an existing SQLite analysis database
  version          Print the pygco CLI version
  help             Print this message or the help of the given subcommand(s)

Options:
      --no-color  Disable ANSI color in errors and help output
      --verbose   Print detailed error chains for debugging and agent logs
  -h, --help      Print help
  -V, --version   Print version

Typical workflows:
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
SQLite analysis files are rebuildable cache artifacts; keep the source dump files for durable evidence.
```

## `pygco open`

```text
Import dumps into a cache session and serve the local Web UI

Usage: pygco open [OPTIONS] <DUMPS>...

Arguments:
  <DUMPS>...  One or more gzip JSONL dump files or HTTP(S) dump URLs from pygco_dump

Options:
      --no-color
          Disable ANSI color in errors and help output
      --session-dir <DIR>
          Use an explicit session directory instead of the user cache root
      --host <HOST>
          Host interface to bind; keep 127.0.0.1 for local-only use [default: 127.0.0.1]
      --verbose
          Print detailed error chains for debugging and agent logs
      --port <PORT>
          Port to bind; 0 asks the OS for a free port [default: 0]
      --no-browser
          Do not open a browser; print the URL instead
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          React dev server URL used with --dev [default: http://127.0.0.1:5173/]
      --cleanup-on-exit
          Delete the session directory after the server exits
      --profile
          Include import phase timings in import.log and JSON summaries
      --progress <PROGRESS>
          Import progress on stderr: auto, always, or never [default: auto] [possible values: auto, always, never]
      --header <KEY=VALUE>
          HTTP request header for URL dumps; repeat for multiple headers. Secret values are not logged.
      --timeout <TIMEOUT>
          HTTP request timeout in seconds for URL dumps [default: 30]
      --max-bytes <BYTES>
          Maximum response bytes per URL dump, for example 100mb
  -h, --help
          Print help

Examples:
  pygco open dump.jsonl.gz
  pygco open before.jsonl.gz after.jsonl.gz --no-browser
  pygco open dump.jsonl.gz --session-dir .pygco/sessions/manual --cleanup-on-exit

Notes:
  Without --session-dir, sessions are stored under PYGCO_HOME, XDG_CACHE_HOME/pygco, or ~/.cache/pygco.
  The session contains analysis.sqlite, import.log, and manifest.json.
  Use `pygco sessions list` to discover cache sessions later.
```

## `pygco fetch`

```text
Download a dump URL to a local file with hashing and redacted source metadata

Usage: pygco fetch [OPTIONS] <URL>

Arguments:
  <URL>  HTTP or HTTPS dump URL to download

Options:
      --no-color            Disable ANSI color in errors and help output
  -o, --output <PATH>       Local output dump path; defaults to a filename inferred from HTTP headers or URL
      --header <KEY=VALUE>  HTTP request header; repeat for multiple headers. Secret values are not logged.
      --verbose             Print detailed error chains for debugging and agent logs
      --timeout <TIMEOUT>   HTTP request timeout in seconds [default: 30]
      --max-bytes <BYTES>   Maximum response bytes, for example 100mb
      --format <FORMAT>     Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>     Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                Print help

Examples:
  pygco fetch https://example.com/gc-heap-dump -o dump.jsonl.gz
  pygco fetch https://example.com/gc-heap-dump --header Authorization=Bearer... --format json

Fetch records original/final URL in redacted form and never prints secret header values.
```

## `pygco import`

```text
Import dumps into an explicit SQLite analysis database

Usage: pygco import [OPTIONS] --output <SQLITE> <DUMPS>...

Arguments:
  <DUMPS>...  One or more gzip JSONL dump files from pygco_dump

Options:
      --no-color
          Disable ANSI color in errors and help output
  -o, --output <SQLITE>
          Output SQLite analysis database path
      --rebuild
          Replace the output database if it already exists
      --verbose
          Print detailed error chains for debugging and agent logs
      --no-reachability
          Skip reachable-size computation for faster shallow imports
      --reachability-mode <REACHABILITY_MODE>
          Reachability computation mode [default: full] [possible values: full, off]
      --reachability-depth <REACHABILITY_DEPTH>
          Maximum depth for bounded reachable-size estimation [default: 3]
      --reachability-node-limit <REACHABILITY_NODE_LIMIT>
          Maximum nodes visited per reachable-size computation [default: 10000]
      --reachability-fanout-limit <REACHABILITY_FANOUT_LIMIT>
          Maximum outgoing edges explored per node during reachable-size estimation [default: 1000]
      --rules <TOML>
          Optional cohort rules TOML file for cache/async/connection classification
      --profile
          Include import phase timings in the JSON output and import log
      --progress <PROGRESS>
          Import progress on stderr: auto, always, or never [default: auto] [possible values: auto, always, never]
      --format <FORMAT>
          Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>
          Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help
          Print help

Examples:
  pygco import dump.jsonl.gz -o analysis.sqlite --rebuild
  pygco import before.jsonl.gz after.jsonl.gz -o comparison.sqlite --rebuild --profile
  pygco import dump.jsonl.gz -o fast.sqlite --rebuild --no-reachability

Writes a fresh SQLite analysis database for CLI, API, or Web UI commands.
Use --no-reachability for faster shallow analysis when reachable-size sorting is not needed.
```

## `pygco sessions`

```text
Inspect cached analysis sessions created by `pygco open`

Usage: pygco sessions [OPTIONS] <COMMAND>

Commands:
  list  List cached analysis sessions
  help  Print this message or the help of the given subcommand(s)

Options:
      --no-color  Disable ANSI color in errors and help output
      --verbose   Print detailed error chains for debugging and agent logs
  -h, --help      Print help
```

## `pygco sessions list`

```text
List cached analysis sessions

Usage: pygco sessions list [OPTIONS]

Options:
      --format <FORMAT>  Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --no-color         Disable ANSI color in errors and help output
      --fields <FIELDS>  Comma-separated projection for row/object fields, for example object_id,type,shallow_size
      --verbose          Print detailed error chains for debugging and agent logs
  -h, --help             Print help

Examples:
  pygco sessions list --format table
  pygco sessions list --format json --fields id,status,size_bytes,database_path

Cache root order:
  1. PYGCO_HOME
  2. XDG_CACHE_HOME/pygco
  3. ~/.cache/pygco

Statuses:
  status=ready means analysis.sqlite and manifest.json are present.
  status=missing-db, missing-manifest, or invalid-manifest marks an incomplete cache session.
```

## `pygco summary`

```text
Show snapshot overview, top types/modules/cohorts, warnings, and findings

Usage: pygco summary [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database produced by `pygco import` or `pygco open`

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to query; defaults to the latest/only snapshot when supported
      --limit <LIMIT>        Maximum rows or top-N entries returned by commands that support limits [default: 20]
      --verbose              Print detailed error chains for debugging and agent logs
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco summary analysis.sqlite --format table
  pygco summary analysis.sqlite --snapshot 2 --limit 30 --format json

Useful first check:
  Confirm object_count, edge_count, top type/module growth, missing referents, and warnings.
```

## `pygco overview`

```text
Compact leak triage entrypoint with quality, top cohorts, and next commands

Usage: pygco overview [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database produced by `pygco import` or `pygco open`

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to query; defaults to the latest/only snapshot
      --limit <LIMIT>        Maximum rows per overview section [default: 20]
      --verbose              Print detailed error chains for debugging and agent logs
      --with-suspects        Run heavier suspects analysis inside overview
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco overview analysis.sqlite --snapshot 1 --format table
  pygco overview analysis.sqlite --snapshot 1 --with-suspects --format json

By default overview avoids heavy suspect queries and prints the next command to run them.
```

## `pygco objects`

```text
List objects with filters, sorting, pagination, and agent-friendly projections

Usage: pygco objects [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --no-color                    Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>         Snapshot id to inspect
      --q <TEXT>                    Case-insensitive search over type, module, qualname, repr, and labels
      --verbose                     Print detailed error chains for debugging and agent logs
      --type <TYPE>                 Filter by exact or pattern-like Python type name
      --module <MODULE>             Filter by module name
      --cohort <COHORT>             Filter by analysis cohort such as cache-heavy or async-backlog
      --min-shallow-size <BYTES>    Minimum shallow size in bytes
      --min-reachable-size <BYTES>  Minimum estimated reachable size in bytes
      --min-in-edges <N>            Minimum incoming edge count
      --min-out-edges <N>           Minimum outgoing edge count
      --has-referrers               Only include objects with at least one referrer
      --missing-referents           Only include objects with missing referent records
      --stub <STUB>                 Filter stub objects: true for stubs, false for non-stubs [possible values: true, false]
      --sort <SORT>                 Sort key such as reachable-size, shallow-size, in-edges, out-edges, object-id, type, or module [default: reachable-size]
      --order <ORDER>               Sort order: asc or desc [default: desc]
      --limit <LIMIT>               Maximum object rows to return [default: 100]
      --offset <OFFSET>             Pagination offset [default: 0]
      --format <FORMAT>             Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>             Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                        Print help

Examples:
  pygco objects analysis.sqlite --sort reachable-size --limit 20 --format table
  pygco objects analysis.sqlite --type dict --min-reachable-size 1mb --format json
  pygco objects analysis.sqlite --fields object_id,type,shallow_size,reachable_size --format jsonl

Sort keys:
  reachable-size, shallow-size, in-edges, out-edges, object-id, type, module.

Agent tip:
  Use --fields object_id,type,shallow_size,reachable_size,next_command to keep outputs compact.
```

## `pygco object`

```text
Show one object's metadata, metrics, direct edges, and next investigation commands

Usage: pygco object [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --id <ID>              Object id to inspect
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to inspect
      --verbose              Print detailed error chains for debugging and agent logs
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco object analysis.sqlite --id 281470886362416 --format json
  pygco object analysis.sqlite --snapshot 2 --id 100 --format markdown

Object ids are emitted as strings in JSON to preserve JavaScript precision.
```

## `pygco edges`

```text
List direct referents or referrers for one object

Usage: pygco edges [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --from <OBJECT_ID>     List referents of this object id
      --no-color             Disable ANSI color in errors and help output
      --to <OBJECT_ID>       List referrers pointing to this object id
      --verbose              Print detailed error chains for debugging and agent logs
      --snapshot <SNAPSHOT>  Snapshot id to inspect
      --limit <LIMIT>        Maximum edge rows to return [default: 100]
      --offset <OFFSET>      Pagination offset [default: 0]
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco edges analysis.sqlite --from 100 --limit 50 --format table
  pygco edges analysis.sqlite --to 100 --snapshot 2 --format json

Use exactly one of --from or --to.
```

## `pygco paths`

```text
Sample bounded owner/reference paths around an object

Usage: pygco paths [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --id <ID>                Root object id for bounded path sampling
      --no-color               Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>    Snapshot id to inspect
      --verbose                Print detailed error chains for debugging and agent logs
      --direction <DIRECTION>  Path direction: referrers, referents, or both when supported [default: referrers]
      --depth <DEPTH>          Maximum path depth [default: 5]
      --fanout <FANOUT>        Maximum branches sampled per node [default: 30]
      --limit <LIMIT>          Maximum paths to return [default: 50]
      --annotate               Annotate every path node with object summary and diagnostic facts
      --format <FORMAT>        Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>        Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                   Print help

Examples:
  pygco paths analysis.sqlite --id 100 --direction referrers --depth 5 --fanout 30 --format json
  pygco paths analysis.sqlite --id 100 --direction referents --limit 20 --format table

This is a bounded exploration helper, not an exhaustive graph traversal.
```

## `pygco diff`

```text
Compare aggregate changes between two snapshots

Usage: pygco diff [OPTIONS] --from <SNAPSHOT_ID> --to <SNAPSHOT_ID> <DB>

Arguments:
  <DB>  SQLite analysis database containing both snapshots

Options:
      --from <SNAPSHOT_ID>  Baseline snapshot id
      --no-color            Disable ANSI color in errors and help output
      --to <SNAPSHOT_ID>    Comparison snapshot id
      --verbose             Print detailed error chains for debugging and agent logs
      --limit <LIMIT>       Maximum rows per diff section [default: 100]
      --format <FORMAT>     Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>     Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                Print help

Examples:
  pygco diff analysis.sqlite --from 1 --to 2 --format markdown
  pygco diff analysis.sqlite --from 1 --to 2 --limit 50 --format json

Best for before/after dump pairs imported into the same SQLite database.
```

## `pygco diff-objects`

```text
Compare object lifecycle rows between two snapshots

Usage: pygco diff-objects [OPTIONS] --from <SNAPSHOT_ID> --to <SNAPSHOT_ID> <DB>

Arguments:
  <DB>  SQLite analysis database containing both snapshots

Options:
      --from <SNAPSHOT_ID>  Baseline snapshot id
      --no-color            Disable ANSI color in errors and help output
      --to <SNAPSHOT_ID>    Comparison snapshot id
      --verbose             Print detailed error chains for debugging and agent logs
      --state <STATE>       Lifecycle state: new, gone, retained, or changed when supported [default: new]
      --type <TYPE>         Filter lifecycle rows by type
      --module <MODULE>     Filter lifecycle rows by module
      --limit <LIMIT>       Maximum object rows to return [default: 100]
      --offset <OFFSET>     Pagination offset [default: 0]
      --ids-only            Print only object ids, one per line
      --format <FORMAT>     Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>     Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                Print help

Examples:
  pygco diff-objects analysis.sqlite --from 1 --to 2 --state new --format table
  pygco diff-objects analysis.sqlite --from 1 --to 2 --state retained --type dict --ids-only

Object-level lifecycle confidence is highest for dumps from the same Python process run.
```

## `pygco findings`

```text
List persisted diagnostic findings produced during import

Usage: pygco findings [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to inspect
      --kind <KIND>          Finding kind filter, for example large-type
      --verbose              Print detailed error chains for debugging and agent logs
      --severity <SEVERITY>  Severity filter such as info, warn, or error
      --limit <LIMIT>        Maximum finding rows to return [default: 100]
      --offset <OFFSET>      Pagination offset [default: 0]
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco findings analysis.sqlite --snapshot 1 --format table
  pygco findings analysis.sqlite --kind large-type --severity warn --format json

Findings are leads, not final conclusions. Use object/path commands to verify.
```

## `pygco suspects`

```text
Generate heuristic memory investigation leads without writing SQL

Usage: pygco suspects [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --no-color                       Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>            Snapshot id to inspect
      --kind <KIND>                    Suspect kind; repeat for multiple kinds
      --verbose                        Print detailed error chains for debugging and agent logs
      --min-reachable <MIN_REACHABLE>  Minimum estimated reachable size, for example 100b, 1mb, 2gb [default: 1mb]
      --non-builtin                    Prefer non-builtin/module-owned suspects
      --include-stub                   Include stub objects in suspect generation
      --limit <LIMIT>                  Maximum suspect rows to return [default: 20]
      --offset <OFFSET>                Pagination offset [default: 0]
      --format <FORMAT>                Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>                Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                           Print help

Examples:
  pygco suspects analysis.sqlite --snapshot 1 --kind orphan-retained --min-reachable 1mb --format table
  pygco suspects analysis.sqlite --kind cache --kind async --kind connection --format json

Kinds include orphan-retained, high-retained-root, truncated-root, type-footprint,
metadata-heavy, cache-heavy, async-backlog, and connection-heavy.
```

## `pygco container`

```text
Explain direct contents of common containers such as deque, queue, cache, dict, list, and set

Usage: pygco container [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --id <ID>              Container object id to inspect
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to inspect
      --verbose              Print detailed error chains for debugging and agent logs
      --top-items            Include largest direct referent items
      --item-types           Include direct referent type aggregation
      --limit <LIMIT>        Maximum item rows per section [default: 20]
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco container analysis.sqlite --id 281470886362416 --top-items --item-types --format table
  pygco container analysis.sqlite --snapshot 1 --id 100 --format json

Container analysis uses direct referents from the dump graph. Field names, dict keys, and queue internals require richer dump data.
```

## `pygco idset`

```text
Run set operations over two read-only object-id SQL queries

Usage: pygco idset [OPTIONS] --left-query <SQL> --right-query <SQL> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to apply to details output when relevant
      --left-query <SQL>     Left read-only SQL query returning object_id
      --verbose              Print detailed error chains for debugging and agent logs
      --right-query <SQL>    Right read-only SQL query returning object_id
      --op <OP>              Set operation: intersect, union, left-only, or right-only [default: intersect]
      --details              Return object details for the resulting id set
      --limit <LIMIT>        Maximum ids or detail rows to return [default: 1000]
      --ids-only             Print only object ids, one per line
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco idset analysis.sqlite --left-query 'select object_id from objects' --right-query 'select to_id as object_id from edges' --op intersect --details
  pygco idset analysis.sqlite --snapshot 1 --op left-only --ids-only

Both SQL queries must return an object_id column.
```

## `pygco sql`

```text
Run read-only SQL or EXPLAIN QUERY PLAN against the analysis database

Usage: pygco sql [OPTIONS] --query <SQL> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --no-color         Disable ANSI color in errors and help output
  -q, --query <SQL>      Read-only SQL query to run
      --limit <LIMIT>    Maximum result rows to return [default: 1000]
      --verbose          Print detailed error chains for debugging and agent logs
      --explain          Return SQLite EXPLAIN QUERY PLAN output instead of query rows
      --format <FORMAT>  Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>  Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help             Print help

Examples:
  pygco sql analysis.sqlite --query 'select type, count(*) from objects group by type order by count(*) desc limit 20' --format table
  pygco sql analysis.sqlite --query 'select object_id from objects limit 10' --explain --format json

Read-only SQL workbench:
  Only SELECT-style read queries are accepted.
  Use --explain to inspect SQLite query plans before expensive probes.
```

## `pygco schema`

```text
Print SQLite schema summary for query planning and agent discovery

Usage: pygco schema [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database produced by `pygco import` or `pygco open`

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to query; defaults to the latest/only snapshot when supported
      --limit <LIMIT>        Maximum rows or top-N entries returned by commands that support limits [default: 20]
      --verbose              Print detailed error chains for debugging and agent logs
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco schema analysis.sqlite --format table
  pygco schema analysis.sqlite --format json

Use this before writing ad hoc SQL against unfamiliar databases.
```

## `pygco export-subgraph`

```text
Export a bounded object neighborhood as JSON, JSONL, or DOT

Usage: pygco export-subgraph [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  SQLite analysis database

Options:
      --id <ID>                      Root object id for the exported neighborhood
      --no-color                     Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>          Snapshot id to inspect
      --verbose                      Print detailed error chains for debugging and agent logs
      --depth <DEPTH>                Maximum graph traversal depth [default: 2]
      --direction <DIRECTION>        Traversal direction: referents, referrers, or both [default: both]
      --node-limit <NODE_LIMIT>      Maximum graph nodes to export [default: 500]
      --edge-limit <EDGE_LIMIT>      Maximum graph edges to export [default: 2000]
      --graph-format <GRAPH_FORMAT>  Graph payload format [default: json] [possible values: json, jsonl, dot]
      --format <FORMAT>              Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>              Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                         Print help

Examples:
  pygco export-subgraph analysis.sqlite --id 100 --depth 2 --direction both --graph-format dot
  pygco export-subgraph analysis.sqlite --id 100 --node-limit 500 --edge-limit 2000 --format json

Use DOT for graph visualization and JSON for agent-side post-processing.
```

## `pygco report`

```text
Generate a human-readable or JSON memory forensics report

Usage: pygco report [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database produced by `pygco import` or `pygco open`

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to query; defaults to the latest/only snapshot when supported
      --limit <LIMIT>        Maximum rows or top-N entries returned by commands that support limits [default: 20]
      --verbose              Print detailed error chains for debugging and agent logs
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco report analysis.sqlite --snapshot 1 --format markdown
  pygco report analysis.sqlite --format json

Markdown reports are suitable for issue/PR/HAT artifacts.
```

## `pygco doctor`

```text
Check database health, schema version, indexes, and snapshot availability

Usage: pygco doctor [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database produced by `pygco import` or `pygco open`

Options:
      --no-color             Disable ANSI color in errors and help output
      --snapshot <SNAPSHOT>  Snapshot id to query; defaults to the latest/only snapshot when supported
      --limit <LIMIT>        Maximum rows or top-N entries returned by commands that support limits [default: 20]
      --verbose              Print detailed error chains for debugging and agent logs
      --format <FORMAT>      Output format: json for agents, jsonl for streams, table for humans, markdown for reports [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      Comma-separated projection for row/object fields, for example object_id,type,shallow_size
  -h, --help                 Print help

Examples:
  pygco doctor analysis.sqlite --format table
  pygco doctor analysis.sqlite --format json

Run this when a database fails to open, queries are unexpectedly slow, or a session looks incomplete.
```

## `pygco web`

```text
Serve the Web UI for an existing SQLite analysis database

Usage: pygco web [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database to serve

Options:
      --host <HOST>
          Host interface to bind; keep 127.0.0.1 for local-only use [default: 127.0.0.1]
      --no-color
          Disable ANSI color in errors and help output
      --port <PORT>
          Port to bind; 0 asks the OS for a free port [default: 0]
      --verbose
          Print detailed error chains for debugging and agent logs
      --no-browser
          Do not open a browser; print the URL instead
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          React dev server URL used with --dev [default: http://127.0.0.1:5173/]
  -h, --help
          Print help

Examples:
  pygco web analysis.sqlite --host 127.0.0.1 --port 3791
  pygco web analysis.sqlite --dev --no-browser

Use this after an explicit `pygco import -o` workflow.
```

## `pygco api`

```text
Serve the local API for an existing SQLite analysis database

Usage: pygco api [OPTIONS] <DB>

Arguments:
  <DB>  SQLite analysis database to serve

Options:
      --host <HOST>
          Host interface to bind; keep 127.0.0.1 for local-only use [default: 127.0.0.1]
      --no-color
          Disable ANSI color in errors and help output
      --port <PORT>
          Port to bind; 0 asks the OS for a free port [default: 0]
      --verbose
          Print detailed error chains for debugging and agent logs
      --no-browser
          Do not open a browser; print the URL instead
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          React dev server URL used with --dev [default: http://127.0.0.1:5173/]
  -h, --help
          Print help

Examples:
  pygco api analysis.sqlite --host 127.0.0.1 --port 5174 --no-browser

The API is local-first and binds to 127.0.0.1 by default.
```

## `pygco version`

```text
Print the pygco CLI version

Usage: pygco version [OPTIONS]

Options:
      --no-color  Disable ANSI color in errors and help output
      --verbose   Print detailed error chains for debugging and agent logs
  -h, --help      Print help
```
