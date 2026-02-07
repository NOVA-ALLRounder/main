"""
HTTP Emitter - 센서에서 Collector로 이벤트 전송.
"""

import json
import logging
import uuid
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, Any, Optional
from urllib.request import urlopen, Request
from urllib.error import URLError

logger = logging.getLogger(__name__)


@dataclass
class EmitConfig:
    """Emitter 설정."""
    host: str = "127.0.0.1"
    port: int = 8080
    timeout: float = 2.0
    
    @property
    def endpoint(self) -> str:
        return f"http://{self.host}:{self.port}/events"


class HttpEmitter:
    """HTTP POST로 이벤트 전송."""
    
    def __init__(self, config: Optional[EmitConfig] = None):
        self.config = config or EmitConfig()
        self._consecutive_errors = 0
        self._max_errors = 10
    
    def send_event(self, event: Dict[str, Any]) -> bool:
        """이벤트 전송."""
        try:
            data = json.dumps(event, ensure_ascii=False).encode("utf-8")
            request = Request(
                self.config.endpoint,
                data=data,
                headers={"Content-Type": "application/json"}
            )
            
            with urlopen(request, timeout=self.config.timeout) as response:
                if response.status == 200:
                    self._consecutive_errors = 0
                    return True
                    
        except URLError as e:
            self._consecutive_errors += 1
            if self._consecutive_errors <= 3:
                logger.warning(f"Failed to send event: {e}")
            elif self._consecutive_errors == self._max_errors:
                logger.error(f"Too many consecutive errors ({self._max_errors}), suppressing further warnings")
            return False
            
        except Exception as e:
            logger.error(f"Unexpected error sending event: {e}")
            return False
        
        return False


def build_event(
    source: str,
    event_type: str,
    app: str,
    payload: Dict[str, Any],
    priority: str = "P2"
) -> Dict[str, Any]:
    """이벤트 빌드."""
    return {
        "schema_version": "1.0",
        "event_id": str(uuid.uuid4()),
        "ts": datetime.utcnow().isoformat() + "Z",
        "source": source,
        "app": app,
        "event_type": event_type,
        "priority": priority,
        "payload": payload
    }
