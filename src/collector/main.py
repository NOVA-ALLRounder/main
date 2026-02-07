"""
Collector Main - 데이터 수집기 엔트리포인트.

HTTP 서버로 이벤트를 수신하고, 센서를 자동 시작하며,
5분마다 자동 집약을 실행합니다.
"""

import argparse
import json
import logging
import os
import re
import signal
import sqlite3
import subprocess
import sys
import threading
from datetime import datetime
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from queue import Queue
from typing import Optional, Dict, Any, List

import yaml

# 로컬 임포트
try:
    from .aggregator import create_aggregator
    from .startup_workflow import generate_workflow_on_startup
except ImportError:
    from aggregator import create_aggregator
    from startup_workflow import generate_workflow_on_startup

logger = logging.getLogger(__name__)


class PrivacyGuard:
    """개인정보 보호 필터."""
    
    def __init__(self, config_path: str = "configs/privacy_rules.yaml"):
        self.allowlist_apps: List[str] = []
        self.denylist_apps: List[str] = []
        self.sensitive_patterns: List[str] = []
        self.redaction_patterns: List[Dict] = []
        
        self._load_config(config_path)
    
    def _load_config(self, config_path: str):
        """설정 파일 로드."""
        try:
            with open(config_path, 'r', encoding='utf-8') as f:
                config = yaml.safe_load(f) or {}
            
            self.allowlist_apps = [a.upper() for a in config.get("allowlist_apps", [])]
            self.denylist_apps = [a.upper() for a in config.get("denylist_apps", [])]
            self.sensitive_patterns = config.get("sensitive_window_patterns", [])
            self.redaction_patterns = config.get("redaction_patterns", [])
            
            logger.info(f"Privacy rules loaded: allowlist={len(self.allowlist_apps)}, denylist={len(self.denylist_apps)}")
            
        except FileNotFoundError:
            logger.warning(f"Privacy config not found: {config_path}, using defaults (collect all)")
        except Exception as e:
            logger.error(f"Failed to load privacy config: {e}")
    
    def should_collect(self, event: Dict[str, Any]) -> bool:
        """이벤트 수집 여부 결정."""
        app = event.get("app", "")
        
        # 앱 이름 추출 (창 제목에서)
        app_name = app.split(" - ")[-1].upper() if " - " in app else app.upper()
        
        # Denylist 체크 (우선)
        for denied in self.denylist_apps:
            if denied in app_name:
                return False
        
        # 민감한 창 제목 체크
        for pattern in self.sensitive_patterns:
            if pattern.lower() in app.lower():
                return False
        
        # Allowlist 체크 (비어있으면 모든 앱 허용)
        if self.allowlist_apps:
            for allowed in self.allowlist_apps:
                if allowed in app_name:
                    return True
            return False
        
        return True
    
    def redact(self, event: Dict[str, Any]) -> Dict[str, Any]:
        """민감 정보 마스킹."""
        payload = event.get("payload", {})
        if isinstance(payload, str):
            try:
                payload = json.loads(payload)
            except:
                return event
        
        # element_value 마스킹
        if "element_value" in payload:
            value = payload["element_value"]
            for pattern_config in self.redaction_patterns:
                pattern = pattern_config.get("pattern", "")
                replacement = pattern_config.get("replacement", "[REDACTED]")
                try:
                    value = re.sub(pattern, replacement, str(value))
                except:
                    pass
            payload["element_value"] = value
        
        event["payload"] = payload
        return event


