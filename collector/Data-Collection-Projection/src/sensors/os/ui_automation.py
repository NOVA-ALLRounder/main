"""
UI Automation 센서 - 활성 UI 요소 추적 (통합 버전).

os_data의 상세 캡처 + os_test의 emit 시그니처.

주요 기능:
- focus.change: 포커스 변경 시 이전 컨트롤의 최종 값 포함
- value.change: 입력값 변경 추적
- 3가지 값 추출: ValuePattern → TextPattern → LegacyIAccessible
- 적응형 키보드 모드 (Electron 앱용 IPC 시그널)
- MAX_VALUE_LENGTH = 500
"""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import signal
import sys
import threading
import time
from dataclasses import dataclass, field
from http.server import HTTPServer, BaseHTTPRequestHandler
from typing import Any, Dict, List, Optional

try:
    import uiautomation as auto
except ImportError:
    auto = None  # type: ignore

import sys
from pathlib import Path

# Add src directory to sys.path to allow importing 'emit' module
src_path = Path(__file__).resolve().parents[2]
if str(src_path) not in sys.path:
    sys.path.append(str(src_path))

from .emit import EmitConfig, HttpEmitter, build_event

logger = logging.getLogger(__name__)

# ── 설정 ──────────────────────────────────────────────

MAX_VALUE_LENGTH = 500
POLL_INTERVAL = 0.5

# 데이터 경량화: 무의미한 컨트롤 타입 필터
SKIP_CONTROL_TYPES = {
    "PaneControl", "GroupControl", "WindowControl",
    "DocumentControl", "CustomControl", "ToolBarControl",
    "MenuBarControl", "StatusBarControl", "ScrollBarControl",
    "ThumbControl", "SeparatorControl", "ImageControl",
    "TitleBarControl",
}

# ── ControlInfo ──────────────────────────────────────

@dataclass
class ControlInfo:
    """현재 포커스된 UI 요소 정보."""
    control_type: str = ""
    name: str = ""
    value: str = ""
    auto_id: str = ""
    window_title: str = ""
    window_handle: int = 0
    pid: int = 0
    class_name: str = ""
    control_path: str = ""

    def get_dedup_key(self) -> str:
        """중복 방지용 고유 키."""
        raw = f"{self.window_handle}|{self.auto_id}|{self.control_type}|{self.name}"
        return hashlib.md5(raw.encode()).hexdigest()[:12]

    def to_payload(self) -> Dict[str, Any]:
        return {
            "window_title": self.window_title,
            "control_type": self.control_type,
            "element_name": self.name,
            "element_value": self.value[:MAX_VALUE_LENGTH] if self.value else "",
            "automation_id": self.auto_id,
            "class_name": self.class_name,
            "control_path": self.control_path,
        }


# ── 값 추출 (3가지 방법) ──────────────────────────────

def _try_get_value(ctrl) -> str:
    """ValuePattern → TextPattern → LegacyIAccessible 순으로 값 추출."""
    if ctrl is None:
        return ""

    # 1) ValuePattern
    try:
        pattern = ctrl.GetValuePattern()
        if pattern:
            val = pattern.Value
            if val:
                return str(val)[:MAX_VALUE_LENGTH]
    except Exception:
        pass

    # 2) TextPattern
    try:
        pattern = ctrl.GetTextPattern()
        if pattern:
            val = pattern.DocumentRange.GetText(MAX_VALUE_LENGTH)
            if val:
                return str(val)[:MAX_VALUE_LENGTH]
    except Exception:
        pass

    # 3) LegacyIAccessiblePattern
    try:
        pattern = ctrl.GetLegacyIAccessiblePattern()
        if pattern:
            val = pattern.Value
            if val:
                return str(val)[:MAX_VALUE_LENGTH]
    except Exception:
        pass

    return ""


