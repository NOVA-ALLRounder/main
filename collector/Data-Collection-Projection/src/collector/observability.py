from __future__ import annotations

import json
import threading
import time
from collections import Counter
from typing import Any, Dict, Optional


class Observability:
    def __init__(
        self,
        log_interval_sec: int = 60,
        activity_log: bool = True,
        activity_top_n: int = 3,
        activity_min_duration_sec: int = 5,
        activity_include_title: bool = False,
        activity_title_apps: Optional[list[str]] = None,
        activity_title_max_len: int = 128,
        timezone_name: str = "local",
    ) -> None:
        self._lock = threading.Lock()
        self._counters: Counter[str] = Counter()
        self._gauges: Dict[str, float] = {}
        self._minute_bucket = int(time.time() // 60)
        self._minute_counters: Counter[str] = Counter()
        self._minute_apps: Counter[str] = Counter()
        self._minute_key_events: Counter[str] = Counter()
        self._last_log = time.time()
        self._log_interval_sec = max(10, int(log_interval_sec))
        self._last_event_ts: Optional[str] = None
        self._activity_log = bool(activity_log)
        self._activity_top_n = max(1, int(activity_top_n))
        self._activity_min_duration_sec = max(0, int(activity_min_duration_sec))
        self._activity_include_title = bool(activity_include_title)
        self._activity_title_apps = {
            str(item).lower()
            for item in (activity_title_apps or [])
            if str(item).strip()
        }
        self._activity_title_max_len = max(0, int(activity_title_max_len))
        self._tzinfo = _resolve_tz(timezone_name)

    def inc(self, name: str, count: int = 1, track_minute: bool = True) -> None:
        if not name:
            return
        with self._lock:
            self._counters[name] += count
            self._tick_minute()
            if track_minute:
                self._minute_counters[name] += count

    def set_gauge(self, name: str, value: float) -> None:
        if not name:
            return
        with self._lock:
            self._gauges[name] = value

    def set_last_event_ts(self, ts: Optional[str]) -> None:
        if not ts:
            return
        with self._lock:
            self._last_event_ts = ts

    def record_drop(self, reason: str) -> None:
        self.inc("pipeline.dropped_total")
        if reason:
            self.inc(f"drop.reason.{reason}")

    def record_priority(self, priority: str) -> None:
        if not priority:
            return
        key = priority.strip().upper()
        if key in {"P0", "P1", "P2"}:
            self.inc(f"priority.{key.lower()}_total")

    def record_privacy_denied(self) -> None:
        self.inc("privacy.denied_total")
        self.record_drop("denylist")

    def record_privacy_redacted(self) -> None:
        self.inc("privacy.redacted_total")

    def record_ingest_received(self) -> None:
        self.inc("ingest.received_total")

    def record_ingest_ok(self) -> None:
        self.inc("ingest.ok_total")

    def record_ingest_invalid(self) -> None:
        self.inc("ingest.invalid_total")
        self.record_drop("schema")

    def record_store_insert_ok(self) -> None:
        self.inc("store.insert_ok_total")

    def record_store_insert_fail(self) -> None:
        self.inc("store.insert_fail_total")
        self.record_drop("store_fail")

    def record_activity(
        self,
        app: str,
        event_type: str,
        payload: Dict[str, Any],
        priority: str,
    ) -> None:
        if not self._activity_log:
            return
        app_key = str(app or "").strip()
        event_key = str(event_type or "").lower()
        priority_key = str(priority or "").upper()
        duration = payload.get("duration_sec")
        with self._lock:
            self._tick_minute()
            if event_key == "os.app_focus_block" and app_key:
                if isinstance(duration, (int, float)) and duration >= self._activity_min_duration_sec:
                    self._minute_apps[app_key] += int(duration)
            if priority_key == "P0" and event_key:
                self._minute_key_events[event_key] += 1

    def activity_block_payload(
        self,
        app: str,
        event_type: str,
        payload: Dict[str, Any],
        start_ts: Optional[str] = None,
    ) -> Optional[Dict[str, Any]]:
        if not self._activity_log:
            return None
        event_key = str(event_type or "").lower()
        if event_key != "os.app_focus_block":
            return None
        duration = payload.get("duration_sec")
        if not isinstance(duration, (int, float)) or duration < self._activity_min_duration_sec:
            return None
        app_name = str(app or "").strip()
        if not app_name:
            return None
        data: Dict[str, Any] = {
            "event": "activity_block",
            "app": app_name,
            "duration_sec": int(duration),
            "duration_human": _format_duration(int(duration)),
        }
        if start_ts:
            data["start_ts"] = _format_ts(start_ts, self._tzinfo)
            end_ts = _add_seconds(start_ts, int(duration), self._tzinfo)
            if end_ts:
                data["end_ts"] = end_ts
        if self._activity_include_title or app_name.lower() in self._activity_title_apps:
            title = payload.get("window_title")
            if isinstance(title, str) and title.strip():
                title_hint = _normalize_title(app_name, title.strip())
                if self._activity_title_max_len > 0 and len(title_hint) > self._activity_title_max_len:
                    title_hint = title_hint[: self._activity_title_max_len]
                data["title_hint"] = title_hint
        return data

    def snapshot(self, db_size_bytes: int) -> Dict[str, Any]:
        with self._lock:
            self._tick_minute()
            return {
                "counters": dict(self._counters),
                "gauges": dict(self._gauges),
                "minute": self._minute_bucket,
                "minute_counters": dict(self._minute_counters),
                "db_size_bytes": db_size_bytes,
                "last_event_ts": self._last_event_ts,
            }

    def maybe_log(self, logger, db_size_bytes: int) -> None:
        now = time.time()
        if now - self._last_log < self._log_interval_sec:
            return
        self._last_log = now
        payload = self.snapshot(db_size_bytes=db_size_bytes)
        payload["event"] = "metrics_minute"
        logger.info(json.dumps(payload, separators=(",", ":")))
        activity_payload = self._activity_minute_payload()
        if activity_payload:
            logger.info(json.dumps(activity_payload, separators=(",", ":")))

    def _tick_minute(self) -> None:
        now_bucket = int(time.time() // 60)
        if now_bucket != self._minute_bucket:
            self._minute_bucket = now_bucket
            self._minute_counters = Counter()
            self._minute_apps = Counter()
            self._minute_key_events = Counter()

    def _activity_minute_payload(self) -> Optional[Dict[str, Any]]:
        if not self._activity_log:
            return None
        with self._lock:
            self._tick_minute()
            top_apps = self._minute_apps.most_common(self._activity_top_n)
            key_events = dict(self._minute_key_events)
            if not top_apps and not key_events:
                return None
            minute_ts = _format_epoch(self._minute_bucket * 60, self._tzinfo, with_seconds=False)
        return {
            "event": "activity_minute",
            "minute": minute_ts,
            "top_apps": top_apps,
            "key_events": key_events,
        }


def _format_duration(seconds: int) -> str:
    if seconds < 60:
        return f"{seconds}s"
    minutes, sec = divmod(seconds, 60)
    if minutes < 60:
        return f"{minutes}m {sec:02d}s"
    hours, minutes = divmod(minutes, 60)
    return f"{hours}h {minutes:02d}m"


def _format_ts(ts_value: str, tzinfo) -> str:
    try:
        from .utils.time import parse_ts
    except Exception:
        return ts_value
    parsed = parse_ts(ts_value)
    if not parsed:
        return ts_value
    if tzinfo is None:
        return time.strftime("%Y-%m-%d %H:%M:%S", parsed.astimezone().timetuple())
    return time.strftime("%Y-%m-%d %H:%M:%S", parsed.astimezone(tzinfo).timetuple())


def _add_seconds(ts_value: str, seconds: int, tzinfo) -> Optional[str]:
    try:
        from .utils.time import parse_ts
    except Exception:
        return None
    parsed = parse_ts(ts_value)
    if not parsed:
        return None
    try:
        from datetime import timedelta
    except Exception:
        return None
    end_ts = parsed + timedelta(seconds=seconds)
    if tzinfo is None:
        return time.strftime("%Y-%m-%d %H:%M:%S", end_ts.astimezone().timetuple())
    return time.strftime("%Y-%m-%d %H:%M:%S", end_ts.astimezone(tzinfo).timetuple())


def _resolve_tz(name: str):
    if not name:
        return None
    if str(name).lower() in {"local", "system", "default"}:
        return None
    try:
        from zoneinfo import ZoneInfo
    except Exception:
        return None
    try:
        return ZoneInfo(str(name))
    except Exception:
        return None


def _format_epoch(epoch_seconds: float, tzinfo, with_seconds: bool = True) -> str:
    try:
        from datetime import datetime, timezone
    except Exception:
        return time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(epoch_seconds))
    if tzinfo is None:
        dt_value = datetime.fromtimestamp(epoch_seconds).astimezone()
    else:
        dt_value = datetime.fromtimestamp(epoch_seconds, tz=timezone.utc).astimezone(tzinfo)
    fmt = "%Y-%m-%d %H:%M:%S" if with_seconds else "%Y-%m-%d %H:%M"
    return dt_value.strftime(fmt)


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
