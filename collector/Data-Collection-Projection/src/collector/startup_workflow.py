"""
Startup Workflow Generator - 수집기 시작 시 이전 날짜까지의 패턴 분석.

수집기가 시작될 때 자동으로 호출되어:
1. 어제까지의 DB 데이터를 분석
2. 반복 패턴을 감지
3. JSON 워크플로우 파일 생성 (날짜별 버전 관리)
"""

import json
import logging
import os
import sqlite3
from collections import defaultdict
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional

logger = logging.getLogger(__name__)


class StartupWorkflowGenerator:
    """수집기 시작 시 워크플로우 자동 생성."""
    
    def __init__(
        self,
        db_path: str = "collector.db",
        output_dir: str = "workflows",
        min_events: int = 100,  # 최소 이벤트 수
        pattern_threshold: int = 3  # 최소 반복 횟수
    ):
        self.db_path = db_path
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(exist_ok=True)
        self.min_events = min_events
        self.pattern_threshold = pattern_threshold
    
    def generate_on_startup(self) -> Optional[Path]:
        """시작 시 워크플로우 생성."""
        yesterday = (datetime.now() - timedelta(days=1)).strftime("%Y-%m-%d")
        output_file = self.output_dir / f"workflow_{yesterday}.json"
        
        # 이미 오늘 생성했으면 SKIP
        if output_file.exists():
            logger.info(f"Workflow already exists: {output_file}")
            return None
        
        logger.info(f"Analyzing patterns up to {yesterday}...")
        
        try:
            conn = self._get_connection()
            
            # 어제까지의 이벤트 조회
            events = self._fetch_events_until(conn, yesterday)
            
            if len(events) < self.min_events:
                logger.info(f"Not enough events ({len(events)} < {self.min_events}). Skipping workflow generation.")
                conn.close()
                return None
            
            # 패턴 분석
            patterns = self._analyze_patterns(events)
            
            if not patterns:
                logger.info("No significant patterns detected.")
                conn.close()
                return None
            
            # 워크플로우 생성
            workflow = self._build_workflow(patterns, yesterday)
            
            # 저장
            with open(output_file, "w", encoding="utf-8") as f:
                json.dump(workflow, f, ensure_ascii=False, indent=2)
            
            logger.info(f"Workflow generated: {output_file}")
            conn.close()
            
            return output_file
            
        except Exception as e:
            logger.error(f"Failed to generate workflow: {e}")
            return None
    
    def _get_connection(self) -> sqlite3.Connection:
        """DB 연결."""
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        return conn
    
    def _fetch_events_until(self, conn: sqlite3.Connection, until_date: str) -> List[Dict]:
        """특정 날짜까지의 이벤트 조회."""
        cursor = conn.cursor()
        
        # events 테이블 확인
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='events'")
        if not cursor.fetchone():
            return []
        
        cursor.execute("""
            SELECT ts, app, event_type, payload
            FROM events
            WHERE date(ts) <= ?
            ORDER BY ts ASC
        """, (until_date,))
        
        events = []
        for row in cursor.fetchall():
            payload = row["payload"]
            if isinstance(payload, str):
                try:
                    payload = json.loads(payload)
                except:
                    payload = {}
            
            events.append({
                "ts": row["ts"],
                "app": row["app"],
                "event_type": row["event_type"],
                "payload": payload
            })
        
        return events
    
    def _analyze_patterns(self, events: List[Dict]) -> List[Dict]:
        """반복 패턴 감지."""
        # 앱 + 액션 조합별 빈도 계산
        action_counts = defaultdict(lambda: {"count": 0, "examples": []})
        
        for event in events:
            app = event.get("app", "unknown")
            payload = event.get("payload", {})
            control_type = payload.get("control_type", "")
            element_name = payload.get("element_name", "")
            
            if not control_type or not element_name:
                continue
            
            # 앱 이름 정규화
            app_short = app.split(" - ")[-1].split(".")[0] if " - " in app else app.split(".")[0]
            
            key = f"{app_short}|{control_type}|{element_name}"
            action_counts[key]["count"] += 1
            
            if len(action_counts[key]["examples"]) < 3:
                action_counts[key]["examples"].append({
                    "ts": event["ts"],
                    "window_title": payload.get("window_title", ""),
                    "automation_id": payload.get("automation_id", "")
                })
        
        # 임계값 이상 반복된 패턴만 추출
        patterns = []
        for key, data in action_counts.items():
            if data["count"] >= self.pattern_threshold:
                parts = key.split("|")
                patterns.append({
                    "app": parts[0],
                    "control_type": parts[1],
                    "element_name": parts[2],
                    "frequency": data["count"],
                    "examples": data["examples"]
                })
        
        # 빈도순 정렬
        patterns.sort(key=lambda x: x["frequency"], reverse=True)
        
        return patterns[:20]  # 상위 20개만
    
    def _build_workflow(self, patterns: List[Dict], date: str) -> Dict:
        """패턴에서 워크플로우 생성."""
        steps = []
        
        for i, pattern in enumerate(patterns, 1):
            step = {
                "step_number": i,
                "action_type": self._infer_action_type(pattern["control_type"]),
                "target": {
                    "app": pattern["app"],
                    "control_type": pattern["control_type"],
                    "element_name": pattern["element_name"],
                    "window_title": pattern["examples"][0].get("window_title", "") if pattern["examples"] else "",
                    "automation_id": pattern["examples"][0].get("automation_id", "") if pattern["examples"] else ""
                },
                "frequency": pattern["frequency"],
                "description": f"{pattern['app']}에서 {pattern['control_type']} '{pattern['element_name']}' ({pattern['frequency']}회 반복)"
            }
            steps.append(step)
        
        workflow = {
            "workflow_name": f"Daily Patterns - {date}",
            "description": f"{date}까지의 사용자 행동 패턴 분석 결과",
            "created_at": datetime.now().isoformat(),
            "analysis_period": {
                "until": date,
                "events_analyzed": sum(p["frequency"] for p in patterns)
            },
            "patterns": steps,
            "metadata": {
                "total_patterns": len(patterns),
                "top_apps": list(set(p["app"] for p in patterns[:5])),
                "generated_by": "startup_workflow_generator"
            }
        }
        
        return workflow
    
    def _infer_action_type(self, control_type: str) -> str:
        """컨트롤 타입에서 액션 추론."""
        action_map = {
            "Button": "click",
            "MenuItem": "click",
            "Edit": "type",
            "Text": "read",
            "ListItem": "select",
            "TreeItem": "select",
            "TabItem": "click",
            "Hyperlink": "click",
            "CheckBox": "toggle",
            "RadioButton": "select"
        }
        return action_map.get(control_type, "interact")


def generate_workflow_on_startup(db_path: str = "collector.db") -> Optional[Path]:
    """수집기에서 호출하는 헬퍼 함수."""
    generator = StartupWorkflowGenerator(db_path=db_path)
    return generator.generate_on_startup()
