# Generated CLI Help

## Source Manifest

- Generator: `scripts/generate_cli_docs.py --pygco target/debug/pygco`
- Clap source: `crates/pygco-cli/src/main.rs`
- Contract: `docs/cli.md`

Do not edit command help text in this file by hand; regenerate it from the binary.

## `pygco`

```text
Local Python GC object memory forensics

Usage: pygco [OPTIONS] <COMMAND>

Commands:
  open             
  import           
  summary          
  objects          
  object           
  edges            
  paths            
  diff             
  diff-objects     
  findings         
  suspects         
  idset            
  sql              
  schema           
  export-subgraph  
  report           
  doctor           
  web              
  api              
  version          
  help             Print this message or the help of the given subcommand(s)

Options:
      --no-color  
      --verbose   
  -h, --help      Print help
  -V, --version   Print version
```

## `pygco open`

```text
Usage: pygco open [OPTIONS] <DUMPS>...

Arguments:
  <DUMPS>...  

Options:
      --no-color
          
      --session-dir <SESSION_DIR>
          
      --host <HOST>
          [default: 127.0.0.1]
      --verbose
          
      --port <PORT>
          [default: 0]
      --no-browser
          
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          [default: http://127.0.0.1:5173/]
      --cleanup-on-exit
          
      --profile
          
  -h, --help
          Print help
```

## `pygco import`

```text
Usage: pygco import [OPTIONS] --output <OUTPUT> <DUMPS>...

Arguments:
  <DUMPS>...  

Options:
      --no-color
          
  -o, --output <OUTPUT>
          
      --rebuild
          
      --verbose
          
      --no-reachability
          
      --reachability-mode <REACHABILITY_MODE>
          [default: full] [possible values: full, off]
      --reachability-depth <REACHABILITY_DEPTH>
          [default: 3]
      --reachability-node-limit <REACHABILITY_NODE_LIMIT>
          [default: 10000]
      --reachability-fanout-limit <REACHABILITY_FANOUT_LIMIT>
          [default: 1000]
      --rules <RULES>
          
      --profile
          
      --format <FORMAT>
          [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>
          
  -h, --help
          Print help
```

## `pygco summary`

```text
Usage: pygco summary [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color             
      --snapshot <SNAPSHOT>  
      --limit <LIMIT>        [default: 20]
      --verbose              
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco objects`

```text
Usage: pygco objects [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color
          
      --snapshot <SNAPSHOT>
          
      --q <Q>
          
      --verbose
          
      --type <TYPE_NAME>
          
      --module <MODULE>
          
      --cohort <COHORT>
          
      --min-shallow-size <MIN_SHALLOW_SIZE>
          
      --min-reachable-size <MIN_REACHABLE_SIZE>
          
      --min-in-edges <MIN_IN_EDGES>
          
      --min-out-edges <MIN_OUT_EDGES>
          
      --has-referrers
          
      --missing-referents
          
      --stub <STUB>
          [possible values: true, false]
      --sort <SORT>
          [default: reachable-size]
      --order <ORDER>
          [default: desc]
      --limit <LIMIT>
          [default: 100]
      --offset <OFFSET>
          [default: 0]
      --format <FORMAT>
          [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>
          
  -h, --help
          Print help
```

## `pygco object`

```text
Usage: pygco object [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  

Options:
      --id <ID>              
      --no-color             
      --snapshot <SNAPSHOT>  
      --verbose              
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco edges`

```text
Usage: pygco edges [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --from <FROM_ID>       
      --no-color             
      --to <TO_ID>           
      --verbose              
      --snapshot <SNAPSHOT>  
      --limit <LIMIT>        [default: 100]
      --offset <OFFSET>      [default: 0]
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco paths`

```text
Usage: pygco paths [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  

Options:
      --id <ID>                
      --no-color               
      --snapshot <SNAPSHOT>    
      --verbose                
      --direction <DIRECTION>  [default: referrers]
      --depth <DEPTH>          [default: 5]
      --fanout <FANOUT>        [default: 30]
      --limit <LIMIT>          [default: 50]
      --format <FORMAT>        [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>        
  -h, --help                   Print help
```

## `pygco diff`

```text
Usage: pygco diff [OPTIONS] --from <FROM_SNAPSHOT> --to <TO_SNAPSHOT> <DB>

Arguments:
  <DB>  

Options:
      --from <FROM_SNAPSHOT>  
      --no-color              
      --to <TO_SNAPSHOT>      
      --verbose               
      --limit <LIMIT>         [default: 100]
      --format <FORMAT>       [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>       
  -h, --help                  Print help
```

## `pygco diff-objects`

