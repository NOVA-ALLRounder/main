import json
import logging
import uuid
from datetime import datetime, timezone
from logging.handlers import RotatingFileHandler
from pathlib import Path
from typing import Any, Dict, Optional

try:
    from zoneinfo import ZoneInfo
except Exception:  # pragma: no cover - fallback for environments without zoneinfo
    ZoneInfo = None  # type: ignore


class JsonFormatter(logging.Formatter):
    def __init__(
        self,
        run_id: str,
        tzinfo: Optional[timezone] = None,
        include_run_id: bool = True,
    ) -> None:
        super().__init__()
        self._run_id = run_id
        self._tzinfo = tzinfo
        self._include_run_id = include_run_id

    def format(self, record: logging.LogRecord) -> str:
        ts = _format_ts(record.created, self._tzinfo)
        payload: Dict[str, Any] = {
            "ts": ts,
            "level": record.levelname,
            "component": record.name,
        }
        if self._include_run_id:
            payload["run_id"] = self._run_id

        message = record.getMessage()
        parsed = _parse_json(message)
        if parsed is not None:
            event = parsed.get("event")
            if event:
                payload["event"] = event
            payload["meta"] = parsed
        else:
            payload["event"] = getattr(record, "event", None) or message
            meta = getattr(record, "meta", None)
            if meta is not None:
                payload["meta"] = meta

        if record.exc_info:
            payload["error"] = self.formatException(record.exc_info)

        return json.dumps(payload, separators=(",", ":"), ensure_ascii=False)


def setup_logging(
    level: str = "INFO",
    *,
    log_dir: Optional[Path] = None,
    log_file: str = "collector.log",
    max_mb: int = 20,
    backup_count: int = 10,
    use_json: bool = True,
    to_console: bool = True,
    activity_detail_file: Optional[str] = None,
    activity_detail_max_mb: int = 20,
    activity_detail_backup_count: int = 10,
    activity_detail_text_file: Optional[str] = None,
    activity_detail_text_max_mb: int = 10,
    activity_detail_text_backup_count: int = 5,
    timezone_name: str = "local",
    include_run_id: bool = True,
    prune_days: int = 0,
    run_id: Optional[str] = None,
) -> str:
    run_id = run_id or uuid.uuid4().hex
    root = logging.getLogger()
    root.setLevel(getattr(logging, level.upper(), logging.INFO))
    for handler in list(root.handlers):
        root.removeHandler(handler)

    tzinfo = _resolve_tz(timezone_name)
    if use_json:
        formatter = JsonFormatter(run_id, tzinfo=tzinfo, include_run_id=include_run_id)
    else:
        formatter = logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s")

    if log_dir:
        log_dir.mkdir(parents=True, exist_ok=True)
        if prune_days and prune_days > 0:
            _prune_logs(log_dir, prune_days)
        log_path = log_dir / log_file
        max_bytes = max(1, int(max_mb)) * 1024 * 1024
        file_handler = RotatingFileHandler(
            log_path, maxBytes=max_bytes, backupCount=max(1, int(backup_count))
        )
        file_handler.setFormatter(formatter)
        root.addHandler(file_handler)

        if activity_detail_file:
            activity_logger = logging.getLogger("collector.activity")
            activity_logger.setLevel(root.level)
            activity_logger.propagate = True
            for handler in list(activity_logger.handlers):
                activity_logger.removeHandler(handler)
            activity_log_path = log_dir / activity_detail_file
            activity_max_bytes = max(1, int(activity_detail_max_mb)) * 1024 * 1024
            activity_handler = RotatingFileHandler(
                activity_log_path,
                maxBytes=activity_max_bytes,
                backupCount=max(1, int(activity_detail_backup_count)),
            )
            activity_handler.setFormatter(formatter)
            activity_logger.addHandler(activity_handler)

        if activity_detail_text_file:
            activity_text_logger = logging.getLogger("collector.activity_text")
            activity_text_logger.setLevel(root.level)
            activity_text_logger.propagate = True
            for handler in list(activity_text_logger.handlers):
                activity_text_logger.removeHandler(handler)
            text_log_path = log_dir / activity_detail_text_file
            text_max_bytes = max(1, int(activity_detail_text_max_mb)) * 1024 * 1024
            text_handler = RotatingFileHandler(
                text_log_path,
                maxBytes=text_max_bytes,
                backupCount=max(1, int(activity_detail_text_backup_count)),
            )
            text_handler.setFormatter(TextFormatter(tzinfo=tzinfo))
            activity_text_logger.addHandler(text_handler)

    if to_console or not log_dir:
        console_handler = logging.StreamHandler()
        console_handler.setFormatter(formatter)
        root.addHandler(console_handler)

    return run_id


def _parse_json(message: str) -> Optional[Dict[str, Any]]:
    if not message:
        return None
    try:
        parsed = json.loads(message)
    except (TypeError, ValueError, json.JSONDecodeError):
        return None
    if isinstance(parsed, dict):
        return parsed
    return None


class TextFormatter(logging.Formatter):
    def __init__(self, tzinfo: Optional[timezone] = None) -> None:
        super().__init__()
        self._tzinfo = tzinfo

    def format(self, record: logging.LogRecord) -> str:
        ts = _format_ts(record.created, self._tzinfo)
        message = record.getMessage()
        return f"{ts} {message}"


def _resolve_tz(name: str) -> Optional[timezone]:
    if not name:
        return None
    if str(name).lower() in {"local", "system", "default"}:
        return None
    if ZoneInfo is None:
        return None
    try:
        return ZoneInfo(str(name))
    except Exception:
        return None


def _format_ts(epoch_seconds: float, tzinfo: Optional[timezone]) -> str:
    if tzinfo is None:
        return (
            datetime.fromtimestamp(epoch_seconds)
            .astimezone()
            .strftime("%Y-%m-%d %H:%M:%S")
        )
    return (
        datetime.fromtimestamp(epoch_seconds, tz=timezone.utc)
        .astimezone(tzinfo)
        .strftime("%Y-%m-%d %H:%M:%S")
    )


def _prune_logs(log_dir: Path, prune_days: int) -> None:
    cutoff = datetime.now().timestamp() - (prune_days * 86400)
    for pattern in ("*.log*", "*.txt"):
        for path in log_dir.glob(pattern):
            try:
                if path.stat().st_mtime < cutoff:
                    path.unlink()
            except Exception:
                continue
