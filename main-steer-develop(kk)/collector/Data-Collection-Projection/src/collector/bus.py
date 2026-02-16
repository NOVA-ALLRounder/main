from __future__ import annotations

import base64
import json
import logging
import queue
import threading
import time
from typing import Any, Dict, Optional

from .normalize import NormalizationError, normalize_event
from .privacy import PrivacyGuard
from .priority import PriorityProcessor
from .store import SQLiteStore

try:
    from .observability import Observability
except ImportError:  # pragma: no cover - optional for test import order
    Observability = None  # type: ignore

logger = logging.getLogger(__name__)
activity_logger = logging.getLogger("collector.activity")
activity_text_logger = logging.getLogger("collector.activity_text")


class EventBus:
    def __init__(
        self,
        store: SQLiteStore,
        privacy_guard: PrivacyGuard,
        priority: PriorityProcessor,
        validation_level: str = "lenient",
        queue_size: int = 1000,
        insert_batch_size: int = 100,
        insert_flush_ms: int = 1000,
        insert_retry_attempts: int = 3,
        insert_retry_backoff_ms: int = 50,
        activity_detail_enabled: bool = False,
        activity_detail_min_duration_sec: int = 5,
        activity_detail_store_hint: bool = True,
        activity_detail_hash_salt: str = "",
        activity_detail_full_title_apps: Optional[list[str]] = None,
        activity_detail_max_title_len: int = 256,
        metrics: Optional["Observability"] = None,
    ) -> None:
        self._store = store
        self._privacy_guard = privacy_guard
        self._priority = priority
        self._validation_level = validation_level
        self._queue: queue.Queue[Dict[str, Any]] = queue.Queue(maxsize=queue_size)
        self._stop_event = threading.Event()
        self._worker = threading.Thread(target=self._run, daemon=True)
        self._metrics = metrics
        self._buffer = []
        self._last_flush = time.time()
        self._batch_size = max(1, int(insert_batch_size))
        self._flush_interval = max(0.1, int(insert_flush_ms) / 1000.0)
        self._retry_attempts = max(0, int(insert_retry_attempts))
        self._retry_backoff_ms = max(0, int(insert_retry_backoff_ms))
        self._activity_detail_enabled = bool(activity_detail_enabled)
        self._activity_detail_min_duration_sec = max(
            0, int(activity_detail_min_duration_sec)
        )
        self._activity_detail_store_hint = bool(activity_detail_store_hint)
        self._activity_detail_hash_salt = activity_detail_hash_salt
        self._activity_detail_full_title_apps = {
            str(item).lower()
            for item in (activity_detail_full_title_apps or [])
            if str(item).strip()
        }
        self._activity_detail_max_title_len = max(0, int(activity_detail_max_title_len))

    def start(self) -> None:
        self._worker.start()

    def stop(self, drain_seconds: int = 0) -> None:
        deadline = time.time() + max(0, int(drain_seconds))
        while drain_seconds > 0 and not self._queue.empty() and time.time() < deadline:
            time.sleep(0.05)
        self._stop_event.set()
        self._worker.join(timeout=5)
        self._buffer.extend(self._priority.flush())
        self._flush_buffer(force=True)

    def enqueue(self, event: Dict[str, Any]) -> bool:
        try:
            self._queue.put_nowait(event)
            if self._metrics:
                self._metrics.set_gauge("queue.depth", self._queue.qsize())
            return True
        except queue.Full:
            if self._metrics:
                self._metrics.set_gauge("queue.depth", self._queue.qsize())
            return False

    def _run(self) -> None:
        while not self._stop_event.is_set():
            try:
                item = self._queue.get(timeout=0.5)
            except queue.Empty:
                if self._metrics:
                    self._metrics.set_gauge("queue.depth", self._queue.qsize())
                self._flush_buffer()
                continue
            try:
                envelope = normalize_event(item, validation_level=self._validation_level)
                envelope = self._privacy_guard.apply(envelope)
                if envelope is None:
                    continue
                queue_ratio = _queue_ratio(self._queue)
                for output in self._priority.process(envelope, queue_ratio):
                    self._buffer.append(output)
                    if len(self._buffer) >= self._batch_size:
                        self._flush_buffer(force=True)
                self._flush_buffer()
            except NormalizationError as exc:
                logger.warning("drop event: %s", exc)
                if self._metrics:
                    self._metrics.record_ingest_invalid()
            except Exception:
                logger.exception("failed to process event")
                if self._metrics:
                    self._metrics.record_store_insert_fail()
            finally:
                if self._metrics:
                    self._metrics.set_gauge("queue.depth", self._queue.qsize())
                    self._metrics.maybe_log(logger, self._store.get_db_size())

    def _flush_buffer(self, force: bool = False) -> None:
        if not self._buffer:
            return
        now = time.time()
        if not force and (now - self._last_flush) < self._flush_interval:
            return
        batch = self._buffer
        self._buffer = []
        try:
            self._store.insert_events(
                batch,
                retry_attempts=self._retry_attempts,
                retry_backoff_ms=self._retry_backoff_ms,
            )
            detail_records: list[tuple[str, str, str, str, str, int]] = []
            if self._activity_detail_enabled:
                detail_records = _build_activity_detail_records(
                    batch,
                    min_duration_sec=self._activity_detail_min_duration_sec,
                    store_hint=self._activity_detail_store_hint,
                    hash_salt=self._activity_detail_hash_salt,
                    full_title_apps=self._activity_detail_full_title_apps,
                    max_title_len=self._activity_detail_max_title_len,
                )
                self._store.upsert_activity_details(detail_records)
            if self._metrics:
                for output in batch:
                    self._metrics.record_priority(output.priority)
                    self._metrics.record_store_insert_ok()
                    self._metrics.set_last_event_ts(output.ts)
                    self._metrics.record_activity(
                        output.app, output.event_type, output.payload, output.priority
                    )
                    activity_payload = self._metrics.activity_block_payload(
                        output.app, output.event_type, output.payload, output.ts
                    )
                    if activity_payload:
                        logger.info(
                            json.dumps(activity_payload, separators=(",", ":"))
                        )
                        activity_text_logger.info(
                            _format_activity_text(activity_payload)
                        )
                    if (output.event_type or "").lower().startswith("browser."):
                        browser_payload = _build_browser_activity_payload(output)
                        if browser_payload:
                            activity_logger.info(
                                json.dumps(browser_payload, separators=(",", ":"))
                            )
                            activity_text_logger.info(
                                _format_activity_text(browser_payload)
                            )
                if detail_records and self._activity_detail_full_title_apps:
                    for app, title_hash, title_hint, first_ts, last_ts, duration in detail_records:
                        if app.lower() not in self._activity_detail_full_title_apps:
                            continue
                        if not title_hint:
                            continue
                        activity_logger.info(
                            json.dumps(
                                {
                                    "event": "activity_detail",
                                    "app": app,
                                    "duration_sec": duration,
                                    "title_hint": title_hint,
                                    "first_seen_ts": first_ts,
                                    "last_seen_ts": last_ts,
                                    "title_label": _title_label(app, title_hash),
                                },
                                separators=(",", ":"),
                            )
                        )
                        activity_text_logger.info(
                            _format_activity_text(
                                {
                                    "event": "activity_detail",
                                    "app": app,
                                    "duration_sec": duration,
                                    "title_hint": title_hint,
                                    "first_seen_ts": first_ts,
                                    "last_seen_ts": last_ts,
                                    "title_label": _title_label(app, title_hash),
                                }
                            )
                        )
        except Exception:
            logger.exception("failed to insert batch")
            if self._metrics:
                self._metrics.record_store_insert_fail()
        finally:
            self._last_flush = now


