#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path


def render_release_notes(changelog: Path, version: str) -> str:
    text = changelog.read_text(encoding="utf-8")
    text = text.replace("## 0.1.0 - Unreleased", f"## {version}", 1)
    text = text.replace("pygco-0.1.0-", f"pygco-{version}-")
    return text


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
