"""
Real-time Aggregator - 5분마다 자동 집약 및 데이터 정리.

Collector와 함께 실행되어 실시간으로 데이터를 집약합니다.
- 5분마다: 분 단위 요약 생성
- 하루 끝: 일일 요약 생성
- 7일 후: 원시 이벤트 자동 삭제
"""

import gzip
import json
import logging
import sqlite3
import threading
import time
from collections import defaultdict
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional, Callable

logger = logging.getLogger(__name__)


class RealtimeAggregator:
    """실시간 데이터 집약기."""
    
    def __init__(
        self,
        db_path: str,
        aggregation_interval: int = 300,  # 5분 = 300초
        raw_retention_days: int = 7,
        summary_retention_days: int = 30
    ):
        self.db_path = db_path
        self.aggregation_interval = aggregation_interval
        self.raw_retention_days = raw_retention_days
        self.summary_retention_days = summary_retention_days
        
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._last_aggregation: Optional[datetime] = None
        self._last_daily_cleanup: Optional[str] = None
        
    def start(self):
        """백그라운드 집약 시작."""
        if self._running:
            return
            
        self._running = True
        self._thread = threading.Thread(target=self._run_loop, daemon=True)
        self._thread.start()
        logger.info(f"Aggregator started (interval: {self.aggregation_interval}s)")
        
    def stop(self):
        """집약 중지 - 종료 시 일일 요약 생성."""
        self._running = False
        
        # 종료 시 오늘 일일 요약 생성
        logger.info("Generating daily summary before shutdown...")
        try:
            conn = self._get_connection()
            self._ensure_tables(conn)
            
            # 마지막 5분 집약 실행
            self._do_aggregation()
            
            # 오늘 일일 요약 생성
            today = datetime.now().strftime("%Y-%m-%d")
            self._build_daily_summary(conn, today)
            
            # 오래된 이벤트 정리
            self._cleanup_old_events(conn)
            
            conn.close()
            logger.info(f"Daily summary generated for {today}")
        except Exception as e:
            logger.error(f"Failed to generate daily summary: {e}")
        
        if self._thread:
            self._thread.join(timeout=5)
        logger.info("Aggregator stopped")
        
    def _run_loop(self):
        """메인 루프."""
        while self._running:
            try:
                self._do_aggregation()
                self._check_daily_tasks()
            except Exception as e:
                logger.error(f"Aggregation error: {e}")
            
            # 인터벌 대기 (10초 단위로 체크하여 빠른 종료 가능)
            for _ in range(self.aggregation_interval // 10):
                if not self._running:
                    break
                time.sleep(10)
    
    def _get_connection(self) -> sqlite3.Connection:
        """DB 연결 생성."""
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        return conn
    
    def _ensure_tables(self, conn: sqlite3.Connection):
        """집약 테이블 생성."""
        cursor = conn.cursor()
        
        # 5분 단위 집약 테이블
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS minute_aggregates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                app TEXT NOT NULL,
                event_count INTEGER DEFAULT 0,
                actions_json TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(timestamp, app)
            )
        """)
        
        # 시간 단위 집약 테이블
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS hourly_aggregates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL,
                hour INTEGER NOT NULL,
                app TEXT NOT NULL,
                event_count INTEGER DEFAULT 0,
                unique_elements INTEGER DEFAULT 0,
                top_actions TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(date, hour, app)
            )
        """)
        
        # 일일 요약 테이블
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS daily_summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT UNIQUE NOT NULL,
                total_events INTEGER DEFAULT 0,
                total_apps INTEGER DEFAULT 0,
                active_hours INTEGER DEFAULT 0,
                app_usage_json TEXT,
                top_actions_json TEXT,
                summary_text TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        """)
        
        # 인덱스 생성
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_ma_ts ON minute_aggregates(timestamp)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_ha_date ON hourly_aggregates(date)")
        
        conn.commit()
    
    def _do_aggregation(self):
        """5분 단위 집약 실행."""
        now = datetime.now()
        
        # 5분 경계로 정렬 (예: 12:05, 12:10, 12:15...)
        minute_bucket = now.replace(second=0, microsecond=0)
        minute_bucket = minute_bucket - timedelta(minutes=minute_bucket.minute % 5)
        
        # 이전 5분 구간 집약
        start_time = minute_bucket - timedelta(minutes=5)
        end_time = minute_bucket
        
        conn = self._get_connection()
        try:
            self._ensure_tables(conn)
            
            # events 테이블 확인
            cursor = conn.cursor()
            cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='events'")
            if not cursor.fetchone():
                return
            
            # 해당 구간 이벤트 조회
            cursor.execute("""
                SELECT * FROM events 
                WHERE ts >= ? AND ts < ?
                ORDER BY ts
            """, (start_time.isoformat(), end_time.isoformat()))
            
            events = cursor.fetchall()
            
            if not events:
                return
            
            # 앱별 집약
            app_data = defaultdict(lambda: {"count": 0, "actions": defaultdict(int)})
            
            for event in events:
                event_dict = dict(event)
                app = event_dict.get("app", "unknown")
                
                payload = event_dict.get("payload", {})
                if isinstance(payload, str):
                    try:
                        payload = json.loads(payload)
                    except:
                        payload = {}
                
                app_data[app]["count"] += 1
                
                control_type = payload.get("control_type", "")
                element_name = payload.get("element_name", "")
                if control_type and element_name:
                    action = f"{control_type}:{element_name[:30]}"
                    app_data[app]["actions"][action] += 1
            
            # 저장
            timestamp = start_time.strftime("%Y-%m-%d %H:%M")
            
            for app, data in app_data.items():
                top_actions = sorted(data["actions"].items(), key=lambda x: x[1], reverse=True)[:5]
                
                cursor.execute("""
                    INSERT OR REPLACE INTO minute_aggregates
                    (timestamp, app, event_count, actions_json)
                    VALUES (?, ?, ?, ?)
                """, (
                    timestamp,
                    app,
                    data["count"],
                    json.dumps(top_actions, ensure_ascii=False)
                ))
            
            conn.commit()
            logger.debug(f"Aggregated {len(events)} events for {timestamp}")
            
            self._last_aggregation = now
            
        finally:
            conn.close()
    
    def _check_daily_tasks(self):
        """일일 작업 체크 (자정 근처에 실행)."""
        now = datetime.now()
        today = now.strftime("%Y-%m-%d")
        
        # 이미 오늘 실행했으면 SKIP
        if self._last_daily_cleanup == today:
            return
        
        # 오전 0시~1시 사이에만 실행
        if now.hour != 0:
            return
        
        logger.info("Running daily aggregation and cleanup...")
        
        conn = self._get_connection()
        try:
            # 어제 일일 요약 생성
            yesterday = (now - timedelta(days=1)).strftime("%Y-%m-%d")
            self._build_daily_summary(conn, yesterday)
            
            # 오래된 원시 이벤트 삭제
            self._cleanup_old_events(conn)
            
            self._last_daily_cleanup = today
            
        finally:
            conn.close()
    
    def _build_daily_summary(self, conn: sqlite3.Connection, target_date: str):
        """일일 요약 생성."""
        cursor = conn.cursor()
        
        # minute_aggregates에서 집계
        cursor.execute("""
            SELECT app, SUM(event_count) as total, actions_json
            FROM minute_aggregates
            WHERE date(timestamp) = ?
            GROUP BY app
            ORDER BY total DESC
        """, (target_date,))
        
        rows = cursor.fetchall()
        
        if not rows:
            return
        
        total_events = 0
        app_usage = {}
        all_actions = defaultdict(int)
        
        for row in rows:
            app = row["app"]
            count = row["total"]
            total_events += count
            app_usage[app] = count
            
            try:
                actions = json.loads(row["actions_json"] or "[]")
                for action, cnt in actions:
                    all_actions[action] += cnt
            except:
                pass
        
        # 활동 시간대 계산
        cursor.execute("""
            SELECT DISTINCT substr(timestamp, 12, 2) as hour
            FROM minute_aggregates
            WHERE date(timestamp) = ?
        """, (target_date,))
        active_hours = len(cursor.fetchall())
        
        # 상위 액션
        top_actions = sorted(all_actions.items(), key=lambda x: x[1], reverse=True)[:20]
        
        # 요약 텍스트
        summary_parts = [
            f"날짜: {target_date}",
            f"총 이벤트: {total_events}건",
            f"사용 앱: {len(app_usage)}개",
            f"활동 시간: {active_hours}시간"
        ]
        
        # 저장
        cursor.execute("""
            INSERT OR REPLACE INTO daily_summaries
            (date, total_events, total_apps, active_hours, app_usage_json, top_actions_json, summary_text)
            VALUES (?, ?, ?, ?, ?, ?, ?)
        """, (
            target_date,
            total_events,
            len(app_usage),
            active_hours,
            json.dumps(app_usage, ensure_ascii=False),
            json.dumps(dict(top_actions), ensure_ascii=False),
            "\n".join(summary_parts)
        ))
        
        conn.commit()
        logger.info(f"Built daily summary for {target_date}: {total_events} events")
    
    def _cleanup_old_events(self, conn: sqlite3.Connection):
        """오래된 원시 이벤트 삭제."""
        cursor = conn.cursor()
        
        # 원시 이벤트 삭제 (7일 이상)
        raw_cutoff = (datetime.now() - timedelta(days=self.raw_retention_days)).strftime("%Y-%m-%d")
        
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='events'")
        if cursor.fetchone():
            cursor.execute("DELETE FROM events WHERE date(ts) < ?", (raw_cutoff,))
            deleted = cursor.rowcount
            if deleted > 0:
                logger.info(f"Deleted {deleted} raw events older than {raw_cutoff}")
        
        # 분 단위 집약 삭제 (30일 이상)
        minute_cutoff = (datetime.now() - timedelta(days=self.summary_retention_days)).strftime("%Y-%m-%d")
        cursor.execute("DELETE FROM minute_aggregates WHERE date(timestamp) < ?", (minute_cutoff,))
        
        conn.commit()
        
        # VACUUM
        try:
            conn.execute("VACUUM")
        except:
            pass


def create_aggregator(db_path: str = "collector.db") -> RealtimeAggregator:
    """Collector에서 사용할 집약기 생성."""
    return RealtimeAggregator(
        db_path=db_path,
        aggregation_interval=300,  # 5분
        raw_retention_days=7,
        summary_retention_days=30
    )
