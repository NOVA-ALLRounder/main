from __future__ import annotations

import argparse
import json
import logging
import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict, List

from .bus import EventBus
from .config import load_config
from .logging_ import setup_logging
from .observability import Observability
from .privacy import PrivacyGuard, load_privacy_rules
from .priority import PriorityProcessor
from .retention import retention_result_json, run_retention
from .store import SQLiteStore

logger = logging.getLogger(__name__)


class IngestServer(ThreadingHTTPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(
        self,
        server_address,
        handler,
        bus: EventBus,
        ingest_config,
        metrics: Observability,
        db_path,
    ):
        super().__init__(server_address, handler)
        self.bus = bus
        self.ingest_config = ingest_config
        self.metrics = metrics
        self.db_path = db_path


class IngestHandler(BaseHTTPRequestHandler):
    server_version = "CollectorHTTP/0.1"

    def do_GET(self) -> None:
        if self.path != "/health":
            if self.path == "/stats":
                self._handle_stats()
                return
            self._send_json(404, {"error": "not found"})
            return
        self._send_json(200, {"ok": True})

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self._set_cors_headers()
        self.end_headers()

    def do_POST(self) -> None:
        if self.path != "/events":
            self._send_json(404, {"error": "not found"})
            return
        if not self._check_token():
            return
        metrics = getattr(self.server, "metrics", None)
        content_length = self.headers.get("Content-Length")
        if not content_length:
            if metrics:
                metrics.record_ingest_invalid()
            self._send_json(411, {"error": "missing content-length"})
            return
        try:
            body = self.rfile.read(int(content_length))
            payload = json.loads(body)
        except (ValueError, json.JSONDecodeError):
            if metrics:
                metrics.record_ingest_invalid()
            self._send_json(400, {"error": "invalid json"})
            return

        events = _normalize_payload(payload)
        if events is None:
            if metrics:
                metrics.record_ingest_invalid()
            self._send_json(400, {"error": "payload must be object or list"})
            return

        for event in events:
            if not isinstance(event, dict):
                if metrics:
                    metrics.record_ingest_invalid()
                self._send_json(400, {"error": "event must be object"})
                return

        queued = 0
        if metrics:
            metrics.inc("ingest.received_total", len(events))
        for event in events:
            if not self.server.bus.enqueue(event):
                if metrics:
                    metrics.record_drop("queue_full")
                self._send_json(429, {"error": "queue full", "queued": queued})
                return
            queued += 1

        if metrics:
            metrics.inc("ingest.ok_total", queued)
        self._send_json(200, {"status": "queued", "count": queued})

    def log_message(self, format: str, *args: Any) -> None:
        logger.info("%s - %s", self.address_string(), format % args)

    def _send_json(self, status: int, payload: Dict[str, Any]) -> None:
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self._set_cors_headers()
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _set_cors_headers(self) -> None:
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, X-Collector-Token")

    def _check_token(self) -> bool:
        ingest_config = getattr(self.server, "ingest_config", None)
        token = getattr(ingest_config, "token", "")
        if not token:
            return True
        provided = self.headers.get("X-Collector-Token")
        if provided != token:
            self._send_json(401, {"error": "unauthorized"})
            return False
        return True

    def _handle_stats(self) -> None:
        metrics = getattr(self.server, "metrics", None)
        db_path = getattr(self.server, "db_path", None)
        if metrics is None:
            self._send_json(503, {"error": "metrics disabled"})
            return
        db_size = 0
        if db_path and db_path.exists():
            db_size = db_path.stat().st_size
        payload = metrics.snapshot(db_size)
        self._send_json(200, payload)


def _normalize_payload(payload: Any) -> List[Dict[str, Any]] | None:
    if isinstance(payload, dict):
        return [payload]
    if isinstance(payload, list):
        return payload
    return None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Data Collector Core")
    parser.add_argument(
        "--config", default="configs/config.yaml", help="path to config file"
    )
    return parser.parse_args()


def run() -> None:
    args = parse_args()
    config = load_config(args.config)
    setup_logging(
        config.log_level,
        log_dir=config.logging.dir,
        log_file=config.logging.file_name,
        max_mb=config.logging.max_mb,
        backup_count=config.logging.backup_count,
        use_json=config.logging.json,
        to_console=config.logging.to_console,
        activity_detail_file=config.logging.activity_detail_file,
        activity_detail_max_mb=config.logging.activity_detail_max_mb,
        activity_detail_backup_count=config.logging.activity_detail_backup_count,
        activity_detail_text_file=config.logging.activity_detail_text_file,
        activity_detail_text_max_mb=config.logging.activity_detail_text_max_mb,
        activity_detail_text_backup_count=config.logging.activity_detail_text_backup_count,
        timezone_name=config.logging.timezone,
        include_run_id=config.logging.include_run_id,
        prune_days=config.logging.prune_days,
    )

    logger.info("starting collector")

    store = SQLiteStore(
        config.db_path,
        wal_mode=config.wal_mode,
        busy_timeout_ms=config.store.busy_timeout_ms,
        encryption=config.encryption,
    )
    store.connect()
    store.migrate(config.migrations_path)

    metrics = Observability(
        log_interval_sec=config.observability.log_interval_sec,
        activity_log=config.observability.activity_log,
        activity_top_n=config.observability.activity_top_n,
        activity_min_duration_sec=config.observability.activity_min_duration_sec,
        activity_include_title=config.observability.activity_include_title,
        activity_title_apps=config.observability.activity_title_apps,
        activity_title_max_len=config.observability.activity_title_max_len,
        timezone_name=config.logging.timezone,
    )

    privacy_rules = load_privacy_rules(config.privacy_rules_path)
    privacy_guard = PrivacyGuard(
        privacy_rules,
        config.privacy.hash_salt,
        url_mode=config.privacy.url_mode,
        metrics=metrics,
    )

    priority = PriorityProcessor(
        debounce_seconds=config.priority.debounce_seconds,
        focus_event_types=config.priority.focus_event_types,
        focus_block_event_type=config.priority.focus_block_event_type,
        drop_p2_when_queue_over=config.priority.drop_p2_when_queue_over,
        p0_event_types=config.priority.p0_event_types,
        p1_event_types=config.priority.p1_event_types,
        p2_event_types=config.priority.p2_event_types,
        metrics=metrics,
    )

    bus = EventBus(
        store,
        privacy_guard,
        priority,
        validation_level=config.validation_level,
        queue_size=config.queue.max_size,
        insert_batch_size=config.store.insert_batch_size,
        insert_flush_ms=config.store.insert_flush_ms,
        insert_retry_attempts=config.store.insert_retry_attempts,
        insert_retry_backoff_ms=config.store.insert_retry_backoff_ms,
        activity_detail_enabled=config.activity_detail.enabled,
        activity_detail_min_duration_sec=config.activity_detail.min_duration_sec,
        activity_detail_store_hint=config.activity_detail.store_hint,
        activity_detail_hash_salt=config.privacy.hash_salt,
        activity_detail_full_title_apps=config.activity_detail.full_title_apps,
        activity_detail_max_title_len=config.activity_detail.max_title_len,
        metrics=metrics,
    )
    bus.start()

    server: IngestServer | None = None
    server_thread: threading.Thread | None = None

    if config.ingest.enabled:
        server = IngestServer(
            (config.ingest.host, config.ingest.port),
            IngestHandler,
            bus,
            config.ingest,
            metrics,
            config.db_path,
        )
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()
        logger.info(
            "ingest listening on http://%s:%s",
            config.ingest.host,
            config.ingest.port,
        )
    else:
        logger.info("ingest disabled")

    sensor_processes: list[subprocess.Popen] = []
    if config.ingest.enabled:
        sensor_processes = _start_sensors(config)

    stop_event = threading.Event()
    retention_thread: threading.Thread | None = None

    if config.retention.enabled:
        vacuum_seconds = max(0, config.retention.vacuum_hours) * 3600
        last_vacuum = 0.0

        def _retention_loop() -> None:
            nonlocal last_vacuum
            while not stop_event.is_set():
                try:
                    force_vacuum = False
                    if vacuum_seconds > 0:
                        now_ts = time.time()
                        if last_vacuum <= 0 or (now_ts - last_vacuum) >= vacuum_seconds:
                            force_vacuum = True
                    result = run_retention(
                        store, config.retention, force_vacuum=force_vacuum
                    )
                    if result.vacuumed:
                        last_vacuum = time.time()
                    logger.info(retention_result_json(result))
                except Exception:
                    logger.exception("retention failed")
                stop_event.wait(config.retention.interval_minutes * 60)

        retention_thread = threading.Thread(target=_retention_loop, daemon=True)
        retention_thread.start()

    def _handle_signal(signum, frame):
        logger.info("shutdown requested")
        stop_event.set()
        if server is not None:
            server.shutdown()
        _stop_sensors(sensor_processes)

    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    try:
        heartbeat_interval = max(5, config.observability.log_interval_sec // 2)

        def _metrics_loop() -> None:
            while not stop_event.is_set():
                try:
                    metrics.maybe_log(logger, store.get_db_size())
                except Exception:
                    logger.exception("metrics heartbeat failed")
                stop_event.wait(heartbeat_interval)

        metrics_thread = threading.Thread(target=_metrics_loop, daemon=True)
        metrics_thread.start()

        while not stop_event.is_set():
            time.sleep(0.25)
    finally:
        if server is not None:
            server.shutdown()
            server.server_close()
        if server_thread is not None:
            server_thread.join(timeout=5)
        _stop_sensors(sensor_processes)
        bus.stop(drain_seconds=config.queue.shutdown_drain_seconds)
        if retention_thread is not None:
            retention_thread.join(timeout=5)
        store.close()
        _run_post_collection(config)
        logger.info("collector stopped")


def _run_post_collection(config) -> None:
    if not getattr(config, "post_collection", None):
        return
    post = config.post_collection
    if not post.enabled:
        return

    project_root = Path(__file__).resolve().parents[2]
    scripts_dir = project_root / "scripts"
    python_exe = sys.executable

    output_dir = str(Path(post.output_dir or config.logging.dir))

    logger.info("post_collection starting")
    try:
        if post.run_sessions:
            cmd = [
                python_exe,
                str(scripts_dir / "build_sessions.py"),
                "--config",
                str(config.config_path),
                "--since-hours",
                "24",
                "--gap-minutes",
                str(int(post.session_gap_minutes)),
            ]
            subprocess.run(cmd, check=False)
        if post.run_routines:
            cmd = [
                python_exe,
                str(scripts_dir / "build_routines.py"),
                "--config",
                str(config.config_path),
                "--days",
                str(int(post.routine_days)),
                "--min-support",
                str(int(post.routine_min_support)),
                "--n-min",
                str(int(post.routine_n_min)),
                "--n-max",
                str(int(post.routine_n_max)),
            ]
            subprocess.run(cmd, check=False)
        if post.run_handoff:
            cmd = [
                python_exe,
                str(scripts_dir / "build_handoff.py"),
                "--keep-latest-pending",
            ]
            subprocess.run(cmd, check=False)
        if post.run_daily_summary:
            cmd = [
                python_exe,
                str(scripts_dir / "build_daily_summary.py"),
                "--config",
                str(config.config_path),
                "--store-db",
            ]
            subprocess.run(cmd, check=False)
        if post.run_pattern_summary:
            cmd = [
                python_exe,
                str(scripts_dir / "build_pattern_summary.py"),
                "--summaries-dir",
                output_dir,
                "--since-days",
                "7",
                "--config",
                str(config.config_path),
                "--store-db",
            ]
            subprocess.run(cmd, check=False)
        if post.run_llm_input:
            # Best-effort: pick latest daily_summary in output_dir.
            daily_path = ""
            daily_files = sorted(Path(output_dir).glob("daily_summary_*.json"))
            if daily_files:
                daily_path = str(daily_files[-1])
            pattern_path = str(Path(output_dir) / "pattern_summary.json")
            cmd = [
                python_exe,
                str(scripts_dir / "build_llm_input.py"),
                "--config",
                str(config.config_path),
                "--daily",
                daily_path or "",
                "--pattern",
                pattern_path,
                "--output",
                str(Path(output_dir) / "llm_input.json"),
                "--max-bytes",
                str(int(post.llm_max_bytes)),
                "--store-db",
            ]
            subprocess.run(cmd, check=False)
            cmd = [
                python_exe,
                str(scripts_dir / "generate_recommendations.py"),
                "--config",
                str(config.config_path),
                "--input",
                str(Path(output_dir) / "llm_input.json"),
                "--output-md",
                str(Path(output_dir) / "activity_recommendations.md"),
                "--output-json",
                str(Path(output_dir) / "activity_recommendations.json"),
            ]
            subprocess.run(cmd, check=False)
        if getattr(config, "automation", None) and config.automation.enabled:
            cmd = [
                python_exe,
                str(scripts_dir / "execute_recommendations.py"),
                "--input",
                str(Path(output_dir) / "activity_recommendations.json"),
                "--allow",
                ",".join(config.automation.allow_actions),
            ]
            if config.automation.dry_run:
                cmd.append("--dry-run")
            subprocess.run(cmd, check=False)
        if post.run_pattern_report:
            cmd = [
                python_exe,
                str(scripts_dir / "report_patterns.py"),
                "--config",
                str(config.config_path),
                "--since-days",
                "3",
                "--output",
                str(Path(output_dir) / "pattern_report.md"),
            ]
            subprocess.run(cmd, check=False)
    except Exception:
        logger.exception("post_collection failed")
    else:
        logger.info("post_collection finished")


def _start_sensors(config) -> list[subprocess.Popen]:
    sensors = []
    sensor_cfg = getattr(config, "sensors", None)
    if not sensor_cfg or not sensor_cfg.auto_start:
        return sensors
    project_root = Path(__file__).resolve().parents[2]
    for proc_cfg in sensor_cfg.processes:
        if not proc_cfg.enabled:
            continue
        module = (proc_cfg.module or "").strip()
        if not module:
            continue
        cmd = [sys.executable, "-m", module]
        if proc_cfg.args:
            cmd.extend(proc_cfg.args)
        env = os.environ.copy()
        py_path = env.get("PYTHONPATH", "")
        src_path = str(project_root / "src")
        if src_path not in py_path.split(os.pathsep):
            env["PYTHONPATH"] = os.pathsep.join(
                [src_path] + ([py_path] if py_path else [])
            )
        try:
            proc = subprocess.Popen(cmd, cwd=str(project_root), env=env)
            sensors.append(proc)
            logger.info("sensor started: %s", " ".join(cmd))
        except Exception:
            logger.exception("failed to start sensor: %s", module)
    return sensors


def _stop_sensors(sensor_processes: list[subprocess.Popen]) -> None:
    if not sensor_processes:
        return
    for proc in sensor_processes:
        try:
            if proc.poll() is None:
                proc.terminate()
        except Exception:
            logger.exception("failed to terminate sensor")
    for proc in sensor_processes:
        try:
            proc.wait(timeout=5)
        except Exception:
            try:
                proc.kill()
            except Exception:
                logger.exception("failed to kill sensor")


if __name__ == "__main__":
    run()
