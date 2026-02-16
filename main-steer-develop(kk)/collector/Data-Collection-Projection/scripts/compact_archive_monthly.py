from __future__ import annotations

import argparse
import gzip
from collections import defaultdict
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Compact daily archives into monthly")
    parser.add_argument("--archive-dir", default="archive/raw")
    parser.add_argument("--output-dir", default="archive/monthly")
    parser.add_argument("--delete-after", action="store_true")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    archive_dir = Path(args.archive_dir)
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    groups = defaultdict(list)
    for path in archive_dir.glob("raw_*.jsonl.gz"):
        name = path.stem.replace("raw_", "")
        month = name[:7]  # YYYY-MM
        groups[month].append(path)

    for month, files in sorted(groups.items()):
        out_path = output_dir / f"raw_{month}.jsonl.gz"
        with gzip.open(out_path, "wt", encoding="utf-8") as out_f:
            for file in sorted(files):
                with gzip.open(file, "rt", encoding="utf-8") as in_f:
                    for line in in_f:
                        out_f.write(line)
        print(f"created {out_path} from {len(files)} files")
        if args.delete_after:
            for file in files:
                file.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