def _queue_ratio(q: queue.Queue) -> float:
    maxsize = q.maxsize
    if maxsize <= 0:
        return 0.0
    return q.qsize() / maxsize


def _build_browser_activity_payload(output: Any) -> Optional[Dict[str, Any]]:
    payload = getattr(output, "payload", {}) or {}
    title = payload.get("window_title")
    url = payload.get("url")
    domain = payload.get("domain")
    data: Dict[str, Any] = {
        "event": "browser_activity",
        "app": getattr(output, "app", "") or "",
    }
    if isinstance(title, str) and title.strip():
        data["title_hint"] = title.strip()
    if isinstance(url, str) and url.strip():
        data["url"] = url.strip()
    if isinstance(domain, str) and domain.strip():
        data["domain"] = domain.strip()
    ts_value = getattr(output, "ts", "") or ""
    if ts_value:
        data["event_ts"] = ts_value
    if len(data) <= 2:
        return None
    return data


def _title_label(app: str, title_hash: str) -> str:
    app_key = (app or "").split(".", 1)[0].upper() or "APP"
    code = title_hash
    try:
        raw = bytes.fromhex(title_hash)
        code = base64.b32encode(raw).decode("ascii").rstrip("=")
    except (ValueError, TypeError):
        code = title_hash or "UNKNOWN"
    return f"{app_key}-{code[:8]}"


