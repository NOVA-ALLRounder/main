"""
Input Hook 센서 - 마우스 클릭, 단축키, 적응형 키보드 입력 (통합 버전).

os_test 기능 유지:
- 마우스 클릭 (UI 요소 기준, x/y 좌표 제거)
- 특수키 (Enter, Tab, Esc, F1-F12)
- 단축키 (Ctrl+S, Alt+Tab 등)

os_data에서 통합:
- 적응형 키보드 모드: UI Automation 미지원 앱에서만 문자 키 버퍼링
- IPC로 ui_automation 센서에서 keyboard_logging 상태 수신
- HangulConverter로 영문→한글 변환
"""

from __future__ import annotations

import argparse
import json
import logging
import signal
import sys
import threading
import time
import urllib.request
import urllib.error
from typing import Any, Dict, Optional, Set

try:
    from pynput import keyboard, mouse
except ImportError:
    keyboard = None  # type: ignore
    mouse = None  # type: ignore

import sys
from pathlib import Path

# Add src directory to sys.path to allow importing 'emit' module
src_path = Path(__file__).resolve().parents[2]
if str(src_path) not in sys.path:
    sys.path.append(str(src_path))

from .emit import EmitConfig, HttpEmitter, build_event

logger = logging.getLogger(__name__)

# ── 설정 ──────────────────────────────────────────────

DEBOUNCE_SEC = 0.3
KEYSTROKE_IDLE_TIMEOUT = 120  # 2분

LOGGABLE_SPECIAL_KEYS = {
    "Key.enter", "Key.tab", "Key.esc", "Key.delete",
    "Key.f1", "Key.f2", "Key.f3", "Key.f4", "Key.f5",
    "Key.f6", "Key.f7", "Key.f8", "Key.f9", "Key.f10",
    "Key.f11", "Key.f12", "Key.home", "Key.end",
    "Key.page_up", "Key.page_down", "Key.insert",
    "Key.print_screen",
}

MODIFIER_KEYS = {"Key.ctrl_l", "Key.ctrl_r", "Key.alt_l", "Key.alt_r",
                 "Key.shift", "Key.shift_l", "Key.shift_r",
                 "Key.cmd", "Key.cmd_l", "Key.cmd_r"}

FLUSH_TRIGGER_KEYS = {"Key.enter", "Key.tab", "Key.esc"}


# ── KeystrokeBuffer (내장, 간소화) ──────────────────────

class KeystrokeBuffer:
    """적응형 모드에서 문자 키 입력 버퍼링.

    UI Automation이 안 되는 앱(Electron 등)에서만 활성화됩니다.
    Enter/Tab/창전환/idle timeout 시 flush하여 keyboard.input 이벤트로 전송.
    """

    def __init__(self, emitter: HttpEmitter, idle_timeout: int = KEYSTROKE_IDLE_TIMEOUT):
        self._emitter = emitter
        self._idle_timeout = idle_timeout
        self._buffer: str = ""
        self._started_at: Optional[float] = None
        self._last_activity: float = time.time()
        self._lock = threading.Lock()
        self._current_app: str = "unknown"
        self._current_window_id: Optional[str] = None

        # idle 타이머
        self._running = True
        self._timer = threading.Thread(target=self._idle_loop, daemon=True)
        self._timer.start()

    def set_context(self, app: str, window_id: Optional[str] = None):
        with self._lock:
            if self._current_app != app:
                self._flush()
            self._current_app = app
            self._current_window_id = window_id

    def on_char(self, char: str):
        with self._lock:
            if not self._started_at:
                self._started_at = time.time()
            self._buffer += char
            self._last_activity = time.time()

    def on_backspace(self):
        with self._lock:
            if self._buffer:
                self._buffer = self._buffer[:-1]
            self._last_activity = time.time()

    def on_flush_trigger(self):
        with self._lock:
            self._flush()

    def stop(self):
        self._running = False
        with self._lock:
            self._flush()

    def _flush(self):
        if not self._buffer:
            self._buffer = ""
            self._started_at = None
            return

        duration_ms = 0
        if self._started_at:
            duration_ms = int((time.time() - self._started_at) * 1000)

        # 한글 변환 시도
        text = self._buffer
        try:
            # hangul_converter는 collector 레벨에 있으므로 여기서는 그대로 전송
            # EventBus에서 변환 처리
            pass
        except Exception:
            pass

        event = build_event(
            source="sensor.input_hook",
            app=self._current_app,
            event_type="keyboard.input",
            resource_type="text_input",
            resource_id="keystroke_buffer",
            payload={
                "text": text[:500],
                "char_count": len(text),
                "duration_ms": duration_ms,
            },
            priority="P1",
            window_id=self._current_window_id,
        )
        self._emitter.send_event(event)

        self._buffer = ""
        self._started_at = None

    def _idle_loop(self):
        while self._running:
            time.sleep(1)
            with self._lock:
                if self._buffer and (time.time() - self._last_activity) >= self._idle_timeout:
                    self._flush()


