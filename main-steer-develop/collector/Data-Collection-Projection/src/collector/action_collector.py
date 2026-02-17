"""
Action Collector - Event를 Action으로 변환하고 SQLiteStore에 저장.

os_data의 ActionCollector를 os_test의 SQLiteStore와 통합.
독립 SQLite 접근 대신 SQLiteStore의 actions 테이블을 사용합니다.
"""

import json
import logging
from typing import Dict, Any, List, Optional

try:
    from .action_buffer import ActionRecord
except ImportError:
    from action_buffer import ActionRecord

logger = logging.getLogger(__name__)


activity_text_logger = logging.getLogger("collector.activity_text")


class ActionCollector:
    """
    ActionBuffer에서 생성된 Action을 수집하여 SQLiteStore에 저장.

    이벤트 흐름:
    1. UI Automation/Input Hook에서 이벤트 수신
    2. ActionBuffer에서 버퍼링 및 Action 생성
    3. ActionCollector에서 DB 저장 및 로깅
    """

    def __init__(self, store, debug_mode: bool = False):
        """
        Args:
            store: SQLiteStore 인스턴스 (insert_action 메서드 필요)
            debug_mode: 상세 출력 여부
        """
        self._store = store
        self._debug_mode = debug_mode
        self._action_count = 0

    def on_action(self, action: ActionRecord):
        """Action 수신 콜백."""
        saved = self._store.insert_action(action)

        if saved:
            self._action_count += 1
            self._log_action(action)

    def _log_action(self, action: ActionRecord):
        """Action을 사람이 읽기 쉬운 형태로 로깅."""
        app = action.app
        app_display = app.rsplit(".", 1)[0] if "." in app else app

        if action.action_type == "input":
            value = action.final_value
            if len(value) > 50:
                value = value[:47] + "..."
            dur = f" ({action.duration_ms}ms)" if action.duration_ms else ""
            msg = f'[입력]   {app_display:<14} | {action.control_name}: "{value}"{dur}'

        elif action.action_type == "click":
            meta = action.metadata
            button = meta.get("button", "left")
            btn_name = {"left": "왼쪽", "right": "오른쪽", "middle": "가운데"}.get(button, button)
            target = action.control_name or action.window_title or ""
            msg = f"[클릭]   {app_display:<14} | {target} ({btn_name})"

        elif action.action_type == "shortcut":
            msg = f"[단축키] {app_display:<14} | {action.final_value}"

        elif action.action_type == "navigate":
            msg = f"[이동]   {app_display:<14} | -> {action.window_title}"

        else:
            target = action.control_name or action.window_title or ""
            msg = f"[{action.action_type}] {app_display:<14} | {target}"

        activity_text_logger.info(msg)

        if self._debug_mode:
            logger.debug("action detail: %s", action.to_dict())

    def get_stats(self) -> Dict[str, Any]:
        """통계 정보."""
        return {
            "total_actions_collected": self._action_count,
            "debug_mode": self._debug_mode,
        }