class EventStore:
    """SQLite 이벤트 저장소."""
    
    def __init__(self, db_path: str):
        self.db_path = db_path
        self._init_db()
    
    def _init_db(self):
        """DB 및 테이블 초기화."""
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT UNIQUE NOT NULL,
                ts TEXT NOT NULL,
                source TEXT,
                app TEXT,
                event_type TEXT,
                priority TEXT,
                resource_type TEXT,
                resource_id TEXT,
                payload TEXT,
                window_id TEXT,
                pid INTEGER,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        """)
        
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_events_ts ON events(ts)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_events_app ON events(app)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_events_source ON events(source)")
        
        conn.commit()
        conn.close()
        logger.info(f"Database initialized: {self.db_path}")
    
    def insert_event(self, event: Dict[str, Any]) -> bool:
        """이벤트 삽입."""
        try:
            conn = sqlite3.connect(self.db_path)
            cursor = conn.cursor()
            
            resource = event.get("resource", {})
            payload = event.get("payload", {})
            
            cursor.execute("""
                INSERT OR IGNORE INTO events
                (event_id, ts, source, app, event_type, priority, 
                 resource_type, resource_id, payload, window_id, pid)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                event.get("event_id"),
                event.get("ts"),
                event.get("source"),
                event.get("app"),
                event.get("event_type"),
                event.get("priority"),
                resource.get("type"),
                resource.get("id"),
                json.dumps(payload, ensure_ascii=False),
                event.get("window_id"),
                event.get("pid")
            ))
            
            conn.commit()
            conn.close()
            return True
            
        except Exception as e:
            logger.error(f"Failed to insert event: {e}")
            return False


class IngestHandler(BaseHTTPRequestHandler):
    """이벤트 수신 HTTP 핸들러."""
    
    store: Optional[EventStore] = None
    privacy_guard: Optional[PrivacyGuard] = None
    
    def log_message(self, format, *args):
        """HTTP 로그 억제."""
        pass
    
    def do_POST(self):
        """POST /events 처리."""
        if self.path != "/events":
            self.send_response(404)
            self.end_headers()
            return
        
        try:
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            event = json.loads(body.decode("utf-8"))
            
            # Privacy 필터링
            if self.privacy_guard:
                if not self.privacy_guard.should_collect(event):
                    # 수집 제외 - 응답만 하고 저장 안 함
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(b'{"status": "filtered"}')
                    return
                
                # PII 마스킹
                event = self.privacy_guard.redact(event)
            
            if self.store:
                self.store.insert_event(event)
            
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"status": "ok"}')
            
        except Exception as e:
            logger.error(f"Ingest error: {e}")
            self.send_response(500)
            self.end_headers()
    
    def do_GET(self):
        """GET /health 처리."""
        if self.path == "/health":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b'{"status": "healthy"}')
        else:
            self.send_response(404)
            self.end_headers()