```text
Usage: pygco diff-objects [OPTIONS] --from <FROM_SNAPSHOT> --to <TO_SNAPSHOT> <DB>

Arguments:
  <DB>  

Options:
      --from <FROM_SNAPSHOT>  
      --no-color              
      --to <TO_SNAPSHOT>      
      --verbose               
      --state <STATE>         [default: new]
      --type <TYPE_NAME>      
      --module <MODULE>       
      --limit <LIMIT>         [default: 100]
      --offset <OFFSET>       [default: 0]
      --ids-only              
      --format <FORMAT>       [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>       
  -h, --help                  Print help
```

## `pygco findings`

```text
Usage: pygco findings [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color             
      --snapshot <SNAPSHOT>  
      --kind <KIND>          
      --verbose              
      --severity <SEVERITY>  
      --limit <LIMIT>        [default: 100]
      --offset <OFFSET>      [default: 0]
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco suspects`

```text
Usage: pygco suspects [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color                       
      --snapshot <SNAPSHOT>            
      --kind <KINDS>                   
      --verbose                        
      --min-reachable <MIN_REACHABLE>  [default: 1mb]
      --non-builtin                    
      --include-stub                   
      --limit <LIMIT>                  [default: 20]
      --offset <OFFSET>                [default: 0]
      --format <FORMAT>                [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>                
  -h, --help                           Print help
```

## `pygco idset`

```text
Usage: pygco idset [OPTIONS] --left-query <LEFT_QUERY> --right-query <RIGHT_QUERY> <DB>

Arguments:
  <DB>  

Options:
      --no-color                   
      --snapshot <SNAPSHOT>        
      --left-query <LEFT_QUERY>    
      --verbose                    
      --right-query <RIGHT_QUERY>  
      --op <OP>                    [default: intersect]
      --details                    
      --limit <LIMIT>              [default: 1000]
      --ids-only                   
      --format <FORMAT>            [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>            
  -h, --help                       Print help
```

## `pygco sql`

```text
Usage: pygco sql [OPTIONS] --query <QUERY> <DB>

Arguments:
  <DB>  

Options:
      --no-color         
  -q, --query <QUERY>    
      --limit <LIMIT>    [default: 1000]
      --verbose          
      --explain          
      --format <FORMAT>  [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>  
  -h, --help             Print help
```

## `pygco schema`

```text
Usage: pygco schema [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color             
      --snapshot <SNAPSHOT>  
      --limit <LIMIT>        [default: 20]
      --verbose              
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco export-subgraph`

```text
Usage: pygco export-subgraph [OPTIONS] --id <ID> <DB>

Arguments:
  <DB>  

Options:
      --id <ID>                      
      --no-color                     
      --snapshot <SNAPSHOT>          
      --verbose                      
      --depth <DEPTH>                [default: 2]
      --direction <DIRECTION>        [default: both]
      --node-limit <NODE_LIMIT>      [default: 500]
      --edge-limit <EDGE_LIMIT>      [default: 2000]
      --graph-format <GRAPH_FORMAT>  [default: json] [possible values: json, jsonl, dot]
      --format <FORMAT>              [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>              
  -h, --help                         Print help
```

## `pygco report`

```text
Usage: pygco report [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color             
      --snapshot <SNAPSHOT>  
      --limit <LIMIT>        [default: 20]
      --verbose              
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco doctor`

```text
Usage: pygco doctor [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --no-color             
      --snapshot <SNAPSHOT>  
      --limit <LIMIT>        [default: 20]
      --verbose              
      --format <FORMAT>      [default: json] [possible values: json, jsonl, table, markdown]
      --fields <FIELDS>      
  -h, --help                 Print help
```

## `pygco web`

```text
Usage: pygco web [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --host <HOST>
          [default: 127.0.0.1]
      --no-color
          
      --port <PORT>
          [default: 0]
      --verbose
          
      --no-browser
          
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          [default: http://127.0.0.1:5173/]
  -h, --help
          Print help
```

## `pygco api`

```text
Usage: pygco api [OPTIONS] <DB>

Arguments:
  <DB>  

Options:
      --host <HOST>
          [default: 127.0.0.1]
      --no-color
          
      --port <PORT>
          [default: 0]
      --verbose
          
      --no-browser
          
      --dev
          Open the React dev server and let it proxy /api to this server
      --dev-server-url <DEV_SERVER_URL>
          [default: http://127.0.0.1:5173/]
  -h, --help
          Print help
```

## `pygco version`

```text
Usage: pygco version [OPTIONS]

Options:
      --no-color  
      --verbose   
  -h, --help      Print help
```
