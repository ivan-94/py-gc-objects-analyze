default:
    just test

fmt:
    cargo fmt --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --workspace
    python3 -m pytest python/pygco_dump

web:
    pnpm --dir web/app build

fixtures:
    python3 fixtures/generators/generate_golden.py
    python3 fixtures/generators/generate_synthetic.py --all

synthetic objects='10000' fanout='3':
    python3 fixtures/generators/generate_synthetic.py --objects {{objects}} --fanout {{fanout}} --output fixtures/synthetic/synthetic-{{objects}}-f{{fanout}}.jsonl.gz

bench-import dump='fixtures/golden/tiny-v1.jsonl.gz':
    python3 benches/import_benchmark.py --dump {{dump}}

bench-query db='.scratch/bench.sqlite':
    python3 benches/query_api_benchmark.py --db {{db}}

bench-memory:
    python3 benches/memory_benchmark.py --dump fixtures/synthetic/medium.jsonl.gz --dump fixtures/synthetic/large.jsonl.gz --output benches/reports/import-memory.json

bench-smoke:
    mkdir -p .scratch
    python3 fixtures/generators/generate_synthetic.py --objects 200 --fanout 2 --output .scratch/bench-smoke.jsonl.gz
    cargo build -p pygco-cli
    python3 benches/import_benchmark.py --dump .scratch/bench-smoke.jsonl.gz --keep-db .scratch/bench-smoke.sqlite
    python3 benches/query_api_benchmark.py --db .scratch/bench-smoke.sqlite --iterations 3

docs-generated:
    cargo build -p pygco-cli
    python3 scripts/generate_cli_docs.py
    python3 scripts/export_openapi.py

docs-check:
    cargo build -p pygco-cli
    python3 scripts/check_docs_commands.py