class Collector:
    """메인 수집기."""
    
    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 8080,
        db_path: str = "collector.db",
        auto_start_sensors: bool = True
    ):
        self.host = host
        self.port = port
        self.db_path = db_path
        self.auto_start_sensors = auto_start_sensors
        
        self.store = EventStore(db_path)
        self.privacy_guard = PrivacyGuard()
        self.aggregator = create_aggregator(db_path)
        self.server: Optional[HTTPServer] = None
        self.sensor_processes: List[subprocess.Popen] = []
        self._running = False
    
    def start(self):
        """수집기 시작."""
        self._running = True
        
        # 시작 시 이전 날짜 패턴 분석 및 워크플로우 생성
        logger.info("Analyzing previous patterns...")
        try:
            workflow_path = generate_workflow_on_startup(self.db_path)
            if workflow_path:
                print(f"[Workflow] Generated: {workflow_path}")
        except Exception as e:
            logger.warning(f"Workflow generation skipped: {e}")
        
        # HTTP 서버 시작
        IngestHandler.store = self.store
        IngestHandler.privacy_guard = self.privacy_guard
        self.server = HTTPServer((self.host, self.port), IngestHandler)
        
        server_thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        server_thread.start()
        logger.info(f"Ingest server started on http://{self.host}:{self.port}/events")
        
        # Aggregator 시작 (5분마다 자동 집약)
        self.aggregator.start()
        
        # 센서 자동 시작
        if self.auto_start_sensors:
            self._start_sensors()
        
        print(f"\n{'='*50}")
        print(f"Collector started!")
        print(f"  - Ingest: http://{self.host}:{self.port}/events")
        print(f"  - DB: {self.db_path}")
        print(f"  - Aggregation: every 5 minutes")
        print(f"  - Retention: 7 days raw, 30 days summaries")
        print(f"{'='*50}")
        print("Press Ctrl+C to stop...\n")
    
    def _start_sensors(self):
        """센서 프로세스 시작."""
        sensors = [
            ("sensors.os.ui_automation", ["--interval", "0.5"]),
            ("sensors.os.input_hook", ["--debounce-ms", "100"]),
        ]
        
        for module, args in sensors:
            try:
                cmd = [sys.executable, "-m", module] + args
                env = os.environ.copy()
                env["PYTHONPATH"] = str(Path(__file__).parent.parent)
                
                proc = subprocess.Popen(
                    cmd,
                    env=env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True
                )
                self.sensor_processes.append(proc)
                logger.info(f"Started sensor: {module}")
                
            except Exception as e:
                logger.warning(f"Failed to start sensor {module}: {e}")
    
    def stop(self):
        """수집기 중지."""
        self._running = False
        
        # 센서 중지
        for proc in self.sensor_processes:
            try:
                proc.terminate()
                proc.wait(timeout=2)
            except:
                proc.kill()
        
        # Aggregator 중지
        self.aggregator.stop()
        
        # 서버 중지
        if self.server:
            self.server.shutdown()
        
        logger.info("Collector stopped")
    
    def run_forever(self):
        """메인 루프."""
        self.start()
        
        try:
            while self._running:
                # 센서 출력 읽기
                for proc in self.sensor_processes:
                    if proc.stdout:
                        line = proc.stdout.readline()
                        if line:
                            print(line.strip())
                
        except KeyboardInterrupt:
            print("\nShutting down...")
        finally:
            self.stop()


def parse_args():
    parser = argparse.ArgumentParser(description="Data Collector")
    parser.add_argument("--host", default="127.0.0.1", help="Server host")
    parser.add_argument("--port", type=int, default=8080, help="Server port")
    parser.add_argument("--db", default="collector.db", help="Database path")
    parser.add_argument("--no-sensors", action="store_true", help="Don't auto-start sensors")
    return parser.parse_args()


def main():
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s [%(name)s] %(message)s"
    )
    
    args = parse_args()
    
    collector = Collector(
        host=args.host,
        port=args.port,
        db_path=args.db,
        auto_start_sensors=not args.no_sensors
    )
    
    # 시그널 핸들러 (Ctrl+C, SIGTERM)
    def signal_handler(sig, frame):
        logger.info(f"Received signal {sig}, shutting down...")
        collector.stop()
        sys.exit(0)
    
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    # Windows 종료 이벤트 핸들러
    try:
        import win32api
        import win32con
        
        def windows_shutdown_handler(ctrl_type):
            """Windows 콘솔 이벤트 핸들러 (종료/로그오프 감지)."""
            # CTRL_CLOSE_EVENT (콘솔 창 닫기)
            # CTRL_LOGOFF_EVENT (로그오프)
            # CTRL_SHUTDOWN_EVENT (시스템 종료)
            if ctrl_type in (win32con.CTRL_CLOSE_EVENT, 
                            win32con.CTRL_LOGOFF_EVENT, 
                            win32con.CTRL_SHUTDOWN_EVENT):
                logger.info(f"Windows shutdown event detected: {ctrl_type}")
                collector.stop()
                return True
            return False
        
        win32api.SetConsoleCtrlHandler(windows_shutdown_handler, True)
        logger.info("Windows shutdown handler registered")
        
    except ImportError:
        # pywin32가 없으면 atexit fallback
        import atexit
        
        @atexit.register
        def cleanup_on_exit():
            logger.info("Cleanup on exit...")
            collector.stop()
        
        logger.info("Using atexit fallback (install pywin32 for better shutdown handling)")
    
    collector.run_forever()


if __name__ == "__main__":
    main()

