from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Send n8n workflow JSON to a webhook endpoint")
    parser.add_argument("--file", default="n8n_workflow.json", help="workflow json path")
    parser.add_argument("--webhook", default="", help="n8n webhook url")
    parser.add_argument("--log", default="logs/n8n_delivery.log", help="delivery log path")
    parser.add_argument("--retry", default="logs/n8n_delivery_retry.jsonl", help="retry queue path")
    return parser.parse_args()


def _load_json(path: str) -> dict:
    p = Path(path)
    if not p.exists():
        raise FileNotFoundError(path)
    return json.loads(p.read_text(encoding="utf-8"))


def _now_utc() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _append_line(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(payload, ensure_ascii=False) + "\n")


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

    log_path = Path(args.log)
    retry_path = Path(args.retry)

    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            status = getattr(resp, "status", 200)
            _append_line(
                log_path,
                {
                    "ts": _now_utc(),
                    "status": status,
                    "file": args.file,
                    "webhook": webhook,
                },
            )
            print(f"sent={args.file} status={status}")
    except Exception as exc:
        _append_line(
            log_path,
            {
                "ts": _now_utc(),
                "status": "error",
                "file": args.file,
                "webhook": webhook,
                "error": str(exc),
            },
        )
        _append_line(
            retry_path,
            {
                "ts": _now_utc(),
                "file": args.file,
                "webhook": webhook,
                "payload": payload,
                "error": str(exc),
            },
        )
        print(f"send_failed: {exc}")
        sys.exit(1)


if __name__ == "__main__":
    main()
