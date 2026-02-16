from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify archive manifest")
    parser.add_argument("--manifest", default="archive/manifest.json")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    manifest_path = Path(args.manifest)
    if not manifest_path.exists():
        print("manifest not found")
        return
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    entries = manifest.get("entries") or []

    ok = 0
    failed = 0
    for entry in entries:
        path = Path(entry.get("file", ""))
        if not path.exists():
            failed += 1
            print(f"missing: {path}")
            continue
        size = path.stat().st_size
        sha256 = _hash_file(path)
        if size != entry.get("size_bytes") or sha256 != entry.get("sha256"):
            failed += 1
            print(f"mismatch: {path}")
        else:
            ok += 1

    print(f"verify_done ok={ok} failed={failed}")


def _hash_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


if __name__ == "__main__":
    main()
