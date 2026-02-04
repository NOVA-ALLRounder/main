from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Send n8n workflow JSON to a webhook endpoint")
    parser.add_argument("--file", default="n8n_workflow.json", help="workflow json path")
    parser.add_argument("--webhook", default="", help="n8n webhook url")
    return parser.parse_args()


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        raise FileNotFoundError(path)
    return json.loads(p.read_text(encoding="utf-8"))


def main() -> None:
    args = parse_args()
    webhook = args.webhook or os.getenv("N8N_WEBHOOK_URL", "")
    if not webhook:
        print("missing_webhook: set --webhook or N8N_WEBHOOK_URL")
        sys.exit(2)

    payload = _load_json(args.file)
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(
        webhook,
        data=body,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            status = getattr(resp, "status", 200)
            print(f"sent={args.file} status={status}")
    except Exception as exc:
        print(f"send_failed: {exc}")
        sys.exit(1)


if __name__ == "__main__":
    main()