# ── 메인 센서 ──────────────────────────────────────────

class InputHookSensor:
    """마우스/키보드 입력 캡처 센서."""

    def __init__(
        self,
        emitter: HttpEmitter,
        adaptive_mode: bool = True,
        ipc_url: str = "http://127.0.0.1:8089/keyboard-status",
        ipc_poll_sec: float = 2.0,
    ):
        self._emitter = emitter
        self._running = False

        # 단축키 추적
        self._pressed_modifiers: Set[str] = set()
        self._last_event_time: float = 0

        # 적응형 모드
        self._adaptive = adaptive_mode
        self._keyboard_logging = False
        self._ipc_url = ipc_url
        self._ipc_poll_sec = ipc_poll_sec
        self._keystroke_buffer: Optional[KeystrokeBuffer] = None

        if self._adaptive:
            self._keystroke_buffer = KeystrokeBuffer(emitter)

    def start(self):
        if keyboard is None or mouse is None:
            logger.error("pynput 패키지가 설치되지 않았습니다.")
            return
        self._running = True

        # 적응형 IPC 폴링 시작
        if self._adaptive:
            t = threading.Thread(target=self._ipc_poll_loop, daemon=True)
            t.start()

        # pynput 리스너 시작
        mouse_listener = mouse.Listener(on_click=self._on_mouse_click)
        key_listener = keyboard.Listener(
            on_press=self._on_key_press,
            on_release=self._on_key_release,
        )

        mouse_listener.start()
        key_listener.start()
        logger.info("InputHookSensor started (adaptive=%s)", self._adaptive)

        key_listener.join()

    def stop(self):
        self._running = False
        if self._keystroke_buffer:
            self._keystroke_buffer.stop()

    # ── 마우스 ────────────────────────────────────

    def _on_mouse_click(self, x, y, button, pressed):
        if not pressed:
            return
        if not self._debounce():
            return

        # x/y 좌표 제거 → 버튼만 전송 (UI 요소는 ui_automation에서 식별)
        button_name = str(button).replace("Button.", "")
        event = build_event(
            source="sensor.input_hook",
            app="unknown",
            event_type="mouse.click",
            resource_type="mouse",
            resource_id="mouse_click",
            payload={
                "button": button_name,
            },
            priority="P2",
        )
        self._emitter.send_event(event)

    # ── 키보드 ────────────────────────────────────

    def _on_key_press(self, key):
        key_str = self._key_to_str(key)

        # 모디파이어 키 추적
        if key_str in MODIFIER_KEYS:
            self._pressed_modifiers.add(key_str)
            return

        # 단축키 감지 (Ctrl/Alt/Cmd + 키)
        if self._pressed_modifiers:
            self._emit_shortcut(key_str)
            return

        # Enter/Tab → 키스트로크 버퍼 flush + 특수키 이벤트
        if key_str in FLUSH_TRIGGER_KEYS:
            if self._keystroke_buffer and self._keyboard_logging:
                self._keystroke_buffer.on_flush_trigger()
            if self._debounce():
                self._emit_special_key(key_str)
            return

        # Backspace → 키스트로크 버퍼에서 삭제
        if key_str == "Key.backspace":
            if self._keystroke_buffer and self._keyboard_logging:
                self._keystroke_buffer.on_backspace()
            return

        # 특수키
        if key_str in LOGGABLE_SPECIAL_KEYS:
            if self._debounce():
                self._emit_special_key(key_str)
            return

        # ★ 적응형 모드: 문자 키 버퍼링 (keyboard_logging=True일 때만)
        if self._keyboard_logging and self._keystroke_buffer:
            char = self._key_to_char(key)
            if char:
                self._keystroke_buffer.on_char(char)

    def _on_key_release(self, key):
        key_str = self._key_to_str(key)
        self._pressed_modifiers.discard(key_str)

    # ── 이벤트 전송 ──────────────────────────────

    def _emit_shortcut(self, key_str: str):
        if not self._debounce():
            return

        modifiers = sorted(self._pressed_modifiers)
        mod_labels = []
        for m in modifiers:
            m_clean = m.replace("Key.", "").replace("_l", "").replace("_r", "")
            mod_labels.append(m_clean.capitalize())
        combo = "+".join(mod_labels + [key_str.replace("Key.", "").upper()])

        event = build_event(
            source="sensor.input_hook",
            app="unknown",
            event_type="keyboard.shortcut",
            resource_type="keyboard",
            resource_id="shortcut",
            payload={"combination": combo},
            priority="P1",
        )
        self._emitter.send_event(event)

    def _emit_special_key(self, key_str: str):
        key_name = key_str.replace("Key.", "")
        event = build_event(
            source="sensor.input_hook",
            app="unknown",
            event_type="keyboard.special_key",
            resource_type="keyboard",
            resource_id=key_name,
            payload={"key": key_name},
            priority="P2",
        )
        self._emitter.send_event(event)

    # ── 유틸 ──────────────────────────────────────

    def _debounce(self) -> bool:
        now = time.time()
        if now - self._last_event_time < DEBOUNCE_SEC:
            return False
        self._last_event_time = now
        return True

    def _key_to_str(self, key) -> str:
        try:
            return str(key)
        except Exception:
            return ""

    def _key_to_char(self, key) -> Optional[str]:
        """키 객체에서 문자 추출."""
        try:
            if hasattr(key, "char") and key.char:
                return key.char
        except Exception:
            pass
        return None

    # ── 적응형 IPC 폴링 ──────────────────────────

    def _ipc_poll_loop(self):
        """ui_automation 센서에서 keyboard_logging 상태를 주기적으로 확인."""
        while self._running:
            try:
                req = urllib.request.Request(self._ipc_url)
                with urllib.request.urlopen(req, timeout=1.0) as resp:
                    data = json.loads(resp.read().decode())
                    new_state = data.get("keyboard_logging", False)
                    if new_state != self._keyboard_logging:
                        self._keyboard_logging = new_state
                        logger.info(
                            "Adaptive keyboard mode: %s",
                            "ENABLED" if new_state else "DISABLED"
                        )
            except Exception:
                pass
            time.sleep(self._ipc_poll_sec)


# ── CLI ────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Input Hook Sensor")
    parser.add_argument("--ingest-url", default="http://127.0.0.1:8080/events")
    parser.add_argument("--adaptive", action="store_true", default=True)
    parser.add_argument("--no-adaptive", dest="adaptive", action="store_false")
    parser.add_argument("--ipc-url", default="http://127.0.0.1:8089/keyboard-status")
    args = parser.parse_args()

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s – %(message)s",
    )

    config = EmitConfig(ingest_url=args.ingest_url)
    emitter = HttpEmitter(config)
    sensor = InputHookSensor(
        emitter,
        adaptive_mode=args.adaptive,
        ipc_url=args.ipc_url,
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
