from __future__ import annotations

import argparse
import subprocess
from pathlib import Path

COMMANDS = [
    [],
    ["open"],
    ["import"],
    ["summary"],
    ["objects"],
    ["object"],
    ["edges"],
    ["paths"],
    ["diff"],
    ["diff-objects"],
    ["findings"],
    ["suspects"],
    ["idset"],
    ["sql"],
    ["schema"],
    ["export-subgraph"],
    ["report"],
    ["doctor"],
    ["web"],
    ["api"],
    ["version"],
]


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate CLI help docs from pygco clap output")
    parser.add_argument("--pygco", default="target/debug/pygco")
    parser.add_argument("--output", type=Path, default=Path("docs/generated/cli-help.md"))
    args = parser.parse_args()

    args.output.parent.mkdir(parents=True, exist_ok=True)
    sections = [
        "# Generated CLI Help",
        "",
        "## Source Manifest",
        "",
        f"- Generator: `scripts/generate_cli_docs.py --pygco {args.pygco}`",
        "- Clap source: `crates/pygco-cli/src/main.rs`",
        "- Contract: `docs/cli.md`",
        "",
        "Do not edit command help text in this file by hand; regenerate it from the binary.",
        "",
    ]
    for command in COMMANDS:
        title = "pygco" if not command else f"pygco {' '.join(command)}"
        sections.extend([f"## `{title}`", "", "```text", help_text(args.pygco, command), "```", ""])
    args.output.write_text("\n".join(sections), encoding="utf-8")


def help_text(pygco: str, command: list[str]) -> str:
    result = subprocess.run(
        [pygco, *command, "--help"],
        check=True,
        text=True,
        capture_output=True,
    )
    return result.stdout.strip()


if __name__ == "__main__":
    main()