def _normalize_title(app: str, title: str) -> str:
    app_key = (app or "").lower()
    value = title.strip()
    if app_key == "notion.exe":
        for suffix in [" - Notion", " – Notion", " — Notion"]:
            if value.endswith(suffix):
                value = value[: -len(suffix)].strip()
                break
    if app_key == "code.exe":
        for suffix in [
            " - Visual Studio Code",
            " - Visual Studio Code Insiders",
            " - Code",
        ]:
            if value.endswith(suffix):
                value = value[: -len(suffix)].strip()
                break
    return value


def _format_activity_text(payload: Dict[str, Any]) -> str:
    event = payload.get("event", "activity")
    app = payload.get("app", "")
    duration = payload.get("duration_sec")
    title = payload.get("title_hint") or ""
    url = payload.get("url") or ""
    domain = payload.get("domain") or ""
    label = payload.get("title_label") or ""
    parts = [f"{event} app={app}"]
    if duration is not None:
        parts.append(f"duration={duration}s")
    if label:
        parts.append(f"label={label}")
    if title:
        parts.append(f"title=\"{title}\"")
    if url:
        parts.append(f"url=\"{url}\"")
    elif domain:
        parts.append(f"domain={domain}")
    return " | ".join(parts)


def _build_activity_detail_records(
    batch: list[Any],
    *,
    min_duration_sec: int,
    store_hint: bool,
    hash_salt: str,
    full_title_apps: set[str],
    max_title_len: int,
) -> list[tuple[str, str, str, str, str, int]]:
    if not batch:
        return []
    try:
        from .utils.hashing import hmac_sha256
    except Exception:
        return []

    records: list[tuple[str, str, str, str, str, int]] = []
    for output in batch:
        event_type = str(getattr(output, "event_type", "") or "").lower()
        if event_type != "os.app_focus_block":
            continue
        payload = getattr(output, "payload", {}) or {}
        title = payload.get("window_title")
        duration = payload.get("duration_sec")
        if not isinstance(duration, (int, float)) or duration < min_duration_sec:
            continue
        app = str(getattr(output, "app", "") or "").strip()
        if not app:
            continue
        app_key = app.lower()
        if app_key in full_title_apps:
            raw_payload = {}
            raw = getattr(output, "raw", {}) or {}
            if isinstance(raw, dict):
                raw_payload = raw.get("payload") if isinstance(raw.get("payload"), dict) else {}
            raw_title = raw_payload.get("window_title")
            if isinstance(raw_title, str) and raw_title.strip():
                title = raw_title

        if not isinstance(title, str) or not title.strip():
            continue

        title_clean = _normalize_title(app, title.strip())
        title_hash = hmac_sha256(title_clean, hash_salt or "dev-salt")
        title_hint = title_clean if store_hint else ""
        if title_hint and max_title_len > 0 and len(title_hint) > max_title_len:
            title_hint = title_hint[:max_title_len]
        ts = str(getattr(output, "ts", "") or "")
        records.append(
            (
                app,
                title_hash,
                title_hint,
                ts,
                ts,
                int(duration),
            )
        )
    return records
