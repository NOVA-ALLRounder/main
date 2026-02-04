from __future__ import annotations

import argparse
import gzip
import json
import time
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path
from typing import Any, Dict, Iterable, List, Tuple


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Replay archived events to collector")
    parser.add_argument("--file", required=True, help="path to jsonl or jsonl.gz")
    parser.add_argument(
        "--endpoint",
        default="http://127.0.0.1:8080/events",
        help="collector /events endpoint",
    )
    parser.add_argument("--batch", type=int, default=1)
    parser.add_argument("--token", default="")
    return parser.parse_args()


def _chunks(events: List[Dict[str, Any]], batch: int) -> Iterable[List[Dict[str, Any]]]:
    if batch <= 1:
        for event in events:
            yield [event]
        return
    for idx in range(0, len(events), batch):
        yield events[idx : idx + batch]


def _send_payload(
    endpoint: str, payload: Any, token: str
) -> Tuple[bool, int, float, str]:
    data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    headers = {"Content-Type": "application/json"}
    if token:
        headers["X-Collector-Token"] = token
    request = urllib.request.Request(endpoint, data=data, headers=headers, method="POST")
    start = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            elapsed = time.perf_counter() - start
            ok = 200 <= response.status < 300
            return ok, response.status, elapsed, ""
    except urllib.error.HTTPError as exc:
        elapsed = time.perf_counter() - start
        return False, exc.code, elapsed, str(exc)
    except urllib.error.URLError as exc:
        elapsed = time.perf_counter() - start
        return False, 0, elapsed, str(exc)


def _load_events(path: Path) -> List[Dict[str, Any]]:
    if path.suffix == ".gz":
        text = gzip.open(path, "rt", encoding="utf-8").read()
    else:
        text = path.read_text(encoding="utf-8")
    events = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        events.append(json.loads(line))
    return events


def main() -> None:
    args = parse_args()
    path = Path(args.file)
    events = _load_events(path)
    if not events:
        print("no events loaded")
        return

    type_counts = Counter(event.get("event_type", "unknown") for event in events)
    sent = ok = failed = 0
    elapsed_times: List[float] = []

    for batch in _chunks(events, args.batch):
        payload = batch[0] if len(batch) == 1 else batch
        sent += len(batch)
        ok_response, status, elapsed, error = _send_payload(
            args.endpoint, payload, args.token
        )
        elapsed_times.append(elapsed)
        if ok_response:
            ok += len(batch)
        else:
            failed += len(batch)
            print(f"request failed status={status} error={error}")

    avg_ms = (sum(elapsed_times) / len(elapsed_times)) * 1000.0
    print(f"sent={sent} ok={ok} failed={failed} avg_ms={avg_ms:.1f}")
    print("event_types:", dict(type_counts))


if __name__ == "__main__":
    main()
