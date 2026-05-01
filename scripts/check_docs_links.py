#!/usr/bin/env python3
"""Check local Markdown links without requiring network access."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlparse


ROOT = Path(__file__).resolve().parents[1]
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*]\(([^)]+)\)")
EXTERNAL_SCHEMES = {"http", "https", "mailto"}


def markdown_files() -> list[Path]:
    return sorted(
        path
        for path in ROOT.rglob("*.md")
        if ".git" not in path.parts and "target" not in path.parts
    )


def strip_title(target: str) -> str:
    target = target.strip()
    if not target:
        return target
    if target[0] in {"'", '"'}:
        return target
    return target.split()[0]


def local_path(source: Path, target: str) -> Path | None:
    target = strip_title(target)
    parsed = urlparse(target)
    if parsed.scheme in EXTERNAL_SCHEMES or target.startswith("#"):
        return None
    if parsed.scheme:
        return None

    path_text = unquote(parsed.path)
    if not path_text:
        return None

    candidate = Path(path_text)
    if not candidate.is_absolute():
        candidate = source.parent / candidate
    return candidate.resolve()


def main() -> int:
    failures: list[str] = []

    for source in markdown_files():
        text = source.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK.finditer(text):
            target = match.group(1)
            candidate = local_path(source, target)
            if candidate is None:
                continue
            if not candidate.exists():
                failures.append(
                    f"{source.relative_to(ROOT)}: missing link target {target!r}"
                )

    if failures:
        print("Markdown link check failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print(f"Checked local links in {len(markdown_files())} Markdown files.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
