"""
Action Buffer - 동일 컨트롤에서의 연속 입력을 버퍼링하여 최종값만 Action으로 변환.

핵심 기능:
- 포커스 변경 시 이전 버퍼 flush → Action 생성
- Idle Timeout (2분) 시 자동 flush
- 종료 시 모든 버퍼 flush
- 주기적 백업 (30초)
"""

import json
import logging
import os
import threading
import time
import uuid
from dataclasses import dataclass, field, asdict
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any, Callable, List

logger = logging.getLogger(__name__)


@dataclass
class ControlContext:
    """현재 포커스된 컨트롤의 컨텍스트."""
    app: str
    window_title: str
    control_name: str
    control_type: str
    automation_id: str = ""
    window_id: str = ""
    pid: int = 0
    
    def get_key(self) -> str:
        """컨트롤 고유 키 (같은 컨트롤인지 판별용)."""
        return f"{self.window_id}|{self.control_name}|{self.control_type}"


@dataclass
class ActionRecord:
    """저장될 Action 레코드."""
    action_id: str
    action_type: str  # input, click, navigate, submit
    app: str
    window_title: str
    control_name: str
    control_type: str
    final_value: str = ""
    started_at: str = ""
    ended_at: str = ""
    duration_ms: int = 0
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class ActionBuffer:
    """
    동일 컨트롤에서의 연속 입력을 버퍼링하여 최종값만 Action으로 변환.
    
    안전장치:
    - Idle Timeout: 2분 무입력 시 자동 flush
    - Shutdown Flush: 종료 시 모든 버퍼 저장
    - Periodic Backup: 30초마다 버퍼 상태 백업
    - Crash Recovery: 시작 시 백업 파일에서 복구
    """
    
    def __init__(
        self,
        on_action_complete: Optional[Callable[[ActionRecord], None]] = None,
        idle_timeout_sec: int = 120,
        backup_interval_sec: int = 30,
        backup_dir: str = "."
    ):
        self._on_action_complete = on_action_complete
        self._idle_timeout_sec = idle_timeout_sec
        self._backup_interval_sec = backup_interval_sec
        self._backup_dir = Path(backup_dir)
        self._backup_file = self._backup_dir / ".action_buffer_backup.json"
        
        # 현재 버퍼 상태
        self._current_control: Optional[ControlContext] = None
        self._current_value: str = ""
        self._input_started_at: Optional[datetime] = None
        self._last_activity_time: float = time.time()
        
        # 스레드 관련
        self._lock = threading.Lock()
        self._running = False
        self._timer_thread: Optional[threading.Thread] = None
        
        # 크래시 복구
        self._recover_from_backup()
    
    def start(self):
        """백그라운드 타이머 시작 (idle timeout & periodic backup)."""
        if self._running:
            return
        
        self._running = True
        self._timer_thread = threading.Thread(target=self._timer_loop, daemon=True)
        self._timer_thread.start()
        logger.info(f"ActionBuffer started (idle_timeout={self._idle_timeout_sec}s, backup_interval={self._backup_interval_sec}s)")
    
    def stop(self):
        """종료 시 모든 버퍼 flush."""
        self._running = False
        
        # 남은 버퍼 flush
        with self._lock:
            if self._current_control and self._current_value:
                self._flush_current_buffer()
        
        # 백업 파일 삭제
        self._cleanup_backup()
        
        if self._timer_thread:
            self._timer_thread.join(timeout=2)
        
        logger.info("ActionBuffer stopped")
    
    def on_focus_change(self, new_control: Optional[ControlContext], new_value: str = ""):
        """
        포커스 변경 시 호출.
        이전 컨트롤의 버퍼를 flush하고 새 컨트롤로 전환.
        """
        with self._lock:
            # 이전 버퍼 flush
            if self._current_control:
                old_key = self._current_control.get_key()
                new_key = new_control.get_key() if new_control else ""
                
                # 다른 컨트롤로 이동했을 때만 flush
                if old_key != new_key and self._current_value:
                    self._flush_current_buffer()
            
            # 새 컨트롤 설정
            self._current_control = new_control
            self._current_value = new_value
            self._input_started_at = datetime.now() if new_value else None
            self._last_activity_time = time.time()
    
    def on_value_change(self, control: ControlContext, new_value: str):
        """
        값 변경 시 호출.
        같은 컨트롤이면 버퍼 업데이트, 다른 컨트롤이면 포커스 변경 처리.
        """
        with self._lock:
            if self._current_control and control.get_key() == self._current_control.get_key():
                # 같은 컨트롤 - 버퍼 업데이트
                if not self._input_started_at:
                    self._input_started_at = datetime.now()
                self._current_value = new_value
            else:
                # 다른 컨트롤 - 이전 버퍼 flush 후 새 컨트롤 설정
                if self._current_control and self._current_value:
                    self._flush_current_buffer()
                
                self._current_control = control
                self._current_value = new_value
                self._input_started_at = datetime.now()
            
            self._last_activity_time = time.time()
    
    def on_click(self, control: ControlContext, button: str = "left", x: int = 0, y: int = 0):
        """클릭 이벤트 처리 - 즉시 Action 생성."""
        with self._lock:
            # 클릭 전 현재 입력 버퍼 flush
            if self._current_control and self._current_value:
                self._flush_current_buffer()
            
            # Click Action 생성
            now = datetime.now()
            action = ActionRecord(
                action_id=str(uuid.uuid4()),
                action_type="click",
                app=control.app,
                window_title=control.window_title,
                control_name=control.control_name,
                control_type=control.control_type,
                started_at=now.isoformat(),
                ended_at=now.isoformat(),
                duration_ms=0,
                metadata={"button": button, "x": x, "y": y}
            )
            
            self._emit_action(action)
            self._last_activity_time = time.time()
    
    def on_shortcut(self, shortcut: str, control: Optional[ControlContext] = None):
        """단축키 이벤트 처리."""
        with self._lock:
            # 저장/전송 관련 단축키면 현재 버퍼 flush
            submit_shortcuts = {"Ctrl+S", "Ctrl+Enter", "Alt+S"}
            if shortcut in submit_shortcuts and self._current_control and self._current_value:
                self._flush_current_buffer()
            
            # Shortcut Action 생성
            now = datetime.now()
            action = ActionRecord(
                action_id=str(uuid.uuid4()),
                action_type="shortcut",
                app=control.app if control else "system",
                window_title=control.window_title if control else "",
                control_name=control.control_name if control else "",
                control_type="keyboard",
                final_value=shortcut,
                started_at=now.isoformat(),
                ended_at=now.isoformat(),
                duration_ms=0
            )
            
            self._emit_action(action)
            self._last_activity_time = time.time()
    
    def _flush_current_buffer(self):
        """현재 버퍼를 Action으로 변환하여 emit."""
        if not self._current_control or not self._current_value:
            return
        
        now = datetime.now()
        started = self._input_started_at or now
        duration_ms = int((now - started).total_seconds() * 1000)
        
        action = ActionRecord(
            action_id=str(uuid.uuid4()),
            action_type="input",
            app=self._current_control.app,
            window_title=self._current_control.window_title,
            control_name=self._current_control.control_name,
            control_type=self._current_control.control_type,
            final_value=self._current_value,
            started_at=started.isoformat(),
            ended_at=now.isoformat(),
            duration_ms=duration_ms
        )
        
        self._emit_action(action)
        
        # 버퍼 초기화
        self._current_value = ""
        self._input_started_at = None
    
    def _emit_action(self, action: ActionRecord):
        """Action을 콜백으로 전달."""
        logger.debug(f"Action emitted: {action.action_type} - {action.control_name}")
        
        if self._on_action_complete:
            try:
                self._on_action_complete(action)
            except Exception as e:
                logger.error(f"Failed to emit action: {e}")
    
    def _timer_loop(self):
        """백그라운드 타이머 루프."""
        last_backup_time = time.time()
        
        while self._running:
            time.sleep(1)  # 1초 간격 체크
            
            current_time = time.time()
            
            # Idle Timeout 체크
            with self._lock:
                idle_duration = current_time - self._last_activity_time
                if idle_duration >= self._idle_timeout_sec:
                    if self._current_control and self._current_value:
                        logger.info(f"Idle timeout ({self._idle_timeout_sec}s) - flushing buffer")
                        self._flush_current_buffer()
            
            # Periodic Backup
            if current_time - last_backup_time >= self._backup_interval_sec:
                self._save_backup()
                last_backup_time = current_time
    
    def _save_backup(self):
        """현재 버퍼 상태를 파일로 백업."""
        with self._lock:
            if not self._current_control or not self._current_value:
                return
            
            backup_data = {
                "control": asdict(self._current_control),
                "value": self._current_value,
                "started_at": self._input_started_at.isoformat() if self._input_started_at else None,
                "backup_time": datetime.now().isoformat()
            }
        
        try:
            with open(self._backup_file, 'w', encoding='utf-8') as f:
                json.dump(backup_data, f, ensure_ascii=False)
            logger.debug("Buffer backup saved")
        except Exception as e:
            logger.warning(f"Failed to save backup: {e}")
    
    def _recover_from_backup(self):
        """시작 시 백업 파일에서 복구."""
        if not self._backup_file.exists():
            return
        
        try:
            with open(self._backup_file, 'r', encoding='utf-8') as f:
                backup_data = json.load(f)
            
            control_data = backup_data.get("control", {})
            self._current_control = ControlContext(**control_data)
            self._current_value = backup_data.get("value", "")
            
            started_at_str = backup_data.get("started_at")
            if started_at_str:
                self._input_started_at = datetime.fromisoformat(started_at_str)
            
            logger.info(f"Recovered buffer from backup: {self._current_control.control_name}")
            
            # 복구된 버퍼 즉시 flush (크래시 후 복구이므로)
            if self._current_value:
                self._flush_current_buffer()
            
        except Exception as e:
            logger.warning(f"Failed to recover from backup: {e}")
        finally:
            self._cleanup_backup()
    
    def _cleanup_backup(self):
        """백업 파일 삭제."""
        try:
            if self._backup_file.exists():
                os.remove(self._backup_file)
        except Exception as e:
            logger.warning(f"Failed to cleanup backup: {e}")
    
    def get_stats(self) -> Dict[str, Any]:
        """현재 버퍼 상태 통계."""
        with self._lock:
            return {
                "has_pending_buffer": bool(self._current_value),
                "current_control": self._current_control.control_name if self._current_control else None,
                "buffer_length": len(self._current_value),
                "idle_seconds": int(time.time() - self._last_activity_time)
            }
