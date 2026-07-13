#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path


def render_release_notes(changelog: Path, version: str) -> str:
    text = changelog.read_text(encoding="utf-8")
    match = re.search(r"^## \d+\.\d+\.\d+ - Unreleased$", text, re.MULTILINE)
    if match is None:
        raise ValueError("CHANGELOG.md must start its pending release with a '## X.Y.Z - Unreleased' heading")

    next_heading = re.search(r"^## ", text[match.end() :], re.MULTILINE)
    end = match.end() + next_heading.start() if next_heading else len(text)
    return f"{text[:match.start()]}## {version}{text[match.end():end]}"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render release notes from CHANGELOG.md for a release version."
    )
    parser.add_argument("version", help="Release version without the leading v")
    parser.add_argument(
        "--changelog",
        type=Path,
        default=Path("CHANGELOG.md"),
        help="Path to the changelog template",
    )
    args = parser.parse_args()
    print(render_release_notes(args.changelog, args.version), end="")


if __name__ == "__main__":
    main()
