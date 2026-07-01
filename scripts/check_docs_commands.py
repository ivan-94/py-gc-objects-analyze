from __future__ import annotations

import argparse
import re
import shlex
import subprocess
from pathlib import Path


DOC_GLOBS = ["README.md", "CHANGELOG.md", "docs/**/*.md", "python/**/*.md"]
SKIP_PATH_PARTS = {".pytest_cache", "docs/generated"}
SHELL_FENCE_LANGS = {"bash", "console", "sh", "shell"}


def main() -> None:
    parser = argparse.ArgumentParser(description="Check pygco commands documented in markdown")
    parser.add_argument("--pygco", default="target/debug/pygco")
    args = parser.parse_args()

    commands, global_flags = command_surface(args.pygco)
    errors: list[str] = []
    for path in markdown_files():
        for line_number, line in pygco_lines(path):
            errors.extend(validate_line(commands, global_flags, path, line_number, line))

    if errors:
        raise SystemExit("stale documented pygco command(s):\n" + "\n".join(errors))


def command_surface(pygco: str) -> tuple[dict[str, set[str]], set[str]]:
    root_help = help_text(pygco, [])
    commands = {command: set() for command in parse_commands(root_help)}
    global_flags = parse_long_flags(root_help)
    for command in list(commands):
        commands[command] = parse_long_flags(help_text(pygco, [command]))
    return commands, global_flags


def help_text(pygco: str, command: list[str]) -> str:
    return subprocess.run(
        [pygco, *command, "--help"],
        check=True,
        text=True,
        capture_output=True,
    ).stdout


def parse_commands(help_output: str) -> set[str]:
    commands: set[str] = set()
    in_commands = False
    for raw_line in help_output.splitlines():
        line = raw_line.rstrip()
        if line == "Commands:":
            in_commands = True
            continue
        if in_commands and (line == "Options:" or not line.strip()):
            break
        if in_commands:
            command = line.strip().split(maxsplit=1)[0]
            if command != "help":
                commands.add(command)
    return commands


def parse_long_flags(help_output: str) -> set[str]:
    return set(re.findall(r"(?<![\w-])--[a-z0-9][a-z0-9-]*", help_output))


def markdown_files() -> list[Path]:
    files: set[Path] = set()
    for pattern in DOC_GLOBS:
        for path in Path(".").glob(pattern):
            if not path.is_file():
                continue
            if any(part in SKIP_PATH_PARTS for part in path.parts):
                continue
            files.add(path)
    return sorted(files)


def pygco_lines(path: Path) -> list[tuple[int, str]]:
    lines: list[tuple[int, str]] = []
    in_shell_fence = False
    continued_line: str | None = None
    continued_start = 0
    for index, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = raw_line.strip()
        if stripped.startswith("```"):
            if in_shell_fence and continued_line:
                lines.append((continued_start, continued_line))
            continued_line = None
            continued_start = 0
            if in_shell_fence:
                in_shell_fence = False
            else:
                fence_info = stripped.removeprefix("```").strip().split(maxsplit=1)
                lang = fence_info[0] if fence_info else ""
                in_shell_fence = lang in SHELL_FENCE_LANGS
            continue
        if not in_shell_fence:
            continue
        line = stripped.removeprefix("$ ").strip()
        if continued_line is not None:
            continued_line = f"{continued_line} {line.removesuffix('\\').strip()}".strip()
            if not line.endswith("\\"):
                lines.append((continued_start, continued_line))
                continued_line = None
                continued_start = 0
            continue
        if line.startswith("pygco "):
            if line.endswith("\\"):
                continued_line = line.removesuffix("\\").strip()
                continued_start = index
            else:
                lines.append((index, line))
    return lines


def validate_line(
    commands: dict[str, set[str]],
    global_flags: set[str],
    path: Path,
    line_number: int,
    line: str,
) -> list[str]:
    try:
        tokens = shlex.split(line)
    except ValueError as error:
        return [f"{path}:{line_number}: cannot parse shell line {line!r}: {error}"]

    if not tokens or tokens[0] != "pygco":
        return []

    command = next((token for token in tokens[1:] if not token.startswith("-")), None)
    if command is None:
        return []
    if command not in commands:
        return [f"{path}:{line_number}: unknown pygco command `{command}` in `{line}`"]

    allowed_flags = global_flags | commands[command]
    errors: list[str] = []
    for token in tokens[1:]:
        if token.startswith("--"):
            flag = token.split("=", 1)[0]
            if flag not in allowed_flags:
                errors.append(f"{path}:{line_number}: unknown flag `{flag}` for `pygco {command}` in `{line}`")
    return errors


if __name__ == "__main__":
    main()