def _get_control_path(ctrl) -> str:
    """UI 요소의 경로 (Automation ID 기반)."""
    parts: List[str] = []
    try:
        current = ctrl
        depth = 0
        while current and depth < 5:
            aid = current.AutomationId
            name = current.Name or ""
            ctype = current.ControlTypeName or ""
            if aid:
                parts.append(f"{ctype}[{aid}]")
            elif name:
                short = name[:30]
                parts.append(f"{ctype}['{short}']")
            else:
                parts.append(ctype)
            try:
                current = current.GetParentControl()
            except Exception:
                break
            depth += 1
    except Exception:
        pass
    parts.reverse()
    return " > ".join(parts) if parts else ""


# ── 적응형 키보드 IPC ──────────────────────────────────

class _AdaptiveSignalHandler(BaseHTTPRequestHandler):
    """input_hook에서 키보드 상태 쿼리를 받는 IPC 핸들러."""

    _keyboard_needed = False

    def do_GET(self):
        if self.path == "/keyboard-status":
            resp = json.dumps({"keyboard_logging": self._keyboard_needed}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(resp)
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        pass  # suppress request logs


def _start_adaptive_server(port: int = 8089) -> HTTPServer:
    server = HTTPServer(("127.0.0.1", port), _AdaptiveSignalHandler)
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    logger.info("Adaptive keyboard IPC server on port %d", port)
    return server


# ── 메인 센서 루프 ──────────────────────────────────────

class UIAutomationSensor:
    """UI Automation 기반 활성 요소 추적 센서."""

    def __init__(
        self,
        emitter: HttpEmitter,
        poll_interval: float = POLL_INTERVAL,
        adaptive_keyboard: bool = True,
        ipc_port: int = 8089,
    ):
        self._emitter = emitter
        self._poll = poll_interval
        self._running = False

        # 이전 컨트롤 정보
        self._prev_info: Optional[ControlInfo] = None
        self._prev_dedup_key: str = ""
        self._prev_value: str = ""

        # 적응형 키보드
        self._adaptive = adaptive_keyboard
        self._ipc_server: Optional[HTTPServer] = None
        self._ipc_port = ipc_port

        # UI Automation 지원 여부 캐시 (per window)
        self._ua_support_cache: Dict[int, bool] = {}

    def start(self):
        if auto is None:
            logger.error("uiautomation 패키지가 설치되지 않았습니다.")
            return
        self._running = True
        if self._adaptive:
            self._ipc_server = _start_adaptive_server(self._ipc_port)
        logger.info("UIAutomationSensor started (poll=%.1fs)", self._poll)
        self._run_loop()

    def stop(self):
        self._running = False
        if self._ipc_server:
            self._ipc_server.shutdown()

    def _run_loop(self):
        while self._running:
            try:
                self._poll_once()
            except KeyboardInterrupt:
                break
            except Exception as e:
                logger.debug("poll error: %s", e)
            time.sleep(self._poll)

    def _poll_once(self):
        ctrl = auto.GetFocusedControl()
        if ctrl is None:
            return

        info = self._build_info(ctrl)

        # 스킵할 컨트롤 타입 필터링
        if info.control_type in SKIP_CONTROL_TYPES:
            return

        dedup_key = info.get_dedup_key()

        # ── 포커스 변경 감지 ──
        if dedup_key != self._prev_dedup_key:
            self._emit_focus_change(info)
            self._prev_info = info
            self._prev_dedup_key = dedup_key
            self._prev_value = info.value
            return

        # ── 값 변경 감지 ──
        if info.value and info.value != self._prev_value:
            self._emit_value_change(info)
            self._prev_value = info.value

    def _build_info(self, ctrl) -> ControlInfo:
        """UI 요소에서 ControlInfo 생성."""
        info = ControlInfo()
        try:
            info.control_type = ctrl.ControlTypeName or ""
            info.name = ctrl.Name or ""
            info.auto_id = ctrl.AutomationId or ""
            info.class_name = ctrl.ClassName or ""
            info.value = _try_get_value(ctrl)
            info.control_path = _get_control_path(ctrl)
        except Exception as e:
            logger.debug("build_info error: %s", e)

        # 창 정보
        try:
            win = ctrl
            depth = 0
            while win and depth < 10:
                if win.ControlTypeName == "WindowControl":
                    info.window_title = win.Name or ""
                    info.window_handle = win.NativeWindowHandle or 0
                    try:
                        info.pid = win.ProcessId or 0
                    except Exception:
                        pass
                    break
                try:
                    win = win.GetParentControl()
                except Exception:
                    break
                depth += 1
        except Exception:
            pass

        # 적응형 모드: UA 지원 여부 판단
        if self._adaptive and info.window_handle:
            ua_ok = bool(info.control_type and info.name)
            self._ua_support_cache[info.window_handle] = ua_ok
            _AdaptiveSignalHandler._keyboard_needed = not ua_ok

        return info

    def _parse_window_title(self, info: ControlInfo) -> tuple:
        """창 제목에서 앱 이름과 문서 이름 분리.

        예시:
          "개인서.xlsx - Excel"  → ("Excel", "개인서.xlsx")
          "받은 편지함 - hong@company.com - Outlook" → ("Outlook", "받은 편지함 - hong@company.com")
          "Visual Studio Code"  → ("Visual Studio Code", "")
        """
        title = info.window_title
        if " - " in title:
            parts = title.rsplit(" - ", 1)
            app = parts[-1].strip()
            doc = parts[0].strip()
            return app, doc
        return title, ""

    def _emit_focus_change(self, new_info: ControlInfo):
        """포커스 변경 이벤트 전송 (이전 컨트롤의 최종값 포함)."""
        payload = new_info.to_payload()

        # ★ 핵심: 이전 컨트롤의 최종 값을 함께 전송
        if self._prev_info:
            payload["prev_control"] = {
                "control_type": self._prev_info.control_type,
                "element_name": self._prev_info.name,
                "final_value": self._prev_value[:MAX_VALUE_LENGTH] if self._prev_value else "",
                "automation_id": self._prev_info.auto_id,
                "control_path": self._prev_info.control_path,
            }

        app, doc = self._parse_window_title(new_info)
        payload["doc_name"] = doc

        event = build_event(
            source="sensor.ui_automation",
            app=app,
            event_type="focus.change",
            resource_type="ui_element",
            resource_id=new_info.get_dedup_key(),
            payload=payload,
            priority="P1",
            window_id=str(new_info.window_handle) if new_info.window_handle else None,
            pid=new_info.pid or None,
        )
        self._emitter.send_event(event)

    def _emit_value_change(self, info: ControlInfo):
        """값 변경 이벤트 전송."""
        payload = info.to_payload()
        app, doc = self._parse_window_title(info)
        payload["doc_name"] = doc

        event = build_event(
            source="sensor.ui_automation",
            app=app,
            event_type="value.change",
            resource_type="ui_element",
            resource_id=info.get_dedup_key(),
            payload=payload,
            priority="P1",
            window_id=str(info.window_handle) if info.window_handle else None,
            pid=info.pid or None,
        )
        self._emitter.send_event(event)


# ── CLI ──────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="UI Automation Sensor")
    parser.add_argument("--ingest-url", default="http://127.0.0.1:8080/events")
    parser.add_argument("--poll", type=float, default=POLL_INTERVAL)
    parser.add_argument("--adaptive-keyboard", action="store_true", default=True)
    parser.add_argument("--no-adaptive-keyboard", dest="adaptive_keyboard", action="store_false")
    parser.add_argument("--ipc-port", type=int, default=8089)
    args = parser.parse_args()

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s – %(message)s",
    )

    config = EmitConfig(ingest_url=args.ingest_url)
    emitter = HttpEmitter(config)
    sensor = UIAutomationSensor(
        emitter,
        poll_interval=args.poll,
        adaptive_keyboard=args.adaptive_keyboard,
        ipc_port=args.ipc_port,
    )

    def _stop(*_):
        sensor.stop()
        emitter.stop()
        sys.exit(0)

    signal.signal(signal.SIGINT, _stop)
    signal.signal(signal.SIGTERM, _stop)

    sensor.start()


if __name__ == "__main__":
    main()
