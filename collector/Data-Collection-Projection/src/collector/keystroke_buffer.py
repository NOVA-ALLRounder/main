"""
Keystroke Buffer - 키보드 입력을 버퍼링하여 Action으로 변환.

Electron 앱 등 UI Automation이 작동하지 않는 환경에서도
실제 키 입력을 캡처하여 Action 기반 저장을 유지합니다.

동작 방식:
- 문자 키 입력 → 버퍼에 누적
- 창 전환, Enter, Tab, Idle timeout → 버퍼 flush → Action 저장
- Backspace → 버퍼에서 문자 삭제
"""

import logging
import threading
import time
import ctypes
from datetime import datetime
from typing import Optional, Callable
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class WindowInfo:
    """현재 창 정보."""
    title: str
    hwnd: int
    process_name: str = ""
    
    def get_key(self) -> str:
        return f"{self.hwnd}|{self.title}"


def get_active_window_title() -> Optional[WindowInfo]:
    """현재 활성 창 정보 가져오기."""
    try:
        user32 = ctypes.windll.user32
        hwnd = user32.GetForegroundWindow()
        if not hwnd:
            return None
        
        length = user32.GetWindowTextLengthW(hwnd)
        buf = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buf, length + 1)
        
        return WindowInfo(title=buf.value, hwnd=hwnd)
    except Exception as e:
        logger.warning(f"Failed to get window title: {e}")
        return None


class KeystrokeBuffer:
    """
    키보드 입력 버퍼.
    
    연속된 키 입력을 버퍼에 누적하고, 특정 조건 발생 시 flush하여
    Action으로 변환합니다.
    
    Flush 조건:
    - 창 전환 (포커스 변경)
    - Enter/Tab 키 입력
    - 2분 무입력 (idle timeout)
    - 앱 종료
    """
    
    def __init__(
        self,
        on_input_complete: Optional[Callable[[str, WindowInfo, int], None]] = None,
        idle_timeout_sec: int = 120,
    ):
        """
        Args:
            on_input_complete: (입력 문자열, 창 정보, duration_ms) 콜백
            idle_timeout_sec: Idle timeout (기본 2분)
        """
        self._on_input_complete = on_input_complete
        self._idle_timeout_sec = idle_timeout_sec
        
        # 버퍼 상태
        self._buffer: str = ""
        self._current_window: Optional[WindowInfo] = None
        self._input_started_at: Optional[datetime] = None
        self._last_activity_time: float = time.time()
        
        # 스레드 안전
        self._lock = threading.Lock()
        self._running = False
        self._timer_thread: Optional[threading.Thread] = None
    
    def start(self):
        """버퍼 시작."""
        self._running = True
        self._timer_thread = threading.Thread(target=self._timer_loop, daemon=True)
        self._timer_thread.start()
        logger.info("KeystrokeBuffer started")
    
    def stop(self):
        """버퍼 중지 (남은 내용 flush)."""
        self._running = False
        self._flush_buffer()
        logger.info("KeystrokeBuffer stopped")
    
    def on_keystroke(self, char: str):
        """
        문자 키 입력 처리.
        
        Args:
            char: 입력된 문자 (예: "a", "안", "1")
        """
        with self._lock:
            current_window = get_active_window_title()
            
            # 창 변경 감지
            if self._current_window and current_window:
                if self._current_window.get_key() != current_window.get_key():
                    self._flush_buffer()
                    self._current_window = current_window
            elif not self._current_window:
                self._current_window = current_window
            
            # 버퍼에 추가
            if not self._input_started_at:
                self._input_started_at = datetime.now()
            
            self._buffer += char
            self._last_activity_time = time.time()
    
    def on_backspace(self):
        """Backspace 키 처리."""
        with self._lock:
            if self._buffer:
                self._buffer = self._buffer[:-1]
            self._last_activity_time = time.time()
    
    def on_enter_or_tab(self, key: str):
        """
        Enter/Tab 키 처리 → 버퍼 flush.
        
        Args:
            key: "enter" 또는 "tab"
        """
        with self._lock:
            self._flush_buffer()
            self._last_activity_time = time.time()
    
    def on_window_change(self, new_window: Optional[WindowInfo] = None):
        """
        창 전환 처리 → 버퍼 flush.
        
        Args:
            new_window: 새 창 정보 (없으면 자동 감지)
        """
        with self._lock:
            self._flush_buffer()
            self._current_window = new_window or get_active_window_title()
            self._last_activity_time = time.time()
    
    def _flush_buffer(self):
        """버퍼 내용을 Action으로 변환."""
        if not self._buffer or not self._current_window:
            self._buffer = ""
            self._input_started_at = None
            return
        
        # Duration 계산
        now = datetime.now()
        started = self._input_started_at or now
        duration_ms = int((now - started).total_seconds() * 1000)
        
        # 콜백 호출
        if self._on_input_complete:
            try:
                self._on_input_complete(
                    self._buffer,
                    self._current_window,
                    duration_ms
                )
            except Exception as e:
                logger.error(f"Error in on_input_complete callback: {e}")
        
        # 로깅
        display_value = self._buffer[:50] + "..." if len(self._buffer) > 50 else self._buffer
        print(f"[INPUT] {self._current_window.title}: \"{display_value}\"", flush=True)
        
        # 버퍼 초기화
        self._buffer = ""
        self._input_started_at = None
    
    def _timer_loop(self):
        """Idle timeout 체크 루프."""
        while self._running:
            time.sleep(1)
            
            with self._lock:
                if not self._buffer:
                    continue
                
                idle_duration = time.time() - self._last_activity_time
                if idle_duration >= self._idle_timeout_sec:
                    logger.info(f"Idle timeout ({self._idle_timeout_sec}s) - flushing keystroke buffer")
                    self._flush_buffer()
    
    @property
    def current_buffer(self) -> str:
        """현재 버퍼 내용 (디버그용)."""
        with self._lock:
            return self._buffer
