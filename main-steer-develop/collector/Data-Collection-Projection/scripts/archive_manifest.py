from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build archive manifest")
    parser.add_argument("--archive-dir", default="archive/raw")
    parser.add_argument("--include-monthly", action="store_true")
    parser.add_argument("--monthly-dir", default="archive/monthly")
    parser.add_argument("--output", default="archive/manifest.json")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    archive_dir = Path(args.archive_dir)
    archive_dir.mkdir(parents=True, exist_ok=True)

    entries = []
    for path in sorted(archive_dir.glob("raw_*.jsonl.gz")):
        entries.append(_entry(path, "raw"))

    if args.include_monthly:
        monthly_dir = Path(args.monthly_dir)
        monthly_dir.mkdir(parents=True, exist_ok=True)
        for path in sorted(monthly_dir.glob("raw_*.jsonl.gz")):
            entries.append(_entry(path, "monthly"))

    manifest = {
        "archive_dir": str(archive_dir),
        "monthly_dir": str(Path(args.monthly_dir)) if args.include_monthly else None,
        "count": len(entries),
        "entries": entries,
    }

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"manifest_saved={output_path}")


def _hash_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _entry(path: Path, kind: str) -> dict:
    size = path.stat().st_size
    sha256 = _hash_file(path)
    return {
        "file": str(path),
        "type": kind,
        "size_bytes": size,
        "sha256": sha256,
    }


if __name__ == "__main__":
    main()
