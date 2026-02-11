from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Import n8n workflow via REST API")
    parser.add_argument("--file", default="logs/n8n_workflow.json", help="workflow json path")
    parser.add_argument("--api-url", default="", help="n8n base url (e.g., http://localhost:5678)")
    parser.add_argument("--user", default="", help="basic auth user")
    parser.add_argument("--password", default="", help="basic auth password")
    parser.add_argument("--api-key", default="", help="n8n api key (if enabled)")
    parser.add_argument("--activate", action="store_true", help="force active=true")
    parser.add_argument("--log", default="logs/n8n_import.log", help="import log path")
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


def _load_env_value(name: str) -> str:
    if os.getenv(name):
        return os.getenv(name, "")
    try:
        for parent in Path(__file__).resolve().parents:
            env_path = parent / ".env"
            if not env_path.exists():
                continue
            for line in env_path.read_text(encoding="utf-8").splitlines():
                if not line or line.strip().startswith("#"):
                    continue
                if "=" not in line:
                    continue
                key, value = line.split("=", 1)
                if key.strip() == name:
                    return value.strip()
    except Exception:
        return ""
    return ""


def _build_headers(user: str, password: str, api_key: str) -> dict[str, str]:
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["X-N8N-API-KEY"] = api_key
    if user and password:
        token = base64.b64encode(f"{user}:{password}".encode("utf-8")).decode("ascii")
        headers["Authorization"] = f"Basic {token}"
    return headers


def _request(url: str, payload: dict, headers: dict[str, str]) -> tuple[int, str]:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(url, data=body, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            status = getattr(resp, "status", 200)
            raw = resp.read().decode("utf-8", errors="ignore")
            return status, raw
    except urllib.error.HTTPError as exc:
        try:
            raw = exc.read().decode("utf-8", errors="ignore")
        except Exception:
            raw = ""
        return exc.code, raw


def _extract_id(raw: str) -> str:
    try:
        parsed = json.loads(raw)
    except Exception:
        return ""
    if isinstance(parsed, dict):
        if "id" in parsed:
            return str(parsed.get("id"))
        data = parsed.get("data")
        if isinstance(data, dict) and "id" in data:
            return str(data.get("id"))
    return ""


def main() -> None:
    args = parse_args()
    payload = _load_json(args.file)
    if args.activate:
        payload["active"] = True

    api_url = args.api_url or os.getenv("N8N_API_URL") or "http://localhost:5678"
    api_url = api_url.rstrip("/")

    user = args.user or os.getenv("N8N_BASIC_AUTH_USER") or _load_env_value("N8N_BASIC_AUTH_USER") or "admin"
    password = args.password or os.getenv("N8N_BASIC_AUTH_PASSWORD") or _load_env_value("N8N_BASIC_AUTH_PASSWORD") or "admin123"
    api_key = args.api_key or os.getenv("N8N_API_KEY") or _load_env_value("N8N_API_KEY")

    headers = _build_headers(user, password, api_key)
    log_path = Path(args.log)

    endpoints = [
        f"{api_url}/api/v1/workflows",
        f"{api_url}/rest/workflows",
    ]

    for endpoint in endpoints:
        status, raw = _request(endpoint, payload, headers)
        if 200 <= status < 300:
            workflow_id = _extract_id(raw)
            _append_line(
                log_path,
                {
                    "ts": _now_utc(),
                    "status": status,
                    "endpoint": endpoint,
                    "file": args.file,
                    "workflow_id": workflow_id,
                },
            )
            print(f"imported status={status} endpoint={endpoint} id={workflow_id}")
            return
        _append_line(
            log_path,
            {
                "ts": _now_utc(),
                "status": status,
                "endpoint": endpoint,
                "file": args.file,
                "error": raw[:2000],
            },
        )

    print("import_failed: both api/v1 and /rest endpoints rejected the request")
    sys.exit(1)


if __name__ == "__main__":
    main()
