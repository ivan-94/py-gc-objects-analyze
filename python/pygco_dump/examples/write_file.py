from __future__ import annotations

from pathlib import Path

from pygco_dump import write_gc_dump


def main() -> None:
    with Path("heap.jsonl.gz").open("wb") as file:
        write_gc_dump(file, collect=False, include_repr=False)


if __name__ == "__main__":
    main()
