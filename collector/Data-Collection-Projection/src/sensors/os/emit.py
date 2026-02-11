"""
HTTP Emitter - 센서에서 Collector로 이벤트 전송.

os_test의 build_event 시그니처(keyword-only, resource dict) 유지 +
os_data의 비동기 큐 워커(Non-blocking send) 통합.
"""

from __future__ import annotations

import datetime as dt
import json
import logging
import queue
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, Iterable, List, Optional
from uuid import uuid4

logger = logging.getLogger(__name__)


@dataclass
class EmitConfig:
    ingest_url: str = "http://127.0.0.1:8080/events"
    timeout_sec: float = 2.0
    retries: int = 3
    backoff_sec: float = 0.5


def utc_now() -> str:
    return dt.datetime.utcnow().isoformat() + "Z"


def build_event(
    *,
    source: str,
    app: str,
    event_type: str,
    resource_type: str,
    resource_id: str,
    payload: Optional[Dict[str, Any]] = None,
    priority: str = "P1",
    window_id: Optional[str] = None,
    pid: Optional[int] = None,
) -> Dict[str, Any]:
    return {
        "schema_version": "1.0",
        "event_id": str(uuid4()),
        "ts": utc_now(),
        "source": source,
        "app": app,
        "event_type": event_type,
        "priority": priority,
        "resource": {
            "type": resource_type,
            "id": resource_id,
        },
        "payload": payload or {},
        "privacy": {"pii_level": "unknown", "redaction": []},
        "window_id": window_id,
        "pid": pid,
    }


class HttpEmitter:
    """비동기 큐 기반 HTTP Emitter.

    센서 스레드를 차단하지 않고 백그라운드 워커가 HTTP 전송을 처리합니다.
    큐가 가득 차면 가장 오래된 이벤트를 폐기하여 최신 데이터를 우선합니다.
    """

    def __init__(self, config: EmitConfig) -> None:
        self._config = config
        self._queue: queue.Queue = queue.Queue(maxsize=1000)
        self._running = True
        self._consecutive_errors = 0
        self._thread = threading.Thread(target=self._worker, daemon=True)
        self._thread.start()

    def send_event(self, event: Dict[str, Any]) -> bool:
        """이벤트를 큐에 추가 (Non-blocking)."""
        if not self._running:
            return False
        try:
            if self._queue.full():
                try:
                    self._queue.get_nowait()
                except queue.Empty:
                    pass
            self._queue.put_nowait(event)
            return True
        except Exception as e:
            logger.error("Failed to enqueue event: %s", e)
            return False

    def send_events(self, events: Iterable[Dict[str, Any]]) -> bool:
        ok = True
        for ev in events:
            if not self.send_event(ev):
                ok = False
        return ok

    def stop(self) -> None:
        """남은 이벤트를 전송하고 종료."""
        self._running = False
        if self._thread.is_alive():
            self._thread.join(timeout=3.0)

    # ── internal ──────────────────────────────────────────────

    def _worker(self) -> None:
        while self._running or not self._queue.empty():
            try:
                event = self._queue.get(timeout=0.1)
            except queue.Empty:
                continue
            self._send_sync([event])
            self._queue.task_done()

    def _send_sync(self, payload: List[Dict[str, Any]]) -> bool:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        request = urllib.request.Request(
            self._config.ingest_url,
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        for attempt in range(self._config.retries):
            try:
                with urllib.request.urlopen(
                    request, timeout=self._config.timeout_sec
                ) as response:
                    if 200 <= response.status < 300:
                        self._consecutive_errors = 0
                        return True
                    logger.warning("ingest responded with %s", response.status)
            except (urllib.error.URLError, TimeoutError, ConnectionAbortedError, ConnectionResetError, BrokenPipeError) as exc:
                self._consecutive_errors += 1
                if self._consecutive_errors <= 1:
                    logger.debug("ingest send failed: %s", exc)
                elif self._consecutive_errors == 10:
                    logger.debug("Too many consecutive errors, suppressing")

            delay = self._config.backoff_sec * (2 ** attempt)
            time.sleep(delay)

        return False
